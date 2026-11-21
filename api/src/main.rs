// Copyright 2024 the JSR authors. All rights reserved. MIT license.

#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

mod api;
mod auth;
mod config;
mod docs;
mod emails;
mod external;
mod iam;
#[cfg(any(target_arch = "wasm32", test))]
mod inbound_email;
#[cfg(not(target_arch = "wasm32"))]
mod jemalloc_profiling;
mod object_cache;
mod provenance;
#[cfg(test)]
mod router_tests;
mod sitemap;
mod source_links;
mod task_dispatch;
mod token;
mod tree_sitter;
mod util;

use crate::api::api_router;
use crate::config::Config;
use crate::emails::EmailQueue;
use crate::emails::EmailSender;
use crate::sitemap::packages_sitemap_handler;
use crate::sitemap::scopes_sitemap_handler;
use crate::sitemap::sitemap_index_handler;

use shared::FallbackRegistryUrl;
use shared::NpmUrl;
use shared::PublishQueue;
use shared::RegistryUrl;
use shared::db::Database;
use shared::external::algolia::AlgoliaClient;
use shared::external::cloudflare::CachePurge;
use shared::external::cloudflare::Turnstile;
use shared::external::cloudflare::TurnstileClient;
use shared::queue;
use shared::queue::Queue;
use shared::router;
use shared::router::App;
use shared::router::Data;
use shared::s3;
use shared::s3::Buckets;
use shared::traced_router::TracedRouterService;
use shared::tracing::TracingExportTarget;
use shared::tracing::setup_tracing;

use crate::task_dispatch::TaskDispatch;

use axum::Router;
use axum::routing::get;
use axum::routing::post;
use clap::Parser;
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
use std::time::Duration;
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
  license_store: shared::util::LicenseStore,
  registry_url: Url,
  npm_url: Url,
  fallback_registry_url: Option<Url>,
  publish_queue: Option<Queue>,
  email_queue: Option<Queue>,
  cache_purge_client: Option<shared::external::cloudflare::CachePurgeClient>,
  turnstile: Turnstile,
  task_dispatch: TaskDispatch,
}

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
    email_queue,
    cache_purge_client,
    turnstile,
    task_dispatch,
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
    .with(EmailQueue(email_queue))
    .with(CachePurge(cache_purge_client))
    .with(turnstile)
    .with(task_dispatch)
    .with(shared::db::DependentCountCache::new());

  let router = Router::new()
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
    );

  router::app(router, data)
}

/// Migrates the database and upserts the service account token.
async fn setup(database: &Database, service_account_token: Option<&str>) {
  database.migrate().await.expect("database schema error");
  database
    .upsert_service_account_token(
      service_account_token
        .filter(|token| !token.is_empty())
        .map(token::hash),
    )
    .await
    .expect("failed to upsert service account token");
}

/// `registry_api setup`: runs [`setup`] and exits. Deploys run this once
/// before the new version serves, see `terraform/api_setup.tf`.
#[cfg(not(target_arch = "wasm32"))]
async fn setup_command() {
  let config = config::SetupConfig::parse_from(std::env::args().skip(1));
  let tls = match (config.db_client_cert, config.db_client_key) {
    (Some(client_cert), Some(client_key)) => Some(shared::db::DbTls {
      client_cert,
      client_key,
      server_ca: config.db_server_ca,
    }),
    _ => None,
  };
  let database =
    Database::connect(&config.database_url, 1, Duration::from_secs(15), tls)
      .await
      .unwrap();
  setup(&database, config.service_account_token.as_deref()).await;
}

async fn build_router(config: Config) -> App {
  main_router(build_options(config).await)
}

