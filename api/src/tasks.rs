// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashSet;

use bytes::Bytes;
use chrono::DateTime;
use chrono::Utc;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use deno_semver::VersionReq;
use futures::stream;
use futures::StreamExt;
use hyper::Body;
use hyper::Request;
use routerify::ext::RequestExt;
use routerify::Router;
use routerify_query::RequestQueryExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tracing::error;
use tracing::field;
use tracing::info;
use tracing::instrument;
use tracing::Span;

use crate::analysis::rebuild_npm_tarball;
use crate::analysis::RebuildNpmTarballData;
use crate::api::ApiError;
use crate::buckets::Buckets;
use crate::buckets::UploadTaskBody;
use crate::db::Database;
use crate::db::NewNpmTarball;
use crate::db::VersionDownloadCount;
use crate::gcp;
use crate::gcp::GcsUploadOptions;
use crate::gcp::CACHE_CONTROL_DO_NOT_CACHE;
use crate::gcp::CACHE_CONTROL_IMMUTABLE;
use crate::gcs_paths;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::npm::generate_npm_version_manifest;
use crate::npm::NPM_TARBALL_REVISION;
use crate::publish;
use crate::util;
use crate::util::decode_json;
use crate::util::ApiResult;
use crate::NpmUrl;
use crate::RegistryUrl;

