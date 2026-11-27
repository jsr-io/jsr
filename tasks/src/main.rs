// Copyright 2024 the JSR authors. All rights reserved. MIT license.

mod analysis;
mod config;
mod errors;
mod npm;
mod publish;
mod tarball;
mod tasks;

#[cfg(test)]
mod test;

use axum::Router;
use clap::Parser;
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
use std::time::Duration;
use url::Url;

use tasks::AnalyticsEngineConfig;
use tasks::NpmTarballBuildQueue;
use tasks::tasks_router;

use shared::FallbackRegistryUrl;
use shared::NpmUrl;
use shared::PublishQueue;
use shared::RegistryUrl;
use shared::db::Database;
use shared::emails::EmailQueue;
use shared::emails::EmailSender;
use shared::external::algolia::AlgoliaClient;
use shared::external::cloudflare::CachePurge;
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

use crate::config::Config;

pub struct MainRouterOptions {
  database: Database,
  buckets: Buckets,
  algolia_client: Option<AlgoliaClient>,
  email_sender: Option<EmailSender>,
  license_store: shared::util::LicenseStore,
  registry_url: Url,
  npm_url: Url,
  fallback_registry_url: Option<Url>,
  publish_queue: Option<Queue>,
  npm_tarball_build_queue: Option<Queue>,
  email_queue: Option<Queue>,
  analytics_engine_config: Option<(
    shared::external::cloudflare::AnalyticsEngineClient,
    /* dataset_name */ String,
  )>,
  cache_purge_client: Option<shared::external::cloudflare::CachePurgeClient>,
}

fn main_router(
  MainRouterOptions {
    database,
    buckets,
    algolia_client,
    email_sender,
    license_store,
    registry_url,
    npm_url,
    fallback_registry_url,
    publish_queue,
    npm_tarball_build_queue,
    email_queue,
    analytics_engine_config,
    cache_purge_client,
  }: MainRouterOptions,
) -> App {
  let data = Data::default()
    .with(database)
    .with(buckets)
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
    .with(CachePurge(cache_purge_client));

  // The api addresses the handlers under `/tasks`.
  router::app(Router::new().nest("/tasks", tasks_router()), data)
}

async fn build_router(config: Config) -> App {
  main_router(build_options(config).await)
}

