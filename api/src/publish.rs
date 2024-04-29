// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::collections::HashSet;

use crate::api::ApiError;
use crate::buckets::Buckets;
use crate::buckets::UploadTaskBody;
use crate::db::DependencyKind;
use crate::db::ExportsMap;
use crate::db::NewNpmTarball;
use crate::db::NewPackageFile;
use crate::db::NewPackageVersion;
use crate::db::NewPackageVersionDependency;
use crate::db::PublishingTask;
use crate::db::PublishingTaskError;
use crate::db::PublishingTaskStatus;
use crate::db::{Database, PackageVersionMeta};
use crate::gcp::GcsUploadOptions;
use crate::gcp::CACHE_CONTROL_DO_NOT_CACHE;
use crate::gcp::CACHE_CONTROL_IMMUTABLE;
use crate::ids::PackagePath;
use crate::metadata::ManifestEntry;
use crate::metadata::PackageMetadata;
use crate::metadata::VersionMetadata;
use crate::npm::generate_npm_version_manifest;
use crate::npm::NPM_TARBALL_REVISION;
use crate::orama::OramaClient;
use crate::tarball::process_tarball;
use crate::tarball::NpmTarballInfo;
use crate::tarball::ProcessTarballOutput;
use crate::util::decode_json;
use crate::util::ApiResult;
use crate::NpmUrl;
use crate::RegistryUrl;
use deno_semver::package::PackageReqReference;
use hyper::Body;
use hyper::Request;
use indexmap::IndexMap;
use routerify::ext::RequestExt;
use tracing::error;
use tracing::instrument;
use url::Url;
use uuid::Uuid;

#[instrument(
  name = "POST /tasks/publish",
  skip(req),
  err,
  fields(publishing_task_id)
)]
pub async fn publish_handler(mut req: Request<Body>) -> ApiResult<()> {
  let publishing_task_id: Uuid = decode_json(&mut req).await?;

  let db = req.data::<Database>().unwrap().clone();
  let buckets = req.data::<Buckets>().unwrap().clone();
  let orama_client = req.data::<Option<OramaClient>>().unwrap().clone();
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();
  let npm_url = req.data::<NpmUrl>().unwrap().0.clone();

  publish_task(
    publishing_task_id,
    buckets,
    registry_url,
    npm_url,
    db,
    orama_client,
  )
  .await?;

  Ok(())
}

#[instrument(
  name = "publish_task",
  skip(buckets, db, registry_url, orama_client),
  err
)]
pub async fn publish_task(
  publish_id: Uuid,
  buckets: Buckets,
  registry_url: Url,
  npm_url: Url,
  db: Database,
  orama_client: Option<OramaClient>,
) -> Result<(), ApiError> {
  let mut publishing_task = db
    .get_publishing_task(publish_id)
    .await?
    .ok_or(ApiError::PublishNotFound)?;
  loop {
    // If the task is pending, we can start processing it. If the task is
    // already processing, don't do anything. If the task is already
    // processed, we can skip processing and go straight to uploading the
    // package metadata file. If the task is failed or succeeded, we can
    // just return.
    match publishing_task.status {
      PublishingTaskStatus::Pending => {
        let res = process_publishing_task(
          &db,
          &buckets,
          registry_url.clone(),
          &mut publishing_task,
        )
        .await;
        if let Err(err) = res {
          // retryable errors
          db.update_publishing_task_status(
            publishing_task.id,
            PublishingTaskStatus::Processing,
            PublishingTaskStatus::Pending,
            None,
          )
          .await?;
          return Err(err.into());
        }
      }
      PublishingTaskStatus::Processing => {
        error!("publishing task already processing");
        return Err(ApiError::InternalServerError);
      }
      PublishingTaskStatus::Processed => {
        upload_package_manifest(&db, &buckets, &publishing_task).await?;
        upload_npm_version_manifest(&db, &buckets, &npm_url, &publishing_task)
          .await?;
        publishing_task = db
          .update_publishing_task_status(
            publishing_task.id,
            PublishingTaskStatus::Processed,
            PublishingTaskStatus::Success,
            None,
          )
          .await?;
      }
      PublishingTaskStatus::Failure => return Ok(()),
      PublishingTaskStatus::Success => {
        if let Some(orama_client) = orama_client {
          let (package, _, meta) = db
            .get_package(
              &publishing_task.package_scope,
              &publishing_task.package_name,
            )
            .await?
            .ok_or_else(|| ApiError::InternalServerError)?;
          orama_client.upsert_package(&package, &meta);
        }
        return Ok(());
      }
    }
  }
}

