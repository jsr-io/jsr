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

data "cloudflare_zone" "zone" {
  zone_id = var.cloudflare_zone_id
}

resource "google_dns_record_set" "ns" {
  name         = "${var.domain_name}."
  managed_zone = google_dns_managed_zone.default.name
  type         = "NS"
  ttl          = 300
  rrdatas      = [for record in data.cloudflare_zone.zone.name_servers : "${record}."]
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
