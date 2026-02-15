// Copyright 2024 the JSR authors. All rights reserved. MIT license.
locals {
  api_envs = {
    "DATABASE_URL" = local.postgres_url
    "NO_COLOR"     = "true"

    "PUBLISHING_BUCKET" = google_storage_bucket.publishing.name
    "MODULES_BUCKET"    = google_storage_bucket.modules.name
    "DOCS_BUCKET"       = google_storage_bucket.docs.name
    "NPM_BUCKET"        = google_storage_bucket.npm.name

    "S3_REGION"      = "auto"
    "S3_ENDPOINT"    = "${var.cloudflare_account_id}.r2.cloudflarestorage.com"
    "S3_ACCESS_KEY"  = cloudflare_api_token.r2_publishing.id
    "S3_SECRET_KEY"  = sha256(cloudflare_api_token.r2_publishing.value)

    "METADATA_STRATEGY" = "instance_metadata"

    "GITHUB_CLIENT_ID" = var.github_client_id
    # GITHUB_CLIENT_SECRET is defined inline, because it comes from Secrets Manager

    # POSTMARK_TOKEN is defined inline, because it comes from Secrets Manager

    # ORAMA_PACKAGES_PROJECT_KEY is defined inline, because it comes from Secrets Manager
    "ORAMA_PACKAGES_PROJECT_ID"  = var.orama_packages_project_id
    "ORAMA_PACKAGES_DATA_SOURCE" = var.orama_packages_data_source
    # ORAMA_SYMBOLS_PROJECT_KEY is defined inline, because it comes from Secrets Manager
    "ORAMA_SYMBOLS_PROJECT_ID"  = var.orama_symbols_project_id
    "ORAMA_SYMBOLS_DATA_SOURCE" = var.orama_symbols_data_source

    "REGISTRY_URL" = "https://${var.domain_name}"
    "NPM_URL"      = "https://${local.npm_domain}"

    "EMAIL_FROM"      = "help@${var.domain_name}"
    "EMAIL_FROM_NAME" = var.email_from_name

    "PUBLISH_QUEUE_ID"           = "projects/${var.gcp_project}/locations/us-central1/queues/${local.publishing_tasks_queue_name}"
    "NPM_TARBALL_BUILD_QUEUE_ID" = "projects/${var.gcp_project}/locations/us-central1/queues/${local.npm_tarball_build_tasks_queue_name}"

    "LOGS_BIGQUERY_TABLE_ID" = "${data.google_bigquery_dataset.default.dataset_id}._Default"
    "GCP_PROJECT_ID"         = var.gcp_project

    "CLOUDFLARE_ACCOUNT_ID"        = var.cloudflare_account_id
    "CLOUDFLARE_ANALYTICS_DATASET" = local.worker_download_analytics_dataset
  }
}

### API service

resource "google_cloud_run_v2_service" "registry_api" {
  name     = "registry-api"
  location = "us-central1"
  ingress  = "INGRESS_TRAFFIC_ALL"

  template {
    service_account = google_service_account.registry_api.email

    scaling {
      min_instance_count = var.production ? 1 : 0
      max_instance_count = 20
    }

    max_instance_request_concurrency = 250

    containers {
      image = var.api_image_id
      args = [
        "--cloud_trace", "--api", "--tasks=false", "--database_pool_size=4"
      ]

      resources {
        limits = {
          cpu    = "1"
          memory = "1Gi"
        }
      }

      dynamic "env" {
        for_each = local.api_envs
        content {
          name  = env.key
          value = env.value
        }
      }

      env {
        name = "GITHUB_CLIENT_SECRET"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.github_client_secret.id
            version = "latest"
          }
        }
      }

      env {
        name = "POSTMARK_TOKEN"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.postmark_token.id
            version = "latest"
          }
        }
      }

      env {
        name = "ORAMA_PACKAGES_PROJECT_KEY"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.orama_packages_project_key.id
            version = "latest"
          }
        }
      }

      env {
        name = "ORAMA_SYMBOLS_PROJECT_KEY"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.orama_symbols_project_key.id
            version = "latest"
          }
        }
      }

      env {
        name = "CLOUDFLARE_API_TOKEN"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.cloudflare_api_token.id
            version = "latest"
          }
        }
      }
    }

    vpc_access {
      connector = google_vpc_access_connector.default.id
      egress    = "PRIVATE_RANGES_ONLY"
    }
  }
}

resource "google_compute_region_network_endpoint_group" "registry_api" {
  name                  = "registry-api-neg"
  network_endpoint_type = "SERVERLESS"
  region                = "us-central1"

  cloud_run {
    service = google_cloud_run_v2_service.registry_api.name
  }
}

