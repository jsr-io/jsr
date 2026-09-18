mod analysis;
mod errors;
mod npm;
mod publish;
mod tarball;
mod tasks;

use routerify::Router;
use routerify::ext::RequestExt;
use std::net::SocketAddr;
use std::time::Duration;

use crate::errors::TaskError;
use shared::external::gcp;
use shared::s3;
use shared::util;

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
  }: MainRouterOptions,
) -> Router<hyper::Body, TaskError> {
  Router::builder()
    .data(database)
    .data(buckets)
    .data(generate_ctx_cache)
    .data(object_cache)
    .data(registry_metadata_cache)
    .data(github_client)
    .data(gitlab_client)
    .data(algolia_client)
    .data(email_sender)
    .data(license_store)
    .data(shared::RegistryUrl(registry_url))
    .data(shared::NpmUrl(npm_url))
    .data(shared::FallbackRegistryUrl(fallback_registry_url))
    .data(shared::PublishQueue(publish_queue))
    .data(NpmTarballBuildQueue(npm_tarball_build_queue))
    .data(EmailQueue(email_queue))
    .data(AnalyticsEngineConfig(analytics_engine_config))
    .data(CachePurge(cache_purge_client))
    .data(turnstile)
    .data(postmark_webhook_password)
    .data(inbound_trusted_authserv_id)
    .data(shared::db::DependentCountCache::new())
    .middleware(routerify_query::query_parser())
    .err_handler_with_info(errors::error_handler)
    .post("/publish", util::json(publish::publish_handler))
    .post(
      "/npm_tarball_build",
      util::json(tasks::npm_tarball_build_handler),
    )
    .post(
      "/npm_tarball_enqueue",
      util::json(tasks::npm_tarball_enqueue_handler),
    )
    .post(
      "/scrape_download_counts",
      util::json(tasks::scrape_download_counts_handler),
    )
    .post(
      "/clean_oauth_states",
      util::json(tasks::clean_oauth_states_handler),
    )
    .post("/send_email", tasks::send_email_handler)
    .post(
      "/sweep_pending_emails",
      util::json(tasks::sweep_pending_emails_handler),
    )
    .post(
      "/clean_download_counts_4h",
      util::json(tasks::clean_download_counts_4h_handler),
    )
    .post(
      "/requeue_stuck_publishing_tasks",
      util::json(tasks::requeue_stuck_publishing_tasks_handler),
    )
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

  let license_store = shared::util::license_store();

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
