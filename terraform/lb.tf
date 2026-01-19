# Data source to get Cloud Run URLs
data "google_cloud_run_service" "registry_api" {
  name     = "registry-api"
  location = "us-central1"
}

data "google_cloud_run_service" "registry_frontend_us_central1" {
  name     = "registry-frontend"
  location = "us-central1"
}

data "google_cloud_run_service" "registry_frontend_europe_west1" {
  name     = "registry-frontend"
  location = "europe-west1"
}

data "google_cloud_run_service" "registry_frontend_asia_northeast1" {
  name     = "registry-frontend"
  location = "asia-northeast1"
}

data "google_cloud_run_service" "registry_frontend_asia_south1" {
  name     = "registry-frontend"
  location = "asia-south1"
}

data "google_cloud_run_service" "registry_frontend_asia_southeast1" {
  name     = "registry-frontend"
  location = "asia-southeast1"
}

data "google_cloud_run_service" "registry_frontend_southamerica_east1" {
  name     = "registry-frontend"
  location = "southamerica-east1"
}

data "google_cloud_run_service" "registry_frontend_australia_southeast1" {
  name     = "registry-frontend"
  location = "australia-southeast1"
}

locals {
  worker_env_vars = {
    ROOT_DOMAIN = var.domain_name
    API_DOMAIN  = local.api_domain
    NPM_DOMAIN  = local.npm_domain
  }

  worker_secrets = {
    REGISTRY_API_URL = data.google_cloud_run_service.registry_api.status[0].url
    REGISTRY_FRONTEND_URLS = jsonencode({
      "us-central1"          = data.google_cloud_run_service.registry_frontend_us_central1.status[0].url
      "europe-west1"         = data.google_cloud_run_service.registry_frontend_europe_west1.status[0].url
      "asia-northeast1"      = data.google_cloud_run_service.registry_frontend_asia_northeast1.status[0].url
      "asia-south1"          = data.google_cloud_run_service.registry_frontend_asia_south1.status[0].url
      "asia-southeast1"      = data.google_cloud_run_service.registry_frontend_asia_southeast1.status[0].url
      "southamerica-east1"   = data.google_cloud_run_service.registry_frontend_southamerica_east1.status[0].url
      "australia-southeast1" = data.google_cloud_run_service.registry_frontend_australia_southeast1.status[0].url
    })
    MODULES_BUCKET = google_storage_bucket.modules.name
    NPM_BUCKET     = google_storage_bucket.npm.name
  }
}

resource "cloudflare_workers_script" "jsr_lb" {
  account_id  = var.cloudflare_account_id
  script_name = "${var.gcp_project}-jsr-lb"
  content     = file("${path.module}/../lb/dist/main.js")

  # Plain text environment variables
  dynamic "plain_text_binding" {
    for_each = local.worker_env_vars
    content {
      name = plain_text_binding.key
      text = plain_text_binding.value
    }
  }

  # Secret text bindings for backend URLs and bucket names
  dynamic "secret_text_binding" {
    for_each = local.worker_secrets
    content {
      name = secret_text_binding.key
      text = secret_text_binding.value
    }
  }

  bindings = [
    {
      type    = "analytics_engine"
      name    = "DOWNLOADS"
      dataset = var.cloudflare_analytics_dataset
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
