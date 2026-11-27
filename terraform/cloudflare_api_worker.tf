// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The API as an emscripten Worker. The LB and the frontend reach it through
// service bindings (see lb.tf and cloudflare_frontend.tf). The background
// tasks run on a second worker, see cloudflare_api_tasks_worker.tf.

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
  }

  api_worker_envs = {
    "NO_COLOR" = "true"

    "DEPLOYMENT_ENVIRONMENT" = var.production ? "production" : "staging"

    "PUBLISHING_BUCKET" = cloudflare_r2_bucket.publishing.name
    "MODULES_BUCKET"    = cloudflare_r2_bucket.modules.name
    "DOCS_BUCKET"       = cloudflare_r2_bucket.docs.name
    "NPM_BUCKET"        = cloudflare_r2_bucket.npm.name

    "TICKET_ATTACHMENTS_BUCKET" = cloudflare_r2_bucket.ticket_attachments.name

    "S3_REGION"     = "auto"
    "S3_ENDPOINT"   = "${var.cloudflare_account_id}.r2.cloudflarestorage.com"
    "S3_ACCESS_KEY" = cloudflare_account_token.buckets_rw.id

    "GITHUB_CLIENT_ID" = var.github_client_id
    "GITLAB_CLIENT_ID" = var.gitlab_client_id

    "ALGOLIA_APP_ID"         = var.algolia_app_id
    "ALGOLIA_PACKAGES_INDEX" = algolia_index.packages.name
    "ALGOLIA_SYMBOLS_INDEX"  = algolia_index.symbols.name

    "REGISTRY_URL" = "https://${var.domain_name}"
    "NPM_URL"      = "https://${local.npm_domain}"

    "EMAIL_FROM" = "help@${var.domain_name}"

    // Support mail is forwarded from the Google Workspace inbox to Postmark, so
    // Postmark's own SPF check sees the forwarder rather than the sender. The
    // Workspace results, recorded before the hop, are trusted instead.
    "INBOUND_TRUSTED_AUTHSERV_ID" = "mx.google.com"
    "EMAIL_FROM_NAME"             = var.email_from_name

    "CLOUDFLARE_ACCOUNT_ID"        = var.cloudflare_account_id
    "CLOUDFLARE_ZONE_ID"           = var.cloudflare_zone_id
    "CLOUDFLARE_ANALYTICS_DATASET" = local.worker_download_analytics_dataset
  }

  # The API worker (api/) and the tasks worker (tasks/) are separate builds
  # with the same bindings.
  api_worker_modules = { for crate in ["api", "tasks"] : crate => [
    {
      name         = "index.js"
      content_file = "${path.module}/../${crate}/build/index.js"
      content_type = "application/javascript+module"
    },
    {
      name         = "index_bg.wasm"
      content_file = "${path.module}/../${crate}/build/index_bg.wasm"
      content_type = "application/wasm"
    },
  ] }

  api_worker_bindings = concat(
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
    [for name, queue in local.api_queues : {
      type       = "queue"
      name       = queue.binding
      queue_name = cloudflare_queue.api[name].queue_name
    }],
  )
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
    traces = {
      enabled            = true
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

  placement = {
    region = "gcp:us-central1"
  }

  modules = local.api_worker_modules["api"]
  bindings = concat(local.api_worker_bindings, [
    {
      # For work the API hands to the tasks worker directly, see
      # api/src/task_dispatch.rs.
      type    = "service"
      name    = "TASKS"
      service = cloudflare_worker.jsr_api_tasks.name
    },
  ])
}

resource "cloudflare_workers_deployment" "jsr_api" {
  # Serve the new version only once the database is migrated.
  depends_on = [terraform_data.api_setup]

  account_id  = var.cloudflare_account_id
  script_name = cloudflare_worker.jsr_api.name
  strategy    = "percentage"
  versions = [{
    percentage = 100
    version_id = cloudflare_worker_version.jsr_api.id
  }]
}

# Named after their fingerprints, so a certificate left behind in the account
# doesn't block creating these: names must be unique, and Cloudflare refuses to
# delete a certificate it still counts as in use. A certificate imported under
# an older name, whose private key Cloudflare never returns, is kept as is; a
# change to the certificate itself still replaces it under the new name.
# Cloudflare stores each certificate with a trailing newline, hence the added one.
resource "cloudflare_mtls_certificate" "db_client" {
  account_id   = var.cloudflare_account_id
  name         = "${var.gcp_project}-db-client-${substr(google_sql_ssl_cert.api.sha1_fingerprint, 0, 8)}"
  ca           = false
  certificates = "${trimspace(google_sql_ssl_cert.api.cert)}\n"
  private_key  = google_sql_ssl_cert.api.private_key

  lifecycle {
    ignore_changes = [name, private_key]
  }
}

resource "cloudflare_mtls_certificate" "db_ca" {
  account_id   = var.cloudflare_account_id
  name         = "${var.gcp_project}-db-ca-${substr(google_sql_database_instance.main_pg15.server_ca_cert[0].sha1_fingerprint, 0, 8)}"
  ca           = true
  certificates = "${trimspace(google_sql_database_instance.main_pg15.server_ca_cert[0].cert)}\n"

  lifecycle {
    ignore_changes = [name]
  }
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

  # Leaves room under max_connections (150) for database setup, which
  # connects directly (see api_setup.tf).
  origin_connection_limit = 100
}
