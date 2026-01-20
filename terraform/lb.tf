// Copyright 2024 the JSR authors. All rights reserved. MIT license.

data "google_cloud_run_service" "registry_api" {
  name     = "registry-api"
  location = "us-central1"
}

data "google_cloud_run_service" "registry_frontend_us_central1" {
  name     = "registry-frontend"
  location = "us-central1"
}

locals {
  worker_download_analytics_dataset = "${var.gcp_project}-downloads"
}

resource "cloudflare_workers_script" "jsr_lb" {
  account_id  = var.cloudflare_account_id
  script_name = "${var.gcp_project}-jsr-lb"
  content     = file("${path.module}/../lb/dist/main.js")

  bindings = [
    {
      type    = "analytics_engine"
      name    = "DOWNLOADS"
      dataset = local.worker_download_analytics_dataset
    }, {
      type = "plain_text"
      name = "ROOT_DOMAIN"
      text = var.domain_name
    }, {
      type = "plain_text"
      name = "API_DOMAIN"
      text = local.api_domain
    }, {
      type = "plain_text"
      name = "NPM_DOMAIN"
      text = local.npm_domain
    }, {
      type = "secret_text"
      secret_name = "REGISTRY_API_URL"
      text = data.google_cloud_run_service.registry_api.status[0].url
    }, {
      type = "secret_text"
      secret_name = "REGISTRY_FRONTEND_URL"
      text = data.google_cloud_run_service.registry_frontend_us_central1.status[0].url
    }, {
      type = "secret_text"
      secret_name = "MODULES_BUCKET"
      text = google_storage_bucket.modules.name
    }, {
      type = "secret_text"
      secret_name = "NPM_BUCKET"
      text = google_storage_bucket.npm.name
    }
  ]

  lifecycle {
    create_before_destroy = true
  }
}

resource "cloudflare_workers_route" "jsr_root" {
  zone_id = var.cloudflare_zone_id
  pattern = "${var.domain_name}/*"
  script  = cloudflare_workers_script.jsr_lb.script_name

  depends_on = [cloudflare_workers_script.jsr_lb]
}

resource "cloudflare_workers_route" "jsr_api" {
  zone_id = var.cloudflare_zone_id
  pattern = "${local.api_domain}/*"
  script  = cloudflare_workers_script.jsr_lb.script_name

  depends_on = [cloudflare_workers_script.jsr_lb]
}

resource "cloudflare_workers_route" "jsr_npm" {
  zone_id = var.cloudflare_zone_id
  pattern = "${local.npm_domain}/*"
  script  = cloudflare_workers_script.jsr_lb.script_name

  depends_on = [cloudflare_workers_script.jsr_lb]
}
