// Copyright 2024 the JSR authors. All rights reserved. MIT license.

mod config;

use clap::Parser;
use hyper::Body;
use hyper::Server;
use routerify::Router;
use std::net::SocketAddr;
use std::time::Duration;
use url::Url;

use registry_api_tasks::errors::TaskError;
use registry_api_tasks::errors::error_handler;
use registry_api_tasks::tasks::AnalyticsEngineConfig;
use registry_api_tasks::tasks::NpmTarballBuildQueue;
use registry_api_tasks::tasks::tasks_router;

use shared::FallbackRegistryUrl;
use shared::NpmUrl;
use shared::PublishQueue;
use shared::RegistryUrl;
use shared::db::Database;
use shared::emails::EmailQueue;
use shared::emails::EmailSender;
use shared::external::algolia::AlgoliaClient;
use shared::external::cloudflare::CachePurge;
use shared::external::gcp::Queue;
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
) -> Router<Body, TaskError> {
  Router::builder()
    .data(database)
    .data(buckets)
    .data(algolia_client)
    .data(email_sender)
    .data(license_store)
    .data(RegistryUrl(registry_url))
    .data(NpmUrl(npm_url))
    .data(FallbackRegistryUrl(fallback_registry_url))
    .data(PublishQueue(publish_queue))
    .data(NpmTarballBuildQueue(npm_tarball_build_queue))
    .data(EmailQueue(email_queue))
    .data(AnalyticsEngineConfig(analytics_engine_config))
    .data(CachePurge(cache_purge_client))
    .middleware(routerify_query::query_parser())
    .err_handler_with_info(error_handler)
    // Cloud Tasks queues and Cloud Scheduler jobs (see terraform/) address the
    // handlers under `/tasks`, the prefix they had when they were served by
    // the api binary.
    .scope("/tasks", tasks_router::<TaskError>())
    .build()
    .unwrap()
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

  let gcp_client = shared::external::gcp::Client::new(config.metadata_strategy);
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
    s3::Bucket::new(
      config.npm_bucket,
      s3_region.clone(),
      s3_credentials.clone(),
    )
    .unwrap(),
  );
  let ticket_attachments_bucket = s3::BucketWithQueue::new(
    s3::Bucket::new(
      config.ticket_attachments_bucket,
      s3_region,
      s3_credentials,
    )
    .unwrap(),
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

  let router = main_router(MainRouterOptions {
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
  });

  // Create a Service from the router above to handle incoming requests.
  let service = TracedRouterService::new(router, true).unwrap();

  // The address on which the server will be listening.
  let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

  // Create a server by passing the created service to `.serve` method.
  let server = Server::bind(&addr).serve(service);

  println!("Tasks server is running on: {}", addr);
  if let Err(err) = server.await {
    eprintln!("Server error: {}", err);
  }
}
