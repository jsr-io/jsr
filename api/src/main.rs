// Copyright 2024 the JSR authors. All rights reserved. MIT license.

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

mod analysis;
mod api;
mod auth;
mod config;
mod db;
mod docs;
mod emails;
mod errors_internal;
mod external;
mod gcp;
mod iam;
mod ids;
mod jemalloc_profiling;
mod metadata;
mod npm;
mod provenance;
mod publish;
mod s3;
mod s3_paths;
mod sitemap;
mod tarball;
mod task_queue;
mod tasks;
mod token;
mod traced_router;
mod tracing;
mod tree_sitter;
mod util;

use crate::api::ApiError;
use crate::api::PublishQueue;
use crate::api::api_router;
use crate::config::Config;
use crate::db::Database;
use crate::emails::EmailSender;
use crate::errors_internal::error_handler;
use crate::external::orama::OramaClient;
use crate::gcp::Queue;
use crate::s3::Buckets;
use crate::sitemap::packages_sitemap_handler;
use crate::sitemap::scopes_sitemap_handler;
use crate::sitemap::sitemap_index_handler;
use crate::tasks::NpmTarballBuildQueue;
use crate::tasks::tasks_router;
use crate::traced_router::TracedRouterService;
use crate::tracing::TracingExportTarget;
use crate::tracing::setup_tracing;

use clap::Parser;
use hyper::Body;
use hyper::Server;
use routerify::Router;
use std::net::SocketAddr;
use std::time::Duration;
use tasks::AnalyticsEngineConfig;
use url::Url;

pub struct MainRouterOptions {
  database: Database,
  buckets: Buckets,
  generate_ctx_cache: crate::docs::GenerateCtxCache,
  github_client: auth::github::Oauth2Client,
  gitlab_client: auth::gitlab::Oauth2Client,
  orama_client: Option<OramaClient>,
  email_sender: Option<EmailSender>,
  license_store: util::LicenseStore,
  registry_url: Url,
  npm_url: Url,
  publish_queue: Option<Queue>,
  npm_tarball_build_queue: Option<Queue>,
  analytics_engine_config: Option<(
    external::cloudflare::AnalyticsEngineClient,
    /* dataset_name */ String,
  )>,
  expose_api: bool,
  expose_tasks: bool,
}

pub struct RegistryUrl(pub Url);
pub struct NpmUrl(pub Url);

