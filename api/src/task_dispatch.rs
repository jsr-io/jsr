// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! The api's client for the tasks service.
//!
//! Work that has to run in the task handlers is requested over HTTP from the
//! service serving the `/tasks/*` routes (`TASKS_URL`). On Cloud Run that
//! service only admits authenticated invokers, so each request carries an
//! OIDC identity token for this instance's service account.

use anyhow::Context;
use serde::Serialize;
use url::Url;
use uuid::Uuid;

use shared::external::gcp;
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
  gcp: gcp::Client,
}

impl TaskDispatch {
  /// `tasks_url` is the base the `/tasks/*` routes are served under.
  pub fn new(tasks_url: Url, gcp: gcp::Client) -> Self {
    let base_url = if tasks_url.path().ends_with('/') {
      tasks_url
    } else {
      Url::parse(&format!("{tasks_url}/")).unwrap()
    };
    Self { base_url, gcp }
  }

  /// Posts `body` to a task route and returns the response once it has been
  /// checked for a success status.
  async fn post<B: Serialize + ?Sized>(
    &self,
    route: &str,
    body: &B,
  ) -> anyhow::Result<reqwest::Response> {
    let url = self.base_url.join(route).unwrap();
    // Cloud Run checks the token's audience against the service URL.
    let audience = format!(
      "{}://{}",
      self.base_url.scheme(),
      self.base_url.host_str().unwrap_or_default()
    );
    let mut request = shared_http_client().post(url.clone()).json(body);
    if let Some(token) = self
      .gcp
      .get_identity_token(&audience)
      .await
      .context("getting an identity token for the tasks service")?
    {
      request = request.bearer_auth(token);
    }
    let response = request
      .send()
      .await
      .with_context(|| format!("requesting {url}"))?;
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
