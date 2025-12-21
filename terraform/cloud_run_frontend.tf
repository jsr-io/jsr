// Copyright 2024 the JSR authors. All rights reserved. MIT license.
locals {
  frontend_envs = {
    "FRONTEND_ROOT"                = "https://${var.domain_name}"
    "API_ROOT"                     = "https://${local.api_domain}"
    "CLOUD_TRACE"                  = "true"

    "ORAMA_PACKAGE_PUBLIC_API_KEY" = var.orama_package_public_api_key
    "ORAMA_PACKAGE_PROJECT_ID"     = var.orama_package_project_id

    "ORAMA_SYMBOLS_PUBLIC_API_KEY" = var.orama_symbols_public_api_key
    "ORAMA_SYMBOLS_PROJECT_ID"     = var.orama_symbols_project_id

    "ORAMA_DOCS_PUBLIC_API_KEY"    = var.orama_docs_public_api_key
    "ORAMA_DOCS_PROJECT_ID"        = var.orama_docs_project_id
  }
  frontend_regions = toset([
    "us-central1",          # Iowa
    "europe-west1",         # Belgium
    "asia-northeast1",      # Japan
    "asia-south1",          # India
    "asia-southeast1",      # Singapore
    "southamerica-east1",   # Brazil
    "australia-southeast1", # Sydney
  ])
}

### Frontend service

resource "google_cloud_run_v2_service" "registry_frontend" {
  for_each = local.frontend_regions

  name     = "registry-frontend-${each.key}"
  location = each.key
  ingress  = "INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER"

  template {
    service_account = google_service_account.registry_frontend.email

    scaling {
      min_instance_count = var.production ? 1 : 0
      max_instance_count = 10
    }

    containers {
      image = var.frontend_image_id

      dynamic "env" {
        for_each = local.frontend_envs
        content {
          name  = env.key
          value = env.value
        }
      }
    }
  }
}

resource "google_compute_region_network_endpoint_group" "registry_frontend" {
  for_each = local.frontend_regions

  name                  = "registry-frontend-neg"
  network_endpoint_type = "SERVERLESS"
  region                = each.key

  cloud_run {
    service = google_cloud_run_v2_service.registry_frontend[each.key].name
  }
}

resource "google_compute_backend_service" "registry_frontend" {
  name                  = "registry-frontend-backend"
  load_balancing_scheme = "EXTERNAL_MANAGED"

  custom_response_headers = [
    "x-jsr-cache-id: {cdn_cache_id}",
    "x-jsr-cache-status: {cdn_cache_status}",
  ]

  enable_cdn = true
  cdn_policy {
    cache_mode = "USE_ORIGIN_HEADERS"
    cache_key_policy {
      include_query_string  = true
      include_named_cookies = ["token"] # segment cache by user
    }
    serve_while_stale = 0        # don't serve stale content
    default_ttl       = 0        # no caching unless specified by the backend
    max_ttl           = 31622400 # 1 year
    client_ttl        = 31622400 # 1 year
  }

  dynamic "backend" {
    for_each = local.frontend_regions
    content {
      group = google_compute_region_network_endpoint_group.registry_frontend[backend.key].id
    }
  }

  log_config {
    enable      = true
    sample_rate = 1
  }

  lifecycle {
    ignore_changes = [cdn_policy[0].client_ttl, cdn_policy[0].max_ttl]
  }
}

resource "google_cloud_run_service_iam_member" "frontend_public_policy" {
  for_each = local.frontend_regions

  location = google_cloud_run_v2_service.registry_frontend[each.key].location
  project  = google_cloud_run_v2_service.registry_frontend[each.key].project
  service  = google_cloud_run_v2_service.registry_frontend[each.key].name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

### IAM

resource "google_service_account" "registry_frontend" {
  account_id   = "registry-frontend"
  display_name = "service account for registry_frontend cloud run instance"
  project      = var.gcp_project
}

resource "google_project_iam_member" "frontend_cloud_trace" {
  project = var.gcp_project
  role    = "roles/cloudtrace.agent"
  member  = "serviceAccount:${google_service_account.registry_frontend.email}"
}
