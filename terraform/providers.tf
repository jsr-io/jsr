// Copyright 2024 the JSR authors. All rights reserved. MIT license.
terraform {
  backend "gcs" {
    # gsutil mb -p deno-registry3-staging -l us-central1 -b on gs://deno-registry3-staging-terraform
    # Bucket must be specified via -backend-config="bucket=<project>-terraform" or backend config file
    prefix = "terraform/state"
  }
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = ">= 6.0.0, < 7.0.0"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = ">= 5.0.0, < 6.0.0"
    }
  }
}

provider "google" {
  project = var.gcp_project
}

provider "cloudflare" {
  api_token = var.cloudflare_api_token
}
