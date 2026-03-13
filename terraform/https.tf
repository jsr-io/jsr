// Copyright 2024 the JSR authors. All rights reserved. MIT license.
# HTTPS

resource "google_compute_ssl_policy" "frontend" {
  name            = "frontend"
  min_tls_version = "TLS_1_2"
  profile         = "MODERN"
}

resource "google_compute_managed_ssl_certificate" "frontend_cert" {
  name = "frontend-certs6"

  managed {
    domains = [var.domain_name, local.api_domain, local.npm_domain]
  }

  lifecycle {
    create_before_destroy = true
  }
}
