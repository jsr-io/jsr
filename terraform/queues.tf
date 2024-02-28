// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_cloud_tasks_queue" "publishing_tasks" {
  name     = "publishing-tasks"
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

  # TODO: Set up queue-level routing from terraform.
  # Blocked on: https://github.com/hashicorp/terraform-provider-google/issues/15022
  # For now, manually set up queue level routing with these settings:
  #  {
  #   "httpTarget": {
  #     "uriOverride": {
  #       "host": "${stripPrefix("https://", google_cloud_run_v2_service.registry_api_tasks.uri)}",
  #       "pathOverride": { "path": "/tasks/publish" }
  #     },
  #     "oidcToken": {
  #       "serviceAccountEmail": "${google_service_account.task_dispatcher.email}"
  #     }
  #   }
  # }
  # The command to do this:
  # curl -X PATCH -d @./data.json -i \
  #   -H "Authorization: Bearer $(gcloud auth print-access-token)" \
  #   -H "Content-Type: application/json" \
  #   "https://cloudtasks.googleapis.com/v2beta3/projects/$PROJECT_ID/locations/us-central1/queues/publishing-tasks?updateMask=httpTarget.uriOverride,httpTarget.oidcToken"
}

resource "google_cloud_tasks_queue" "npm_tarball_build_tasks" {
  name     = "npm-tarball-build-tasks2"
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

  # TODO: Set up queue-level routing from terraform.
  # Blocked on: https://github.com/hashicorp/terraform-provider-google/issues/15022
  # For now, manually set up queue level routing with these settings:
  #  {
  #   "httpTarget": {
  #     "uriOverride": {
  #       "host": "${stripPrefix("https://", google_cloud_run_v2_service.registry_api_tasks.uri)}",
  #       "pathOverride": { "path": "/tasks/npm_tarball_build" }
  #     },
  #     "oidcToken": {
  #       "serviceAccountEmail": "${google_service_account.task_dispatcher.email}"
  #     }
  #   }
  # }
  # The command to do this:
  # curl -X PATCH -d @./data.json -i \
  #   -H "Authorization: Bearer $(gcloud auth print-access-token)" \
  #   -H "Content-Type: application/json" \
  #   "https://cloudtasks.googleapis.com/v2beta3/projects/$PROJECT_ID/locations/us-central1/queues/npm-tarball-build-tasks2?updateMask=httpTarget.uriOverride,httpTarget.oidcToken"
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
