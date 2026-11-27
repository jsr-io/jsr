// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use axum::Router;
use axum::body::Body;
use axum::routing::post;
use bytes::Bytes;
use chrono::Duration;
use chrono::Utc;
use deno_semver::StackString;
use deno_semver::VersionReq;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use deno_semver::package::PackageSubPath;
use futures::StreamExt;
use futures::stream;
use hyper::Request;
use serde::Deserialize;
use serde::Serialize;
use shared::router::RequestExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;
use tracing::Span;
use tracing::error;
use tracing::field;
use tracing::instrument;
use url::Url;

use crate::analysis::PackageAnalysisData;
use crate::analysis::RebuildNpmTarballData;
use crate::analysis::analyze_package;
use crate::analysis::rebuild_npm_tarball;
use crate::errors::*;
use shared::npm::generate_npm_version_manifest;

use shared::NpmUrl;
use shared::PublishQueue;
use shared::RegistryUrl;
use shared::db::ABANDON_PUBLISHING_TASK_SECS;
use shared::db::Database;
use shared::db::STALE_PUBLISHING_TASK_SECS;
use shared::emails;
use shared::emails::EmailQueue;
use shared::emails::EmailSender;
use shared::emails::SendEmailTask;
use shared::errors::SharedError;
use shared::external::cloudflare;
use shared::external::cloudflare::CachePurge;
use shared::ids::PackageName;
use shared::ids::PackagePath;
use shared::ids::ScopeName;
use shared::ids::Version;
use shared::models::DownloadKind;
use shared::models::NewNpmTarball;
use shared::models::PackageVersionMeta;
use shared::models::VersionDownloadCount;
use shared::npm::NPM_TARBALL_REVISION;
use shared::queue::Queue;
use shared::s3::Buckets;
use shared::s3::CACHE_CONTROL_IMMUTABLE;
use shared::s3::CACHE_CONTROL_MANIFEST;
use shared::s3::S3UploadOptions;
use shared::s3::UploadTaskBody;
use shared::s3_paths;
use shared::util;
use shared::util::decode_json;

pub struct NpmTarballBuildQueue(pub Option<Queue>);
pub struct AnalyticsEngineConfig(
  pub  Option<(
    cloudflare::AnalyticsEngineClient,
    /* dataset name */ String,
  )>,
);

/// The tasks as routes, which is how they are run natively. The worker calls
/// them directly instead, from its `queue` and `scheduled` handlers (see
/// `main.rs`).
pub fn tasks_router() -> Router {
  Router::new()
    .route(
      "/publish",
      post(util::json(crate::publish::publish_handler)),
    )
    .route(
      "/npm_tarball_build",
      post(util::json(async |mut req: Request<Body>| {
        let job = decode_json(&mut req).await?;
        npm_tarball_build(
          job,
          req.data::<Database>().unwrap(),
          req.data::<Buckets>().unwrap().clone(),
          req.data::<RegistryUrl>().unwrap().0.clone(),
          req.data::<NpmUrl>().unwrap().0.clone(),
          req.data::<CachePurge>().unwrap().clone(),
        )
        .await
      })),
    )
    .route(
      "/npm_tarball_enqueue",
      post(util::json(async |req: Request<Body>| {
        npm_tarball_enqueue(
          req.data::<Database>().unwrap(),
          req.data::<NpmTarballBuildQueue>().unwrap().0.as_ref(),
        )
        .await
      })),
    )
    .route(
      "/scrape_download_counts",
      post(util::json(async |req: Request<Body>| {
        let time_window = req
          .query("intervalHrs")
          .ok_or_else(|| SharedError::MalformedRequest {
            msg: "intervalHrs query param is required".into(),
          })?
          .parse()
          .map_err(|_| SharedError::MalformedRequest {
            msg: "intervalHrs query param must be an integer".into(),
          })?;
        scrape_download_counts(
          req.data::<Database>().unwrap(),
          req.data::<AnalyticsEngineConfig>().unwrap(),
          time_window,
        )
        .await
      })),
    )
    .route(
      "/clean_oauth_states",
      post(util::json(async |req: Request<Body>| {
        clean_oauth_states(req.data::<Database>().unwrap()).await
      })),
    )
    .route(
      "/send_email",
      post(util::json(async |mut req: Request<Body>| {
        let task = decode_json(&mut req).await?;
        send_email(
          task,
          req.data::<Database>().unwrap(),
          req.data::<Option<EmailSender>>().unwrap().as_ref(),
        )
        .await
      })),
    )
    .route(
      "/sweep_pending_emails",
      post(util::json(async |req: Request<Body>| {
        sweep_pending_emails(
          req.data::<Database>().unwrap(),
          req.data::<Option<EmailSender>>().unwrap().as_ref(),
          req.data::<EmailQueue>().unwrap().0.as_ref(),
        )
        .await
      })),
    )
    .route(
      "/clean_download_counts_4h",
      post(util::json(async |req: Request<Body>| {
        clean_download_counts_4h(req.data::<Database>().unwrap()).await
      })),
    )
    .route(
      "/requeue_stuck_publishing_tasks",
      post(util::json(async |req: Request<Body>| {
        requeue_stuck_publishing_tasks(
          req.data::<Database>().unwrap(),
          req.data::<PublishQueue>().unwrap().0.as_ref(),
        )
        .await
      })),
    )
    .route(
      "/recompute_version_meta",
      post(util::json(recompute_version_meta_handler)),
    )
    .route(
      "/anonymize_deleted_users",
      post(util::json(async |req: Request<Body>| {
        anonymize_deleted_users(req.data::<Database>().unwrap()).await
      })),
    )
}

