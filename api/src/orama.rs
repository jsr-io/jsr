// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use crate::api::ApiPackageScore;
use crate::db::Package;
use crate::db::PackageVersionMeta;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::util::USER_AGENT;
use deno_doc::html::search::SearchIndexNode;
use std::sync::Arc;
use tracing::error;
use tracing::instrument;
use tracing::Instrument;
use tracing::Span;

const MAX_ORAMA_INSERT_SIZE: f64 = 3f64 * 1024f64 * 1024f64;

#[derive(Clone)]
pub struct OramaClient {
  private_api_key: Arc<str>,
  package_index_id: Arc<str>,
  symbols_index_id: Arc<str>,
}

impl OramaClient {
  pub fn new(
    private_api_key: String,
    package_index_id: String,
    symbols_index_id: String,
  ) -> Self {
    Self {
      private_api_key: private_api_key.into(),
      package_index_id: package_index_id.into(),
      symbols_index_id: symbols_index_id.into(),
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
    if package.version_count == 0 || package.is_archived {
      return;
    }

    if package.description.starts_with("INTERNAL") {
      self.delete_package(&package.scope, &package.name);
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
          "_omc:number": score.unwrap_or(0),
        }
      ]
    });
    let span = Span::current();
    let client = self.clone();
    let path = format!("/webhooks/{}/notify", self.package_index_id);
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
    let path = format!("/webhooks/{}/notify", self.package_index_id);
    tokio::spawn(
      async move {
        let res = match client.request(&path, body).await {
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

  #[instrument(name = "OramaClient::upsert_symbols", skip(self))]
  pub fn upsert_symbols(
    &self,
    scope_name: &ScopeName,
    package_name: &PackageName,
    search: &[SearchIndexNode],
  ) {
    let package = format!("{scope_name}/{package_name}");
    let body = serde_json::json!({
      "remove": [
        {
          "scope": &scope_name,
          "name": &package_name,
        }
      ]
    });
    let span = Span::current();
    let client = self.clone();
    let path = format!("/webhooks/{}/notify", self.symbols_index_id);
    tokio::spawn(
      async move {
        let res = match client.request(&path, body).await {
          Ok(res) => res,
          Err(err) => {
            error!("failed to delete on OramaClient::upsert_symbols: {err}");
            return;
          }
        };
        let status = res.status();
        if !status.is_success() {
          let response = res.text().await.unwrap_or_default();
          error!(
            "failed to delete on OramaClient::upsert_symbols for {package} (status {status}): {response}"
          );
        }
      }
        .instrument(span),
    );

    let search = search
      .iter()
      .map(|node| {
        serde_json::json!({
          "target_id": node.id,
          "name": node.name,
          "file": node.file,
          "doc": node.doc,
          "url": node.url,
          "deprecated": node.deprecated,
          "scope": scope_name.to_string(),
          "package": package_name.to_string(),
        })
      })
      .collect::<Vec<_>>();

    let chunks = {
      let str_data = serde_json::to_string(&search).unwrap();
      ((str_data.len() as f64 / MAX_ORAMA_INSERT_SIZE).ceil() as usize).max(1)
    };

    let chunks_size = search.len() / chunks;
    if chunks_size != 0 {
      for chunk in search.chunks(chunks_size) {
        let body = serde_json::json!({ "upsert": &chunk });
        let package = format!("{scope_name}/{package_name}");
        let span = Span::current();
        let client = self.clone();
        let path = format!("/webhooks/{}/notify", self.symbols_index_id);
        tokio::spawn(
          async move {
            let res = match client.request(&path, body).await {
              Ok(res) => res,
              Err(err) => {
                error!("failed to insert on OramaClient::upsert_symbols: {err}");
                return;
              }
            };
            let status = res.status();
            if !status.is_success() {
              let response = res.text().await.unwrap_or_default();
              error!(
            "failed to insert on OramaClient::upsert_symbols for {package} (status {status}): {response}"
          );
            }
          }
            .instrument(span),
        );
      }
    }
  }
}
