# Cloudflare Worker configuration for JSR load balancer
#
# This Terraform configuration deploys the Cloudflare Worker that replaces
# the GCP Global Load Balancer for routing traffic to JSR services.
#
# All environment variables and secrets are managed here instead of wrangler.toml

terraform {
  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
  }
}

# Cloudflare provider configuration
# API token can be provided via environment variable: CLOUDFLARE_API_TOKEN
provider "cloudflare" {
  api_token = var.cloudflare_api_token
}

# Variables
variable "cloudflare_api_token" {
  type        = string
  description = "Cloudflare API token with Workers Scripts:Edit and Zone:Read permissions"
  sensitive   = true
  default     = null # Can be set via CLOUDFLARE_API_TOKEN env var
}

variable "cloudflare_account_id" {
  type        = string
  description = "Cloudflare account ID"
}

variable "cloudflare_zone_id" {
  type        = string
  description = "Cloudflare zone ID for the domain"
}

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

# Local values for cleaner configuration
locals {
  worker_name = "jsr-lb-${var.environment}"

  # All environment variables for the worker
  worker_env_vars = {
    ROOT_DOMAIN          = var.domain_name
    API_DOMAIN           = local.api_domain
    NPM_DOMAIN           = local.npm_domain
    ENABLE_CACHE         = "true"
    ENABLE_BOT_DETECTION = "true"
  }

  # All secret environment variables (fetched from GCP)
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

# Worker script
resource "cloudflare_worker_script" "jsr_lb" {
  account_id = var.cloudflare_account_id
  name       = local.worker_name
  content    = file("${path.module}/../lb/dist/main.js")

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

  # Analytics Engine binding for detailed metrics
  analytics_engine_binding {
    name    = "ANALYTICS_DATASET"
    dataset = cloudflare_workers_analytics_engine.jsr_lb.name
  }

  # Ensure worker is redeployed when source changes
  lifecycle {
    create_before_destroy = true
  }
}

# Worker routes for all three domains
resource "cloudflare_worker_route" "jsr_root" {
  zone_id     = var.cloudflare_zone_id
  pattern     = "${var.domain_name}/*"
  script_name = cloudflare_worker_script.jsr_lb.name

  depends_on = [cloudflare_worker_script.jsr_lb]
}

resource "cloudflare_worker_route" "jsr_api" {
  zone_id     = var.cloudflare_zone_id
  pattern     = "${local.api_domain}/*"
  script_name = cloudflare_worker_script.jsr_lb.name

  depends_on = [cloudflare_worker_script.jsr_lb]
}

resource "cloudflare_worker_route" "jsr_npm" {
  zone_id     = var.cloudflare_zone_id
  pattern     = "${local.npm_domain}/*"
  script_name = cloudflare_worker_script.jsr_lb.name

  depends_on = [cloudflare_worker_script.jsr_lb]
}

# Analytics Engine dataset for observability
resource "cloudflare_workers_analytics_engine" "jsr_lb" {
  account_id = var.cloudflare_account_id
  name       = "jsr_lb_analytics_${var.environment}"
}

# Outputs
output "worker_script_name" {
  value       = cloudflare_worker_script.jsr_lb.name
  description = "Name of the deployed Cloudflare Worker"
}

output "worker_routes" {
  value = {
    root = cloudflare_worker_route.jsr_root.pattern
    api  = cloudflare_worker_route.jsr_api.pattern
    npm  = cloudflare_worker_route.jsr_npm.pattern
  }
  description = "Cloudflare Worker route patterns"
}