#[derive(Debug, Deserialize)]
struct RecomputeVersionMetaJob {
  scope: ScopeName,
  name: PackageName,
  version: Version,
}

/// Re-runs the publish analysis over a published version's stored files and
/// returns the recomputed meta, or the analysis error message when the
/// analysis rejects the package. Called by the api's admin "recompute meta"
/// endpoint.
#[instrument(
  name = "POST /recompute_version_meta",
  skip(req),
  err,
  fields(job)
)]
pub async fn recompute_version_meta_handler(
  mut req: Request<Body>,
) -> TaskResult<Result<PackageVersionMeta, String>> {
  let job: RecomputeVersionMetaJob = decode_json(&mut req).await?;
  Span::current().record("job", field::debug(&job));

  let db = req.data::<Database>().unwrap().clone();
  let buckets = req.data::<Buckets>().unwrap().clone();
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();

  let package_version = db
    .get_package_version(&job.scope, &job.name, &job.version)
    .await?
    .ok_or(SharedError::PackageVersionNotFound)?;

  let mut files = HashMap::new();
  for file in db
    .list_package_files(&job.scope, &job.name, &job.version)
    .await?
  {
    let s3_path =
      s3_paths::file_path(&job.scope, &job.name, &job.version, &file.path);
    let bytes = buckets
      .modules_bucket
      .download(s3_path.into())
      .await?
      .ok_or_else(|| {
        error!(
          "module file '{}' of @{}/{}@{} is missing from the modules bucket",
          file.path, job.scope, job.name, job.version
        );
        SharedError::InternalServerError
      })?;
    files.insert(file.path, bytes.to_vec());
  }

  // the config file path is only used in analysis error messages; the
  // exports driving the analysis come from the database
  let config_file = ["/jsr.json", "/jsr.jsonc", "/deno.json", "/deno.jsonc"]
    .into_iter()
    .map(|path| PackagePath::new(path.to_string()).unwrap())
    .find(|path| files.contains_key(path))
    .unwrap_or_else(|| PackagePath::new("/jsr.json".to_string()).unwrap());

  let span = Span::current();
  let output = shared::util::spawn_blocking(move || {
    analyze_package(
      span,
      registry_url,
      job.scope,
      job.name,
      job.version,
      config_file,
      PackageAnalysisData {
        exports: package_version.exports,
        files,
      },
    )
  })
  .await
  .map_err(|join_error| {
    error!("analysis task panicked: {join_error:?}");
    SharedError::InternalServerError
  })?;

  Ok(
    output
      .map(|output| output.meta)
      .map_err(|analysis_error| analysis_error.to_string()),
  )
}

