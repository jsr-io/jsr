// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Publishing fixtures for this crate's tests, on top of the shared harness.

use bytes::Bytes;

use crate::errors::TaskError;
use crate::publish::publish_task;
use shared::db::CreatePackageResult;
use shared::db::CreatePublishingTaskResult;
use shared::external::cloudflare::CachePurge;
use shared::ids::PackageName;
use shared::ids::PackagePath;
use shared::ids::Version;
use shared::models::NewPublishingTask;
use shared::models::PublishingTask;
use shared::s3::S3UploadOptions;
use shared::s3_paths::bucket_tarball_path;
use shared::test::TestSetup;

pub async fn process_tarball_setup(
  t: &TestSetup,
  tarball_data: Bytes,
) -> PublishingTask {
  let package_name = PackageName::try_from("foo").unwrap();
  let version = Version::try_from("1.2.3").unwrap();
  process_tarball_setup2(t, tarball_data, &package_name, &version, false).await
}

pub async fn process_tarball_setup2(
  t: &TestSetup,
  tarball_data: Bytes,
  package_name: &PackageName,
  version: &Version,
  jsonc: bool,
) -> PublishingTask {
  try_process_tarball_setup2(t, tarball_data, package_name, version, jsonc)
    .await
    .unwrap()
}

/// Like [`process_tarball_setup2`], but surfaces a retryable publish failure
/// instead of panicking on it.
pub async fn try_process_tarball_setup2(
  t: &TestSetup,
  tarball_data: Bytes,
  package_name: &PackageName,
  version: &Version,
  jsonc: bool,
) -> Result<PublishingTask, TaskError> {
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

  let tarball_path = bucket_tarball_path(task.0.id);
  t.buckets
    .publishing_bucket
    .upload(
      tarball_path.into(),
      shared::s3::UploadTaskBody::Bytes(tarball_data),
      S3UploadOptions {
        content_type: Some("application/x-tar".into()),
        cache_control: None,
        gzip_encoded: true,
      },
    )
    .await
    .unwrap();

  publish_task(
    task.0.id,
    0,
    t.buckets(),
    t.license_store(),
    t.registry_url(),
    t.npm_url(),
    t.fallback_registry_url.clone(),
    t.db(),
    None,
    CachePurge(None),
  )
  .await?;
  Ok(
    t.db()
      .get_publishing_task(task.0.id)
      .await
      .unwrap()
      .unwrap()
      .0,
  )
}
