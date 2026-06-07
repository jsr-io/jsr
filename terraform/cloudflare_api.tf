// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The `api` Cloudflare Worker (workers-rs, wasm32) that fronts `api.jsr.io`.
// It serves the lightweight CRUD/DB/auth surface directly — reaching the
// existing Postgres through Cloudflare Hyperdrive (no sqlx) — and proxies the
// compute-only paths (publish, docs, source, diff, graph, /tasks/*) to the
// Cloud Run compute service. The `lb` Worker service-binds this Worker for the
// `api.jsr.io` backend (see lb.tf), exactly as it already does the frontend.
//
// Deployed as the same worker/version/deployment triple as the frontend (see
// cloudflare_frontend.tf): an immutable version holding the built wasm bundle,
// and a deployment pinning 100% of traffic to it.

# Hyperdrive carries the client certificate the Worker presents to Cloud SQL,
# uploaded as an account mTLS certificate. Same cert/key as google_sql_ssl_cert
# .api, which both Cloud Run services also present (see db.tf, cloud_run_api.tf).
resource "cloudflare_mtls_certificate" "api_db_client" {
  account_id   = var.cloudflare_account_id
  name         = "${var.gcp_project}-jsr-api-db-client"
  certificates = google_sql_ssl_cert.api.cert
  private_key  = google_sql_ssl_cert.api.private_key
  ca           = false
}

# Cloud SQL's server CA, uploaded so Hyperdrive can verify the origin (verify-ca:
# we connect by IP, so the hostname isn't checked, but the CA is).
resource "cloudflare_mtls_certificate" "api_db_ca" {
  account_id   = var.cloudflare_account_id
  name         = "${var.gcp_project}-jsr-api-db-ca"
  certificates = google_sql_ssl_cert.api.server_ca_cert
  ca           = true
}

# Hyperdrive connection to the existing Postgres over the public IP, with the
# client certificate (mTLS) as the access boundary — the DB requires it
# (ssl_mode = TRUSTED_CLIENT_CERTIFICATE_REQUIRED, see db.tf).
resource "cloudflare_hyperdrive_config" "api" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-jsr-api"

  origin = {
    scheme   = "postgres"
    database = google_sql_database.database.name
    host     = google_sql_database_instance.main_pg15.public_ip_address
    port     = 5432
    user     = google_sql_user.api.name
    password = google_sql_user.api.password
  }

  mtls = {
    sslmode             = "verify-ca"
    ca_certificate_id   = cloudflare_mtls_certificate.api_db_ca.id
    mtls_certificate_id = cloudflare_mtls_certificate.api_db_client.id
  }
}

resource "cloudflare_worker" "jsr_api" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-jsr-api"

  observability = {
    enabled = true
    logs = {
      enabled            = true
      invocation_logs    = false
      head_sampling_rate = 0.01
      persist            = false
      destinations       = [var.cloudflare_otlp_logs_destination]
    }
    traces = {
      enabled            = true
      head_sampling_rate = 0.01
      persist            = false
      destinations       = [var.cloudflare_otlp_traces_destination]
    }
  }
}

resource "cloudflare_worker_version" "jsr_api" {
  account_id          = var.cloudflare_account_id
  worker_id           = cloudflare_worker.jsr_api.id
  main_module         = "shim.mjs"
  compatibility_date  = "2026-05-19"
  compatibility_flags = ["nodejs_compat"]

  # `worker-build --release` (run in CI before terraform) emits the two-file
  # bundle into workers-rs/build/worker: the JS shim entrypoint and the wasm it
  # imports as `./index.wasm`.
  modules = [
    {
      name         = "shim.mjs"
      content_file = "${path.module}/../workers-rs/build/worker/shim.mjs"
      content_type = "application/javascript+module"
    },
    {
      name         = "index.wasm"
      content_file = "${path.module}/../workers-rs/build/worker/index.wasm"
      content_type = "application/wasm"
    },
  ]

  bindings = [
    {
      type = "hyperdrive"
      name = "HYPERDRIVE"
      id   = cloudflare_hyperdrive_config.api.id
      }, {
      # The Cloud Run compute service the Worker proxies compute-only paths to.
      # Public Cloud Run URL (the same value lb used for REGISTRY_API_URL before
      # the cutover); reached over `fetch` (see workers-rs proxy_to_compute).
      type = "plain_text"
      name = "COMPUTE_API_URL"
      text = google_cloud_run_v2_service.registry_api.uri
    }
  ]
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
