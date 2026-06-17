// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::sync::Arc;

use crate::api::ApiPackageScore;
use crate::db::Package;
use crate::db::PackageVersionMeta;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use percent_encoding::NON_ALPHANUMERIC;
use percent_encoding::utf8_percent_encode;
use reqwest::Method;
use tokio::sync::Semaphore;
use tracing::Instrument;
use tracing::Span;
use tracing::error;
use tracing::instrument;

// Algolia accepts batch payloads up to ~10MB. Stay well below that to leave
// headroom for request overhead.
const MAX_ALGOLIA_BATCH_SIZE: f64 = 3f64 * 1024f64 * 1024f64;
const MAX_CONCURRENT_ALGOLIA_TASKS: usize = 32;

/// A minimal Algolia indexing client built on top of the REST API. It only
/// implements the operations JSR needs (upserting and deleting documents); all
/// searching happens client-side in the frontend.
#[derive(Clone)]
pub struct AlgoliaClient {
  http: reqwest::Client,
  app_id: Arc<str>,
  api_key: Arc<str>,
  packages_index: Arc<str>,
  symbols_index: Arc<str>,
  semaphore: Arc<Semaphore>,
}

impl AlgoliaClient {
  pub fn new(
    app_id: String,
    api_key: String,
    packages_index: String,
    symbols_index: String,
  ) -> Self {
    Self {
      http: reqwest::Client::new(),
      app_id: app_id.into(),
      api_key: api_key.into(),
      packages_index: packages_index.into(),
      symbols_index: symbols_index.into(),
      semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_ALGOLIA_TASKS)),
    }
  }

  fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
    let url = format!("https://{}.algolia.net{path}", self.app_id);
    self
      .http
      .request(method, url)
      .header("X-Algolia-Application-Id", &*self.app_id)
      .header("X-Algolia-API-Key", &*self.api_key)
  }

  #[instrument(name = "AlgoliaClient::upsert_package", skip(self))]
  pub fn upsert_package(&self, package: &Package, meta: &PackageVersionMeta) {
    if package.version_count == 0 || package.is_archived {
      return;
    }

    if package.description.starts_with("INTERNAL") {
      self.delete_package(&package.scope, &package.name);
      return;
    }

    let object_id = format!("@{}/{}", package.scope, package.name);
    let score = package
      .latest_version
      .as_ref()
      .map(|_| ApiPackageScore::from((meta, package)).score_percentage());
    let body = serde_json::json!({
      "objectID": object_id,
      "scope": &package.scope,
      "name": &package.name,
      "description": &package.description,
      "runtimeCompat": &package.runtime_compat,
      "score": score,
    });

    let span = Span::current();
    let client = self.clone();
    tokio::spawn(
      async move {
        let _permit = client.semaphore.acquire().await;
        let path = format!(
          "/1/indexes/{}/{}",
          client.packages_index,
          utf8_percent_encode(&object_id, NON_ALPHANUMERIC),
        );
        let res = client
          .request(Method::PUT, &path)
          .json(&body)
          .send()
          .await
          .and_then(|res| res.error_for_status());
        if let Err(err) = res {
          error!("failed to AlgoliaClient::upsert_package: {err}");
        }
      }
      .instrument(span),
    );
  }

  #[instrument(name = "AlgoliaClient::delete_package", skip(self))]
  pub fn delete_package(&self, scope: &ScopeName, package: &PackageName) {
    let object_id = format!("@{scope}/{package}");
    let span = Span::current();
    let client = self.clone();
    tokio::spawn(
      async move {
        let _permit = client.semaphore.acquire().await;
        let path = format!(
          "/1/indexes/{}/{}",
          client.packages_index,
          utf8_percent_encode(&object_id, NON_ALPHANUMERIC),
        );
        let res = client
          .request(Method::DELETE, &path)
          .send()
          .await
          .and_then(|res| res.error_for_status());
        if let Err(err) = res {
          error!("failed to AlgoliaClient::delete_package: {err}");
        }
      }
      .instrument(span),
    );
  }

  #[instrument(name = "AlgoliaClient::upsert_symbols", skip(self))]
  pub fn upsert_symbols(
    &self,
    scope_name: &ScopeName,
    package_name: &PackageName,
    search: serde_json::Value,
  ) {
    // Replace all existing symbols for this package: delete the old documents
    // (matched by the `scope`/`package` facets) and insert the new ones.
    let filters =
      format!("scope:\"{scope_name}\" AND package:\"{package_name}\"");
    let span = Span::current();
    let client = self.clone();
    tokio::spawn(
      async move {
        let _permit = client.semaphore.acquire().await;
        let path = format!("/1/indexes/{}/deleteByQuery", client.symbols_index);
        let res = client
          .request(Method::POST, &path)
          .json(&serde_json::json!({ "filters": filters }))
          .send()
          .await
          .and_then(|res| res.error_for_status());
        if let Err(err) = res {
          error!("failed to delete on AlgoliaClient::upsert_symbols: {err}");
        }
      }
      .instrument(span),
    );

    let new_symbols = if let serde_json::Value::Array(mut array) = search {
      for entry in &mut array {
        let obj = entry.as_object_mut().unwrap();
        obj.insert("scope".to_string(), scope_name.to_string().into());
        obj.insert("package".to_string(), package_name.to_string().into());
      }

      array
    } else {
      unreachable!()
    };

    let chunks = {
      let str_data = serde_json::to_string(&new_symbols).unwrap();
      ((str_data.len() as f64 / MAX_ALGOLIA_BATCH_SIZE).ceil() as usize).max(1)
    };

    let chunks_size = new_symbols.len() / chunks;
    if chunks_size != 0 {
      for chunk in new_symbols.chunks(chunks_size) {
        let span = Span::current();
        let requests = chunk
          .iter()
          .map(
            |body| serde_json::json!({ "action": "addObject", "body": body }),
          )
          .collect::<Vec<_>>();
        let client = self.clone();

        tokio::spawn(
          async move {
            let _permit = client.semaphore.acquire().await;
            let path = format!("/1/indexes/{}/batch", client.symbols_index);
            let res = client
              .request(Method::POST, &path)
              .json(&serde_json::json!({ "requests": requests }))
              .send()
              .await
              .and_then(|res| res.error_for_status());
            if let Err(err) = res {
              error!("failed to AlgoliaClient::upsert_symbols: {err}");
            }
          }
          .instrument(span),
        );
      }
    }
  }
}
