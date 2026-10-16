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
mod object_cache;
mod provenance;
mod publish;
mod router;
mod s3;
mod s3_paths;
mod sitemap;
mod source_links;
mod tarball;
mod task_queue;
mod tasks;
mod token;
mod traced_router;
mod tracing;
mod tree_sitter;
mod util;

use crate::api::InboundTrustedAuthservId;
use crate::api::PostmarkWebhookPassword;
use crate::api::PublishQueue;
use crate::api::api_router;
use crate::config::Config;
use crate::db::Database;
use crate::emails::EmailQueue;
use crate::emails::EmailSender;
use crate::external::algolia::AlgoliaClient;
use crate::external::cloudflare::CachePurge;
use crate::external::cloudflare::Turnstile;
use crate::external::cloudflare::TurnstileClient;
use crate::gcp::Queue;
use crate::router::App;
use crate::router::Data;
use crate::s3::Buckets;
use crate::sitemap::packages_sitemap_handler;
use crate::sitemap::scopes_sitemap_handler;
use crate::sitemap::sitemap_index_handler;
use crate::tasks::NpmTarballBuildQueue;
use crate::tasks::tasks_router;
use crate::traced_router::TracedRouterService;
use crate::tracing::TracingExportTarget;
use crate::tracing::setup_tracing;

use axum::Router;
use axum::routing::get;
use axum::routing::post;
use clap::Parser;
use std::net::SocketAddr;
use std::time::Duration;
use tasks::AnalyticsEngineConfig;
use url::Url;

pub struct MainRouterOptions {
  database: Database,
  buckets: Buckets,
  generate_ctx_cache: crate::docs::GenerateCtxCache,
  object_cache: crate::object_cache::ObjectCache,
  registry_metadata_cache: crate::api::RegistryMetadataCache,
  github_client: auth::github::Oauth2Client,
  gitlab_client: auth::gitlab::Oauth2Client,
  algolia_client: Option<AlgoliaClient>,
  email_sender: Option<EmailSender>,
  license_store: util::LicenseStore,
  registry_url: Url,
  npm_url: Url,
  fallback_registry_url: Option<Url>,
  publish_queue: Option<Queue>,
  npm_tarball_build_queue: Option<Queue>,
  email_queue: Option<Queue>,
  analytics_engine_config: Option<(
    external::cloudflare::AnalyticsEngineClient,
    /* dataset_name */ String,
  )>,
  cache_purge_client: Option<external::cloudflare::CachePurgeClient>,
  turnstile: Turnstile,
  postmark_webhook_password: PostmarkWebhookPassword,
  inbound_trusted_authserv_id: InboundTrustedAuthservId,
  expose_api: bool,
  expose_tasks: bool,
}

pub struct RegistryUrl(pub Url);
pub struct NpmUrl(pub Url);
pub struct FallbackRegistryUrl(pub Option<Url>);

pub(crate) fn main_router(
  MainRouterOptions {
    database,
    buckets,
    generate_ctx_cache,
    object_cache,
    registry_metadata_cache,
    github_client,
    gitlab_client,
    algolia_client,
    license_store,
    email_sender,
    registry_url,
    npm_url,
    fallback_registry_url,
    publish_queue,
    npm_tarball_build_queue,
    email_queue,
    analytics_engine_config,
    cache_purge_client,
    turnstile,
    postmark_webhook_password,
    inbound_trusted_authserv_id,
    expose_api,
    expose_tasks,
  }: MainRouterOptions,
) -> App {
  let data = Data::default()
    .with(database)
    .with(buckets)
    .with(generate_ctx_cache)
    .with(object_cache)
    .with(registry_metadata_cache)
    .with(github_client)
    .with(gitlab_client)
    .with(algolia_client)
    .with(email_sender)
    .with(license_store)
    .with(RegistryUrl(registry_url))
    .with(NpmUrl(npm_url))
    .with(FallbackRegistryUrl(fallback_registry_url))
    .with(PublishQueue(publish_queue))
    .with(NpmTarballBuildQueue(npm_tarball_build_queue))
    .with(EmailQueue(email_queue))
    .with(AnalyticsEngineConfig(analytics_engine_config))
    .with(CachePurge(cache_purge_client))
    .with(turnstile)
    .with(postmark_webhook_password)
    .with(inbound_trusted_authserv_id)
    .with(db::DependentCountCache::new());

  let router = Router::new();

  let router = if expose_api {
    router
      .nest("/api", api_router())
      .route("/sitemap.xml", get(sitemap_index_handler))
      .route("/sitemap-scopes.xml", get(scopes_sitemap_handler))
      .route("/sitemap-packages.xml", get(packages_sitemap_handler))
      // POST, not GET: the login form carries the Turnstile response token in
      // its body, which keeps it out of URLs, logs and `Referer` headers. It
      // also means a bare link to this route can no longer start a login flow,
      // so the captcha cannot be sidestepped by navigating straight here.
      .route("/login/{service}", post(auth::login_handler))
      .route(
        "/login/callback/{service}",
        get(auth::login_callback_handler),
      )
      .route("/logout", get(auth::logout_handler))
      .route(
        "/connect/{service}",
        get(util::full_auth(auth::connect_handler)),
      )
      .route(
        "/connect/callback/{service}",
        get(util::full_auth(auth::connect_callback_handler)),
      )
      .route(
        "/disconnect/{service}",
        get(util::full_auth(auth::disconnect_handler)),
      )
  } else {
    router
  };

  let router = if expose_tasks {
    router.nest("/tasks", tasks_router())
  } else {
    router
  };

  router::app(router, data)
}

