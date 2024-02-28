// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_project_service" "service_cloudresourcemanager" {
  project            = var.gcp_project
  service            = "cloudresourcemanager.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_artifactregistry" {
  project            = var.gcp_project
  service            = "artifactregistry.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_iam" {
  project            = var.gcp_project
  service            = "iam.googleapis.com"
  disable_on_destroy = false
}
