// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_storage_bucket" "modules" {
  name          = "${var.gcp_project}-modules"
  location      = "US"
  force_destroy = true
  website {
    main_page_suffix = "#invalid" # this file can never be uploaded, because it contains a # - it is just used to make the bucket a website
    not_found_page   = "404.txt"
  }
}

resource "google_storage_bucket_object" "modules_404_txt" {
  bucket        = google_storage_bucket.modules.name
  cache_control = "public, max-age=0, no-cache"
  content       = "404 - Not Found"
  content_type  = "text/plain"
  name          = "404.txt"
}

resource "google_storage_bucket" "publishing" {
  name          = "${var.gcp_project}-publishing"
  location      = "US"
  force_destroy = true
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
      "com.cloudflare.edge.r2.bucket.${var.cloudflare_account_id}_default_${cloudflare_r2_bucket.publishing.name}" = "*",
      "com.cloudflare.edge.r2.bucket.${var.cloudflare_account_id}_default_${cloudflare_r2_bucket.docs.name}"       = "*",
      "com.cloudflare.edge.r2.bucket.${var.cloudflare_account_id}_default_${cloudflare_r2_bucket.npm.name}"        = "*"
    })
  }]
}

locals {
  r2_secret_access_key = sha256(cloudflare_account_token.buckets_rw.value)
}

resource "google_service_account" "r2_sippy" {
  account_id   = "r2-sippy"
  display_name = "R2 Sippy"
  description  = "Service account for Cloudflare R2 Sippy to read from GCS buckets"
}

resource "google_storage_bucket_iam_member" "r2_sippy_npm_reader" {
  bucket = google_storage_bucket.npm.name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.r2_sippy.email}"
}

resource "google_service_account_key" "r2_sippy" {
  service_account_id = google_service_account.r2_sippy.name
}

resource "cloudflare_r2_bucket_sippy" "r2_npm_sippy" {
  account_id  = var.cloudflare_account_id
  bucket_name = cloudflare_r2_bucket.npm.name
  destination = {
    access_key_id     = cloudflare_account_token.buckets_rw.id
    cloud_provider    = "r2"
    secret_access_key = local.r2_secret_access_key
  }
  source = {
    client_email   = google_service_account.r2_sippy.email
    private_key    = jsondecode(base64decode(google_service_account_key.r2_sippy.private_key)).private_key
    bucket         = google_storage_bucket.npm.name
    cloud_provider = "gcs"
  }

  depends_on = [google_storage_bucket_iam_member.r2_sippy_npm_reader]
}

resource "google_storage_bucket" "docs" {
  name          = "${var.gcp_project}-docs"
  location      = "US"
  force_destroy = true
}

resource "google_storage_bucket" "npm" {
  name          = "${var.gcp_project}-npm"
  location      = "US"
  force_destroy = true
  website {
    main_page_suffix = "#invalid" # this file can never be uploaded, because it contains a # - it is just used to make the bucket a website
    not_found_page   = "404.txt"
  }
}

resource "google_storage_bucket_object" "npm_root_json" {
  bucket        = google_storage_bucket.npm.name
  cache_control = "public, max-age=0, no-cache"
  content       = "{}"
  content_type  = "application/json"
  name          = "root.json"
}

resource "google_storage_bucket_object" "npm_404_txt" {
  bucket        = google_storage_bucket.npm.name
  cache_control = "public, max-age=0, no-cache"
  content       = "404 - Not Found"
  content_type  = "text/plain"
  name          = "404.txt"
}

resource "google_storage_bucket_iam_member" "modules_public_view" {
  bucket = google_storage_bucket.modules.name
  role   = "roles/storage.legacyObjectReader"
  member = "allUsers"
}

resource "google_compute_backend_bucket" "modules" {
  name             = "modules"
  description      = "CDN for raw module data"
  bucket_name      = google_storage_bucket.modules.name
  enable_cdn       = true
  compression_mode = "AUTOMATIC"
  custom_response_headers = [
    "Content-Security-Policy: default-src 'none'; script-src 'none'; style-src 'none'; img-src 'none'; font-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; frame-ancestors 'none'; sandbox; form-action 'none';",
    "x-jsr-cache-id: {cdn_cache_id}",
    "x-jsr-cache-status: {cdn_cache_status}",
    "X-Robots-Tag: noindex",
    "access-control-allow-origin: *",
    "access-control-expose-headers: *",
    "Cross-Origin-Resource-Policy: cross-origin",
    "X-Content-Type-Options: nosniff",
  ]

  cdn_policy {
    cache_mode         = "USE_ORIGIN_HEADERS"
    default_ttl        = 0        # no caching unless specified by the backend
    max_ttl            = 31622400 # 1 year
    serve_while_stale  = 0        # no caching unless specified by the backend
    request_coalescing = true
  }

  lifecycle {
    ignore_changes = [cdn_policy[0].client_ttl, cdn_policy[0].max_ttl]
  }
}

resource "google_storage_bucket_iam_member" "npm_public_view" {
  bucket = google_storage_bucket.npm.name
  role   = "roles/storage.legacyObjectReader"
  member = "allUsers"
}

resource "google_compute_backend_bucket" "npm" {
  name             = "npm"
  description      = "CDN for npm tarballs and metadata"
  bucket_name      = google_storage_bucket.npm.name
  enable_cdn       = true
  compression_mode = "AUTOMATIC"
  custom_response_headers = [
    "Content-Security-Policy: default-src 'none'; script-src 'none'; style-src 'none'; img-src 'none'; font-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; frame-ancestors 'none'; sandbox; form-action 'none';",
    "x-jsr-cache-id: {cdn_cache_id}",
    "x-jsr-cache-status: {cdn_cache_status}",
    "X-Robots-Tag: noindex",
    "access-control-allow-origin: *",
    "access-control-expose-headers: *",
    "Cross-Origin-Resource-Policy: cross-origin",
    "X-Content-Type-Options: nosniff",
  ]

  cdn_policy {
    cache_mode         = "USE_ORIGIN_HEADERS"
    default_ttl        = 0        # no caching unless specified by the backend
    max_ttl            = 31622400 # 1 year
    serve_while_stale  = 0        # no caching unless specified by the backend
    request_coalescing = true
  }

  lifecycle {
    ignore_changes = [cdn_policy[0].client_ttl, cdn_policy[0].max_ttl]
  }
}
