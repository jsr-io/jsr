// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_cloud_scheduler_job" "npm_tarball_rebuild_missing" {
  name        = "npm-tarball-rebuild-missing"
  description = "Find missing npm tarballs and enqueue them for rebuild."
  schedule    = "*/15 * * * *"
  region      = "us-central1"

  http_target {
    http_method = "POST"
    uri         = "${google_cloud_run_v2_service.registry_api_tasks.uri}/tasks/npm_tarball_enqueue"
    oidc_token {
      service_account_email = google_service_account.task_dispatcher.email
    }
  }
}

resource "google_cloud_scheduler_job" "clean_oauth_states" {
  name        = "clean-oauth-states"
  description = "Delete expired OAuth states older than 1 hour."
  schedule    = "0 0 * * *"
  region      = "us-central1"

  http_target {
    http_method = "POST"
    uri         = "${google_cloud_run_v2_service.registry_api_tasks.uri}/tasks/clean_oauth_states"
    oidc_token {
      service_account_email = google_service_account.task_dispatcher.email
    }
  }
}

resource "google_cloud_scheduler_job" "clean_download_counts_4h" {
  name        = "clean-download-counts-4h"
  description = "Delete version_download_counts_4h rows older than 7 days."
  schedule    = "0 3 * * *"
  region      = "us-central1"

  http_target {
    http_method = "POST"
    uri         = "${google_cloud_run_v2_service.registry_api_tasks.uri}/tasks/clean_download_counts_4h"
    oidc_token {
      service_account_email = google_service_account.task_dispatcher.email
    }
  }
}

resource "google_cloud_scheduler_job" "scrape_download_counts" {
  name        = "scrape-download-counts"
  description = "Scrape download counts from Analytics Engine and insert them into Postgres."
  schedule    = "15 */3 * * *"
  region      = "us-central1"

  http_target {
    http_method = "POST"
    uri         = "${google_cloud_run_v2_service.registry_api_tasks.uri}/tasks/scrape_download_counts?intervalHrs=12"
    oidc_token {
      service_account_email = google_service_account.task_dispatcher.email
    }
  }
}