resource "google_compute_backend_service" "registry_api" {
  name                  = "registry-api-backend"
  load_balancing_scheme = "EXTERNAL_MANAGED"

  custom_response_headers = [
    "x-jsr-cache-id: {cdn_cache_id}",
    "x-jsr-cache-status: {cdn_cache_status}",
    "X-Robots-Tag: noindex",
    "access-control-allow-origin: *",
    "access-control-expose-headers: *",
    "Cross-Origin-Resource-Policy: cross-origin",
    "X-Content-Type-Options: nosniff",
  ]

  enable_cdn = true
  cdn_policy {
    cache_mode = "USE_ORIGIN_HEADERS"
    cache_key_policy {
      include_query_string  = true
      include_named_cookies = ["token"] # segment cache by user
    }
    bypass_cache_on_request_headers {
      header_name = "authorization"
    }
    serve_while_stale = 600 # 10 minutes
    default_ttl       = 0
    max_ttl           = 31622400 # 1 year
    client_ttl        = 31622400 # 1 year
  }

  backend {
    group = google_compute_region_network_endpoint_group.registry_api.id
  }

  lifecycle {
    ignore_changes = [cdn_policy[0].client_ttl, cdn_policy[0].max_ttl]
  }
}

resource "google_cloud_run_service_iam_member" "api_public_policy" {
  location = google_cloud_run_v2_service.registry_api.location
  project  = google_cloud_run_v2_service.registry_api.project
  service  = google_cloud_run_v2_service.registry_api.name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

### Background processing

resource "google_cloud_run_v2_service" "registry_api_tasks" {
  name     = "registry-api-tasks"
  location = "us-central1"
  ingress  = "INGRESS_TRAFFIC_INTERNAL_ONLY" # only accessed by Cloud Tasks

  template {
    service_account = google_service_account.registry_api.email

    scaling {
      min_instance_count = 0
      max_instance_count = 20
    }

    # Do not concurrently process background tasks for optimal performance per
    # task.
    max_instance_request_concurrency = 1

    containers {
      image = var.api_image_id
      args = [
        "--cloud_trace", "--tasks", "--api=false", "--database_pool_size=1"
      ]

      dynamic "env" {
        for_each = local.api_envs
        content {
          name  = env.key
          value = env.value
        }
      }

      env {
        name = "GITHUB_CLIENT_SECRET"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.github_client_secret.id
            version = "latest"
          }
        }
      }

      env {
        name = "ORAMA_PACKAGES_PROJECT_KEY"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.orama_packages_project_key.id
            version = "latest"
          }
        }
      }

      env {
        name = "ORAMA_SYMBOLS_PROJECT_KEY"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.orama_symbols_project_key.id
            version = "latest"
          }
        }
      }

      env {
        name = "CLOUDFLARE_API_TOKEN"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.cloudflare_api_token.id
            version = "latest"
          }
        }
      }
    }

    vpc_access {
      connector = google_vpc_access_connector.default.id
      egress    = "PRIVATE_RANGES_ONLY"
    }
  }
}

### IAM

resource "google_service_account" "registry_api" {
  account_id   = "registry-api"
  display_name = "service account for registry_api cloud run instance"
  project      = var.gcp_project
}

resource "google_project_iam_member" "registry_api_cloudsql" {
  project = var.gcp_project
  role    = "roles/cloudsql.client"
  member  = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_storage_bucket_iam_member" "publishing_bucket_access" {
  bucket = google_storage_bucket.publishing.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_storage_bucket_iam_member" "modules_bucket_access" {
  bucket = google_storage_bucket.modules.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_storage_bucket_iam_member" "docs_bucket_access" {
  bucket = google_storage_bucket.docs.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_storage_bucket_iam_member" "npm_bucket_access" {
  bucket = google_storage_bucket.npm.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_secret_manager_secret_iam_member" "github_client_secret" {
  secret_id = google_secret_manager_secret.github_client_secret.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_secret_manager_secret_iam_member" "postmark_token" {
  secret_id = google_secret_manager_secret.postmark_token.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_secret_manager_secret_iam_member" "orama_packages_project_key" {
  secret_id = google_secret_manager_secret.orama_packages_project_key.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_secret_manager_secret_iam_member" "orama_symbols_project_key" {
  secret_id = google_secret_manager_secret.orama_symbols_project_key.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_secret_manager_secret_iam_member" "cloudflare_api_token" {
  secret_id = google_secret_manager_secret.cloudflare_api_token.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_project_iam_member" "api_cloud_trace" {
  project = google_cloud_run_v2_service.registry_api.project
  role    = "roles/cloudtrace.agent"
  member  = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_cloud_tasks_queue_iam_member" "publishing_tasks" {
  name   = google_cloud_tasks_queue.publishing_tasks.id
  role   = "roles/cloudtasks.enqueuer"
  member = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_cloud_tasks_queue_iam_member" "npm_tarball_build_tasks" {
  name   = google_cloud_tasks_queue.npm_tarball_build_tasks.id
  role   = "roles/cloudtasks.enqueuer"
  member = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_service_account_iam_member" "act_as_task_dispatcher" {
  service_account_id = google_service_account.task_dispatcher.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_project_iam_member" "bigquery" {
  project = var.gcp_project
  role    = "roles/bigquery.jobUser"
  member  = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_bigquery_dataset_iam_member" "registry_api_logs" {
  dataset_id = data.google_bigquery_dataset.default.dataset_id
  role       = "roles/bigquery.dataViewer"
  member     = "serviceAccount:${google_service_account.registry_api.email}"
}
