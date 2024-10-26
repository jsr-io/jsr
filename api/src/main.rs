// Copyright 2024 the JSR authors. All rights reserved. MIT license.
mod analysis;
mod api;
mod auth;
mod buckets;
mod config;
mod db;
mod docs;
mod emails;
mod errors_internal;
mod gcp;
mod gcs_paths;
mod github;
mod iam;
mod ids;
mod metadata;
mod npm;
mod orama;
mod provenance;
mod publish;
mod sitemap;
mod tarball;
mod task_queue;
mod tasks;
mod token;
mod traced_router;
mod tracing;
mod util;

use crate::api::api_router;
use crate::api::ApiError;
use crate::api::PublishQueue;
use crate::auth::GithubOauth2Client;
use crate::buckets::BucketWithQueue;
use crate::buckets::Buckets;
use crate::config::Config;
use crate::db::Database;
use crate::emails::EmailSender;
use crate::errors_internal::error_handler;
use crate::gcp::Queue;
use crate::orama::OramaClient;
use crate::sitemap::packages_sitemap_handler;
use crate::sitemap::scopes_sitemap_handler;
use crate::sitemap::sitemap_index_handler;
use crate::tasks::tasks_router;
use crate::tasks::NpmTarballBuildQueue;
use crate::traced_router::TracedRouterService;
use crate::tracing::setup_tracing;
use crate::tracing::TracingExportTarget;

use clap::Parser;
use hyper::Body;
use hyper::Server;
use routerify::Router;
use std::net::SocketAddr;
use std::time::Duration;
use tasks::LogsBigQueryTable;
use url::Url;

pub struct MainRouterOptions {
  database: Database,
  buckets: Buckets,
  github_client: GithubOauth2Client,
  orama_client: Option<OramaClient>,
  email_sender: Option<EmailSender>,
  registry_url: Url,
  npm_url: Url,
  publish_queue: Option<Queue>,
  npm_tarball_build_queue: Option<Queue>,
  logs_bigquery_table: Option<(gcp::BigQuery, /* logs_table_id */ String)>,
  expose_api: bool,
  expose_tasks: bool,
}

pub struct RegistryUrl(pub Url);
pub struct NpmUrl(pub Url);

pub(crate) fn main_router(
  MainRouterOptions {
    database,
    buckets,
    github_client,
    orama_client,
    email_sender,
    registry_url,
    npm_url,
    publish_queue,
    npm_tarball_build_queue,
    logs_bigquery_table,
    expose_api,
    expose_tasks,
  }: MainRouterOptions,
) -> Router<Body, ApiError> {
  let builder = Router::builder()
    .data(database)
    .data(buckets)
    .data(github_client)
    .data(orama_client)
    .data(email_sender)
    .data(RegistryUrl(registry_url))
    .data(NpmUrl(npm_url))
    .data(PublishQueue(publish_queue))
    .data(NpmTarballBuildQueue(npm_tarball_build_queue))
    .data(LogsBigQueryTable(logs_bigquery_table))
    .middleware(routerify_query::query_parser())
    .err_handler_with_info(error_handler);

  let builder = if expose_api {
    builder
      .scope("/api", api_router())
      .get("/sitemap.xml", sitemap_index_handler)
      .get("/sitemap-scopes.xml", scopes_sitemap_handler)
      .get("/sitemap-packages.xml", packages_sitemap_handler)
      .get("/login", auth::login_handler)
      .get("/login/callback", auth::login_callback_handler)
      .get("/logout", auth::logout_handler)
  } else {
    builder
  };

  let builder = if expose_tasks {
    builder.scope("/tasks", tasks_router())
  } else {
    builder
  };

  builder.build().unwrap()
}