async fn process_publishing_task(
  db: &Database,
  buckets: &Buckets,
  registry_url: Url,
  publishing_task: &mut PublishingTask,
) -> Result<(), anyhow::Error> {
  *publishing_task = db
    .update_publishing_task_status(
      publishing_task.id,
      PublishingTaskStatus::Pending,
      PublishingTaskStatus::Processing,
      None,
    )
    .await?;

  let output =
    match process_tarball(db, buckets, registry_url, publishing_task).await {
      Ok(output) => output,
      Err(err) => match err.user_error_code() {
        Some(code) => {
          // non retryable, fatal error
          error!("Error processing tarball, fatal: {}", err);
          *publishing_task = db
            .update_publishing_task_status(
              publishing_task.id,
              PublishingTaskStatus::Processing,
              PublishingTaskStatus::Failure,
              Some(PublishingTaskError {
                code: code.to_owned(),
                message: err.to_string(),
              }),
            )
            .await?;
          return Ok(());
        }
        None => {
          // retryable errors
          return Err(anyhow::Error::from(err));
        }
      },
    };

  let ProcessTarballOutput {
    file_infos,
    module_graph_2,
    exports,
    dependencies,
    npm_tarball_info,
    readme_path,
    meta,
  } = output;

  upload_version_manifest(
    buckets,
    publishing_task,
    &file_infos,
    exports.clone().into_inner(),
    module_graph_2,
  )
  .await?;

  create_package_version_and_npm_tarball_and_update_publishing_task(
    db,
    publishing_task,
    &file_infos,
    exports,
    dependencies,
    &npm_tarball_info,
    readme_path,
    meta,
  )
  .await?;

  Ok(())
}

