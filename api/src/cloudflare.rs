// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use tracing::error;

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct AnalyticsEngineClient {
  http: reqwest::Client,
  account_id: String,
  api_token: String,
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
  pub count: i64,
}

impl AnalyticsEngineClient {
  pub fn new(account_id: String, api_token: String) -> Self {
    let http = reqwest::ClientBuilder::new()
      .connect_timeout(HTTP_CONNECT_TIMEOUT)
      .build()
      .unwrap();

    Self {
      http,
      account_id,
      api_token,
    }
  }

  pub async fn query_downloads(
    &self,
    query: String,
  ) -> Result<Vec<DownloadRecord>, anyhow::Error> {
    let response = self
      .http
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
