// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use crate::api::ApiPackageScore;
use crate::db::Package;
use crate::db::PackageVersionMeta;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::util::USER_AGENT;
use tracing::instrument;

pub struct OramaClient {
  private_api_key: String,
  index_id: String,
}

impl OramaClient {
  pub fn new(private_api_key: String, index_id: String) -> Self {
    Self {
      private_api_key,
      index_id,
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

  #[instrument(name = "OramaClient::upsert_package", skip(self), err)]
  pub async fn upsert_package(
    &self,
    package: &Package,
    meta: &PackageVersionMeta,
  ) -> Result<(), anyhow::Error> {
    let score = package
      .latest_version
      .as_ref()
      .map(|_| ApiPackageScore::from((meta, package)).score_percentage());

    let id = format!("@{}/{}", package.scope, package.name);
    let res = self
      .request(
        &format!("/webhooks/{}/notify", self.index_id),
        serde_json::json!({
          "upsert": [
            {
              "id": id,
              "scope": &package.scope,
              "name": &package.name,
              "description": &package.description,
              "runtimeCompat": &package.runtime_compat,
              "score": score,
            }
          ]
        }),
      )
      .await?;
    let status = res.status();
    if status.is_success() {
      Ok(())
    } else {
      let response = res.text().await?;
      Err(anyhow::anyhow!(
        "failed to deploy changes (status {status}): {response}"
      ))
    }
  }

  #[instrument(name = "OramaClient::upsert_package", skip(self), err)]
  pub async fn delete_package(
    &self,
    scope: &ScopeName,
    package: &PackageName,
  ) -> Result<(), anyhow::Error> {
    let id = format!("@{scope}/{package}");
    let res = self
      .request(
        &format!("/webhooks/{}/notify", self.index_id),
        serde_json::json!({ "remove": [id] }),
      )
      .await?;
    let status = res.status();
    if status.is_success() {
      Ok(())
    } else {
      let response = res.text().await?;
      Err(anyhow::anyhow!(
        "failed to deploy changes (status {status}): {response}"
      ))
    }
  }
}
