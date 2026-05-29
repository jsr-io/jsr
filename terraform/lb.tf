// Copyright 2024 the JSR authors. All rights reserved. MIT license.

locals {
  worker_download_analytics_dataset = "${var.gcp_project}-downloads"
}

// The LB Cloudflare Worker, deployed as the modern "versions and deployments"
// trio (mirroring terraform/cloudflare_frontend.tf):
//
//   1. `cloudflare_worker.jsr_lb`         — the worker shell (name, tags).
//   2. `cloudflare_worker_version.jsr_lb` — an immutable version. It carries
//      all bindings AND the `ApiContainer` Durable Object class, which is what
//      makes this worker a Cloudflare Container host. Creating a new version
//      kicks off the container image rollout.
//   3. `cloudflare_workers_deployment.jsr_lb` — pins 100% of traffic to the
//      current version id. Updating it is the promotion.
//
// The API server image itself is NOT managed here — terraform can only attach
// the DO class (`containers = [{ class_name }]`), not set the image. CI pushes
// the image with `wrangler containers push` and then, between creating the
// version and promoting the deployment, waits for the container rollout to
// finish (tools/wait-container-rollout.ts). See .github/workflows/ci.yml.

resource "cloudflare_worker" "jsr_lb" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-jsr-lb"

  observability = {
    enabled            = true
    head_sampling_rate = 0.01
    logs = {
      enabled         = true
      invocation_logs = true
      persist         = true
    }
  }
}

