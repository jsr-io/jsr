// Copyright 2024 the JSR authors. All rights reserved. MIT license.
terraform {
  backend "gcs" {
    # gsutil mb -p deno-registry3-staging -l us-central1 -b on gs://deno-registry3-staging-terraform
    bucket = "${var.gcp_project}-terraform"
    prefix = "terraform/state"
  }
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = ">= 6.0.0, < 7.0.0"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = ">= 5.17.0, < 6.0.0"
    }
    aws = {
      source  = "hashicorp/aws"
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

provider "aws" {
  region     = "auto"
  access_key = cloudflare_account_token.buckets_rw.id
  secret_key = local.r2_secret_access_key

  skip_credentials_validation = true
  skip_region_validation      = true
  skip_requesting_account_id  = true

  endpoints {
    s3 = "https://${var.cloudflare_account_id}.r2.cloudflarestorage.com"
  }
}
