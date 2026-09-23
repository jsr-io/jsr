// Copyright 2024 the JSR authors. All rights reserved. MIT license.
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
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use routerify::Router;
use routerify::ext::RequestExt;
use routerify_query::RequestQueryExt;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::str::FromStr;
use tracing::Span;
use tracing::error;
use tracing::field;
use tracing::instrument;

use crate::NpmUrl;
use crate::RegistryUrl;
use crate::analysis::RebuildNpmTarballData;
use crate::analysis::rebuild_npm_tarball;
use crate::api::ApiError;
use crate::api::PublishQueue;
use crate::db::Database;
use crate::db::DownloadKind;
use crate::db::NewNpmTarball;
use crate::db::PublishingTaskStatus;
use crate::db::STALE_PUBLISHING_TASK_SECS;
use crate::db::VersionDownloadCount;
use crate::emails;
use crate::emails::EmailQueue;
use crate::emails::EmailSender;
use crate::emails::SendEmailTask;
use crate::external::cloudflare;
use crate::external::cloudflare::CachePurge;
use crate::gcp;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::npm::NPM_TARBALL_REVISION;
use crate::npm::generate_npm_version_manifest;
use crate::publish;
use crate::s3::Buckets;
use crate::s3::CACHE_CONTROL_IMMUTABLE;
use crate::s3::CACHE_CONTROL_MANIFEST;
use crate::s3::S3UploadOptions;
use crate::s3::UploadTaskBody;
use crate::s3_paths;
use crate::util;
use crate::util::ApiResult;
use crate::util::decode_json;

pub struct NpmTarballBuildQueue(pub Option<gcp::Queue>);
pub struct AnalyticsEngineConfig(
  pub  Option<(
    cloudflare::AnalyticsEngineClient,
    /* dataset name */ String,
  )>,
);

pub fn tasks_router() -> Router<Body, ApiError> {
  Router::builder()
    .post("/publish", util::json(publish::publish_handler))
    .post("/npm_tarball_build", util::json(npm_tarball_build_handler))
    .post(
      "/npm_tarball_enqueue",
      util::json(npm_tarball_enqueue_handler),
    )
    .post(
      "/scrape_download_counts",
      util::json(scrape_download_counts_handler),
    )
    .post(
      "/clean_oauth_states",
      util::json(clean_oauth_states_handler),
    )
    .post("/send_email", send_email_handler)
    .post(
      "/sweep_pending_emails",
      util::json(sweep_pending_emails_handler),
    )
    .post(
      "/clean_download_counts_4h",
      util::json(clean_download_counts_4h_handler),
    )
    .post(
      "/requeue_stuck_publishing_tasks",
      util::json(requeue_stuck_publishing_tasks_handler),
    )
    .build()
    .unwrap()
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
/// `create_publishing_task`. This handler, run periodically by Cloud
/// Scheduler, finds those tasks and pushes them back through the publish
/// queue, which runs `publish_task`'s state machine to completion.
#[instrument(
  name = "POST /tasks/requeue_stuck_publishing_tasks",
  skip(req),
  err
)]
pub async fn requeue_stuck_publishing_tasks_handler(
  req: Request<Body>,
) -> ApiResult<()> {
  let db = req.data::<Database>().unwrap().clone();
  let queue = req.data::<PublishQueue>().unwrap().0.clone();
  let queue = queue.ok_or(ApiError::InternalServerError)?;

  let stale = db
    .list_stale_publishing_tasks(STALE_PUBLISHING_TASK_SECS)
    .await?;

  for (id, status) in stale {
    // A `processing` task never committed its version row (the finalize
    // transaction is atomic), so it is safe to reset it to `pending` and let
    // the worker reprocess the tarball from scratch. A `processed` task
    // already has its rows committed and only needs the metadata-upload step
    // re-driven, so it is requeued as-is.
    if status == PublishingTaskStatus::Processing
      && let Err(err) = db
        .update_publishing_task_status(
          None,
          id,
          PublishingTaskStatus::Processing,
          PublishingTaskStatus::Pending,
          None,
        )
        .await
    {
      // Lost a race (the task changed status concurrently) or a transient DB
      // error. Skip it — a later run will pick it up again if still stuck.
      error!("failed to reset stuck publishing task {id}: {err}");
      continue;
    }

    let body = serde_json::to_vec(&id)?;
    queue.task_buffer(None, Some(body.into())).await?;
  }

  Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct NpmTarballBuildJob {
  pub scope: ScopeName,
  pub name: PackageName,
  pub version: Version,
}

#[instrument(
  name = "POST /tasks/npm_tarball_build",
  skip(req),
  err,
  fields(job)
)]
pub async fn npm_tarball_build_handler(
  mut req: Request<Body>,
) -> ApiResult<()> {
  let job: NpmTarballBuildJob = decode_json(&mut req).await?;
  Span::current().record("job", field::debug(&job));

  let db = req.data::<Database>().unwrap().clone();
  let buckets = req.data::<Buckets>().unwrap().clone();
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();
  let npm_url = req.data::<NpmUrl>().unwrap().0.clone();
  let cache_purge = req.data::<CachePurge>().unwrap().clone();

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
      .ok_or(ApiError::PackageVersionNotFound)?;
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
    crate::s3_paths::npm_version_manifest_path(&job.scope, &job.name);
  let npm_version_manifest =
    generate_npm_version_manifest(&db, &npm_url, &job.scope, &job.name).await?;
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
    .purge(crate::s3_paths::npm_version_manifest_purge_urls(
      &npm_url, &job.scope, &job.name,
    ))
    .await;

  Ok(())
}

