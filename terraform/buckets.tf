// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "cloudflare_r2_bucket" "modules" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-modules"
  location   = "enam"
}

resource "cloudflare_r2_bucket" "publishing" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-publishing"
  location   = "enam"
}

resource "cloudflare_r2_bucket" "docs" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-docs"
  location   = "enam"
}

resource "cloudflare_r2_bucket" "npm" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-npm"
  location   = "enam"
}

resource "aws_s3_object" "r2_npm_root_json" {
  bucket        = cloudflare_r2_bucket.npm.name
  key           = "root.json"
  content       = "{}"
  content_type  = "application/json"
  cache_control = "public, max-age=0, no-cache"
}

resource "cloudflare_account_token" "buckets_rw" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-buckets-rw"

  policies = [{
    effect = "allow"
    permission_groups = [
      { id = "6a018a9f2fc74eb6b293b0c548f38b39" }, // Workers R2 Storage Bucket Item Read
      { id = "2efd5506f9c8494dacb1fa10a3e7d5b6" }, // Workers R2 Storage Bucket Item Write
    ]
    resources = jsonencode({
      "com.cloudflare.edge.r2.bucket.${var.cloudflare_account_id}_default_${cloudflare_r2_bucket.modules.name}"    = "*",
      "com.cloudflare.edge.r2.bucket.${var.cloudflare_account_id}_default_${cloudflare_r2_bucket.publishing.name}" = "*",
      "com.cloudflare.edge.r2.bucket.${var.cloudflare_account_id}_default_${cloudflare_r2_bucket.docs.name}"       = "*",
      "com.cloudflare.edge.r2.bucket.${var.cloudflare_account_id}_default_${cloudflare_r2_bucket.npm.name}"        = "*"
    })
  }]
}

locals {
  r2_secret_access_key = sha256(cloudflare_account_token.buckets_rw.value)
}
