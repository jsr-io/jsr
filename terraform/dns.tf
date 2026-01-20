// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "cloudflare_dns_record" "a" {
  zone_id = var.cloudflare_zone_id
  name    = "@"
  type    = "A"
  content = google_compute_global_address.frontend_ipv4.address
  proxied = true
  ttl     = 300
}

resource "cloudflare_dns_record" "aaaa" {
  zone_id = var.cloudflare_zone_id
  name    = "@"
  type    = "AAAA"
  content = google_compute_global_address.frontend_ipv6.address
  proxied = true
  ttl     = 300
}

resource "cloudflare_dns_record" "api_a" {
  zone_id = var.cloudflare_zone_id
  name    = "api"
  type    = "A"
  content = google_compute_global_address.frontend_ipv4.address
  proxied = true
  ttl     = 300
}

resource "cloudflare_dns_record" "api_aaaa" {
  zone_id = var.cloudflare_zone_id
  name    = "api"
  type    = "AAAA"
  content = google_compute_global_address.frontend_ipv6.address
  proxied = true
  ttl     = 300
}

resource "cloudflare_dns_record" "npm_a" {
  zone_id = var.cloudflare_zone_id
  name    = "npm"
  type    = "A"
  content = google_compute_global_address.frontend_ipv4.address
  proxied = true
  ttl     = 300
}

resource "cloudflare_dns_record" "npm_aaaa" {
  zone_id = var.cloudflare_zone_id
  name    = "npm"
  type    = "AAAA"
  content = google_compute_global_address.frontend_ipv6.address
  proxied = true
  ttl     = 300
}