async fn upload_version_manifest(
  buckets: &Buckets,
  publishing_task: &PublishingTask,
  file_infos: &[crate::tarball::FileInfo],
  exports: IndexMap<String, String>,
  module_graph_2: HashMap<String, deno_graph::ModuleInfo>,
) -> Result<(), anyhow::Error> {
  let version_metadata_gcs_path = crate::gcs_paths::version_metadata(
    &publishing_task.package_scope,
    &publishing_task.package_name,
    &publishing_task.package_version,
  );
  let manifest = file_infos
    .iter()
    .map(|file_info| {
      (
        file_info.path.clone(),
        ManifestEntry {
          checksum: file_info.hash.clone(),
          size: file_info.size as usize,
        },
      )
    })
    .collect();
  let version_metadata = VersionMetadata {
    exports,
    manifest,
    module_graph_2,
  };
  let content = serde_json::to_vec_pretty(&version_metadata)?;
  buckets
    .modules_bucket
    .upload(
      version_metadata_gcs_path.into(),
      UploadTaskBody::Bytes(content.into()),
      GcsUploadOptions {
        content_type: Some("application/json".into()),
        cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
        gzip_encoded: false,
      },
    )
    .await?;

  Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_package_version_and_npm_tarball_and_update_publishing_task(
  db: &Database,
  publishing_task: &mut PublishingTask,
  file_infos: &[crate::tarball::FileInfo],
  exports: ExportsMap,
  dependencies: HashSet<(DependencyKind, PackageReqReference)>,
  npm_tarball_info: &NpmTarballInfo,
  readme_path: Option<PackagePath>,
  meta: PackageVersionMeta,
) -> Result<(), anyhow::Error> {
  let uses_npm = dependencies
    .iter()
    .any(|(kind, _)| kind == &DependencyKind::Npm);

  let new_package_version = NewPackageVersion {
    scope: &publishing_task.package_scope,
    name: &publishing_task.package_name,
    version: &publishing_task.package_version,
    user_id: publishing_task.user_id.as_ref(),
    readme_path: readme_path.as_ref(),
    uses_npm,
    exports: &exports,
    meta,
  };

  let new_package_files = file_infos
    .iter()
    .map(|file| NewPackageFile {
      scope: &publishing_task.package_scope,
      name: &publishing_task.package_name,
      version: &publishing_task.package_version,
      path: &file.path,
      size: file.size as i32,
      checksum: Some(&file.hash),
    })
    .collect::<Vec<_>>();

  let new_package_version_dependencies = dependencies
    .iter()
    .map(|(kind, req)| NewPackageVersionDependency {
      package_scope: &publishing_task.package_scope,
      package_name: &publishing_task.package_name,
      package_version: &publishing_task.package_version,
      dependency_kind: *kind,
      dependency_name: &req.req.name,
      dependency_constraint: req.req.version_req.version_text(),
      dependency_path: req.sub_path.as_deref().unwrap_or(""),
    })
    .collect::<Vec<_>>();

  let new_npm_tarball = NewNpmTarball {
    scope: &publishing_task.package_scope,
    name: &publishing_task.package_name,
    version: &publishing_task.package_version,
    revision: NPM_TARBALL_REVISION as i32,
    sha1: &npm_tarball_info.sha1,
    sha512: &npm_tarball_info.sha512,
    size: npm_tarball_info.size as i32,
  };

  *publishing_task = db
    .create_package_version_and_npm_tarball_and_finalize_publishing_task(
      publishing_task.id,
      new_package_version,
      &new_package_files,
      &new_package_version_dependencies,
      new_npm_tarball,
    )
    .await?;

  Ok(())
}

async fn upload_package_manifest(
  db: &Database,
  buckets: &Buckets,
  publishing_task: &PublishingTask,
) -> Result<(), anyhow::Error> {
  let package_metadata_gcs_path = crate::gcs_paths::package_metadata(
    &publishing_task.package_scope,
    &publishing_task.package_name,
  );
  let package_metadata = PackageMetadata::create(
    db,
    &publishing_task.package_scope,
    &publishing_task.package_name,
  )
  .await?;
  let content = serde_json::to_vec_pretty(&package_metadata)?;
  buckets
    .modules_bucket
    .upload(
      package_metadata_gcs_path.into(),
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

async fn upload_npm_version_manifest(
  db: &Database,
  buckets: &Buckets,
  npm_url: &Url,
  publishing_task: &PublishingTask,
) -> Result<(), anyhow::Error> {
  let npm_version_manifest_path_gcs_path =
    crate::gcs_paths::npm_version_manifest_path(
      &publishing_task.package_scope,
      &publishing_task.package_name,
    );
  let npm_version_manifest = generate_npm_version_manifest(
    db,
    npm_url,
    &publishing_task.package_scope,
    &publishing_task.package_name,
  )
  .await?;
  let content = serde_json::to_vec_pretty(&npm_version_manifest)?;
  buckets
    .npm_bucket
    .upload(
      npm_version_manifest_path_gcs_path.into(),
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

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::api::ApiPublishingTask;
  use crate::db::CreatePackageResult;
  use crate::db::CreatePublishingTaskResult;
  use crate::db::NewPublishingTask;
  use crate::ids::ScopeName;
  use crate::ids::Version;
  use crate::ids::{PackageName, PackagePath};
  use crate::metadata::VersionMetadata;
  use crate::tarball::gcs_tarball_path;
  use crate::tarball::ConfigFile;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;
  use bytes::Bytes;
  use deno_graph::ModuleInfo;
  use flate2::write::GzEncoder;
  use flate2::Compression;
  use hyper::StatusCode;
  use serde_json::json;
  use std::collections::HashMap;
  use std::io::Write;

  pub async fn process_tarball_setup(
    t: &TestSetup,
    tarball_data: Bytes,
  ) -> PublishingTask {
    let package_name = PackageName::try_from("foo").unwrap();
    let version = Version::try_from("1.2.3").unwrap();
    process_tarball_setup2(t, tarball_data, &package_name, &version, false)
      .await
  }

  pub async fn process_tarball_setup2(
    t: &TestSetup,
    tarball_data: Bytes,
    package_name: &PackageName,
    version: &Version,
    jsonc: bool,
  ) -> PublishingTask {
    let scope_name = "scope".try_into().unwrap();

    let res = t
      .db()
      .create_package(&scope_name, package_name)
      .await
      .unwrap();
    assert!(
      matches!(res, CreatePackageResult::Ok(_))
        || matches!(res, CreatePackageResult::AlreadyExists)
    );

    let CreatePublishingTaskResult::Created(task) = t
      .db()
      .create_publishing_task(NewPublishingTask {
        user_id: Some(t.user1.user.id),
        package_scope: &scope_name,
        package_name,
        package_version: version,
        config_file: &PackagePath::try_from(format!(
          "/jsr.json{}",
          if jsonc { "c" } else { "" }
        ))
        .unwrap(),
      })
      .await
      .unwrap()
    else {
      unreachable!()
    };

    let tarball_path = gcs_tarball_path(task.id);
    t.buckets
      .publishing_bucket
      .upload(
        tarball_path.into(),
        UploadTaskBody::Bytes(tarball_data),
        GcsUploadOptions {
          content_type: Some("application/x-tar".into()),
          cache_control: None,
          gzip_encoded: true,
        },
      )
      .await
      .unwrap();

    publish_task(
      task.id,
      t.buckets(),
      t.registry_url(),
      t.npm_url(),
      t.db(),
      None,
    )
    .await
    .unwrap();
    t.db().get_publishing_task(task.id).await.unwrap().unwrap()
  }

  pub fn create_mock_tarball(name: &str) -> Bytes {
    let mut tar_bytes = Vec::new();
    let mut tar = tar::Builder::new(&mut tar_bytes);
    tar
      .append_dir_all("./", format!("./testdata/tarballs/{name}/"))
      .unwrap();
    tar.finish().unwrap();
    drop(tar);

    let mut gz_bytes = Vec::new();
    let mut encoder = GzEncoder::new(&mut gz_bytes, Compression::default());
    encoder.write_all(&tar_bytes).unwrap();
    encoder.finish().unwrap();

    Bytes::from(gz_bytes)
  }

  pub fn create_case_insensitive_mock_tarball() -> Bytes {
    let mut tar_bytes = Vec::new();
    let mut tar = tar::Builder::new(&mut tar_bytes);
    tar
      .append_dir_all("./", "./testdata/tarballs/case_insensitive/")
      .unwrap();
    let mut file =
      std::fs::File::open("./testdata/tarballs/case_insensitive/README.md")
        .unwrap();
    tar.append_file("./readme.md", &mut file).unwrap();
    tar.finish().unwrap();
    drop(tar);

    let mut gz_bytes = Vec::new();
    let mut encoder = GzEncoder::new(&mut gz_bytes, Compression::default());
    encoder.write_all(&tar_bytes).unwrap();
    encoder.finish().unwrap();

    Bytes::from(gz_bytes)
  }

  pub fn create_invalid_path_mock_tarball() -> Bytes {
    let mut tar_bytes = Vec::new();
    let mut tar = tar::Builder::new(&mut tar_bytes);
    tar.append_dir_all("./", "./testdata/tarballs/ok/").unwrap();
    let mut file =
      std::fs::File::open("./testdata/tarballs/ok/mod.ts").unwrap();
    tar.append_file("./CON.ts", &mut file).unwrap();
    tar.finish().unwrap();
    drop(tar);

    let mut gz_bytes = Vec::new();
    let mut encoder = GzEncoder::new(&mut gz_bytes, Compression::default());
    encoder.write_all(&tar_bytes).unwrap();
    encoder.finish().unwrap();

    gz_bytes.into()
  }

  #[tokio::test]
  async fn payload_too_large() {
    let body = Body::from(vec![0; 999999999]);

    let mut t = TestSetup::new().await;
    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json")
      .gzip()
      .body(body)
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(
        StatusCode::PAYLOAD_TOO_LARGE,
        "tarballSizeLimitExceeded",
      )
      .await;
  }

  #[tokio::test]
  async fn payload_too_large_stream() {
    // Convert the Vec<u8> into a hyper Body with chunked transfer encoding
    let body = Body::wrap_stream(tokio_stream::once(Ok::<_, std::io::Error>(
      vec![0; 999999999],
    )));

    let mut t = TestSetup::new().await;
    let name = PackageName::new("foo".to_owned()).unwrap();
    t.db().create_package(&t.scope.scope, &name).await.unwrap();
    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json")
      .gzip()
      .body(body)
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(
        StatusCode::PAYLOAD_TOO_LARGE,
        "tarballSizeLimitExceeded",
      )
      .await;
  }

  #[tokio::test]
  async fn stream_success() {
    let mut t = TestSetup::new().await;
    let name = PackageName::new("foo".to_owned()).unwrap();
    t.db().create_package(&t.scope.scope, &name).await.unwrap();

    let data = create_mock_tarball("ok");

    // Convert the Vec<u8> into a hyper Body with chunked transfer encoding
    let body_stream = tokio_stream::once(Ok::<_, std::io::Error>(data));
    let body = Body::wrap_stream(body_stream);

    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json")
      .gzip()
      .body(body)
      .call()
      .await
      .unwrap();
    let _task = resp.expect_ok::<ApiPublishingTask>().await;
    // todo: await task completion
  }

  #[tokio::test]
  async fn content_type() {
    let t = TestSetup::new().await;
    let task = process_tarball_setup(&t, create_mock_tarball("with_svg")).await;
    assert_eq!(task.status, PublishingTaskStatus::Success);
    let response = t
      .buckets
      .modules_bucket
      .bucket
      .download_resp("@scope/foo/1.2.3/jsr.json")
      .await
      .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "application/json");
    let response = t
      .buckets
      .modules_bucket
      .bucket
      .download_resp("@scope/foo/1.2.3/mod.ts")
      .await
      .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "text/typescript");
    let response = t
      .buckets
      .modules_bucket
      .bucket
      .download_resp("@scope/foo/1.2.3/logo.svg")
      .await
      .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "image/svg+xml");
  }

  #[tokio::test]
  async fn success_data_url() {
    let t = TestSetup::new().await;
    let task = process_tarball_setup(&t, create_mock_tarball("data_url")).await;
    assert_eq!(
      task.status,
      PublishingTaskStatus::Success,
      "publishing task failed {task:?}"
    );
  }

  #[tokio::test]
  async fn success_dynamic_import() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_mock_tarball("dynamic_import")).await;
    assert_eq!(
      task.status,
      PublishingTaskStatus::Success,
      "publishing task failed {task:?}"
    );

    let dependencies = t
      .db()
      .list_package_version_dependencies(
        &task.package_scope,
        &task.package_name,
        &task.package_version,
      )
      .await
      .unwrap();

    assert_eq!(dependencies.len(), 2);
    assert_eq!(dependencies[0].dependency_kind, DependencyKind::Npm);
    assert_eq!(dependencies[0].dependency_name, "chalk");
    assert_eq!(dependencies[1].dependency_kind, DependencyKind::Npm);
    assert_eq!(dependencies[1].dependency_name, "express");
  }

  #[tokio::test]
  async fn not_allowed() {
    let mut t = TestSetup::new().await;

    let data = create_mock_tarball("ok");

    // Convert the Vec<u8> into a hyper Body with chunked transfer encoding
    let body_stream = tokio_stream::once(Ok::<_, std::io::Error>(data));
    let body = Body::wrap_stream(body_stream);

    let token = t.user2.token.clone();

    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json")
      .gzip()
      .token(Some(&token))
      .body(body)
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;
  }

  #[tokio::test]
  async fn only_from_ci() {
    let mut t = TestSetup::new().await;

    let data = create_mock_tarball("ok");

    t.db()
      .scope_set_require_publishing_from_ci(&t.scope.scope, true)
      .await
      .unwrap();

    let token = t.user2.token.clone();

    let mut resp: hyper::Response<Body> = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json")
      .gzip()
      .token(Some(&token))
      .body(Body::from(data))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "scopeRequiresPublishingFromCi")
      .await;
  }

  #[tokio::test]
  async fn success() {
    let t = TestSetup::new().await;
    let task = process_tarball_setup(&t, create_mock_tarball("ok")).await;
    assert_eq!(task.status, PublishingTaskStatus::Success);
    let json = t
      .buckets
      .modules_bucket
      .download("@scope/foo/1.2.3/jsr.json".into())
      .await
      .unwrap()
      .unwrap();
    let deno_json: ConfigFile = serde_json::from_slice(&json).unwrap();
    assert_eq!(deno_json.name.to_string(), "@scope/foo");
    assert_eq!(deno_json.version.to_string(), "1.2.3");
    {
      let metadata_json = t
        .buckets
        .modules_bucket
        .download("@scope/foo/1.2.3_meta.json".into())
        .await
        .unwrap()
        .unwrap();
      let metadata_json: VersionMetadata =
        serde_json::from_slice(&metadata_json).unwrap();
      assert_eq!(metadata_json.exports.len(), 1);
      assert_eq!(metadata_json.exports.get(".").unwrap(), "./mod.ts");
      assert_eq!(
        serde_json::to_value(metadata_json.manifest).unwrap(),
        serde_json::json!({
            "/jsr.json": {
                "checksum": "sha256-404be7a6cf542ac6ee2c4ba0c9d6a2101e0c0aeee42fe24739a94432646541ac",
                "size": 74
            },
            "/mod.ts": {
                "checksum": "sha256-cac3d193853f12ab7247f20458587cfb20df7a77b5c2583aae5a309752908c16",
                "size": 124
            }
        })
      );
      assert_eq!(
        metadata_json.module_graph_2,
        HashMap::from_iter([(
          "/mod.ts".to_string(),
          ModuleInfo {
            dependencies: vec![],
            ts_references: vec![],
            self_types_specifier: None,
            jsx_import_source: None,
            jsx_import_source_types: None,
            jsdoc_imports: vec![]
          }
        )])
      );
    }
    let scope_name = ScopeName::try_from("scope").unwrap();
    let package_name = PackageName::try_from("foo").unwrap();
    let version = Version::try_from("1.2.3").unwrap();
    let package_version = t
      .db()
      .get_package_version(&scope_name, &package_name, &version)
      .await
      .unwrap()
      .unwrap();
    assert_eq!(package_version.version, version);

    let files = t
      .db()
      .list_package_files(&scope_name, &package_name, &version)
      .await
      .unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "/jsr.json".try_into().unwrap());
    assert_eq!(files[1].path, "/mod.ts".try_into().unwrap());
    assert_eq!(
      files[1].size,
      include_bytes!("../testdata/tarballs/ok/mod.ts").len() as i32
    );

    let package_metadata: PackageMetadata = {
      let json = t
        .buckets
        .modules_bucket
        .download("@scope/foo/meta.json".into())
        .await
        .unwrap()
        .unwrap();
      serde_json::from_slice(&json).unwrap()
    };
    assert_eq!(package_metadata.name, package_name);
    assert_eq!(package_metadata.latest, Some(version));
    assert_eq!(package_metadata.versions.len(), 1);
  }

  #[tokio::test]
  async fn module_graph() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_mock_tarball("module_graph")).await;
    assert_eq!(
      task.status,
      PublishingTaskStatus::Success,
      "{:?}",
      task.error
    );

    let metadata_json = t
      .buckets
      .modules_bucket
      .download("@scope/foo/1.2.3_meta.json".into())
      .await
      .unwrap()
      .unwrap();
    let metadata_json: VersionMetadata =
      serde_json::from_slice(&metadata_json).unwrap();

    println!(
      "{}",
      serde_json::to_string(&metadata_json.module_graph_2).unwrap()
    );

    let expected = json!({
      "/test.js": {
        "selfTypesSpecifier": {
          "text": "./test.d.ts",
          "range": [[0,18],[0,31]]
        }
      },
      "/mod.tsx": {
        "dependencies": [
          {
            "type": "static",
            "kind": "import",
            "specifier": "./test.js",
            "specifierRange": [[3,15],[3,26]]
          },
          {
            "type": "static",
            "kind": "import",
            "typesSpecifier":{
              "text": "./jsr.d.ts",
              "range": [[5,13],[5,25]]
            },
            "specifier": "./jsr.json",
            "specifierRange": [[6,7],[6,19]],
            "importAttributes": { "known": { "type" : "json" } }
          }
        ],
        "jsxImportSource": {
          "text": "npm:react@18",
          "range": [[0,21],[0,33]]
        },
        "jsxImportSourceTypes": {
          "text": "npm:@types/react@18",
          "range": [[1,26],[1,45]]
        }
      },
      "/test.d.ts": {}
    });
    let expected: HashMap<String, ModuleInfo> =
      serde_json::from_value(expected).unwrap();

    pretty_assertions::assert_eq!(metadata_json.module_graph_2, expected);
  }

  #[tokio::test]
  async fn bad_version() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_mock_tarball("bad_version")).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "invalidConfigFile");
  }

  #[tokio::test]
  async fn name_mismatch() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_mock_tarball("name_mismatch")).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "configFileNameMismatch");
  }

  #[tokio::test]
  async fn version_mismatch() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_mock_tarball("version_mismatch")).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "configFileVersionMismatch");
  }

  #[tokio::test]
  async fn big_file() {
    let t = TestSetup::new().await;
    let task = process_tarball_setup(&t, create_mock_tarball("big_file")).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "fileTooLarge");
  }

  #[tokio::test]
  async fn case_insensitive_duplicate() {
    let t = TestSetup::new().await;
    let bytes = create_case_insensitive_mock_tarball();
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "caseInsensitiveDuplicatePath");
  }

  #[tokio::test]
  async fn case_insensitive_exports_reference() {
    let t = TestSetup::new().await;
    let task = process_tarball_setup(
      &t,
      create_mock_tarball("case_insensitive_exports_reference"),
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "configFileExportsInvalid");
  }

  #[tokio::test]
  async fn case_insensitive_dep_reference() {
    let t = TestSetup::new().await;
    let task = process_tarball_setup(
      &t,
      create_mock_tarball("case_insensitive_dep_reference"),
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "graphError");
    assert_eq!(error.message, "failed to build module graph: Module not found \"file:///Youtube.tsx\".\n    at file:///mod.ts:1:8");
  }

  #[tokio::test]
  async fn no_exports() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_mock_tarball("no_exports")).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "configFileExportsInvalid");
  }

  #[tokio::test]
  async fn invalid_exports() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_mock_tarball("invalid_exports")).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "configFileExportsInvalid");
  }

  #[tokio::test]
  async fn exports_not_found() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_mock_tarball("exports_not_found")).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "configFileExportsInvalid");
  }

  #[tokio::test]
  async fn invalid_path() {
    let t = TestSetup::new().await;
    let task =
      process_tarball_setup(&t, create_invalid_path_mock_tarball()).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "invalidPath");
  }

  #[tokio::test]
  async fn import_assertions() {
    let t = TestSetup::new().await;

    let bytes = create_mock_tarball("import_assertions");
    let task = process_tarball_setup2(
      &t,
      bytes,
      &PackageName::try_from("foo").unwrap(),
      &Version::try_from("1.2.3").unwrap(),
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "bannedImportAssertion");
    assert_eq!(error.message, "import assertions are not allowed, use import attributes instead (replace 'assert' with 'with') file:///mod.ts:1:29");
  }

  #[tokio::test]
  async fn import_attributes() {
    let t = TestSetup::new().await;

    let bytes = create_mock_tarball("import_attributes");
    let task = process_tarball_setup2(
      &t,
      bytes,
      &PackageName::try_from("foo").unwrap(),
      &Version::try_from("1.2.3").unwrap(),
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{task:#?}");
  }

  #[tokio::test]
  async fn jsr_jsonc() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("jsr_jsonc");
    let package_name = PackageName::try_from("foo").unwrap();
    let version = Version::try_from("1.2.3").unwrap();
    let task =
      process_tarball_setup2(&t, bytes, &package_name, &version, true).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{task:#?}");
  }

  #[tokio::test]
  async fn https_import() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("https_import");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "invalidExternalImport");
    assert_eq!(error.message, "invalid external import to 'https://deno.land/r/std/http/server.ts', only 'jsr:', 'npm:', 'data:' and 'node:' imports are allowed (http(s) import)");
  }

  async fn uses_npm(t: &TestSetup, task: &crate::db::PublishingTask) -> bool {
    t.db()
      .get_package_version(
        &task.package_scope,
        &task.package_name,
        &task.package_version,
      )
      .await
      .unwrap()
      .unwrap()
      .uses_npm
  }

  #[tokio::test]
  async fn npm_import() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("npm_import");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{task:#?}");
    assert!(uses_npm(&t, &task).await);
  }

  #[tokio::test]
  async fn jsr_import() {
    let t = TestSetup::new().await;

    let bytes = create_mock_tarball("ok");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{task:#?}");

    let bytes = create_mock_tarball("jsr_import");
    let task = process_tarball_setup2(
      &t,
      bytes,
      &PackageName::try_from("bar").unwrap(),
      &Version::try_from("1.2.3").unwrap(),
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{task:#?}");
    assert!(!uses_npm(&t, &task).await);
  }

  #[tokio::test]
  async fn jsr_import_missing_dependency() {
    let t = TestSetup::new().await;

    let bytes = create_mock_tarball("jsr_import");
    let task = process_tarball_setup2(
      &t,
      bytes,
      &PackageName::try_from("bar").unwrap(),
      &Version::try_from("1.2.3").unwrap(),
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "unresolvableJsrDependency");
    assert_eq!(error.message, "unresolvable 'jsr:' dependency: '@scope/foo@1', no published version matches the constraint");
  }

  #[tokio::test]
  async fn jsr_import_without_constraint() {
    let t = TestSetup::new().await;

    let bytes = create_mock_tarball("ok");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{task:#?}");

    let bytes = create_mock_tarball("jsr_import_without_constraint");
    let task = process_tarball_setup2(
      &t,
      bytes,
      &PackageName::try_from("bar").unwrap(),
      &Version::try_from("1.2.3").unwrap(),
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    assert_eq!(task.error.unwrap().code, "jsrMissingConstraint");
  }

  #[tokio::test]
  async fn syntax_error() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("syntax_error");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "graphError");
    assert_eq!(error.message, "failed to build module graph: The module's source code could not be parsed: Expression expected at file:///mod.ts:1:27\n\n  const invalidTypeScript = ;\n                            ~");
  }

  #[tokio::test]
  async fn syntax_error_extra_file() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("syntax_error_extra_file");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{task:#?}");
  }

  #[tokio::test]
  async fn syntax_error_in_graph() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("syntax_error_in_graph");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "graphError");
    assert_eq!(error.message, "failed to build module graph: The module's source code could not be parsed: Expression expected at file:///other.js:1:27\n\n  const invalidJavaScript = ;\n                            ~");
  }

  #[tokio::test]
  async fn non_utf8() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("non_utf8");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "graphError");
    assert_eq!(error.message, "failed to build module graph: invalid data");
  }

  #[tokio::test]
  async fn no_jsr_json() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("no_jsr_json");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "missingConfigFile");
  }

  #[tokio::test]
  async fn no_long_paths() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("no_long_paths");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "invalidPath");
    assert!(error.message.contains("a_very_long_filename_created_specifically_to_test_the_limitations_of_the_set_path_method_in_the_rust_tar_crate_exceeding_one_hundred_bytes"));
  }

  #[tokio::test]
  async fn global_type_augmentation1() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("global_type_augmentation1");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "globalTypeAugmentation");
  }

  #[tokio::test]
  async fn global_type_augmentation2() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("global_type_augmentation2");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Failure, "{task:#?}");
    let error = task.error.unwrap();
    assert_eq!(error.code, "globalTypeAugmentation");
  }

  #[tokio::test]
  async fn triple_slash_reference_in_jsdoc() {
    let t = TestSetup::new().await;
    let bytes = create_mock_tarball("triple_slash_reference_in_jsdoc");
    let task = process_tarball_setup(&t, bytes).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{task:#?}");
  }

  #[tokio::test]
  async fn npm_tarball() {
    let t = TestSetup::new().await;
    let task = process_tarball_setup(&t, create_mock_tarball("ok")).await;
    assert_eq!(task.status, PublishingTaskStatus::Success);

    let response = t
      .buckets
      .npm_bucket
      .bucket
      .download_resp("@jsr/scope__foo")
      .await
      .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "application/json");
    let mut json: serde_json::Value = response.json().await.unwrap();
    json.as_object_mut().unwrap().remove("time");
    let dist = json
      .as_object_mut()
      .unwrap()
      .get_mut("versions")
      .unwrap()
      .get_mut("1.2.3")
      .unwrap()
      .get_mut("dist")
      .unwrap()
      .as_object_mut()
      .unwrap();
    dist.remove("shasum");
    dist.remove("integrity");

    let tarball_url = format!(
      "http://npm.jsr-tests.test/~/{}/@jsr/scope__foo/1.2.3.tgz",
      NPM_TARBALL_REVISION
    );
    assert_eq!(
      json,
      serde_json::json!({
        "name": "@jsr/scope__foo",
        "description": "",
        "dist-tags": {
          "latest": "1.2.3"
        },
        "versions": {
          "1.2.3": {
            "name": "@jsr/scope__foo",
            "version": "1.2.3",
            "description": "",
            "dist": {
              "tarball": tarball_url
            },
            "dependencies": {},
          }
        },
      })
    );

    let res_url =
      format!("~/{}/@jsr/scope__foo/1.2.3.tgz", NPM_TARBALL_REVISION);

    let response = t
      .buckets
      .npm_bucket
      .bucket
      .download_resp(res_url.as_str())
      .await
      .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
      response.headers()["content-type"],
      "application/octet-stream"
    );
  }
}
