// Copyright 2024 the JSR authors. All rights reserved. MIT license.
# HTTPS

resource "google_compute_global_forwarding_rule" "frontend_https_ipv4" {
  name                  = "frontend-https-ipv4-1"
  target                = google_compute_target_https_proxy.frontend.id
  port_range            = "443"
  ip_address            = google_compute_global_address.frontend_ipv4.address
  ip_protocol           = "TCP"
  load_balancing_scheme = "EXTERNAL_MANAGED"
}


resource "google_compute_global_forwarding_rule" "frontend_https_ipv6" {
  name                  = "frontend-https-ipv6-1"
  target                = google_compute_target_https_proxy.frontend.id
  port_range            = "443"
  ip_address            = google_compute_global_address.frontend_ipv6.address
  ip_protocol           = "TCP"
  load_balancing_scheme = "EXTERNAL_MANAGED"
}

resource "google_compute_target_https_proxy" "frontend" {
  name             = "frontend"
  url_map          = google_compute_url_map.frontend_https.id
  ssl_certificates = [google_compute_managed_ssl_certificate.frontend_cert.id]
  ssl_policy       = google_compute_ssl_policy.frontend.id
}

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


resource "google_compute_url_map" "frontend_https" {
  name = "frontend-https"

  host_rule {
    hosts        = [local.api_domain]
    path_matcher = "api"
  }

  path_matcher {
    name = "api"

    default_route_action {
      url_rewrite {
        path_prefix_rewrite = "/api"
      }
      cors_policy {
        allow_methods     = ["HEAD", "GET", "POST", "PUT", "PATCH", "DELETE"]
        allow_credentials = false
        expose_headers    = ["*"]
        allow_origins     = ["*"]
        allow_headers     = ["Authorization", "X-Cloud-Trace-Context"]
        max_age           = 3600
      }
    }
    default_service = google_compute_backend_service.registry_api.self_link
  }

  host_rule {
    hosts        = [local.npm_domain]
    path_matcher = "npm"
  }

  path_matcher {
    name = "npm"

    path_rule {
      paths = ["/", "/-/ping"]
      route_action {
        url_rewrite {
          path_prefix_rewrite = "/root.json"
        }
      }
      service = google_compute_backend_bucket.npm.self_link
    }

    default_service = google_compute_backend_bucket.npm.self_link
    default_route_action {
      cors_policy {
        allow_methods     = ["HEAD", "GET"]
        allow_credentials = false
        expose_headers    = ["*"]
        allow_origins     = ["*"]
        allow_headers     = ["Authorization", "X-Cloud-Trace-Context"]
        max_age           = 3600
      }
    }
  }

  host_rule {
    hosts        = [var.domain_name]
    path_matcher = "root"
  }

  # By default, requests to jsr.io are proxied to the frontend hosted on Cloud
  # Run.
  #
  # GET or HEAD requests to jsr.io/@* are routed to the modules bucket if they
  # do no have an 'Accept' header that starts with 'text/html' and either:
  #  - they do not have a 'Sec-Fetch-Dest' header or the value is 'empty'
  #  - they have a 'Sec-Fetch-Dest' header with value 'image' or 'video' and
  #    a 'Sec-Fetch-Site' with value 'same-origin'
  #
  # Additionally, any requests originating from the Googlebot user agent are
  # punched through to the frontend service, never to the modules bucket.
  # 
  # These restrictions are in place to prevent users from accessing hosted files
  # in navigation requests, while allowing access to them (even cross-site) when
  # using `fetch`. We disallow loading resources directly from `<img>` and
  # `<video>` tags (unless they are same-origin, to allow rendering them in
  # markdown previews), to prevent hotlinking.
  #
  # Since jsr.io URLs appear in stack traces, and every character counts, we've
  # introduced this complexity and a potential security risk to avoid the extra
  # two characters. This is instead of a simpler, more secure setup using a
  # subdomain like "p.jsr.io", which would map directly onto a bucket.
  #
  # As an additional security mitigation, we add the strictest possible CSP
  # header to all responses served from the modules bucket. This is done in the
  # backend bucket configuration.
  #
  # WARNING: Exercise extreme caution when modifying this. Untrusted files are
  # stored under the /@ prefix. It's crucial that the browser never loads these
  # untrusted files.
  path_matcher {
    name            = "root"
    default_service = google_compute_backend_service.registry_frontend.self_link

    # API requests are proxied to the API backend service.
    route_rules {
      priority = 1
      service  = google_compute_backend_service.registry_api.self_link
      match_rules {
        prefix_match = "/api/"
      }
      match_rules {
        full_path_match = "/sitemap.xml"
      }
      match_rules {
        full_path_match = "/sitemap-scopes.xml"
      }
      match_rules {
        full_path_match = "/sitemap-packages.xml"
      }
      match_rules {
        full_path_match = "/login"
      }
      match_rules {
        full_path_match = "/login/callback"
      }
      match_rules {
        full_path_match = "/logout"
      }
    }


    # Punch Googlebot and other crawlers through to the frontend.
    route_rules {
      priority = 2
      service  = google_compute_backend_service.registry_frontend.self_link

      match_rules {
        header_matches {
          header_name  = "from"
          prefix_match = "googlebot(at)googlebot.com"
        }
      }
      match_rules {
        header_matches {
          header_name  = "user-agent"
          prefix_match = "Slack" # Slackbot & Slack-ImgProxy
        }
      }
      match_rules {
        header_matches {
          header_name  = "user-agent"
          prefix_match = "Iframely" // Use by Notion
        }
      }
      match_rules {
        header_matches {
          header_name  = "user-agent"
          prefix_match = "Twitter"
        }
      }
      match_rules {
        header_matches {
          header_name  = "user-agent"
          prefix_match = "WhatsApp"
        }
      }
      match_rules {
        header_matches {
          header_name  = "user-agent"
          prefix_match = "Mozilla/5.0 (compatible; Discordbot"
        }
      }
    }

    route_rules {
      priority = 3
      service  = google_compute_backend_bucket.modules.self_link
      route_action {
        cors_policy {
          allow_methods     = ["HEAD", "GET"]
          allow_credentials = false
          expose_headers    = ["*"]
          allow_origins     = ["*"]
          allow_headers     = ["Authorization", "X-Cloud-Trace-Context"]
          max_age           = 3600
        }
      }

      # HEAD requests with no Accept header, and no Sec-Fetch-Dest header
      match_rules {
        prefix_match = "/@"
        header_matches {
          header_name = ":method"
          exact_match = "HEAD"
        }
        header_matches {
          header_name  = "Accept"
          invert_match = true
          prefix_match = "text/html"
        }
        header_matches {
          header_name   = "Sec-Fetch-Dest"
          invert_match  = true
          present_match = true
        }
      }
      # HEAD requests with no Accept header, and Sec-Fetch-Dest: empty
      match_rules {
        prefix_match = "/@"
        header_matches {
          header_name = ":method"
          exact_match = "HEAD"
        }
        header_matches {
          header_name  = "Accept"
          invert_match = true
          prefix_match = "text/html"
        }
        header_matches {
          header_name = "Sec-Fetch-Dest"
          exact_match = "empty"
        }
      }
      # HEAD requests with no Accept header, and Sec-Fetch-Dest: image, and
      # Sec-Fetch-Site: same-origin
      match_rules {
        prefix_match = "/@"
        header_matches {
          header_name = ":method"
          exact_match = "HEAD"
        }
        header_matches {
          header_name  = "Accept"
          invert_match = true
          prefix_match = "text/html"
        }
        header_matches {
          header_name = "Sec-Fetch-Dest"
          exact_match = "image"
        }
        header_matches {
          header_name = "Sec-Fetch-Site"
          exact_match = "same-origin"
        }
      }
      # HEAD requests with no Accept header, and Sec-Fetch-Dest: video, and
      # Sec-Fetch-Site: same-origin
      match_rules {
        prefix_match = "/@"
        header_matches {
          header_name = ":method"
          exact_match = "HEAD"
        }
        header_matches {
          header_name  = "Accept"
          invert_match = true
          prefix_match = "text/html"
        }
        header_matches {
          header_name = "Sec-Fetch-Dest"
          exact_match = "video"
        }
        header_matches {
          header_name = "Sec-Fetch-Site"
          exact_match = "same-origin"
        }
      }
      # GET requests with no Accept header, and no Sec-Fetch-Dest header
      match_rules {
        prefix_match = "/@"
        header_matches {
          header_name = ":method"
          exact_match = "GET"
        }
        header_matches {
          header_name  = "Accept"
          invert_match = true
          prefix_match = "text/html"
        }
        header_matches {
          header_name   = "Sec-Fetch-Dest"
          invert_match  = true
          present_match = true
        }
      }
      # GET requests with no Accept header, and Sec-Fetch-Dest: empty
      match_rules {
        prefix_match = "/@"
        header_matches {
          header_name = ":method"
          exact_match = "GET"
        }
        header_matches {
          header_name  = "Accept"
          invert_match = true
          prefix_match = "text/html"
        }
        header_matches {
          header_name = "Sec-Fetch-Dest"
          exact_match = "empty"
        }
      }
      # GET requests with no Accept header, and Sec-Fetch-Dest: image, and
      # Sec-Fetch-Site: same-origin
      match_rules {
        prefix_match = "/@"
        header_matches {
          header_name = ":method"
          exact_match = "GET"
        }
        header_matches {
          header_name  = "Accept"
          invert_match = true
          prefix_match = "text/html"
        }
        header_matches {
          header_name = "Sec-Fetch-Dest"
          exact_match = "image"
        }
        header_matches {
          header_name = "Sec-Fetch-Site"
          exact_match = "same-origin"
        }
      }
      # GET requests with no Accept header, and Sec-Fetch-Dest: video, and
      # Sec-Fetch-Site: same-origin
      match_rules {
        prefix_match = "/@"
        header_matches {
          header_name = ":method"
          exact_match = "GET"
        }
        header_matches {
          header_name  = "Accept"
          invert_match = true
          prefix_match = "text/html"
        }
        header_matches {
          header_name = "Sec-Fetch-Dest"
          exact_match = "video"
        }
        header_matches {
          header_name = "Sec-Fetch-Site"
          exact_match = "same-origin"
        }
      }
    }
  }

  default_url_redirect {
    host_redirect = var.domain_name
    strip_query   = false
  }
}
