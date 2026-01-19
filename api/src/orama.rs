// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use std::sync::Arc;

use crate::api::ApiPackageScore;
use crate::db::Package;
use crate::db::PackageVersionMeta;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use oramacore_client::OramaCloud;
use oramacore_client::cloud::CloudSearchParams;
use oramacore_client::cloud::DataSourceNamespace;
use oramacore_client::cloud::ProjectManagerConfig;
use tracing::Instrument;
use tracing::Span;
use tracing::error;
use tracing::instrument;

#[derive(Clone)]
pub struct OramaClient {
  symbols_client: Arc<OramaCloud>,
  package_datasource: Arc<DataSourceNamespace>,
  symbols_datasource: Arc<DataSourceNamespace>,
}

impl OramaClient {
  pub async fn new(
    package_project_id: String,
    package_project_key: String,
    package_data_source: String,
    symbol_project_id: String,
    symbol_project_key: String,
    symbol_data_source: String,
  ) -> Self {
    let package_client = OramaCloud::new(ProjectManagerConfig::new(
      package_project_id,
      package_project_key,
    ))
    .await
    .unwrap();

    let package_datasource = package_client.data_source(package_data_source);

    let symbols_client = OramaCloud::new(ProjectManagerConfig::new(
      symbol_project_id,
      symbol_project_key,
    ))
    .await
    .unwrap();

    let symbols_datasource = symbols_client.data_source(symbol_data_source);

    Self {
      symbols_client: Arc::new(symbols_client),
      package_datasource: Arc::new(package_datasource),
      symbols_datasource: Arc::new(symbols_datasource),
    }
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
      "id": id,
      "scope": &package.scope,
      "name": &package.name,
      "description": &package.description,
      "runtimeCompat": &package.runtime_compat,
      "score": score,
      "_omc:number": score.unwrap_or(0),
    });
    let span = Span::current();
    let package_datasource = self.package_datasource.clone();
    tokio::spawn(
      async move {
        if let Err(err) = package_datasource.upsert_documents(vec![body]).await
        {
          error!("failed to OramaClient::upsert_package: {err}");
        }
      }
      .instrument(span),
    );
  }

  #[instrument(name = "OramaClient::delete_package", skip(self))]
  pub fn delete_package(&self, scope: &ScopeName, package: &PackageName) {
    let id = format!("@{scope}/{package}");
    let span = Span::current();
    let package_datasource = self.package_datasource.clone();
    tokio::spawn(
      async move {
        if let Err(err) = package_datasource.delete_documents(vec![id]).await {
          error!("failed to OramaClient::delete_package: {err}");
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
    search: serde_json::Value,
  ) {
    let where_clause = serde_json::json!({
      "scope": &scope_name,
      "name": &package_name,
    });
    let span = Span::current();
    let symbols_client = self.symbols_client.clone();
    let symbols_datasource = self.symbols_datasource.clone();
    tokio::spawn(
      async move {
        #[derive(serde::Deserialize)]
        struct IDDocument {
          id: String,
        }

        let res = match symbols_client
          .search::<IDDocument>(&CloudSearchParams {
            where_clause: Some(where_clause),
            ..Default::default()
          })
          .await
        {
          Ok(res) => res,
          Err(err) => {
            error!("failed to delete on OramaClient::upsert_symbols: {err}");
            return;
          }
        };

        if let Err(err) = symbols_datasource
          .delete_documents(
            res
              .hits
              .into_iter()
              .map(|doc| doc.document.id)
              .collect::<Vec<_>>(),
          )
          .await
        {
          error!("failed to OramaClient::upsert_symbols: {err}");
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

    let span = Span::current();
    let symbols_datasource = self.symbols_datasource.clone();
    tokio::spawn(
      async move {
        if let Err(err) = symbols_datasource.insert_documents(new_symbols).await
        {
          error!("failed to OramaClient::upsert_symbols: {err}");
        }
      }
      .instrument(span),
    );
  }
}