const NPM_TARBALL_BUILD_ENQUEUE_PARALLELISM: usize = 32;

#[instrument(name = "POST /tasks/npm_tarball_enqueue", skip(req), err)]
pub async fn npm_tarball_enqueue_handler(req: Request<Body>) -> ApiResult<()> {
  let db = req.data::<Database>().unwrap().clone();
  let queue = req.data::<NpmTarballBuildQueue>().unwrap();

  let queue = queue.0.as_ref().ok_or(ApiError::InternalServerError)?;

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
      queue.task_buffer(None, Some(body.into()))
    })
    .buffer_unordered(NPM_TARBALL_BUILD_ENQUEUE_PARALLELISM);

  while let Some(result) = futs.next().await {
    result?;
  }

  Ok(())
}

#[instrument(name = "POST /tasks/scrape_download_counts", skip(req), err)]
pub async fn scrape_download_counts_handler(
  req: Request<Body>,
) -> ApiResult<()> {
  let db = req.data::<Database>().unwrap().clone();

  let time_window: i64 = req
    .query("intervalHrs")
    .ok_or_else(|| ApiError::MalformedRequest {
      msg: "intervalHrs query param is required".into(),
    })?
    .parse()
    .map_err(|_| ApiError::MalformedRequest {
      msg: "intervalHrs query param must be an integer".into(),
    })?;

  let analytics_engine = req.data::<AnalyticsEngineConfig>().unwrap();
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
        ApiError::InternalServerError
      })?;

    insert_analytics_download_entries(
      &db,
      jsr_downloads,
      DownloadKind::JsrMeta,
    )
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
        ApiError::InternalServerError
      })?;

    insert_analytics_download_entries(&db, npm_downloads, DownloadKind::NpmTgz)
      .await?;
  };

  Ok(())
}

#[instrument(name = "POST /tasks/clean_oauth_states", skip(req), err)]
pub async fn clean_oauth_states_handler(req: Request<Body>) -> ApiResult<()> {
  let db = req.data::<Database>().unwrap().clone();
  let cutoff = Utc::now() - Duration::hours(1);
  let deleted = db.delete_expired_oauth_states(cutoff).await?;
  tracing::info!(deleted, "cleaned up expired oauth states");
  Ok(())
}

