// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_logging_project_bucket_config" "default" {
  project   = var.gcp_project
  location  = "global"
  bucket_id = "_Default"

  enable_analytics = true

  retention_days = 30
}

resource "google_logging_linked_dataset" "default" {
  link_id = "logs"
  bucket  = google_logging_project_bucket_config.default.id
}

data "google_bigquery_dataset" "default" {
  dataset_id = google_logging_linked_dataset.default.link_id
}
