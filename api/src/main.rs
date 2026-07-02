// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// jemalloc doesn't build for wasm32 (emscripten worker build); use the default
// allocator there and keep jemalloc only for the native compute service.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
use crate::external::algolia::AlgoliaClient;
use crate::external::cloudflare::CachePurge;
use crate::gcp::Queue;
use crate::s3::Buckets;
use crate::sitemap::packages_sitemap_handler;
use crate::sitemap::scopes_sitemap_handler;
use crate::sitemap::sitemap_index_handler;
use crate::tasks::NpmTarballBuildQueue;
use crate::tasks::tasks_router;
#[cfg(not(target_arch = "wasm32"))]
use crate::traced_router::TracedRouterService;
use crate::tracing::TracingExportTarget;
use crate::tracing::setup_tracing;

use clap::Parser;
use hyper::Body;
#[cfg(not(target_arch = "wasm32"))]
use hyper::Server;
use routerify::Router;
#[cfg(not(target_arch = "wasm32"))]
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
  algolia_client: Option<AlgoliaClient>,
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
  cache_purge_client: Option<external::cloudflare::CachePurgeClient>,
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
    algolia_client,
    license_store,
    email_sender,
    registry_url,
    npm_url,
    publish_queue,
    npm_tarball_build_queue,
    analytics_engine_config,
    cache_purge_client,
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
    .data(algolia_client)
    .data(email_sender)
    .data(license_store)
    .data(RegistryUrl(registry_url))
    .data(NpmUrl(npm_url))
    .data(PublishQueue(publish_queue))
    .data(NpmTarballBuildQueue(npm_tarball_build_queue))
    .data(AnalyticsEngineConfig(analytics_engine_config))
    .data(CachePurge(cache_purge_client))
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

/// Build the fully-wired application router from a parsed [`Config`]. Shared by
/// the native listener entry point (`main`) and the emscripten worker `fetch`
/// export, so both drive exactly the same routes and state.
async fn build_router(config: Config) -> Router<Body, ApiError> {
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

  main_router(MainRouterOptions {
    database,
    buckets,
    generate_ctx_cache,
    github_client,
    gitlab_client,
    algolia_client,
    email_sender,
    license_store,
    registry_url: config.registry_url,
    npm_url: config.npm_url,
    publish_queue,
    npm_tarball_build_queue,
    analytics_engine_config,
    cache_purge_client,
    expose_api: config.api,
    expose_tasks: config.tasks,
  })
}

// Native (Cloud Run) entry point: bind a TCP listener and serve the router with
// hyper, exactly as before.
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
  dotenvy::from_filename(".env.local").ok();
  dotenvy::dotenv().ok();
  let config = Config::parse();
  println!("{config:?}");
  let port = config.port;

  let router = build_router(config).await;

  // Create a Service from the router above to handle incoming requests.
  let service = TracedRouterService::new(router, true).unwrap();

  // The address on which the server will be listening.
  let addr = SocketAddr::from(([0, 0, 0, 0], port));

  // Create a server by passing the created service to `.serve` method.
  let server = Server::bind(&addr).serve(service);

  println!("App is running on: {}", addr);
  if let Err(err) = server.await {
    eprintln!("Server error: {}", err);
  }
}

