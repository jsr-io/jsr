// Copyright 2024 the JSR authors. All rights reserved. MIT license.

locals {
  worker_download_analytics_dataset = "${var.gcp_project}-downloads"
}

resource "cloudflare_workers_script" "jsr_lb" {
  account_id  = var.cloudflare_account_id
  script_name = "${var.gcp_project}-jsr-lb"
  content     = file("${path.module}/../lb/dist/main.js")
  main_module = "worker.js"

  observability = {
    enabled            = true
    logs = {
      enabled            = true
      invocation_logs    = true
      head_sampling_rate = 0.01
      persist            = true
    }
  }

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
      type = "secret_text"
      name = "REGISTRY_FRONTEND_URL"
      text = google_cloud_run_v2_service.registry_frontend["us-central1"].uri
      }, {
        type       = "durable_object_namespace"
        name       = "API_CONTAINER"
        class_name = "ApiContainer"
      },

      // API container environment variables (passed through to containers via Worker env)
      {
        type = "secret_text"
        name = "DATABASE_URL"
        text = "postgres://${google_sql_user.api.name}:${google_sql_user.api.password}@${google_sql_database_instance.main_pg15.public_ip_address}/${google_sql_database.database.name}?sslmode=require"
      }, {
        type = "secret_text"
        name = "METADATA_STRATEGY"
        text = base64decode(google_service_account_key.registry_api.private_key)
      }, {
        type = "plain_text"
        name = "PUBLISHING_BUCKET"
        text = google_storage_bucket.publishing.name
      }, {
        type = "plain_text"
        name = "MODULES_BUCKET_NAME"
        text = google_storage_bucket.modules.name
      }, {
        type = "plain_text"
        name = "DOCS_BUCKET"
        text = google_storage_bucket.docs.name
      }, {
        type = "plain_text"
        name = "NPM_BUCKET_NAME"
        text = google_storage_bucket.npm.name
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

  lifecycle {
    create_before_destroy = true
  }
}

resource "cloudflare_workers_route" "jsr_root" {
  zone_id = var.cloudflare_zone_id
  pattern = "${var.domain_name}/*"
  script  = cloudflare_workers_script.jsr_lb.script_name

  depends_on = [cloudflare_workers_script.jsr_lb]
}

resource "cloudflare_workers_route" "jsr_api" {
  zone_id = var.cloudflare_zone_id
  pattern = "${local.api_domain}/*"
  script  = cloudflare_workers_script.jsr_lb.script_name

  depends_on = [cloudflare_workers_script.jsr_lb]
}

resource "cloudflare_workers_route" "jsr_npm" {
  zone_id = var.cloudflare_zone_id
  pattern = "${local.npm_domain}/*"
  script  = cloudflare_workers_script.jsr_lb.script_name

  depends_on = [cloudflare_workers_script.jsr_lb]
}
