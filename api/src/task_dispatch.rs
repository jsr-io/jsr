// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use uuid::Uuid;

use shared::ids::PackageName;
use shared::ids::PackagePath;
use shared::ids::ScopeName;
use shared::ids::Version;
use shared::models::ExportsMap;
use shared::models::PackageVersionMeta;

pub struct AnalyzeVersionRequest {
  pub scope: ScopeName,
  pub package: PackageName,
  pub version: Version,
  pub config_file: PackagePath,
  pub exports: ExportsMap,
  pub files: HashMap<PackagePath, Vec<u8>>,
}

#[derive(Debug)]
pub enum AnalyzeVersionError {
  Failed(String),
  Internal(anyhow::Error),
}

type PublishFn = dyn Fn(Uuid) -> BoxFuture<'static, ()> + Send + Sync;
type AnalyzeFn = dyn Fn(
    AnalyzeVersionRequest,
  ) -> BoxFuture<'static, Result<PackageVersionMeta, AnalyzeVersionError>>
  + Send
  + Sync;

#[derive(Clone)]
pub struct TaskDispatch {
  publish: Arc<PublishFn>,
  analyze: Arc<AnalyzeFn>,
}

impl TaskDispatch {
  pub fn new(publish: Arc<PublishFn>, analyze: Arc<AnalyzeFn>) -> Self {
    Self { publish, analyze }
  }

  pub fn publish(&self, publishing_task_id: Uuid) -> BoxFuture<'static, ()> {
    (self.publish)(publishing_task_id)
  }

  pub fn analyze(
    &self,
    req: AnalyzeVersionRequest,
  ) -> BoxFuture<'static, Result<PackageVersionMeta, AnalyzeVersionError>> {
    (self.analyze)(req)
  }
}