// Emscripten worker entry point. A Cloudflare Worker cannot `listen()` on a
// socket, so instead of `Server::bind().serve()` we expose a `fetch` export that
// bridges the runtime's `web_sys::Request`/`Response` to hyper + routerify: the
// router is built once (from the JS `env` bindings) and reused across requests.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod worker {
  use super::*;
  use crate::traced_router::TracedRequestServiceBuilder;
  use hyper::service::Service as _;
  use std::sync::Mutex;
  use tokio::sync::OnceCell;
  use wasm_bindgen::prelude::*;

  // Built once on the first request and reused. `RequestServiceBuilder::build`
  // takes `&mut self`, so guard it with a `Mutex`; building a per-request
  // service is cheap. The heavy state (DB pool, S3 clients, …) lives inside.
  static BUILDER: OnceCell<Mutex<TracedRequestServiceBuilder<Body, ApiError>>> =
    OnceCell::const_new();

  /// Copy the string-valued entries of the worker `env` object into the process
  /// environment so clap's `env = "..."` config parsing sees them.
  fn env_into_process(env: &JsValue) {
    let Ok(entries) = js_sys::Object::entries(&js_sys::Object::from(env.clone()))
      .dyn_into::<js_sys::Array>()
    else {
      return;
    };
    for entry in entries.iter() {
      let pair = js_sys::Array::from(&entry);
      if let (Some(k), Some(v)) = (pair.get(0).as_string(), pair.get(1).as_string())
      {
        // SAFETY: single-threaded emscripten worker; no other threads race here.
        unsafe { std::env::set_var(k, v) };
      }
    }
  }

  async fn init_builder(
    env: &JsValue,
  ) -> Result<Mutex<TracedRequestServiceBuilder<Body, ApiError>>, String> {
    env_into_process(env);
    let config = Config::try_parse().map_err(|e| format!("config: {e}"))?;
    let router = build_router(config).await;
    let builder = TracedRequestServiceBuilder::new(router)
      .map_err(|e| format!("router build: {e}"))?;
    Ok(Mutex::new(builder))
  }

  /// Convert the runtime's `web_sys::Request` into a `hyper::Request<Body>`.
  async fn to_hyper_request(
    req: web_sys::Request,
  ) -> Result<hyper::Request<Body>, JsValue> {
    let method = hyper::Method::from_bytes(req.method().as_bytes())
      .map_err(|e| JsValue::from_str(&format!("bad method: {e}")))?;
    let uri: hyper::Uri = req
      .url()
      .parse()
      .map_err(|e| JsValue::from_str(&format!("bad uri: {e}")))?;
    let mut builder = hyper::Request::builder().method(method).uri(uri);

    let headers = req.headers();
    if let Some(iter) = js_sys::try_iter(&headers)? {
      for entry in iter {
        let pair = js_sys::Array::from(&entry?);
        if let (Some(k), Some(v)) =
          (pair.get(0).as_string(), pair.get(1).as_string())
        {
          builder = builder.header(k, v);
        }
      }
    }

    let buf = wasm_bindgen_futures::JsFuture::from(req.array_buffer()?).await?;
    let bytes = js_sys::Uint8Array::new(&buf).to_vec();
    let body = if bytes.is_empty() {
      Body::empty()
    } else {
      Body::from(bytes)
    };
    builder
      .body(body)
      .map_err(|e| JsValue::from_str(&format!("build request: {e}")))
  }

  /// Convert a `hyper::Response<Body>` into the runtime's `web_sys::Response`.
  async fn to_web_response(
    resp: hyper::Response<Body>,
  ) -> Result<web_sys::Response, JsValue> {
    let (parts, body) = resp.into_parts();
    let bytes = hyper::body::to_bytes(body)
      .await
      .map_err(|e| JsValue::from_str(&format!("read response body: {e}")))?;

    let headers = web_sys::Headers::new()?;
    for (k, v) in parts.headers.iter() {
      if let Ok(v) = v.to_str() {
        headers.append(k.as_str(), v)?;
      }
    }

    let init = web_sys::ResponseInit::new();
    init.set_status(parts.status.as_u16());
    init.set_headers(&headers);

    let mut body_vec = bytes.to_vec();
    web_sys::Response::new_with_opt_u8_array_and_init(Some(&mut body_vec), &init)
  }

  #[wasm_bindgen(tokio, js_namespace = ["default"])]
  pub async fn fetch(
    request: web_sys::Request,
    env: JsValue,
    _ctx: JsValue,
  ) -> Result<web_sys::Response, JsValue> {
    let builder = BUILDER
      .get_or_try_init(|| init_builder(&env))
      .await
      .map_err(|e| JsValue::from_str(&e))?;

    // Build a per-request service under the lock, then release it before the
    // (async) request handling runs.
    let mut service = {
      let mut guard = builder.lock().unwrap();
      guard.build("127.0.0.1:0".parse().unwrap(), true)
    };

    let hyper_req = to_hyper_request(request).await?;
    let hyper_resp = service
      .call(hyper_req)
      .await
      .map_err(|e| JsValue::from_str(&format!("route error: {e}")))?;
    to_web_response(hyper_resp).await
  }
}