/// Re-drive publishing tasks that got stranded in a non-terminal state.
///
/// This is the self-healing counterpart to the manual admin requeue endpoint.
/// A queue worker that dies mid-publish (Cloud Run timeout, cancelled CI run,
/// transient S3/Cloudflare error after the version row was committed) can
/// leave a task stuck in `processing` or `processed`. Such a task never
/// finishes regenerating the package-level `meta.json`, so the published
/// version stays invisible to Deno's resolver, and the version cannot be
/// re-published because of the `status != 'failure'` guard in
/// `create_publishing_task`. A publish request that dies before enqueueing its
/// task leaves it stuck in `pending` with the same effect. The queue retries
/// a task at most a minute apart and a worker run is capped at 10 minutes, so
/// a task that has not changed for `STALE_PUBLISHING_TASK_SECS` is not in
/// flight. This task, run periodically, finds those tasks and pushes them
/// back through the publish queue, which runs `publish_task`'s state machine to completion.
#[instrument(name = "tasks::requeue_stuck_publishing_tasks", skip_all, err)]
pub async fn requeue_stuck_publishing_tasks(
  db: &Database,
  queue: Option<&Queue>,
) -> TaskResult<()> {
  let queue = queue.ok_or(SharedError::InternalServerError)?;

  let stale = db
    .claim_stale_publishing_tasks(
      STALE_PUBLISHING_TASK_SECS,
      ABANDON_PUBLISHING_TASK_SECS,
    )
    .await?;

  for id in &stale.abandoned {
    error!("abandoned stuck publishing task {id}");
  }

  for id in stale.claimed {
    let body = serde_json::to_vec(&id)?;
    queue.send(body.into()).await?;
  }

  Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmTarballBuildJob {
  pub scope: ScopeName,
  pub name: PackageName,
  pub version: Version,
}

#[instrument(
  name = "tasks::npm_tarball_build",
  skip(db, buckets, registry_url, npm_url, cache_purge),
  err
)]
pub async fn npm_tarball_build(
  job: NpmTarballBuildJob,
  db: &Database,
  buckets: Buckets,
  registry_url: Url,
  npm_url: Url,
  cache_purge: CachePurge,
) -> TaskResult<()> {
  let is_already_built = db
    .get_npm_tarball(
      &job.scope,
      &job.name,
      &job.version,
      NPM_TARBALL_REVISION as i32,
    )
    .await?
    .is_some();

  if !is_already_built {
    let version = db
      .get_package_version(&job.scope, &job.name, &job.version)
      .await?
      .ok_or(SharedError::PackageVersionNotFound)?;
    let dependencies = db
      .list_package_version_dependencies(&job.scope, &job.name, &job.version)
      .await?;
    let files: HashSet<_> = db
      .list_package_files(&job.scope, &job.name, &job.version)
      .await?
      .into_iter()
      .map(|f| f.path)
      .collect();

    let dependencies = dependencies
      .into_iter()
      .map(|dep| {
        let sub_path = if dep.dependency_path.is_empty() {
          None
        } else {
          Some(PackageSubPath::from_string(dep.dependency_path))
        };
        let version_req =
          VersionReq::parse_from_specifier(&dep.dependency_constraint).unwrap();
        let req = PackageReq {
          name: StackString::from_string(dep.dependency_name),
          version_req,
        };
        (dep.dependency_kind, PackageReqReference { req, sub_path })
      })
      .collect();

    let span = Span::current();
    let data = RebuildNpmTarballData {
      files,
      scope: version.scope,
      name: version.name,
      version: version.version,
      dependencies,
      exports: version.exports,
    };
    let npm_tarball = tokio::task::spawn_blocking(|| {
      rebuild_npm_tarball(span, registry_url, buckets.modules_bucket, data)
    })
    .await
    .unwrap()?;

    let new_npm_tarball = NewNpmTarball {
      scope: &job.scope,
      name: &job.name,
      version: &job.version,
      revision: NPM_TARBALL_REVISION as i32,
      size: npm_tarball.tarball.len() as i32,
      sha1: &npm_tarball.sha1,
      sha512: &npm_tarball.sha512,
    };

    let npm_tarball_path = s3_paths::npm_tarball_path(
      &job.scope,
      &job.name,
      &job.version,
      NPM_TARBALL_REVISION,
    );
    buckets
      .npm_bucket
      .upload(
        npm_tarball_path.into(),
        UploadTaskBody::Bytes(Bytes::from(npm_tarball.tarball)),
        S3UploadOptions {
          content_type: Some("application/octet-stream".into()),
          cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
          gzip_encoded: false,
        },
      )
      .await?;

    db.create_npm_tarball(new_npm_tarball).await?;
  }

  let npm_version_manifest_path =
    shared::s3_paths::npm_version_manifest_path(&job.scope, &job.name);
  let npm_version_manifest =
    generate_npm_version_manifest(db, &npm_url, &job.scope, &job.name).await?;
  let content = serde_json::to_vec_pretty(&npm_version_manifest)?;
  buckets
    .npm_bucket
    .upload(
      npm_version_manifest_path.into(),
      UploadTaskBody::Bytes(content.into()),
      S3UploadOptions {
        content_type: Some("application/json".into()),
        cache_control: Some(CACHE_CONTROL_MANIFEST.into()),
        gzip_encoded: false,
      },
    )
    .await?;

  cache_purge
    .purge(shared::s3_paths::npm_version_manifest_purge_urls(
      &npm_url, &job.scope, &job.name,
    ))
    .await;

  Ok(())
}

const NPM_TARBALL_BUILD_ENQUEUE_PARALLELISM: usize = 32;

