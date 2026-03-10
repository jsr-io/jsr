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
use crate::cloudflare;
use crate::db::Database;
use crate::db::DownloadKind;
use crate::db::NewNpmTarball;
use crate::db::VersionDownloadCount;
use crate::gcp;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::npm::NPM_TARBALL_REVISION;
use crate::npm::generate_npm_version_manifest;
use crate::publish;
use crate::s3::Buckets;
use crate::s3::CACHE_CONTROL_DO_NOT_CACHE;
use crate::s3::CACHE_CONTROL_IMMUTABLE;
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
    .build()
    .unwrap()
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
        cache_control: Some(CACHE_CONTROL_DO_NOT_CACHE.into()),
        gzip_encoded: false,
      },
    )
    .await?;

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
