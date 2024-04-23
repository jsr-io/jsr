// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use clap::ArgAction;
use clap::Parser;
use url::Url;

use crate::gcp::MetadataStrategy;

#[derive(Parser)]
pub struct Config {
  #[clap(long = "port", env = "PORT", default_value = "8001")]
  /// The bind address for the primary server.
  pub port: u16,

  #[clap(long = "gcs_endpoint", env = "GCS_ENDPOINT")]
  /// The endpoint to use to communicate with GCS. Defaults to the production
  /// GCS endpoint at https://storage.googleapis.com. This is useful for
  /// testing against a local GCS emulator.
  pub gcs_endpoint: Option<String>,

  #[clap(
    long = "publishing_bucket",
    env = "PUBLISHING_BUCKET",
    default_value = "publishing"
  )]
  /// The name of the GCS bucket to use to store tarballs during publishing.
  pub publishing_bucket: String,

  #[clap(
    long = "modules_bucket",
    env = "MODULES_BUCKET",
    default_value = "modules"
  )]
  /// The name of the GCS bucket where module files and metadata is stored.
  pub modules_bucket: String,

  #[clap(long = "docs_bucket", env = "DOCS_BUCKET", default_value = "docs")]
  /// The name of the GCS bucket where docs are stored.
  pub docs_bucket: String,

  #[clap(long = "npm_bucket", env = "NPM_BUCKET", default_value = "npm")]
  /// The name of the GCS bucket where npm tarballs and metadata are stored.
  pub npm_bucket: String,

  #[clap(
    long = "metadata_strategy",
    env = "METADATA_STRATEGY",
    default_value = "testing"
  )]
  /// The strategy to use to retrieve metadata from the GCP environment.
  pub metadata_strategy: MetadataStrategy,

  #[clap(
    long = "database_url",
    env = "DATABASE_URL",
    default_value = "postgres://user:password@localhost/registry"
  )]
  /// The URL to use to connect to the database.
  pub database_url: String,

  #[clap(long = "github_client_id", env = "GITHUB_CLIENT_ID")]
  /// The GitHub Client ID
  pub github_client_id: String,

  #[clap(long = "github_client_secret", env = "GITHUB_CLIENT_SECRET")]
  /// The GitHub Client Secret
  pub github_client_secret: String,

  #[clap(long = "orama_package_index_id", env = "ORAMA_PACKAGE_INDEX_ID")]
  /// The GitHub Client ID
  pub orama_package_index_id: Option<String>,

  #[clap(
    long = "orama_package_private_api_key",
    env = "ORAMA_PACKAGE_PRIVATE_API_KEY"
  )]
  /// The GitHub Client Secret
  pub orama_package_private_api_key: Option<String>,

  #[clap(long = "otlp_endpoint", env = "OTLP_ENDPOINT", group = "trace")]
  /// OTLP endpoint to send traces to.
  pub otlp_endpoint: Option<String>,

  #[clap(long = "cloud_trace", group = "trace")]
  /// Whether to enable cloud trace.
  pub cloud_trace: bool,

  #[clap(long = "registry_url", env = "REGISTRY_URL")]
  /// The base URL of the registry, where module code and metadata can be
  /// downloaded from.
  pub registry_url: Url,

  #[clap(long = "npm_url", env = "NPM_URL")]
  /// The base URL of the npm registry, where JSR npm tarballs and metadata will
  /// be accessible from.
  pub npm_url: Url,

  #[clap(
    long = "api",
    default_missing_value("true"),
    default_value("true"),
    num_args(0..=1),
    require_equals(true),
    action = ArgAction::Set,
  )]
  /// Enable serving the /api/* routes on the server, which enable users to make
  /// API calls to the service. This also enables the /login, /login/callback,
  /// and /logout routes.
  pub api: bool,

  #[clap(
    long = "tasks",
    default_missing_value("true"),
    default_value("true"),
    num_args(0..=1),
    require_equals(true),
    action = ArgAction::Set,
  )]
  /// Enable serving the /tasks/* routes on the server, which trigger async
  /// background task processing.
  pub tasks: bool,

  #[clap(long = "publish_queue_id", env = "PUBLISH_QUEUE_ID")]
  /// The ID of the publish queue.
  pub publish_queue_id: Option<String>,

  #[clap(
    long = "npm_tarball_build_queue_id",
    env = "NPM_TARBALL_BUILD_QUEUE_ID"
  )]
  /// The ID of the npm tarball build queue.
  pub npm_tarball_build_queue_id: Option<String>,

  #[clap(long = "postmark_token", env = "POSTMARK_TOKEN")]
  /// The Postmark token to use to send emails.
  pub postmark_token: Option<String>,

  #[clap(long = "email_from", env = "EMAIL_FROM")]
  /// The email address to send emails from.
  pub email_from: Option<String>,

  #[clap(long = "email_from_name", env = "EMAIL_FROM_NAME")]
  /// The name to send emails from.
  pub email_from_name: Option<String>,

  #[clap(long = "database_pool_size", default_value = "3")]
  /// The size of the database connection pool.
  pub database_pool_size: u32,
}

impl std::fmt::Debug for Config {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Config")
      .field("port", &self.port)
      .field("gcs_endpoint", &self.gcs_endpoint)
      .field("publishing_bucket", &self.publishing_bucket)
      .field("modules_bucket", &self.modules_bucket)
      .field("metadata_strategy", &self.metadata_strategy)
      .field("database_url", &"***")
      .field("github_client_id", &self.github_client_id)
      .field("github_client_secret", &"***")
      .field("otlp_endpoint", &self.otlp_endpoint)
      .field("cloud_trace", &self.cloud_trace)
      .field("registry_url", &self.registry_url)
      .field("api", &self.api)
      .field("tasks", &self.tasks)
      .field("publish_queue_id", &self.publish_queue_id)
      .field(
        "npm_tarball_build_queue_id",
        &self.npm_tarball_build_queue_id,
      )
      .field(
        "postmark_token",
        &self.postmark_token.as_ref().map(|_| "***"),
      )
      .field("email_from", &self.email_from)
      .field("email_from_name", &self.email_from_name)
      .finish()
  }
}
