// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::sync::Arc;

use async_tar::EntryType;
use bytes::Bytes;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use deno_semver::VersionReq;
use futures::stream;
use futures::AsyncReadExt;
use futures::StreamExt;
use futures::TryStreamExt;
use hyper::Body;
use hyper::Request;
use routerify::ext::RequestExt;
use routerify::Router;
use serde::Deserialize;
use serde::Serialize;
use tracing::field;
use tracing::instrument;
use tracing::Span;

use crate::analysis::rebuild_npm_tarball;
use crate::analysis::RebuildNpmTarballData;
use crate::analysis::RegistryLoader;
use crate::api::ApiError;
use crate::buckets::Buckets;
use crate::buckets::UploadTaskBody;
use crate::db::Database;
use crate::db::NewNpmTarball;
use crate::gcp;
use crate::gcp::GcsUploadOptions;
use crate::gcp::CACHE_CONTROL_DO_NOT_CACHE;
use crate::gcp::CACHE_CONTROL_IMMUTABLE;
use crate::gcs_paths;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::npm::generate_npm_version_manifest;
use crate::npm::NPM_TARBALL_REVISION;
use crate::publish;
use crate::tarball::gcs_tarball_path;
use crate::util;
use crate::util::decode_json;
use crate::util::ApiResult;
use crate::NpmUrl;

pub struct NpmTarballBuildQueue(pub Option<gcp::Queue>);

pub fn tasks_router() -> Router<Body, ApiError> {
  Router::builder()
    .post("/publish", util::json(publish::publish_handler))
    .post("/npm_tarball_build", util::json(npm_tarball_build_handler))
    .post(
      "/npm_tarball_enqueue",
      util::json(npm_tarball_enqueue_handler),
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
  let registry = req.data::<Arc<dyn RegistryLoader>>().unwrap().clone();
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

    let mut files: HashMap<PackagePath, Vec<u8>> = HashMap::new();

    let task = db
      .get_publish_task_by_package(&job.scope, &job.name, &job.version)
      .await?;

    if let Some(task) = task {
      let tarball_path = gcs_tarball_path(task.id);
      let stream = buckets
        .publishing_bucket
        .bucket
        .download_stream(&tarball_path, None)
        .await?
        .ok_or_else(|| ApiError::PackageVersionNotFound)?;

      let async_read = stream
        .map(|v| match v {
          Ok(v) => Ok(v),
          Err(err) => Err(std::io::Error::other(err)),
        })
        .into_async_read();
      let mut tar = async_tar::Archive::new(async_read).entries().unwrap();

      while let Some(res) = tar.next().await {
        let mut entry = res?;

        let header = entry.header();
        let path = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
        let path = if path.starts_with("./") {
          path[1..].to_string()
        } else if !path.starts_with('/') {
          format!("/{}", path)
        } else {
          path
        };

        if header.entry_type() == EntryType::Regular {
          let path = PackagePath::new(path.clone()).unwrap();

          let mut bytes = Vec::new();
          entry.read_to_end(&mut bytes).await?;
          files.insert(path, bytes);
        }
      }
    }

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
      rebuild_npm_tarball(span, registry, buckets.modules_bucket, data)
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
