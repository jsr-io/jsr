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
    "access-control-allow-origin: *",
    "access-control-expose-headers: *",
  ]
  cdn_policy {
    cache_mode         = "USE_ORIGIN_HEADERS"
    default_ttl        = 0        # no caching unless specified by the backend
    max_ttl            = 31622400 # 1 year
    serve_while_stale  = 0        # no caching unless specified by the backend
    request_coalescing = true
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
  ]

  cdn_policy {
    cache_mode         = "USE_ORIGIN_HEADERS"
    default_ttl        = 0        # no caching unless specified by the backend
    max_ttl            = 31622400 # 1 year
    serve_while_stale  = 0        # no caching unless specified by the backend
    request_coalescing = true
  }
}
