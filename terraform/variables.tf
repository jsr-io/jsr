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
  description = "the Docker image ID for the frontend"
  type        = string
}

variable "github_client_id" {
  type = string
}

variable "github_client_secret" {
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

variable "orama_package_private_api_key" {
  type      = string
  sensitive = true
}

variable "orama_package_index_id" {
  type      = string
  sensitive = true
}

variable "orama_package_public_api_key" {
  type = string
}

variable "orama_package_public_index_id" {
  type = string
}

variable "production" {
  type    = bool
  default = false
}
