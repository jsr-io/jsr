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

resource "cloudflare_dns_record" "a" {
  zone_id = var.cloudflare_zone_id
  name    = "@"
  type    = "A"
  content = google_compute_global_address.frontend_ipv4.address
  proxied = true
  ttl     = 1
}

resource "cloudflare_dns_record" "aaaa" {
  zone_id = var.cloudflare_zone_id
  name    = "@"
  type    = "AAAA"
  content = google_compute_global_address.frontend_ipv6.address
  proxied = true
  ttl     = 1
}

resource "cloudflare_dns_record" "api_a" {
  zone_id = var.cloudflare_zone_id
  name    = "api"
  type    = "A"
  content = google_compute_global_address.frontend_ipv4.address
  proxied = true
  ttl     = 1
}

resource "cloudflare_dns_record" "api_aaaa" {
  zone_id = var.cloudflare_zone_id
  name    = "api"
  type    = "AAAA"
  content = google_compute_global_address.frontend_ipv6.address
  proxied = true
  ttl     = 1
}

resource "cloudflare_dns_record" "npm_a" {
  zone_id = var.cloudflare_zone_id
  name    = "npm"
  type    = "A"
  content = google_compute_global_address.frontend_ipv4.address
  proxied = true
  ttl     = 1
}

resource "cloudflare_dns_record" "npm_aaaa" {
  zone_id = var.cloudflare_zone_id
  name    = "npm"
  type    = "AAAA"
  content = google_compute_global_address.frontend_ipv6.address
  proxied = true
  ttl     = 1
}

// old records
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
