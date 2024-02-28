// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_compute_global_address" "frontend_ipv4" {
  name         = "registry-ipv4"
  address_type = "EXTERNAL"
  ip_version   = "IPV4"
  lifecycle {
    prevent_destroy = true
  }
}

resource "google_compute_global_address" "frontend_ipv6" {
  name         = "frontend-ipv6"
  address_type = "EXTERNAL"
  ip_version   = "IPV6"
  lifecycle {
    prevent_destroy = true
  }
}