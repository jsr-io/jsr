// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use clap::Parser;
use url::Url;

#[derive(Parser)]
pub struct Config {
  #[clap(long = "port", env = "PORT", default_value = "8002")]
  /// The bind address for the tasks server.
  pub port: u16,

  #[clap(long = "s3_region", env = "S3_REGION")]
  pub s3_region: String,
  #[clap(long = "s3_endpoint", env = "S3_ENDPOINT")]
  pub s3_endpoint: String,
  #[clap(long = "s3_access_key", env = "S3_ACCESS_KEY")]
  pub s3_access_key: String,
  #[clap(long = "s3_secret_key", env = "S3_SECRET_KEY")]
  pub s3_secret_key: String,

  #[clap(
    long = "publishing_bucket",
    env = "PUBLISHING_BUCKET",
    default_value = "publishing"
  )]
  /// The name of the S3 bucket to use to store tarballs during publishing.
  pub publishing_bucket: String,

  #[clap(
    long = "modules_bucket",
    env = "MODULES_BUCKET",
    default_value = "modules"
  )]
  /// The name of the S3 bucket where module files and metadata is stored.
  pub modules_bucket: String,

  #[clap(long = "docs_bucket", env = "DOCS_BUCKET", default_value = "docs")]
  /// The name of the S3 bucket where docs are stored.
  pub docs_bucket: String,

  #[clap(long = "npm_bucket", env = "NPM_BUCKET", default_value = "npm")]
  /// The name of the S3 bucket where npm tarballs and metadata are stored.
  pub npm_bucket: String,

  #[clap(
    long = "ticket_attachments_bucket",
    env = "TICKET_ATTACHMENTS_BUCKET",
    default_value = "ticket-attachments"
  )]
  /// The name of the S3 bucket where files attached to support ticket emails
  /// are stored. Not used by any task, but part of the shared bucket set.
  pub ticket_attachments_bucket: String,

  #[clap(
    long = "database_url",
    env = "DATABASE_URL",
    default_value = "postgres://user:password@localhost/registry"
  )]
  /// The URL to use to connect to the database.
  pub database_url: String,

  #[clap(long = "algolia_app_id", env = "ALGOLIA_APP_ID")]
  /// The Algolia application id
  pub algolia_app_id: Option<String>,

  #[clap(long = "algolia_write_api_key", env = "ALGOLIA_WRITE_API_KEY")]
  /// The Algolia API key with write access, used for indexing
  pub algolia_write_api_key: Option<String>,

  #[clap(long = "algolia_packages_index", env = "ALGOLIA_PACKAGES_INDEX")]
  /// The Algolia index name for packages
  pub algolia_packages_index: Option<String>,

  #[clap(long = "algolia_symbols_index", env = "ALGOLIA_SYMBOLS_INDEX")]
  /// The Algolia index name for symbols
  pub algolia_symbols_index: Option<String>,

  #[clap(long = "otlp_endpoint", env = "OTLP_ENDPOINT")]
  /// Base OTLP/HTTP endpoint (e.g. Grafana Cloud's
  /// `https://otlp-gateway-<zone>.grafana.net/otlp`), OTEL
  /// `OTEL_EXPORTER_OTLP_ENDPOINT` style: the per-signal path (`/v1/traces`
  /// for spans, `/v1/logs` for logs) is appended automatically. A full
  /// signal URL is also accepted. Export is disabled when unset.
  pub otlp_endpoint: Option<String>,

  #[clap(long = "otlp_headers", env = "OTLP_HEADERS")]
  /// Extra headers sent with every OTLP request, as a comma-separated list of
  /// `key=value` pairs (the OpenTelemetry `OTEL_EXPORTER_OTLP_HEADERS` format).
  /// Used to carry the backend's auth, e.g. `Authorization=Basic <base64>` for
  /// Grafana Cloud. Only the first `=` in each pair separates key from value.
  pub otlp_headers: Option<String>,

  #[clap(long = "deployment_environment", env = "DEPLOYMENT_ENVIRONMENT")]
  /// Deployment environment name (e.g. `staging`, `production`), exported as the
  /// `deployment.environment` OTLP resource attribute so telemetry from each
  /// environment can be told apart in the backend. Unset omits the attribute.
  pub deployment_environment: Option<String>,

  #[clap(long = "registry_url", env = "REGISTRY_URL")]
  /// The base URL of the registry, where module code and metadata can be
  /// downloaded from.
  pub registry_url: Url,

  #[clap(long = "npm_url", env = "NPM_URL")]
  /// The base URL of the npm registry, where JSR npm tarballs and metadata will
  /// be accessible from.
  pub npm_url: Url,

  #[clap(
    long = "fallback_registry_url",
    env = "FALLBACK_REGISTRY_URL",
    value_parser = parse_base_url
  )]
  /// The base URL of a fallback registry to use when resolving JSR dependencies
  /// that are not available locally. This allows self-hosted instances to
  /// depend on packages from jsr.io or other instances.
  pub fallback_registry_url: Option<Url>,

  #[clap(long = "cloudflare_account_id", env = "CLOUDFLARE_ACCOUNT_ID")]
  /// The Cloudflare account ID for Analytics Engine.
  pub cloudflare_account_id: Option<String>,

  #[clap(long = "cloudflare_api_token", env = "CLOUDFLARE_API_TOKEN")]
  /// The Cloudflare API token.
  pub cloudflare_api_token: Option<String>,

  #[clap(long = "cloudflare_zone_id", env = "CLOUDFLARE_ZONE_ID")]
  /// The Cloudflare zone ID for the registry domain, used to purge cached
  /// package and npm version manifests when a package is published or
  /// mutated. Cache purge is skipped if unset.
  pub cloudflare_zone_id: Option<String>,

  #[clap(
    long = "cloudflare_analytics_dataset",
    env = "CLOUDFLARE_ANALYTICS_DATASET"
  )]
  /// The Cloudflare Analytics Engine dataset name for download tracking.
  pub cloudflare_analytics_dataset: Option<String>,

  #[clap(long = "email_from", env = "EMAIL_FROM")]
  /// The email address to send emails from. Email is sent through the Worker's
  /// `EMAIL` binding, so there is no sender without one.
  pub email_from: Option<String>,

  #[clap(long = "email_from_name", env = "EMAIL_FROM_NAME")]
  /// The name to send emails from.
  pub email_from_name: Option<String>,

  #[clap(long = "database_pool_size", default_value = "3")]
  /// The size of the database connection pool.
  pub database_pool_size: u32,

  #[clap(long = "service_account_token", env = "SERVICE_ACCOUNT_TOKEN")]
  /// A static bearer token for the system service account
  /// (00000000-0000-0000-0000-000000000000). When set, a token with this
  /// value's hash is upserted into the database on startup.
  pub service_account_token: Option<String>,

  #[clap(long = "db_client_cert", env = "DB_CLIENT_CERT")]
  /// PEM client certificate presented when connecting to the database over
  /// TLS. Required once the DB enforces `TRUSTED_CLIENT_CERTIFICATE_REQUIRED`;
  /// all three of cert/key/root must be set together to take effect.
  pub db_client_cert: Option<String>,

  #[clap(long = "db_client_key", env = "DB_CLIENT_KEY")]
  /// PEM private key matching `db_client_cert`.
  pub db_client_key: Option<String>,
}