/// Connects to everything the API and the tasks use.
async fn build_options(config: Config) -> MainRouterOptions {
  // Treat a present-but-empty OTLP_ENDPOINT as unset: clap parses an empty env
  // var as Some(""), which would otherwise build a schemeless endpoint and
  // panic the exporter at boot. Filtering here means empty == export disabled.
  let export_target = if let Some(endpoint) =
    config.otlp_endpoint.filter(|s| !s.trim().is_empty())
  {
    TracingExportTarget::Otlp {
      endpoint,
      headers: shared::tracing::parse_otlp_headers(
        config.otlp_headers.as_deref(),
      ),
    }
  } else {
    TracingExportTarget::None
  };
  setup_tracing("api", export_target, config.deployment_environment).await;

  let db_tls = match (config.db_client_cert, config.db_client_key) {
    (Some(client_cert), Some(client_key)) => Some(shared::db::DbTls {
      client_cert,
      client_key,
      server_ca: None,
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

  if config.setup {
    setup(&database, config.service_account_token.as_deref()).await;
  }

  let s3_client = s3::S3Client::new(
    &config.s3_endpoint,
    config.s3_region,
    config.s3_access_key,
    config.s3_secret_key,
  )
  .unwrap();

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

  let publish_queue = queue::binding("PUBLISH_QUEUE");
  let email_queue = queue::binding("EMAIL_QUEUE");

  let cache_purge_client = match (
    config.cloudflare_zone_id.clone(),
    config.cloudflare_api_token.clone(),
  ) {
    (Some(zone_id), Some(api_token)) => Some(
      shared::external::cloudflare::CachePurgeClient::new(zone_id, api_token),
    ),
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

  let email_sender = config.email_from.and_then(|from| {
    EmailSender::from_binding(
      "EMAIL",
      from,
      config
        .email_from_name
        .expect("email_from_name must be set when email_from is set"),
    )
  });

  let license_store = shared::util::license_store();

  let generate_ctx_cache = crate::docs::GenerateCtxCache::new();
  let object_cache = crate::object_cache::ObjectCache::new();
  let registry_metadata_cache = crate::api::RegistryMetadataCache::new();

  let task_dispatch = TaskDispatch::new(config.tasks_url);

  MainRouterOptions {
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
    email_queue,
    cache_purge_client,
    turnstile,
    task_dispatch,
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
  dotenvy::from_filename(".env.local").ok();
  dotenvy::dotenv().ok();
  if std::env::args().nth(1).as_deref() == Some("setup") {
    return setup_command().await;
  }
  let config = Config::parse();
  println!("{config:?}");
  let port = config.port;

  let router = build_router(config).await;

  // Create a Service from the router above to handle incoming requests.
  let service = TracedRouterService::new(router, true);

  // The address on which the server will be listening.
  let addr = SocketAddr::from(([0, 0, 0, 0], port));
  let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

  println!("App is running on: {}", addr);
  if let Err(err) =
    axum::serve(listener, axum::ServiceExt::into_make_service(service)).await
  {
    eprintln!("Server error: {}", err);
  }
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod worker {
  use super::*;
  use inbound_email::InboundContext;
  use inbound_email::InboundError;
  use shared::worker_js;
  use tower::ServiceExt;
  use wasm_bindgen::prelude::*;

  /// Applies the Worker's bindings to the process and reads the config from
  /// them.
  ///
  /// Each invocation runs on its own tokio runtime, so nothing (the DB
  /// connection included) can be reused across invocations.
  fn config(env: JsValue) -> Result<Config, JsValue> {
    worker_js::env_into_process(&env);
    worker_js::set_env(env);
    if let Some(url) = worker_js::hyperdrive_database_url() {
      // SAFETY: single-threaded emscripten worker; no other threads race here.
      unsafe { std::env::set_var("DATABASE_URL", url) };
    }
    Config::try_parse_from(["registry_api", "--setup=false"])
      .map_err(|e| JsValue::from_str(&format!("config: {e}")))
  }

  #[wasm_bindgen(experimental_tokio = "isolated")]
  pub async fn fetch(
    request: web_sys::Request,
    env: JsValue,
    _ctx: JsValue,
  ) -> Result<web_sys::Response, JsValue> {
    let config = config(env)?;
    // Keeps the invocation's span active on every poll: the spans the request
    // starts are parented under whichever span is active.
    ::worker::observability::in_current_span(worker_js::with_background(
      async {
        let app = build_router(config).await;
        let hyper_req = worker_js::to_hyper_request(request).await?;
        let Ok(hyper_resp) =
          TracedRouterService::new(app, true).oneshot(hyper_req).await;
        worker_js::to_web_response(hyper_resp).await
      },
    ))
    .await
  }

  /// Receives the mail Cloudflare Email Routing accepts for the domain.
  ///
  /// A message that can never be ingested is rejected, which tells the sending
  /// server not to try again. Any other failure is returned as an error, so
  /// the message is not accepted and the sending server delivers it again
  /// later.
  #[wasm_bindgen(experimental_tokio = "isolated")]
  pub async fn email(
    message: ::worker::ForwardableEmailMessage,
    env: JsValue,
    _ctx: JsValue,
  ) -> Result<(), JsValue> {
    let config = config(env)?;
    let trusted_authserv_id = config.inbound_trusted_authserv_id.clone();
    ::worker::observability::in_current_span(async {
      let options = build_options(config).await;
      let raw = message
        .raw_bytes()
        .await
        .map_err(|e| JsValue::from_str(&format!("reading message: {e}")))?;

      let result = inbound_email::ingest(
        InboundContext {
          db: &options.database,
          buckets: &options.buckets,
          registry_url: &options.registry_url,
          email_sender: options.email_sender.as_ref(),
          email_queue: &EmailQueue(options.email_queue),
          trusted_authserv_id: trusted_authserv_id.as_deref(),
        },
        &raw,
      )
      .await;

      match result {
        Ok(()) => Ok(()),
        Err(InboundError::Rejected(reason)) => {
          message.set_reject(reason);
          Ok(())
        }
        Err(InboundError::Failed(err)) => {
          Err(JsValue::from_str(&format!("ingesting message: {err:?}")))
        }
      }
    })
    .await
  }
}
