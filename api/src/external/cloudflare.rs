// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use serde::Deserialize;
use serde::Serialize;
use tracing::error;
use tracing::instrument;

#[derive(Clone)]
pub struct AnalyticsEngineClient {
  account_id: String,
  api_token: String,
}

/// Client for the Cloudflare zone cache-purge endpoint, used to invalidate
/// cached package and npm version manifests after a publish or mutation.
///
/// Construction requires both a zone ID and an API token; if either is
/// missing the API server simply does not build a client and all purge
/// calls become no-ops.
#[derive(Clone)]
pub struct CachePurgeClient {
  zone_id: String,
  api_token: String,
}

/// Wrapper around an optional `CachePurgeClient` so it can be stored in
/// the routerify data map alongside other shared services. A `None`
/// value means cache purging is disabled (e.g. local dev), and call
/// sites should treat it as a no-op.
#[derive(Clone)]
pub struct CachePurge(pub Option<CachePurgeClient>);

impl CachePurge {
  /// Purge `urls` if a client is configured. Errors are logged inside
  /// `purge_urls` and converted into `Ok(())` here, since callers want
  /// best-effort behaviour (the manifests have `stale-while-revalidate`
  /// as their durability net).
  pub async fn purge(&self, urls: Vec<String>) {
    let Some(client) = &self.0 else {
      return;
    };
    let _ = client.purge_urls(urls).await;
  }
}

impl CachePurgeClient {
  pub fn new(zone_id: String, api_token: String) -> Self {
    Self { zone_id, api_token }
  }

  /// Purge a set of fully-qualified URLs from the Cloudflare zone cache.
  ///
  /// Errors are logged and returned — callers should treat purge as
  /// best-effort and not fail the publish on a purge failure (the
  /// `stale-while-revalidate` window on the manifests is the safety net).
  #[instrument(name = "cloudflare.purge_cache", skip(self, urls), err)]
  pub async fn purge_urls(
    &self,
    urls: Vec<String>,
  ) -> Result<(), anyhow::Error> {
    if urls.is_empty() {
      return Ok(());
    }

    let body = serde_json::json!({ "files": urls });
    let response = crate::util::shared_http_client()
      .post(format!(
        "https://api.cloudflare.com/client/v4/zones/{}/purge_cache",
        self.zone_id,
      ))
      .bearer_auth(&self.api_token)
      .json(&body)
      .send()
      .await?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response.text().await.unwrap_or_default();
      error!(
        "Cloudflare cache purge failed (status={}): {}",
        status, body
      );
      return Err(anyhow::anyhow!(
        "Cloudflare cache purge failed (status={}): {}",
        status,
        body,
      ));
    }

    Ok(())
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsQueryResult {
  pub data: Vec<DownloadRecord>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DownloadRecord {
  pub time_bucket: String,
  pub scope: String,
  pub package: String,
  // because 'version' is reserved in cloudflare analytics engine
  pub ver: String,
  pub count: String,
}

impl AnalyticsEngineClient {
  pub fn new(account_id: String, api_token: String) -> Self {
    Self {
      account_id,
      api_token,
    }
  }

  pub async fn query_downloads(
    &self,
    query: String,
  ) -> Result<Vec<DownloadRecord>, anyhow::Error> {
    let response = crate::util::shared_http_client()
      .post(format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/analytics_engine/sql",
        self.account_id,
      ))
      .bearer_auth(&self.api_token)
      .body(query)
      .send()
      .await?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response.text().await?;
      error!(
        "Cloudflare Analytics Engine query failed (status={}): {}",
        status, body
      );
      return Err(anyhow::anyhow!(
        "Cloudflare Analytics Engine query failed: {}",
        body
      ));
    }

    let result: AnalyticsQueryResult = response.json().await?;

    Ok(result.data)
  }
}