/// Finds missing npm tarballs and queues them to be built.
#[instrument(name = "tasks::npm_tarball_enqueue", skip_all, err)]
pub async fn npm_tarball_enqueue(
  db: &Database,
  queue: Option<&Queue>,
) -> TaskResult<()> {
  let queue = queue.ok_or(SharedError::InternalServerError)?;

  let missing_tarballs = db
    .list_missing_npm_tarballs(NPM_TARBALL_REVISION as i32)
    .await?;

  let mut futs = stream::iter(missing_tarballs)
    .map(|missing_tarball| {
      let job = NpmTarballBuildJob {
        scope: missing_tarball.0,
        name: missing_tarball.1,
        version: missing_tarball.2,
      };
      let body = serde_json::to_vec(&job).unwrap();
      queue.send(body.into())
    })
    .buffer_unordered(NPM_TARBALL_BUILD_ENQUEUE_PARALLELISM);

  while let Some(result) = futs.next().await {
    result?;
  }

  Ok(())
}

/// Records the downloads of the last `time_window` hours from Analytics
/// Engine.
#[instrument(
  name = "tasks::scrape_download_counts",
  skip(db, analytics_engine),
  err
)]
pub async fn scrape_download_counts(
  db: &Database,
  analytics_engine: &AnalyticsEngineConfig,
  time_window: i64,
) -> TaskResult<()> {
  if let Some((analytics_client, dataset_name)) = analytics_engine.0.as_ref() {
    let jsr_downloads = analytics_client
      .query_downloads(format!(
        r#"
SELECT
  toStartOfInterval(timestamp, INTERVAL '4' HOUR) as time_bucket,
  blob2 as scope,
  blob3 as package,
  blob4 as ver,
  intDiv(sum(_sample_interval), 1) as count
FROM
  '{dataset_name}'
WHERE
  timestamp >= NOW() - INTERVAL '{time_window}' HOUR
  AND blob1 = 'jsr'
GROUP BY
  time_bucket,
  scope,
  package,
  ver
ORDER BY
  time_bucket DESC
      "#
      ))
      .await
      .map_err(|e| {
        error!("Failed to query JSR downloads from Analytics Engine: {}", e);
        SharedError::InternalServerError
      })?;

    insert_analytics_download_entries(db, jsr_downloads, DownloadKind::JsrMeta)
      .await?;

    let npm_downloads = analytics_client
      .query_downloads(format!(
        r#"
SELECT
  toStartOfInterval(timestamp, INTERVAL '4' HOUR) as time_bucket,
  blob2 as scope,
  blob3 as package,
  blob4 as ver,
  intDiv(sum(_sample_interval), 1) as count
FROM
  '{dataset_name}'
WHERE
  timestamp >= NOW() - INTERVAL '{time_window}' HOUR
  AND blob1 = 'npm'
GROUP BY
  time_bucket,
  scope,
  package,
  ver
ORDER BY
  time_bucket DESC
      "#
      ))
      .await
      .map_err(|e| {
        error!("Failed to query NPM downloads from Analytics Engine: {}", e);
        SharedError::InternalServerError
      })?;

    insert_analytics_download_entries(db, npm_downloads, DownloadKind::NpmTgz)
      .await?;
  };

  Ok(())
}

#[instrument(name = "tasks::clean_oauth_states", skip_all, err)]
pub async fn clean_oauth_states(db: &Database) -> TaskResult<()> {
  let cutoff = Utc::now() - Duration::hours(1);
  let deleted = db.delete_expired_oauth_states(cutoff).await?;
  tracing::info!(deleted, "cleaned up expired oauth states");
  Ok(())
}

/// Strips name and email from the `user_delete` audit tombstones of accounts
/// deleted more than a year ago. The identity data is kept for that window so
/// abuse and supply-chain attacks can still be investigated after an account
/// is deleted; afterwards only the pseudonymous identifiers remain (see
/// `Database::delete_user`). Run periodically by a cron trigger.
#[instrument(name = "tasks::anonymize_deleted_users", skip_all, err)]
pub async fn anonymize_deleted_users(db: &Database) -> TaskResult<()> {
  let cutoff = Utc::now() - Duration::days(365);
  let anonymized = db.anonymize_deleted_users(cutoff).await?;
  tracing::info!(anonymized, "anonymized deleted user tombstones");
  Ok(())
}

#[instrument(name = "tasks::clean_download_counts_4h", skip_all, err)]
pub async fn clean_download_counts_4h(db: &Database) -> TaskResult<()> {
  let cutoff = Utc::now() - Duration::days(7);
  let deleted = db.cleanup_download_counts_4h(cutoff).await?;
  tracing::info!(deleted, "cleaned up old 4h download counts");
  Ok(())
}

