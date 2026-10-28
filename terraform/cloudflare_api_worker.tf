// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The API as an emscripten Worker. The LB and the frontend reach it through
// service bindings (see lb.tf and cloudflare_frontend.tf).

locals {
  api_worker_secrets = {
    "S3_SECRET_KEY"             = local.r2_secret_access_key
    "GITHUB_CLIENT_SECRET"      = var.github_client_secret
    "GITLAB_CLIENT_SECRET"      = var.gitlab_client_secret
    "TURNSTILE_SECRET_KEY"      = cloudflare_turnstile_widget.login.secret
    "POSTMARK_TOKEN"            = var.postmark_token
    "POSTMARK_WEBHOOK_PASSWORD" = var.postmark_webhook_password
    "ALGOLIA_WRITE_API_KEY"     = algolia_api_key.write.key
    "CLOUDFLARE_API_TOKEN"      = var.cloudflare_api_token
    "GCP_SERVICE_ACCOUNT_KEY"   = base64decode(google_service_account_key.registry_api_worker.private_key)
  }

  api_worker_envs = merge({
    for name, value in local.api_envs : name => value
    if !contains(concat(
      keys(local.otlp_envs),
      keys(local.api_worker_secrets),
      # The database is reached through Hyperdrive, which does the TLS.
      ["DATABASE_URL", "DB_CLIENT_CERT", "DB_CLIENT_KEY"],
    ), name)
  }, {
    "DATABASE_DISABLE_MIGRATIONS" = "1"
    "METADATA_STRATEGY" = "service_account_key"
    # Staging test only: log the Postgres messages on each connection.
    "DB_PROTOCOL_TRACE" = "1"
  })
}

resource "cloudflare_worker" "jsr_api" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-jsr-api"

  observability = {
    enabled = true
    logs = {
      enabled            = true
      invocation_logs    = true
      head_sampling_rate = 1
      persist            = true
    }
  }
}

resource "cloudflare_worker_version" "jsr_api" {
  account_id          = var.cloudflare_account_id
  worker_id           = cloudflare_worker.jsr_api.id
  main_module         = "index.js"
  compatibility_date  = "2026-09-02"
  compatibility_flags = ["nodejs_compat"]

  modules = [
    {
      name         = "index.js"
      content_file = "${path.module}/../api/build/index.js"
      content_type = "application/javascript+module"
    },
    {
      name         = "index_bg.wasm"
      content_file = "${path.module}/../api/build/index_bg.wasm"
      content_type = "application/wasm"
    },
  ]

  bindings = concat(
    [for name, value in local.api_worker_envs : {
      type = "plain_text"
      name = name
      text = value
    }],
    [for name, value in local.api_worker_secrets : {
      type = "secret_text"
      name = name
      text = value
    }],
    [{
      type = "hyperdrive"
      name = "HYPERDRIVE"
      id   = cloudflare_hyperdrive_config.api.id
    }],
    [for bucket in [
      cloudflare_r2_bucket.publishing,
      cloudflare_r2_bucket.modules,
      cloudflare_r2_bucket.docs,
      cloudflare_r2_bucket.npm,
      cloudflare_r2_bucket.ticket_attachments,
      ] : {
      type        = "r2_bucket"
      name        = "R2_${replace(bucket.name, "-", "_")}"
      bucket_name = bucket.name
    }],
  )
}

resource "cloudflare_workers_deployment" "jsr_api" {
  account_id  = var.cloudflare_account_id
  script_name = cloudflare_worker.jsr_api.name
  strategy    = "percentage"
  versions = [{
    percentage = 100
    version_id = cloudflare_worker_version.jsr_api.id
  }]
}

resource "google_service_account_key" "registry_api_worker" {
  service_account_id = google_service_account.registry_api.name
}

resource "cloudflare_mtls_certificate" "db_client" {
  account_id   = var.cloudflare_account_id
  name         = "${var.gcp_project}-db-client"
  ca           = false
  certificates = google_sql_ssl_cert.api.cert
  private_key  = google_sql_ssl_cert.api.private_key
}

resource "cloudflare_mtls_certificate" "db_ca" {
  account_id   = var.cloudflare_account_id
  name         = "${var.gcp_project}-db-ca"
  ca           = true
  certificates = google_sql_database_instance.main_pg15.server_ca_cert[0].cert
}

resource "cloudflare_hyperdrive_config" "api" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-api"

  origin = {
    scheme   = "postgres"
    host     = google_sql_database_instance.main_pg15.public_ip_address
    port     = 5432
    database = google_sql_database.database.name
    user     = google_sql_user.api.name
    password = google_sql_user.api.password
  }

  mtls = {
    mtls_certificate_id = cloudflare_mtls_certificate.db_client.id
    ca_certificate_id   = cloudflare_mtls_certificate.db_ca.id
    sslmode             = "verify-ca"
  }

  # Cached reads could be up to a minute stale after a publish.
  caching = {
    disabled = true
  }

  # Leaves room under max_connections (150) for the Cloud Run tasks service.
  origin_connection_limit = 100
}
