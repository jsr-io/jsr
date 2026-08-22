// Copyright 2024 the JSR authors. All rights reserved. MIT license.
locals {
  publishing_tasks_queue_name        = var.gcp_project == "deno-registry3-prod" ? "publishing-tasks3" : "publishing-tasks"
  npm_tarball_build_tasks_queue_name = "npm-tarball-build-tasks2"
}

resource "google_cloud_tasks_queue" "publishing_tasks" {
  name     = local.publishing_tasks_queue_name
  location = "us-central1"

  retry_config {
    max_attempts = 30
    min_backoff  = "1s"
    max_backoff  = "60s"
  }

  rate_limits {
    max_concurrent_dispatches = 30 # this is bounded by Cloud Run invoke concurrency
  }

  stackdriver_logging_config {
    sampling_ratio = 1.0
  }

  lifecycle {
    # Names of queues can't be reused for 7 days after deletion, so be careful!
    prevent_destroy = true
  }

  http_target {
    uri_override {
      host = trimprefix(google_cloud_run_v2_service.registry_api_tasks.uri, "https://")
      path_override {
        path = "/tasks/publish"
      }
    }

    oidc_token {
      service_account_email = google_service_account.task_dispatcher.email
    }
  }
}

resource "google_cloud_tasks_queue" "npm_tarball_build_tasks" {
  name     = local.npm_tarball_build_tasks_queue_name
  location = "us-central1"

  retry_config {
    max_attempts = 30
    min_backoff  = "1s"
    max_backoff  = "60s"
  }

  rate_limits {
    max_concurrent_dispatches = 30 # this is bounded by Cloud Run invoke concurrency
  }

  stackdriver_logging_config {
    sampling_ratio = 1.0
  }

  lifecycle {
    # Names of queues can't be reused for 7 days after deletion, so be careful!
    prevent_destroy = true
  }

  http_target {
    uri_override {
      host = trimprefix(google_cloud_run_v2_service.registry_api_tasks.uri, "https://")
      path_override {
        path = "/tasks/npm_tarball_build"
      }
    }

    oidc_token {
      service_account_email = google_service_account.task_dispatcher.email
    }
  }
}

resource "google_service_account" "task_dispatcher" {
  account_id   = "task-dispatcher"
  display_name = "service account used when dispatching tasks to Cloud Run"
  project      = var.gcp_project
}

resource "google_cloud_run_service_iam_member" "task_dispatcher" {
  location = google_cloud_run_v2_service.registry_api_tasks.location
  project  = google_cloud_run_v2_service.registry_api_tasks.project
  service  = google_cloud_run_v2_service.registry_api_tasks.name
  role     = "roles/run.invoker"
  member   = "serviceAccount:${google_service_account.task_dispatcher.email}"
}
