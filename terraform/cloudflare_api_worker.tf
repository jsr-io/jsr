// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The API built for wasm32-unknown-emscripten (see EMSCRIPTEN.md) as a
// Worker, on staging only. The LB sends API traffic to it through a service
// binding (see lb.tf); background tasks stay on the Cloud Run tasks service.

locals {
  api_worker_count = var.production ? 0 : 1

  # The Worker reaches Cloud SQL over its public IP, presenting the same client
  # certificate Cloud Run does (see db.tf).
  api_worker_database_url = "postgres://${google_sql_user.api.name}:${google_sql_user.api.password}@${google_sql_database_instance.main_pg15.public_ip_address}/${google_sql_database.database.name}"

  api_worker_secrets = {
    "DATABASE_URL"              = local.api_worker_database_url
    "DB_CLIENT_KEY"             = google_sql_ssl_cert.api.private_key
    "S3_SECRET_KEY"             = local.r2_secret_access_key
    "GITHUB_CLIENT_SECRET"      = var.github_client_secret
    "GITLAB_CLIENT_SECRET"      = var.gitlab_client_secret
    "TURNSTILE_SECRET_KEY"      = cloudflare_turnstile_widget.login.secret
    "POSTMARK_TOKEN"            = var.postmark_token
    "POSTMARK_WEBHOOK_PASSWORD" = var.postmark_webhook_password
    "ALGOLIA_WRITE_API_KEY"     = algolia_api_key.write.key
    "CLOUDFLARE_API_TOKEN"      = var.cloudflare_api_token
    "GCP_SERVICE_ACCOUNT_KEY"   = try(base64decode(google_service_account_key.registry_api_worker[0].private_key), null)
  }

  # OTLP export is not compiled into the Worker, and migrations are left to
  # Cloud Run rather than run on every request.
  api_worker_envs = merge({
    for name, value in local.api_envs : name => value
    if !contains(concat(keys(local.otlp_envs), keys(local.api_worker_secrets)), name)
  }, {
    "DATABASE_DISABLE_MIGRATIONS" = "1"
    # There is no metadata server, so GCP access tokens (Cloud Tasks) are
    # signed with a key for the same service account Cloud Run uses.
    "METADATA_STRATEGY" = "service_account_key"
  })
}

resource "cloudflare_worker" "jsr_api" {
  count      = local.api_worker_count
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
  count               = local.api_worker_count
  account_id          = var.cloudflare_account_id
  worker_id           = cloudflare_worker.jsr_api[0].id
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
  )
}

resource "cloudflare_workers_deployment" "jsr_api" {
  count       = local.api_worker_count
  account_id  = var.cloudflare_account_id
  script_name = cloudflare_worker.jsr_api[0].name
  strategy    = "percentage"
  versions = [{
    percentage = 100
    version_id = cloudflare_worker_version.jsr_api[0].id
  }]
}

resource "google_service_account_key" "registry_api_worker" {
  count              = local.api_worker_count
  service_account_id = google_service_account.registry_api.name
}