#[instrument(name = "POST /tasks/clean_download_counts_4h", skip(req), err)]
pub async fn clean_download_counts_4h_handler(
  req: Request<Body>,
) -> ApiResult<()> {
  let db = req.data::<Database>().unwrap().clone();
  let cutoff = Utc::now() - Duration::days(7);
  let deleted = db.cleanup_download_counts_4h(cutoff).await?;
  tracing::info!(deleted, "cleaned up old 4h download counts");
  Ok(())
}

async fn insert_analytics_download_entries(
  db: &Database,
  records: Vec<cloudflare::DownloadRecord>,
  kind: DownloadKind,
) -> Result<(), ApiError> {
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
/// are assumed to have lost their Cloud Tasks hand-off and are re-driven.
/// Comfortably longer than a normal delivery, so a task still in flight is not
/// duplicated.
const STALE_EMAIL_DELIVERY_SECS: i64 = 300;

/// How many stale deliveries one sweep re-drives. Bounds the work a single
/// scheduler tick can do if a long Postmark outage has backed the queue up.
const STALE_EMAIL_SWEEP_LIMIT: i64 = 500;

/// Delivers one queued email. Driven by Cloud Tasks, which retries on a non-2xx.
#[instrument(
  name = "POST /tasks/send_email",
  skip(req),
  err,
  fields(delivery_id)
)]
pub async fn send_email_handler(
  mut req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let task: SendEmailTask = decode_json(&mut req).await?;
  Span::current().record("delivery_id", field::display(task.id));

  let db = req.data::<Database>().unwrap();
  let email_sender = req.data::<Option<EmailSender>>().unwrap();

  let done = emails::deliver(db, email_sender.as_ref(), task.id)
    .await
    .map_err(|err| {
      error!("failed to process email delivery: {:?}", err);
      ApiError::InternalServerError
    })?;

  if done {
    Ok(util::create_response(StatusCode::OK, "text/plain", "OK"))
  } else {
    // The attempt failed but is worth another; a 5xx is how Cloud Tasks is told
    // to back off and retry.
    Err(ApiError::InternalServerError)
  }
}

/// Re-drives emails that were queued but never delivered.
///
/// The delivery row is committed before Cloud Tasks is told about it, so a
/// failure to reach Cloud Tasks — or a task dropped after its retries were
/// exhausted at the queue level — leaves a row nothing will ever pick up. This
/// handler, run periodically by Cloud Scheduler, finds those and queues them
/// again. Re-driving a delivery is safe: `deliver` no-ops on a row that has
/// already been sent.
#[instrument(name = "POST /tasks/sweep_pending_emails", skip(req), err)]
pub async fn sweep_pending_emails_handler(req: Request<Body>) -> ApiResult<()> {
  let db = req.data::<Database>().unwrap();
  let email_sender = req.data::<Option<EmailSender>>().unwrap();
  let queue = req.data::<EmailQueue>().unwrap();

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
    // Handing the delivery back to Cloud Tasks is preferred: it retries with
    // backoff and keeps this handler quick. But if the queue cannot be reached
    // at all — it does not exist yet, or this service may not enqueue to it —
    // then every delivery is stuck behind the same failure, and a sweeper that
    // only ever enqueues would never notice. So the fallback is to deliver here
    // instead, which needs nothing but the Postmark client.
    let queued = match &queue.0 {
      Some(queue) => {
        let body = serde_json::to_vec(&SendEmailTask { id }).unwrap();
        // A fresh task id, because the original may still be known to Cloud
        // Tasks and would be rejected as a duplicate.
        match queue.task_buffer(None, Some(body.into())).await {
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

    if !queued
      && let Err(err) = emails::deliver(db, email_sender.as_ref(), id).await
    {
      error!(delivery_id = %id, "failed to re-drive email delivery: {:?}", err);
    }
  }

  Ok(())
}