pub struct NpmTarballBuildQueue(pub Option<gcp::Queue>);
pub struct LogsBigQueryTable(
  pub Option<(gcp::BigQuery, /* logs table id */ String)>,
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
      .ok_or_else(|| ApiError::PackageVersionNotFound)?;
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
          Some(dep.dependency_path)
        };
        let version_req =
          VersionReq::parse_from_specifier(&dep.dependency_constraint).unwrap();
        let req = PackageReq {
          name: dep.dependency_name,
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

    let npm_tarball_path = gcs_paths::npm_tarball_path(
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
        GcsUploadOptions {
          content_type: Some("application/octet-stream".into()),
          cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
          gzip_encoded: false,
        },
      )
      .await?;

    db.create_npm_tarball(new_npm_tarball).await?;
  }

  let npm_version_manifest_path =
    crate::gcs_paths::npm_version_manifest_path(&job.scope, &job.name);
  let npm_version_manifest =
    generate_npm_version_manifest(&db, &npm_url, &job.scope, &job.name).await?;
  let content = serde_json::to_vec_pretty(&npm_version_manifest)?;
  buckets
    .npm_bucket
    .upload(
      npm_version_manifest_path.into(),
      UploadTaskBody::Bytes(content.into()),
      GcsUploadOptions {
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

  let queue = queue
    .0
    .as_ref()
    .ok_or_else(|| ApiError::InternalServerError)?;

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
  let bigquery = req.data::<LogsBigQueryTable>().unwrap();
  let Some((bigquery, logs_table_id)) = bigquery.0.as_ref() else {
    error!("BigQuery not configured");
    return Err(ApiError::InternalServerError);
  };

  let time_window = req
    .query("intervalHrs")
    .ok_or_else(|| ApiError::MalformedRequest {
      msg: "intervalHrs query param is required".into(),
    })?
    .parse()
    .map_err(|_| ApiError::MalformedRequest {
      msg: "intervalHrs query param must be an integer".into(),
    })?;

  let current_timestamp = chrono::Utc::now();
  let start_timestamp =
    current_timestamp - chrono::Duration::hours(time_window);

  fn bigquery_timestamp_serialization(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
  }

  let params = vec![
    json!({
      "name": "start_timestamp",
      "parameterType": {
        "type": "TIMESTAMP"
      },
      "parameterValue": {
        "value": bigquery_timestamp_serialization(start_timestamp)
      }
    }),
    json!({
      "name": "end_timestamp",
      "parameterType": {
        "type": "TIMESTAMP"
      },
      "parameterValue": {
        "value": bigquery_timestamp_serialization(current_timestamp)
      }
    }),
  ];

  let registry_root = req.data::<RegistryUrl>().unwrap().0.to_string();
  let query = format!(
    r#"
SELECT
  t1.time_bucket,
  t1.scope,
  t1.package,
  t1.version,
  COUNT(*) AS count
FROM (
  SELECT
    TIMESTAMP_BUCKET(t2.timestamp, INTERVAL 4 HOUR) AS time_bucket,
    REGEXP_EXTRACT(t2.http_request.request_url, '{registry_root}@([^/]*?)/(?:[^/]*?)/(?:[^/]*?)_meta.json') AS scope,
    REGEXP_EXTRACT(t2.http_request.request_url, '{registry_root}@(?:[^/]*?)/([^/]*?)/(?:[^/]*?)_meta.json') AS package,
    REGEXP_EXTRACT(t2.http_request.request_url, '{registry_root}@(?:[^/]*?)/(?:[^/]*?)/([^/]*?)_meta.json') AS version
  FROM
    `{logs_table_id}` AS t2
  WHERE
    t2.timestamp BETWEEN @start_timestamp
    AND @end_timestamp
    AND t2.log_id = "requests"
    AND REGEXP_CONTAINS(t2.http_request.request_url, '{registry_root}@(?:[^/]*?)/(?:[^/]*?)/(?:[^/]*?)_meta.json') ) AS t1
GROUP BY
  1,
  2,
  3,
  4
ORDER BY
  time_bucket,
  scope,
  package,
  version"#
  );
  let res = bigquery.query(&query, params).await?;
  if !res.job_complete {
    error!("BigQuery job did not complete, errors: {:?}", res.errors);
    return Err(ApiError::InternalServerError);
  }
  let mut rows = res.rows;
  info!("BigQuery job complete, got rows: {rows:?}");
  let mut page_token = res.page_token;
  while let Some(token) = page_token {
    let res = bigquery
      .get_query_results(&res.job_reference.job_id, &token)
      .await?;
    rows.extend(res.rows);
    page_token = res.page_token;
  }

  insert_bigquery_download_entries(&db, rows).await
}

async fn insert_bigquery_download_entries(
  db: &Database,
  rows: Vec<serde_json::Value>,
) -> Result<(), ApiError> {
  let mut entries = Vec::with_capacity(rows.len());
  for row in rows {
    let entry = deserialize_version_download_count_from_bigquery(&row)
      .ok_or_else(|| {
        error!("Failed to deserialize row: {:?}", row);
        ApiError::InternalServerError
      })?;
    entries.push(entry);
  }

  db.insert_download_entries(entries).await?;

  Ok(())
}

fn deserialize_version_download_count_from_bigquery(
  row: &serde_json::Value,
) -> Option<VersionDownloadCount> {
  let f = row.get("f")?;
  let time_bucket_micros: i64 = f.get(0)?.get("v")?.as_str()?.parse().ok()?;
  let time_bucket = DateTime::from_timestamp_micros(time_bucket_micros)?;
  let scope = ScopeName::new(f.get(1)?.get("v")?.as_str()?.to_owned()).ok()?;
  let package =
    PackageName::new(f.get(2)?.get("v")?.as_str()?.to_owned()).ok()?;
  let version = Version::new(f.get(3)?.get("v")?.as_str()?).ok()?;
  let count = f.get(4)?.get("v")?.as_str()?.parse().ok()?;
  Some(VersionDownloadCount {
    time_bucket,
    scope,
    package,
    version,
    count,
  })
}

#[cfg(test)]
mod tests {
  use chrono::DateTime;
  use chrono::Utc;
  use serde_json::json;
  use uuid::Uuid;

  use crate::db::EphemeralDatabase;
  use crate::db::ExportsMap;
  use crate::db::NewPackageVersion;
  use crate::db::PackageVersionMeta;
  use crate::gcp::BigQueryQueryResult;
  use crate::ids::PackageName;
  use crate::ids::ScopeName;
  use crate::ids::Version;

  use super::deserialize_version_download_count_from_bigquery;

  #[test]
  fn test_deserialize_version_download_count_from_bigquery() {
    let value = json!({
      "f": [
        {
          "v": "1721131200000000"
        },
        {
          "v": "luca"
        },
        {
          "v": "flag"
        },
        {
          "v": "1.0.0"
        },
        {
          "v": "154"
        }
      ]
    });
    let res = deserialize_version_download_count_from_bigquery(&value);
    let data = res.unwrap();
    assert_eq!(data.time_bucket.timestamp_micros(), 1721131200000000);
    assert_eq!(data.scope.as_str(), "luca");
    assert_eq!(data.package.as_str(), "flag");
    assert_eq!(data.version.to_string(), "1.0.0");
    assert_eq!(data.count, 154);
  }

  #[tokio::test]
  async fn test_insert_bigquery_download_entries() {
    let db = EphemeralDatabase::create().await;

    let res: BigQueryQueryResult = serde_json::from_str(include_str!(
      "../testdata/bigquery_query_results.json"
    ))
    .unwrap();

    let std = ScopeName::new("std".to_owned()).unwrap();
    let fs = PackageName::new("fs".to_owned()).unwrap();
    let luca = ScopeName::new("luca".to_owned()).unwrap();
    let flag = PackageName::new("flag".to_owned()).unwrap();
    let v0_215_0 = Version::new("0.215.0").unwrap();
    let v0_219_3 = Version::new("0.219.3").unwrap();
    let v1_0_0 = Version::new("1.0.0").unwrap();

    db.create_scope(&std, Uuid::nil()).await.unwrap();
    db.create_scope(&luca, Uuid::nil()).await.unwrap();
    db.create_package(&std, &fs).await.unwrap();
    db.create_package(&luca, &flag).await.unwrap();
    db.create_package_version_for_test(NewPackageVersion {
      scope: &std,
      name: &fs,
      version: &v0_215_0,
      exports: &ExportsMap::mock(),
      user_id: None,
      readme_path: None,
      uses_npm: false,
      meta: PackageVersionMeta::default(),
    })
    .await
    .unwrap();
    db.create_package_version_for_test(NewPackageVersion {
      scope: &luca,
      name: &flag,
      version: &v1_0_0,
      exports: &ExportsMap::mock(),
      user_id: None,
      readme_path: None,
      uses_npm: false,
      meta: PackageVersionMeta::default(),
    })
    .await
    .unwrap();

    let rows = res.rows;
    super::insert_bigquery_download_entries(&db, rows)
      .await
      .unwrap();

    let downloads = db
      .get_package_version_downloads_4h(
        &std,
        &fs,
        &v0_215_0,
        "2024-06-01T00:00:00Z".parse().unwrap(),
        "2024-07-31T00:00:00Z".parse().unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(downloads.len(), 1, "{:?}", downloads);
    assert_eq!(
      downloads[0].time_bucket,
      "2024-07-16T12:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(downloads[0].count, 13);

    let downloads = db
      .get_package_version_downloads_4h(
        &luca,
        &flag,
        &v1_0_0,
        "2024-06-01T00:00:00Z".parse().unwrap(),
        "2024-07-31T00:00:00Z".parse().unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(downloads.len(), 2, "{:?}", downloads);
    assert_eq!(
      downloads[0].time_bucket,
      "2024-07-16T12:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(downloads[0].count, 196);
    assert_eq!(
      downloads[1].time_bucket,
      "2024-07-16T16:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(downloads[1].count, 42);

    // non existant version
    let downloads = db
      .get_package_version_downloads_4h(
        &std,
        &fs,
        &v0_219_3,
        "2024-06-01T00:00:00Z".parse().unwrap(),
        "2024-07-31T00:00:00Z".parse().unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(downloads.len(), 0, "{:?}", downloads);

    // time window with no data
    let downloads = db
      .get_package_version_downloads_4h(
        &std,
        &fs,
        &v0_215_0,
        "2024-06-01T00:00:00Z".parse().unwrap(),
        "2024-06-30T00:00:00Z".parse().unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(downloads.len(), 0, "{:?}", downloads);

    // 24 hour window
    let downloads = db
      .get_package_version_downloads_24h(
        &std,
        &fs,
        &v0_215_0,
        "2024-06-01T00:00:00Z".parse().unwrap(),
        "2024-07-31T00:00:00Z".parse().unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(downloads.len(), 1, "{:?}", downloads);
    assert_eq!(
      downloads[0].time_bucket,
      "2024-07-16T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(downloads[0].count, 13);

    let downloads = db
      .get_package_version_downloads_24h(
        &luca,
        &flag,
        &v1_0_0,
        "2024-06-01T00:00:00Z".parse().unwrap(),
        "2024-07-31T00:00:00Z".parse().unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(downloads.len(), 1, "{:?}", downloads);
    assert_eq!(
      downloads[0].time_bucket,
      "2024-07-16T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(downloads[0].count, 238);
  }
}