async fn insert_analytics_download_entries(
  db: &Database,
  records: Vec<cloudflare::DownloadRecord>,
  kind: DownloadKind,
) -> Result<(), TaskError> {
  let mut entries = Vec::with_capacity(records.len());
  for record in records {
    if let Some(entry) =
      deserialize_version_download_count_from_analytics(record, kind)
    {
      entries.push(entry);
    }
  }

  db.insert_download_entries(entries).await?;

  Ok(())
}

fn deserialize_version_download_count_from_analytics(
  record: cloudflare::DownloadRecord,
  kind: DownloadKind,
) -> Option<VersionDownloadCount> {
  // Cloudflare Analytics Engine (ClickHouse) returns datetimes as
  // "YYYY-MM-DD HH:MM:SS", not RFC3339.
  let time_bucket = chrono::NaiveDateTime::parse_from_str(
    &record.time_bucket,
    "%Y-%m-%d %H:%M:%S",
  )
  .ok()
  .unwrap()
  .and_utc();
  let scope = ScopeName::new(record.scope).ok()?;
  let package = PackageName::new(record.package).ok()?;
  let version = Version::new(&record.ver).ok()?;
  Some(VersionDownloadCount {
    time_bucket,
    scope,
    package,
    version,
    kind,
    count: i64::from_str(&record.count).unwrap(),
  })
}

/// Deliveries queued longer ago than this that never reached a terminal state
/// are assumed to have lost their hand-off to the queue and are re-driven.
/// Comfortably longer than a normal delivery, so a task still in flight is not
/// duplicated.
const STALE_EMAIL_DELIVERY_SECS: i64 = 300;

/// How many stale deliveries one sweep re-drives. Bounds the work a single
/// scheduler tick can do if a long Postmark outage has backed the queue up.
const STALE_EMAIL_SWEEP_LIMIT: i64 = 500;

/// Delivers one queued email. Driven by the email queue, which retries a
/// delivery that failed.
#[instrument(name = "tasks::send_email", skip(db, email_sender), err)]
pub async fn send_email(
  task: SendEmailTask,
  db: &Database,
  email_sender: Option<&EmailSender>,
) -> TaskResult<()> {
  let done =
    emails::deliver(db, email_sender, task.id)
      .await
      .map_err(|err| {
        error!("failed to process email delivery: {:?}", err);
        SharedError::InternalServerError
      })?;

  if done {
    Ok(())
  } else {
    // The attempt failed but is worth another; an error is how the queue is
    // told to back off and retry.
    Err(SharedError::InternalServerError.into())
  }
}

/// Re-drives emails that were queued but never delivered.
///
/// The delivery row is committed before the queue is told about it, so a
/// failure to reach the queue — or a task dropped after its retries were
/// exhausted at the queue level — leaves a row nothing will ever pick up. This
/// task, run periodically, finds those and queues them
/// again. Re-driving a delivery is safe: `deliver` no-ops on a row that has
/// already been sent.
#[instrument(name = "tasks::sweep_pending_emails", skip_all, err)]
pub async fn sweep_pending_emails(
  db: &Database,
  email_sender: Option<&EmailSender>,
  queue: Option<&Queue>,
) -> TaskResult<()> {
  let stale = db
    .list_stale_email_deliveries(
      STALE_EMAIL_DELIVERY_SECS,
      STALE_EMAIL_SWEEP_LIMIT,
    )
    .await?;

  if stale.is_empty() {
    return Ok(());
  }

  tracing::info!("re-driving {} stale email deliveries", stale.len());

  for id in stale {
    // Handing the delivery back to the queue is preferred: it retries with
    // backoff and keeps this handler quick. But if the queue cannot be reached
    // at all — it does not exist yet, or this service may not enqueue to it —
    // then every delivery is stuck behind the same failure, and a sweeper that
    // only ever enqueues would never notice. So the fallback is to deliver here
    // instead, which needs nothing but the Postmark client.
    let queued = match queue {
      Some(queue) => {
        let body = serde_json::to_vec(&SendEmailTask { id }).unwrap();
        match queue.send(body.into()).await {
          Ok(()) => true,
          Err(err) => {
            error!(
              delivery_id = %id,
              "could not queue email delivery, sending it directly instead: {:?}",
              err
            );
            false
          }
        }
      }
      None => false,
    };

    if !queued && let Err(err) = emails::deliver(db, email_sender, id).await {
      error!(delivery_id = %id, "failed to re-drive email delivery: {:?}", err);
    }
  }

  Ok(())
}
