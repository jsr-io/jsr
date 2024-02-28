// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_compute_global_forwarding_rule" "frontend_http_ipv4" {
  name        = "frontend-http-ipv4-1"
  target      = google_compute_target_http_proxy.frontend.id
  port_range  = "80"
  ip_address  = google_compute_global_address.frontend_ipv4.address
  ip_protocol = "TCP"
}

resource "google_compute_global_forwarding_rule" "frontend_http_ipv6" {
  name        = "frontend-http-ipv6-1"
  target      = google_compute_target_http_proxy.frontend.id
  port_range  = "80"
  ip_address  = google_compute_global_address.frontend_ipv6.address
  ip_protocol = "TCP"
}

resource "google_compute_target_http_proxy" "frontend" {
  name    = "frontend"
  url_map = google_compute_url_map.frontend_http.id
}

resource "google_compute_url_map" "frontend_http" {
  name = "frontend-http"

  default_url_redirect {
    https_redirect         = true
    redirect_response_code = "TEMPORARY_REDIRECT"
    strip_query            = false
  }
}