// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use std::sync::Arc;

use crate::api::ApiPackageScore;
use crate::db::Package;
use crate::db::PackageVersionMeta;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::util::USER_AGENT;
use tracing::error;
use tracing::instrument;
use tracing::Instrument;
use tracing::Span;

#[derive(Clone)]
pub struct OramaClient {
  private_api_key: Arc<str>,
  index_id: Arc<str>,
}

impl OramaClient {
  pub fn new(private_api_key: String, index_id: String) -> Self {
    Self {
      private_api_key: private_api_key.into(),
      index_id: index_id.into(),
    }
  }

  async fn request(
    &self,
    path: &str,
    payload: serde_json::Value,
  ) -> Result<reqwest::Response, anyhow::Error> {
    let response = reqwest::Client::builder()
      .user_agent(USER_AGENT)
      .build()?
      .post(format!("https://api.oramasearch.com/api/v1{}", path))
      .json(&payload)
      .bearer_auth(&self.private_api_key)
      .send()
      .await?;
    Ok(response)
  }

  #[instrument(name = "OramaClient::upsert_package", skip(self))]
  pub fn upsert_package(&self, package: &Package, meta: &PackageVersionMeta) {
    if package.version_count == 0 {
      return;
    }
    let id = format!("@{}/{}", package.scope, package.name);
    let score = package
      .latest_version
      .as_ref()
      .map(|_| ApiPackageScore::from((meta, package)).score_percentage());
    let body = serde_json::json!({
      "upsert": [
        {
          "id": id,
          "scope": &package.scope,
          "name": &package.name,
          "description": &package.description,
          "runtimeCompat": &package.runtime_compat,
          "score": score,
          "_omc": score.unwrap_or(0),
        }
      ]
    });
    let span = Span::current();
    let client = self.clone();
    let path = format!("/webhooks/{}/notify", self.index_id);
    tokio::spawn(
      async move {
        let res = match  client.request(&path, body).await {
          Ok(res) => res,
          Err(err) => {
            error!("failed to OramaClient::upsert_package: {err}");
            return;
          }
        };
        let status = res.status();
        if !status.is_success() {
          let response = res.text().await.unwrap_or_default();
          error!(
            "failed to OramaClient::upsert_package for {id} (status {status}): {response}"
          );
        }
      }
      .instrument(span),
    );
  }

  #[instrument(name = "OramaClient::delete_package", skip(self))]
  pub fn delete_package(&self, scope: &ScopeName, package: &PackageName) {
    let id = format!("@{scope}/{package}");
    let body = serde_json::json!({ "remove": [id] });
    let span = Span::current();
    let client = self.clone();
    let path = format!("/webhooks/{}/notify", self.index_id);
    tokio::spawn(
      async move {
        let res = match  client.request(&path, body).await {
          Ok(res) => res,
          Err(err) => {
            error!("failed to OramaClient::delete_package: {err}");
            return;
          }
        };
        let status = res.status();
        if !status.is_success() {
          let response = res.text().await.unwrap_or_default();
          error!(
            "failed to OramaClient::delete_package for {id} (status {status}): {response}"
          );
        }
      }
      .instrument(span),
    );
  }
}
