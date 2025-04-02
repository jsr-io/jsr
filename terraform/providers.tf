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
  }
}

provider "google" {
  project = var.gcp_project
}
