// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_project_service" "service_cloudresourcemanager" {
  project            = var.gcp_project
  service            = "cloudresourcemanager.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_compute" {
  project            = var.gcp_project
  service            = "compute.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_dns" {
  project            = var.gcp_project
  service            = "dns.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_iam" {
  project            = var.gcp_project
  service            = "iam.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_run" {
  project            = var.gcp_project
  service            = "run.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_servicenetworking" {
  project            = var.gcp_project
  service            = "servicenetworking.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_storage" {
  project            = var.gcp_project
  service            = "storage.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_sqladmin" {
  project            = var.gcp_project
  service            = "sqladmin.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_vpcaccess" {
  project            = var.gcp_project
  service            = "vpcaccess.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_secretmanager" {
  project            = var.gcp_project
  service            = "secretmanager.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_cloudtrace" {
  project            = var.gcp_project
  service            = "cloudtrace.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_cloudtasks" {
  project            = var.gcp_project
  service            = "cloudtasks.googleapis.com"
  disable_on_destroy = false
}

resource "google_project_service" "service_cloudscheduler" {
  project            = var.gcp_project
  service            = "cloudscheduler.googleapis.com"
  disable_on_destroy = false
}