pub(crate) fn main_router(
  MainRouterOptions {
    database,
    buckets,
    generate_ctx_cache,
    github_client,
    gitlab_client,
    orama_client,
    license_store,
    email_sender,
    registry_url,
    npm_url,
    publish_queue,
    npm_tarball_build_queue,
    analytics_engine_config,
    expose_api,
    expose_tasks,
  }: MainRouterOptions,
) -> Router<Body, ApiError> {
  let builder = Router::builder()
    .data(database)
    .data(buckets)
    .data(generate_ctx_cache)
    .data(github_client)
    .data(gitlab_client)
    .data(orama_client)
    .data(email_sender)
    .data(license_store)
    .data(RegistryUrl(registry_url))
    .data(NpmUrl(npm_url))
    .data(PublishQueue(publish_queue))
    .data(NpmTarballBuildQueue(npm_tarball_build_queue))
    .data(AnalyticsEngineConfig(analytics_engine_config))
    .data(db::DependentCountCache::new())
    .middleware(routerify_query::query_parser())
    .err_handler_with_info(error_handler);

  let builder = if expose_api {
    builder
      .scope("/api", api_router())
      .get("/sitemap.xml", sitemap_index_handler)
      .get("/sitemap-scopes.xml", scopes_sitemap_handler)
      .get("/sitemap-packages.xml", packages_sitemap_handler)
      .get("/login/:service", auth::login_handler)
      .get("/login/callback/:service", auth::login_callback_handler)
      .get("/logout", auth::logout_handler)
      .get("/connect/:service", util::full_auth(auth::connect_handler))
      .get(
        "/connect/callback/:service",
        util::full_auth(auth::connect_callback_handler),
      )
      .get(
        "/disconnect/:service",
        util::full_auth(auth::disconnect_handler),
      )
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
  dotenvy::from_filename(".env.local").ok();
  dotenvy::dotenv().ok();
  let config = Config::parse();
  println!("{config:?}");

  let export_target = if let Some(otlp_endpoint) = config.otlp_endpoint {
    TracingExportTarget::Otlp(otlp_endpoint)
  } else {
    TracingExportTarget::None
  };
  setup_tracing("api", export_target).await;

  let database = Database::connect(
    &config.database_url,
    config.database_pool_size,
    Duration::from_secs(15),
  )
  .await
  .unwrap();

  let s3_region = ::s3::Region::Custom {
    region: config.s3_region,
    endpoint: config.s3_endpoint,
  };
  let s3_credentials = ::s3::creds::Credentials {
    access_key: Some(config.s3_access_key),
    secret_key: Some(config.s3_secret_key),
    security_token: None,
    session_token: None,
    expiration: None,
  };

  let gcp_client = gcp::Client::new(config.metadata_strategy);
  let publishing_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(
      config.publishing_bucket,
      s3_region.clone(),
      s3_credentials.clone(),
    )
    .unwrap(),
  );
  let modules_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(
      config.modules_bucket,
      s3_region.clone(),
      s3_credentials.clone(),
    )
    .unwrap(),
  );
  let docs_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(
      config.docs_bucket,
      s3_region.clone(),
      s3_credentials.clone(),
    )
    .unwrap(),
  );
  let npm_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(config.npm_bucket, s3_region, s3_credentials).unwrap(),
  );
  let buckets = Buckets {
    publishing_bucket,
    modules_bucket,
    docs_bucket,
    npm_bucket,
  };

  let publish_queue = config
    .publish_queue_id
    .map(|id| Queue::new(gcp_client.clone(), id, None));

  let npm_tarball_build_queue = config
    .npm_tarball_build_queue_id
    .map(|id: String| Queue::new(gcp_client.clone(), id, None));

  let analytics_engine_config = match (
    config.cloudflare_account_id,
    config.cloudflare_api_token,
    config.cloudflare_analytics_dataset,
  ) {
    (Some(account_id), Some(api_token), Some(dataset_name)) => Some((
      external::cloudflare::AnalyticsEngineClient::new(account_id, api_token),
      dataset_name,
    )),
    _ => None,
  };

  let github_client = auth::github::Oauth2Client::new(
    &config.registry_url,
    config.github_client_id,
    config.github_client_secret,
  );

  let gitlab_client = auth::gitlab::Oauth2Client::new(
    &config.registry_url,
    config.gitlab_client_id,
    config.gitlab_client_secret,
  );

  let orama_client = if let Some(orama_packages_project_id) =
    config.orama_packages_project_id
  {
    Some(
        OramaClient::new(
          orama_packages_project_id,
          config.orama_packages_project_key.expect(
            "orama_packages_project_id was provided but no orama_packages_project_key",
          ),
          config.orama_packages_data_source.expect(
            "orama_packages_project_id was provided but no orama_packages_data_source",
          ),
          config.orama_symbols_project_id.expect(
            "orama_packages_project_id was provided but no orama_symbols_project_id",
          ),
          config.orama_symbols_project_key.expect(
            "orama_packages_project_id was provided but no orama_symbols_project_key",
          ),
          config.orama_symbols_data_source.expect(
            "orama_packages_project_id was provided but no orama_symbols_data_source",
          ),
        )
        .await,
      )
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

  let license_store = util::license_store();

  let generate_ctx_cache = crate::docs::GenerateCtxCache::new();

  let router = main_router(MainRouterOptions {
    database,
    buckets,
    generate_ctx_cache,
    github_client,
    gitlab_client,
    orama_client,
    email_sender,
    license_store,
    registry_url: config.registry_url,
    npm_url: config.npm_url,
    publish_queue,
    npm_tarball_build_queue,
    analytics_engine_config,
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