#[tokio::main]
async fn main() {
  dotenvy::from_filename(".env.local").ok();
  dotenvy::dotenv().ok();
  let config = Config::parse();
  println!("{config:?}");

  // Treat a present-but-empty OTLP_ENDPOINT as unset: clap parses an empty env
  // var as Some(""), which would otherwise build a schemeless endpoint and
  // panic the exporter at boot. Filtering here means empty == export disabled.
  let export_target = if let Some(endpoint) =
    config.otlp_endpoint.filter(|s| !s.trim().is_empty())
  {
    TracingExportTarget::Otlp {
      endpoint,
      headers: crate::tracing::parse_otlp_headers(
        config.otlp_headers.as_deref(),
      ),
    }
  } else {
    TracingExportTarget::None
  };
  setup_tracing("api", export_target, config.deployment_environment).await;

  let db_tls = match (config.db_client_cert, config.db_client_key) {
    (Some(client_cert), Some(client_key)) => Some(crate::db::DbTls {
      client_cert,
      client_key,
    }),
    _ => None,
  };

  let database = Database::connect(
    &config.database_url,
    config.database_pool_size,
    Duration::from_secs(15),
    db_tls,
  )
  .await
  .unwrap();

  database
    .upsert_service_account_token(
      config
        .service_account_token
        .as_deref()
        .filter(|token| !token.is_empty())
        .map(token::hash),
    )
    .await
    .expect("failed to upsert service account token");

  let s3_client = s3::S3Client::new(
    &config.s3_endpoint,
    config.s3_region,
    config.s3_access_key,
    config.s3_secret_key,
  )
  .unwrap();

  let gcp_client = gcp::Client::new(config.metadata_strategy);
  let publishing_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(&s3_client, config.publishing_bucket).unwrap(),
  );
  let modules_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(&s3_client, config.modules_bucket).unwrap(),
  );
  let docs_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(&s3_client, config.docs_bucket).unwrap(),
  );
  let npm_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(&s3_client, config.npm_bucket).unwrap(),
  );
  let ticket_attachments_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(&s3_client, config.ticket_attachments_bucket).unwrap(),
  );
  let buckets = Buckets {
    publishing_bucket,
    modules_bucket,
    docs_bucket,
    npm_bucket,
    ticket_attachments_bucket,
  };

  let publish_queue = config
    .publish_queue_id
    .map(|id| Queue::new(gcp_client.clone(), id, None));

  let npm_tarball_build_queue = config
    .npm_tarball_build_queue_id
    .map(|id: String| Queue::new(gcp_client.clone(), id, None));

  let email_queue = config
    .email_queue_id
    .map(|id: String| Queue::new(gcp_client.clone(), id, None));

  let cache_purge_client = match (
    config.cloudflare_zone_id.clone(),
    config.cloudflare_api_token.clone(),
  ) {
    (Some(zone_id), Some(api_token)) => Some(
      external::cloudflare::CachePurgeClient::new(zone_id, api_token),
    ),
    _ => None,
  };

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

  let turnstile =
    Turnstile(config.turnstile_secret_key.map(TurnstileClient::new));

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

  let algolia_client = if let Some(algolia_app_id) = config.algolia_app_id {
    Some(AlgoliaClient::new(
      algolia_app_id,
      config
        .algolia_write_api_key
        .expect("algolia_app_id was provided but no algolia_write_api_key"),
      config
        .algolia_packages_index
        .expect("algolia_app_id was provided but no algolia_packages_index"),
      config
        .algolia_symbols_index
        .expect("algolia_app_id was provided but no algolia_symbols_index"),
    ))
  } else {
    None
  };

  let email_sender = config.postmark_token.map(|token| {
    EmailSender::new(
      postmark::reqwest::PostmarkClient::builder()
        .server_token(token)
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
  let object_cache = crate::object_cache::ObjectCache::new();
  let registry_metadata_cache = crate::api::RegistryMetadataCache::new();

  let router = main_router(MainRouterOptions {
    database,
    buckets,
    generate_ctx_cache,
    object_cache,
    registry_metadata_cache,
    github_client,
    gitlab_client,
    algolia_client,
    email_sender,
    license_store,
    registry_url: config.registry_url,
    npm_url: config.npm_url,
    fallback_registry_url: config.fallback_registry_url,
    publish_queue,
    npm_tarball_build_queue,
    email_queue,
    analytics_engine_config,
    cache_purge_client,
    turnstile,
    postmark_webhook_password: PostmarkWebhookPassword(
      config.postmark_webhook_password,
    ),
    inbound_trusted_authserv_id: InboundTrustedAuthservId(
      config.inbound_trusted_authserv_id,
    ),
    expose_api: config.api,
    expose_tasks: config.tasks,
  });

  // Create a Service from the router above to handle incoming requests.
  let service = TracedRouterService::new(router, true);

  // The address on which the server will be listening.
  let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
  let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

  println!("App is running on: {}", addr);
  if let Err(err) =
    axum::serve(listener, axum::ServiceExt::into_make_service(service)).await
  {
    eprintln!("Server error: {}", err);
  }
}
