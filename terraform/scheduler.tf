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

resource "google_cloud_scheduler_job" "orama_deploy" {
  name        = "orama-deploy"
  description = "Deploy the Orama index with any new changes"
  schedule    = "*/15 * * * *"
  region      = "us-central1"

  http_target {
    http_method = "POST"
    uri         = "https://api.oramasearch.com/api/v1/webhooks/${var.orama_package_index_id}/deploy"
    headers = {
      "Authorization" = "Bearer ${var.orama_package_private_api_key}"
    }
  }
}