/// Connects to everything the tasks use.
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
  setup_tracing("tasks", export_target, config.deployment_environment).await;

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
  let npm_tarball_build_queue = queue::binding("NPM_TARBALL_BUILD_QUEUE");
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

  let analytics_engine_config = match (
    config.cloudflare_account_id,
    config.cloudflare_api_token,
    config.cloudflare_analytics_dataset,
  ) {
    (Some(account_id), Some(api_token), Some(dataset_name)) => Some((
      shared::external::cloudflare::AnalyticsEngineClient::new(
        account_id, api_token,
      ),
      dataset_name,
    )),
    _ => None,
  };
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
    EmailSender::from_postmark_token(
      token,
      config
        .email_from
        .expect("email_from must be set when postmark_token is set"),
      config
        .email_from_name
        .expect("email_from_name must be set when postmark_token is set"),
    )
  });

  let license_store = shared::util::license_store();

  MainRouterOptions {
    database,
    buckets,
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
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
  dotenvy::from_filename(".env.local").ok();
  dotenvy::dotenv().ok();
  // The services share their configuration in local development.
  dotenvy::from_filename("../api/.env.local").ok();
  dotenvy::from_filename("../api/.env").ok();
  let config = Config::parse();
  println!("{config:?}");
  let port = config.port;

  let router = build_router(config).await;

  // Create a Service from the router above to handle incoming requests.
  let service = TracedRouterService::new(router, true);

  // The address on which the server will be listening.
  let addr = SocketAddr::from(([0, 0, 0, 0], port));
  let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

  // The bound address, since `--port=0` lets the OS pick one: the api's test
  // harness starts this binary that way and reads the port off this line.
  println!(
    "Tasks server is running on: {}",
    listener.local_addr().unwrap()
  );
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
  use ::worker::MessageBatch;
  use ::worker::MessageExt;
  use shared::worker_js;
  use tower::ServiceExt;
  use wasm_bindgen::prelude::*;

  /// Sets up the process for an invocation, each of which runs on its own
  /// tokio runtime, so nothing (the DB connection included) can be reused
  /// across invocations.
  fn config(env: JsValue) -> Result<Config, JsValue> {
    worker_js::env_into_process(&env);
    worker_js::set_env(env);
    if let Some(url) = worker_js::hyperdrive_database_url() {
      // SAFETY: single-threaded emscripten worker; no other threads race here.
      unsafe { std::env::set_var("DATABASE_URL", url) };
    }
    Config::try_parse_from(["registry_api_tasks"])
      .map_err(|e| JsValue::from_str(&format!("config: {e}")))
  }

  /// Serves `/tasks/*` to the api worker, which reaches this worker through
  /// its `TASKS` service binding.
  #[wasm_bindgen(experimental_tokio = "isolated")]
  pub async fn fetch(
    request: web_sys::Request,
    env: JsValue,
    _ctx: JsValue,
  ) -> Result<web_sys::Response, JsValue> {
    let config = config(env)?;
    // Keeps the invocation's span active on every poll: the spans the request
    // starts are parented under whichever span is active.
    ::worker::observability::in_current_span(async {
      let app = build_router(config).await;
      let hyper_req = worker_js::to_hyper_request(request).await?;
      let Ok(hyper_resp) =
        TracedRouterService::new(app, true).oneshot(hyper_req).await;
      worker_js::to_web_response(hyper_resp).await
    })
    .await
  }

  /// An error leaves a message unacknowledged, so the queue delivers it again
  /// after the consumer's `retry_delay`.
  #[wasm_bindgen(experimental_tokio = "isolated")]
  pub async fn queue(
    batch: worker_sys::MessageBatch,
    env: JsValue,
    _ctx: JsValue,
  ) -> Result<(), JsValue> {
    fn error(err: impl std::fmt::Display) -> JsValue {
      JsValue::from_str(&err.to_string())
    }

    let config = config(env)?;
    ::worker::observability::in_current_span(async {
      let o = build_options(config).await;
      let cache_purge = CachePurge(o.cache_purge_client.clone());
      let queue = String::from(batch.queue()?);
      if queue.ends_with("-publish") {
        for message in MessageBatch::<uuid::Uuid>::from(batch).iter() {
          let message = message.map_err(error)?;
          publish::publish_task(
            *message.body(),
            // `attempts` is 1 on the first delivery.
            message.attempts().saturating_sub(1),
            o.buckets.clone(),
            o.license_store.clone(),
            o.registry_url.clone(),
            o.npm_url.clone(),
            o.fallback_registry_url.clone(),
            o.database.clone(),
            o.algolia_client.clone(),
            cache_purge.clone(),
          )
          .await
          .map_err(error)?;
          message.ack();
        }
      } else if queue.ends_with("-npm-tarball-build") {
        for message in
          MessageBatch::<tasks::NpmTarballBuildJob>::from(batch).iter()
        {
          let message = message.map_err(error)?;
          tasks::npm_tarball_build(
            message.body().clone(),
            &o.database,
            o.buckets.clone(),
            o.registry_url.clone(),
            o.npm_url.clone(),
            cache_purge.clone(),
          )
          .await
          .map_err(error)?;
          message.ack();
        }
      } else if queue.ends_with("-email") {
        for message in
          MessageBatch::<shared::emails::SendEmailTask>::from(batch).iter()
        {
          let message = message.map_err(error)?;
          let task = message.body().clone();
          tasks::send_email(task, &o.database, o.email_sender.as_ref())
            .await
            .map_err(error)?;
          message.ack();
        }
      } else {
        return Err(error(format!("unknown queue {queue}")));
      }
      Ok(())
    })
    .await
  }

  #[wasm_bindgen(experimental_tokio = "isolated")]
  pub async fn scheduled(
    controller: JsValue,
    env: JsValue,
    _ctx: JsValue,
  ) -> Result<(), JsValue> {
    let config = config(env)?;
    let cron = js_sys::Reflect::get(&controller, &"cron".into())?
      .as_string()
      .unwrap_or_default();
    ::worker::observability::in_current_span(async {
      let options = build_options(config).await;
      let db = &options.database;
      match cron.as_str() {
        "*/5 * * * *" => {
          // One failing does not keep the other from running.
          let requeued = tasks::requeue_stuck_publishing_tasks(
            db,
            options.publish_queue.as_ref(),
          )
          .await;
          let swept = tasks::sweep_pending_emails(
            db,
            options.email_sender.as_ref(),
            options.email_queue.as_ref(),
          )
          .await;
          requeued.and(swept)
        }
        "*/15 * * * *" => {
          tasks::npm_tarball_enqueue(
            db,
            options.npm_tarball_build_queue.as_ref(),
          )
          .await
        }
        "15 * * * *" => {
          let analytics_engine =
            AnalyticsEngineConfig(options.analytics_engine_config.clone());
          tasks::scrape_download_counts(db, &analytics_engine, 12).await
        }
        "0 0 * * *" => tasks::clean_oauth_states(db).await,
        "0 3 * * *" => tasks::clean_download_counts_4h(db).await,
        "0 4 * * *" => tasks::anonymize_deleted_users(db).await,
        cron => {
          return Err(JsValue::from_str(&format!("unknown cron {cron}")));
        }
      }
      .map_err(|e| JsValue::from_str(&e.to_string()))
    })
    .await
  }
}
