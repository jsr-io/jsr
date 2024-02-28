// Copyright 2024 the JSR authors. All rights reserved. MIT license.
locals {
  api_domain = "api.${var.domain_name}"
  npm_domain = "npm.${var.domain_name}"
}

resource "google_dns_managed_zone" "default" {
  name        = "default"
  dns_name    = "${var.domain_name}."
  description = "Managed zone for ${var.domain_name}"
}

resource "google_dns_record_set" "a" {
  name         = "${var.domain_name}."
  managed_zone = google_dns_managed_zone.default.name
  type         = "A"
  ttl          = 300
  rrdatas      = [google_compute_global_address.frontend_ipv4.address]
}

resource "google_dns_record_set" "aaaa" {
  name         = "${var.domain_name}."
  managed_zone = google_dns_managed_zone.default.name
  type         = "AAAA"
  ttl          = 300
  rrdatas      = [google_compute_global_address.frontend_ipv6.address]
}

resource "google_dns_record_set" "api_a" {
  name         = "${local.api_domain}."
  managed_zone = google_dns_managed_zone.default.name
  type         = "A"
  ttl          = 300
  rrdatas      = [google_compute_global_address.frontend_ipv4.address]
}

resource "google_dns_record_set" "api_aaaa" {
  name         = "${local.api_domain}."
  managed_zone = google_dns_managed_zone.default.name
  type         = "AAAA"
  ttl          = 300
  rrdatas      = [google_compute_global_address.frontend_ipv6.address]
}

resource "google_dns_record_set" "npm_a" {
  name         = "${local.npm_domain}."
  managed_zone = google_dns_managed_zone.default.name
  type         = "A"
  ttl          = 300
  rrdatas      = [google_compute_global_address.frontend_ipv4.address]
}

resource "google_dns_record_set" "npm_aaaa" {
  name         = "${local.npm_domain}."
  managed_zone = google_dns_managed_zone.default.name
  type         = "AAAA"
  ttl          = 300
  rrdatas      = [google_compute_global_address.frontend_ipv6.address]
}