resource "cloudflare_worker_version" "jsr_lb" {
  account_id         = var.cloudflare_account_id
  worker_id          = cloudflare_worker.jsr_lb.id
  main_module        = "worker.js"
  compatibility_date = "2025-01-01"

  modules = [
    {
      name         = "worker.js"
      content_file = "${path.module}/../lb/dist/main.js"
      content_type = "application/javascript+module"
    },
  ]

  # The ApiContainer DO class must be SQLite-backed (required for containers).
  #
  # Cloudflare tracks the applied migration tag per worker, so re-sending this
  # block with an unchanged `new_tag` on every deploy is a no-op — it does NOT
  # re-run. Bump var.migrations_version only when adding/renaming DO classes.
  #
  # First-time bootstrap (once per environment): the versions API validates the
  # API_CONTAINER binding before the migration is applied, so the very first
  # creation of the class fails (error 100123) if the binding and migration land
  # in the same version. Bootstrap once by deploying a migration-only version
  # before the binding exists — temporarily comment out the API_CONTAINER
  # binding + the `containers` block, apply, then restore them and apply again.
  migrations = {
    new_tag            = var.migrations_version
    new_sqlite_classes = ["ApiContainer"]
  }

  # Attaches the ApiContainer DO class to this worker as a container. The image
  # and instance config live with the container application (pushed/configured
  # via wrangler), not here — terraform only declares the attachment.
  containers = [{
    class_name = "ApiContainer"
  }]

  bindings = [
    {
      type    = "analytics_engine"
      name    = "DOWNLOADS"
      dataset = local.worker_download_analytics_dataset
      }, {
      type        = "r2_bucket"
      name        = "MODULES_BUCKET"
      bucket_name = cloudflare_r2_bucket.modules.name
      }, {
      type        = "r2_bucket"
      name        = "NPM_BUCKET"
      bucket_name = cloudflare_r2_bucket.npm.name
      }, {
      type = "plain_text"
      name = "ROOT_DOMAIN"
      text = var.domain_name
      }, {
      type = "plain_text"
      name = "API_DOMAIN"
      text = local.api_domain
      }, {
      type = "plain_text"
      name = "NPM_DOMAIN"
      text = local.npm_domain
      }, {
      # The API server runs as a Cloudflare Container fronted by this Durable
      # Object namespace; requests are load-balanced across instances in
      # handleAPIRequest (lb/main.ts) via `getRandom`.
      type       = "durable_object_namespace"
      name       = "API_CONTAINER"
      class_name = "ApiContainer"
      }, {
      # Service binding to the frontend Worker. Terraform uploads new
      # versions via `cloudflare_worker_version.jsr_frontend` and
      # promotes them via `cloudflare_workers_deployment.jsr_frontend`;
      # the `depends_on` below makes the LB binding wait for the
      # promotion so the LB never references an un-promoted version.
      type    = "service"
      name    = "FRONTEND"
      service = "${var.gcp_project}-jsr-frontend"
      }, {
      # Per-IP rate limit applied only on the frontend proxy path (see
      # handleFrontendRoute in lb/main.ts). Keeps scrapers from generating
      # cache-miss load on the frontend Worker without touching modules,
      # API, or npm.
      # namespace_id is a per-account identifier for this rate-limit binding;
      # any unused value works (no Cloudflare-reserved meaning for "1001").
      type         = "ratelimit"
      name         = "FRONTEND_RATELIMIT"
      namespace_id = "1001"
      simple = {
        limit  = 120
        period = 60
      }
    },

    # --- API container environment variables ---
    # The LB worker doesn't read these; it forwards them into the
    # ApiContainer's process env (see apiEnvVars in lb/containers.ts).
    {
      type = "secret_text"
      name = "DATABASE_URL"
      text = "postgres://${google_sql_user.api.name}:${google_sql_user.api.password}@${google_sql_database_instance.main_pg15.public_ip_address}/${google_sql_database.database.name}?sslmode=require"
      }, {
      # The container runs on Cloudflare, not GCP, so it mints GCP tokens from
      # a service account key (below) instead of the instance metadata server.
      type = "plain_text"
      name = "METADATA_STRATEGY"
      text = "service_account_key"
      }, {
      type = "secret_text"
      name = "GCP_SERVICE_ACCOUNT_KEY"
      text = base64decode(google_service_account_key.registry_api.private_key)
      }, {
      type = "plain_text"
      name = "PUBLISHING_BUCKET"
      text = cloudflare_r2_bucket.publishing.name
      }, {
      type = "plain_text"
      name = "MODULES_BUCKET_NAME"
      text = cloudflare_r2_bucket.modules.name
      }, {
      type = "plain_text"
      name = "DOCS_BUCKET"
      text = cloudflare_r2_bucket.docs.name
      }, {
      type = "plain_text"
      name = "NPM_BUCKET_NAME"
      text = cloudflare_r2_bucket.npm.name
      }, {
      type = "plain_text"
      name = "S3_REGION"
      text = "auto"
      }, {
      type = "secret_text"
      name = "S3_ENDPOINT"
      text = "${var.cloudflare_account_id}.r2.cloudflarestorage.com"
      }, {
      type = "secret_text"
      name = "S3_ACCESS_KEY"
      text = cloudflare_account_token.buckets_rw.id
      }, {
      type = "secret_text"
      name = "S3_SECRET_KEY"
      text = local.r2_secret_access_key
      }, {
      type = "plain_text"
      name = "GITHUB_CLIENT_ID"
      text = var.github_client_id
      }, {
      type = "secret_text"
      name = "GITHUB_CLIENT_SECRET"
      text = var.github_client_secret
      }, {
      type = "plain_text"
      name = "GITLAB_CLIENT_ID"
      text = var.gitlab_client_id
      }, {
      type = "secret_text"
      name = "GITLAB_CLIENT_SECRET"
      text = var.gitlab_client_secret
      }, {
      type = "secret_text"
      name = "POSTMARK_TOKEN"
      text = var.postmark_token
      }, {
      type = "plain_text"
      name = "ORAMA_PACKAGES_PROJECT_ID"
      text = var.orama_packages_project_id
      }, {
      type = "secret_text"
      name = "ORAMA_PACKAGES_PROJECT_KEY"
      text = var.orama_packages_project_key
      }, {
      type = "plain_text"
      name = "ORAMA_PACKAGES_DATA_SOURCE"
      text = var.orama_packages_data_source
      }, {
      type = "plain_text"
      name = "ORAMA_SYMBOLS_PROJECT_ID"
      text = var.orama_symbols_project_id
      }, {
      type = "secret_text"
      name = "ORAMA_SYMBOLS_PROJECT_KEY"
      text = var.orama_symbols_project_key
      }, {
      type = "plain_text"
      name = "ORAMA_SYMBOLS_DATA_SOURCE"
      text = var.orama_symbols_data_source
      }, {
      type = "plain_text"
      name = "REGISTRY_URL"
      text = "https://${var.domain_name}"
      }, {
      type = "plain_text"
      name = "NPM_URL"
      text = "https://${local.npm_domain}"
      }, {
      type = "plain_text"
      name = "EMAIL_FROM"
      text = "help@${var.domain_name}"
      }, {
      type = "plain_text"
      name = "EMAIL_FROM_NAME"
      text = var.email_from_name
      }, {
      type = "plain_text"
      name = "PUBLISH_QUEUE_ID"
      text = "projects/${var.gcp_project}/locations/us-central1/queues/${local.publishing_tasks_queue_name}"
      }, {
      type = "plain_text"
      name = "NPM_TARBALL_BUILD_QUEUE_ID"
      text = "projects/${var.gcp_project}/locations/us-central1/queues/${local.npm_tarball_build_tasks_queue_name}"
      }, {
      type = "plain_text"
      name = "CLOUDFLARE_ACCOUNT_ID"
      text = var.cloudflare_account_id
      }, {
      type = "secret_text"
      name = "CLOUDFLARE_API_TOKEN"
      text = var.cloudflare_api_token
      }, {
      type = "plain_text"
      name = "CLOUDFLARE_ANALYTICS_DATASET"
      text = local.worker_download_analytics_dataset
    }
  ]
}

resource "cloudflare_workers_deployment" "jsr_lb" {
  account_id  = var.cloudflare_account_id
  script_name = cloudflare_worker.jsr_lb.name
  strategy    = "percentage"
  versions = [{
    percentage = 100
    version_id = cloudflare_worker_version.jsr_lb.id
  }]

  depends_on = [cloudflare_workers_deployment.jsr_frontend]
}

resource "cloudflare_workers_route" "jsr_root" {
  zone_id = var.cloudflare_zone_id
  pattern = "${var.domain_name}/*"
  script  = cloudflare_worker.jsr_lb.name

  depends_on = [cloudflare_workers_deployment.jsr_lb]
}

resource "cloudflare_workers_route" "jsr_api" {
  zone_id = var.cloudflare_zone_id
  pattern = "${local.api_domain}/*"
  script  = cloudflare_worker.jsr_lb.name

  depends_on = [cloudflare_workers_deployment.jsr_lb]
}

resource "cloudflare_workers_route" "jsr_npm" {
  zone_id = var.cloudflare_zone_id
  pattern = "${local.npm_domain}/*"
  script  = cloudflare_worker.jsr_lb.name

  depends_on = [cloudflare_workers_deployment.jsr_lb]
}
