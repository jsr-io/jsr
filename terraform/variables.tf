// Copyright 2024 the JSR authors. All rights reserved. MIT license.
variable "gcp_project" {
  type = string
}

variable "domain_name" {
  type = string
}

variable "api_image_id" {
  description = "the Docker image ID for the API service"
  type        = string
}

variable "frontend_image_id" {
  description = "the Docker image ID for the (legacy Cloud Run) frontend; kept while the new Cloudflare Worker frontend is in trial. Tear down `cloud_run_frontend.tf` and this variable in a follow-up once traffic has been fully cut over."
  type        = string
}

variable "github_client_id" {
  type = string
}

variable "github_client_secret" {
  type      = string
  sensitive = true
}

variable "gitlab_client_id" {
  type = string
}

variable "gitlab_client_secret" {
  type      = string
  sensitive = true
}

variable "postmark_token" {
  type      = string
  sensitive = true
}

variable "email_from_name" {
  type = string
}

variable "orama_packages_project_key" {
  type      = string
  sensitive = true
}

variable "orama_packages_public_api_key" {
  type = string
}

variable "orama_packages_project_id" {
  type = string
}

variable "orama_packages_data_source" {
  type = string
}

variable "orama_symbols_project_key" {
  type      = string
  sensitive = true
}

variable "orama_symbols_public_api_key" {
  type = string
}

variable "orama_symbols_project_id" {
  type = string
}

variable "orama_symbols_data_source" {
  type = string
}

variable "orama_docs_public_api_key" {
  type = string
}

variable "orama_docs_project_id" {
  type = string
}

variable "production" {
  type    = bool
  default = false
}

variable "cloudflare_api_token" {
  type      = string
  sensitive = true
}

variable "cloudflare_account_id" {
  type = string
}

variable "cloudflare_zone_id" {
  type = string
}

# OTLP/HTTP trace export (wired into the API Cloud Run services). Leave empty to
# disable; set both to point traces at an OTLP/HTTP backend such as Grafana
# Cloud.
variable "otlp_endpoint" {
  description = "OTLP/HTTP traces endpoint, full URL including the path (e.g. Grafana Cloud's https://otlp-gateway-<zone>.grafana.net/otlp/v1/traces). Empty disables trace export."
  type        = string
  default     = ""
}

variable "otlp_headers" {
  description = "Headers sent with every OTLP request, OTEL `key=value,key2=value2` format, carrying the backend auth (e.g. `Authorization=Basic <base64>` for Grafana Cloud)."
  type        = string
  default     = ""
  sensitive   = true
}
