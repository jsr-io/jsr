// Copyright 2024 the JSR authors. All rights reserved. MIT license.
locals {
  publishing_tasks_queue_name        = var.gcp_project == "deno-registry3-prod" ? "publishing-tasks3" : "publishing-tasks"
  npm_tarball_build_tasks_queue_name = "npm-tarball-build-tasks2"
  email_delivery_queue_name          = "email-delivery"
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

// Delivers outgoing email. Rows in `email_deliveries` are the durable record;
// this queue is what actually hands each one to Postmark, so a transient
// Postmark failure retries here instead of failing the request that queued it.
resource "google_cloud_tasks_queue" "email_delivery" {
  name     = local.email_delivery_queue_name
  location = "us-central1"

  retry_config {
    // The handler abandons a delivery after MAX_EMAIL_ATTEMPTS of its own, so
    // this bound only needs to cover transient failures. The long max_backoff
    // keeps a Postmark outage from burning attempts in the first minute.
    max_attempts = 10
    min_backoff  = "10s"
    max_backoff  = "600s"
  }

  rate_limits {
    // Postmark accepts far more than this; the cap is here so a backlog cannot
    // saturate the tasks service at the expense of publishing.
    max_concurrent_dispatches = 10
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
        path = "/tasks/send_email"
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
