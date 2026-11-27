// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! The api's client for the tasks service.
//!
//! Work that has to run in the task handlers is requested over HTTP from the
//! service serving the `/tasks/*` routes: the tasks worker, through the
//! `TASKS` service binding, or `TASKS_URL` natively.

use anyhow::Context;
use serde::Serialize;
use url::Url;
use uuid::Uuid;

use shared::ids::PackageName;
use shared::ids::ScopeName;
use shared::ids::Version;
use shared::models::PackageVersionMeta;
use shared::util::shared_http_client;

#[derive(Debug)]
pub enum AnalyzeVersionError {
  /// The analysis itself rejected the package; the message is user facing.
  Failed(String),
  /// The analysis could not be run.
  Internal(anyhow::Error),
}

#[derive(Clone)]
pub struct TaskDispatch {
  base_url: Url,
}

impl TaskDispatch {
  /// `tasks_url` is the base the `/tasks/*` routes are served under.
  pub fn new(tasks_url: Url) -> Self {
    let base_url = if tasks_url.path().ends_with('/') {
      tasks_url
    } else {
      Url::parse(&format!("{tasks_url}/")).unwrap()
    };
    Self { base_url }
  }

  /// Posts `body` to a task route and returns the response once it has been
  /// checked for a success status.
  async fn post<B: Serialize + ?Sized>(
    &self,
    route: &str,
    body: &B,
  ) -> anyhow::Result<reqwest::Response> {
    let url = self.base_url.join(route).unwrap();
    let request = shared_http_client().post(url.clone()).json(body);
    #[cfg(target_arch = "wasm32")]
    let response = shared::worker_js::fetch_service("TASKS", request.build()?)
      .await
      .map_err(anyhow::Error::msg);
    #[cfg(not(target_arch = "wasm32"))]
    let response = request.send().await;
    let response = response.with_context(|| format!("requesting {url}"))?;
    let status = response.status();
    if !status.is_success() {
      let body = response.text().await.unwrap_or_default();
      anyhow::bail!("{url} responded {status}: {body}");
    }
    Ok(response)
  }

  /// Runs the publish task for a publishing task, for when there is no
  /// publish queue to hand it to. Resolves once the publish has finished, so
  /// callers spawn it rather than await it.
  pub async fn publish(&self, publishing_task_id: Uuid) -> anyhow::Result<()> {
    self.post("tasks/publish", &publishing_task_id).await?;
    Ok(())
  }

  /// Re-runs the publish analysis over a version's stored files and returns
  /// the recomputed meta.
  pub async fn recompute_version_meta(
    &self,
    scope: &ScopeName,
    package: &PackageName,
    version: &Version,
  ) -> Result<PackageVersionMeta, AnalyzeVersionError> {
    #[derive(Serialize)]
    struct Job<'a> {
      scope: &'a ScopeName,
      name: &'a PackageName,
      version: &'a Version,
    }

    let response = self
      .post(
        "tasks/recompute_version_meta",
        &Job {
          scope,
          name: package,
          version,
        },
      )
      .await
      .map_err(AnalyzeVersionError::Internal)?;
    let result: Result<PackageVersionMeta, String> = response
      .json()
      .await
      .context("decoding the tasks/recompute_version_meta response")
      .map_err(AnalyzeVersionError::Internal)?;
    result.map_err(AnalyzeVersionError::Failed)
  }
}
