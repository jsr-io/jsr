// Copyright 2024 the JSR authors. All rights reserved. MIT license.
locals {
  tasks_envs = {
    "DATABASE_URL" = local.postgres_url
    "NO_COLOR"     = "true"

    "PUBLISHING_BUCKET" = cloudflare_r2_bucket.publishing.name
    "MODULES_BUCKET"    = cloudflare_r2_bucket.modules.name
    "DOCS_BUCKET"       = cloudflare_r2_bucket.docs.name
    "NPM_BUCKET"        = cloudflare_r2_bucket.npm.name

    "S3_REGION"     = "auto"
    "S3_ENDPOINT"   = "${var.cloudflare_account_id}.r2.cloudflarestorage.com"
    "S3_ACCESS_KEY" = cloudflare_account_token.buckets_rw.id
    "S3_SECRET_KEY" = local.r2_secret_access_key

    "METADATA_STRATEGY" = "instance_metadata"

    "GITHUB_CLIENT_ID" = var.github_client_id
    # GITHUB_CLIENT_SECRET is defined inline, because it comes from Secrets Manager

    "GITLAB_CLIENT_ID" = var.gitlab_client_id
    # GITLAB_CLIENT_SECRET is defined inline, because it comes from Secrets Manager

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

    "CLOUDFLARE_ACCOUNT_ID"        = var.cloudflare_account_id
    "CLOUDFLARE_ANALYTICS_DATASET" = local.worker_download_analytics_dataset
  }
}

### Background processing (stays on Cloud Run)

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
        "--tasks", "--api=false", "--database_pool_size=1"
      ]

      dynamic "env" {
        for_each = local.tasks_envs
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
        name = "GITLAB_CLIENT_SECRET"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.gitlab_client_secret.id
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
  display_name = "service account for registry_api (Cloudflare Containers & Cloud Run tasks)"
  project      = var.gcp_project
}

resource "google_service_account_key" "registry_api" {
  service_account_id = google_service_account.registry_api.name
}

resource "google_project_iam_member" "registry_api_cloudsql" {
  project = var.gcp_project
  role    = "roles/cloudsql.client"
  member  = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_secret_manager_secret_iam_member" "github_client_secret" {
  secret_id = google_secret_manager_secret.github_client_secret.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.registry_api.email}"
}

resource "google_secret_manager_secret_iam_member" "gitlab_client_secret" {
  secret_id = google_secret_manager_secret.gitlab_client_secret.id
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