impl std::fmt::Debug for Config {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Config")
      .field("port", &self.port)
      .field("publishing_bucket", &self.publishing_bucket)
      .field("modules_bucket", &self.modules_bucket)
      .field("database_url", &"***")
      .field("otlp_endpoint", &self.otlp_endpoint)
      .field("otlp_headers", &self.otlp_headers.as_ref().map(|_| "***"))
      .field("deployment_environment", &self.deployment_environment)
      .field("registry_url", &self.registry_url)
      .field("email_from", &self.email_from)
      .field("email_from_name", &self.email_from_name)
      .field(
        "service_account_token",
        &self.service_account_token.as_ref().map(|_| "***"),
      )
      .field(
        "db_client_cert",
        &self.db_client_cert.as_ref().map(|_| "***"),
      )
      .field("db_client_key", &self.db_client_key.as_ref().map(|_| "***"))
      .finish()
  }
}

/// Parse a base URL, normalizing it to end with a trailing slash. Every
/// consumer of a base URL assumes one: `Url::join` treats a base without a
/// trailing slash as a file and drops its last path segment, and the frontend
/// builds links by direct concatenation — so a subpath-hosted registry
/// (`https://mirror.corp/jsr`) would otherwise silently query and link the
/// wrong URLs. Normalizing once at ingestion keeps all consumers in agreement.
fn parse_base_url(s: &str) -> Result<Url, url::ParseError> {
  if s.ends_with('/') {
    Url::parse(s)
  } else {
    Url::parse(&format!("{s}/"))
  }
}