#[tokio::main]
async fn main() {
  dotenvy::dotenv().ok();
  let config = Config::parse();
  println!("{config:?}");

  let export_target = if config.cloud_trace {
    TracingExportTarget::CloudTrace
  } else if let Some(otlp_endpoint) = config.otlp_endpoint {
    TracingExportTarget::Otlp(otlp_endpoint)
  } else {
    TracingExportTarget::None
  };
  setup_tracing("api", export_target).await;

  let database = Database::connect(
    &config.database_url,
    config.database_pool_size,
    Duration::from_secs(5),
  )
  .await
  .unwrap();

  let gcp_client = gcp::Client::new(config.metadata_strategy);
  let publishing_bucket = BucketWithQueue::new(gcp::Bucket::new(
    gcp_client.clone(),
    config.publishing_bucket,
    config.gcs_endpoint.clone(),
  ));
  let modules_bucket = BucketWithQueue::new(gcp::Bucket::new(
    gcp_client.clone(),
    config.modules_bucket,
    config.gcs_endpoint.clone(),
  ));
  let docs_bucket = BucketWithQueue::new(gcp::Bucket::new(
    gcp_client.clone(),
    config.docs_bucket,
    config.gcs_endpoint.clone(),
  ));
  let npm_bucket = BucketWithQueue::new(gcp::Bucket::new(
    gcp_client.clone(),
    config.npm_bucket,
    config.gcs_endpoint,
  ));
  let buckets = Buckets {
    publishing_bucket,
    modules_bucket: modules_bucket.clone(),
    docs_bucket,
    npm_bucket,
  };

  let publish_queue = config
    .publish_queue_id
    .map(|id| Queue::new(gcp_client.clone(), id, None));

  let npm_tarball_build_queue = config
    .npm_tarball_build_queue_id
    .map(|id: String| Queue::new(gcp_client.clone(), id, None));

  let logs_bigquery_table =
    config.logs_bigquery_table_id.map(|logs_table_id| {
      (
        gcp::BigQuery::new(
          gcp_client.clone(),
          config.gcp_project_id.clone().expect(
            "gcp_project_id must be set when logs_bigquery_table_id is set",
          ),
          None,
        ),
        logs_table_id,
      )
    });

  let github_client = GithubOauth2Client::new(
    oauth2::ClientId::new(config.github_client_id),
    Some(oauth2::ClientSecret::new(config.github_client_secret)),
    oauth2::AuthUrl::new(
      "https://github.com/login/oauth/authorize".to_string(),
    )
    .unwrap(),
    Some(
      oauth2::TokenUrl::new(
        "https://github.com/login/oauth/access_token".to_string(),
      )
      .unwrap(),
    ),
  );

  let orama_client = if let Some(orama_package_private_api_key) =
    config.orama_package_private_api_key
  {
    Some(OramaClient::new(
      orama_package_private_api_key,
      config
        .orama_package_index_id
        .expect("orama_package_private_api_key was provided but no orama_package_index_id"),
      config
        .orama_symbols_index_id
        .expect("orama_package_private_api_key was provided but no orama_symbols_index_id"),
    ))
  } else {
    None
  };

  let email_sender = config.postmark_token.map(|token| {
    EmailSender::new(
      postmark::reqwest::PostmarkClient::builder()
        .token(token)
        .build(),
      config
        .email_from
        .expect("email_from must be set when postmark_token is set"),
      config
        .email_from_name
        .expect("email_from_name must be set when postmark_token is set"),
    )
  });

  let router = main_router(MainRouterOptions {
    database,
    buckets,
    github_client,
    orama_client,
    email_sender,
    registry_url: config.registry_url,
    npm_url: config.npm_url,
    publish_queue,
    npm_tarball_build_queue,
    logs_bigquery_table,
    expose_api: config.api,
    expose_tasks: config.tasks,
  });

  // Create a Service from the router above to handle incoming requests.
  let service = TracedRouterService::new(router, true).unwrap();

  // The address on which the server will be listening.
  let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

  // Create a server by passing the created service to `.serve` method.
  let server = Server::bind(&addr).serve(service);

  println!("App is running on: {}", addr);
  if let Err(err) = server.await {
    eprintln!("Server error: {}", err);
  }
}

#[cfg(test)]
mod tests {
  use crate::util::test::TestSetup;
  use serde_json::json;

  // Test the case where everything is fine and a publishing task is created.
  #[tokio::test]
  async fn alias_route() {
    let mut t = TestSetup::new().await;
    let token = t.staff_user.token.clone();
    let resp = t
      .http()
      .post("/api/admin/aliases")
      .body_json(json!({
        "name": "express",
        "majorVersion": 1,
        "target": "npm:express"
      }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    assert!(
      resp.status().is_success(),
      "unsuccessful response: {:?}",
      resp
    );
    let aliases = t.db().list_aliases_for_package("express").await.unwrap();
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].major_version, 1);
    assert_eq!(
      aliases[0].target,
      crate::db::AliasTarget::Npm("express".to_string())
    );
  }
}
