// Copyright 2024 the JSR authors. All rights reserved. MIT license.

locals {
  website_requests = <<-EOT
    http.host eq "${var.domain_name}"
    and not cf.bot_management.static_resource
    and not starts_with(http.request.uri.path, "/api/")
    and not starts_with(http.request.uri.path, "/login/")
    and not starts_with(http.request.uri.path, "/connect/")
    and not starts_with(http.request.uri.path, "/disconnect/")
    and not (http.request.uri.path in {"/sitemap.xml" "/sitemap-scopes.xml" "/sitemap-packages.xml" "/logout"})
    and not (
      http.request.method in {"GET" "HEAD"}
      and http.request.uri.path matches "^/@[^/]+/[^/]+/(?:meta[.]json|[0-9][^/]*_meta[.]json|[0-9][^/]*/.*)$"
      and not (
        any(lower(http.request.headers.names[*])[*] eq "accept")
        and any(starts_with(http.request.headers["accept"][*], "text/html"))
      )
      and (
        not any(lower(http.request.headers.names[*])[*] eq "sec-fetch-dest")
        or any(http.request.headers["sec-fetch-dest"][*] eq "empty")
        or (
          any(http.request.headers["sec-fetch-dest"][*] in {"image" "video"})
          and any(lower(http.request.headers.names[*])[*] eq "sec-fetch-site")
          and any(http.request.headers["sec-fetch-site"][*] eq "same-origin")
        )
      )
    )
  EOT

  website_bots = "(${local.website_requests}) and not cf.bot_management.verified_bot and cf.bot_management.score lt 30"
}

resource "cloudflare_bot_management" "zone" {
  zone_id                     = var.cloudflare_zone_id
  enable_js                   = true
  auto_update_model           = true
  bot_preference_sync_enabled = true
}

resource "cloudflare_ruleset" "custom_firewall" {
  zone_id = var.cloudflare_zone_id
  name    = "default"
  kind    = "zone"
  phase   = "http_request_firewall_custom"

  rules = [
    {
      ref         = "website_bots"
      description = "Challenge likely bots on the website"
      expression  = local.website_bots
      action      = "managed_challenge"
    },
    {
      ref         = "website_scrapers"
      description = "Challenge scraping detections on the website"
      expression  = "(${local.website_requests}) and not cf.bot_management.verified_bot and any(cf.bot_management.detection_ids[*] in {50331648 50331649})"
      action      = "log"
    },
  ]
}

resource "cloudflare_ruleset" "ratelimit" {
  zone_id = var.cloudflare_zone_id
  name    = "default"
  kind    = "zone"
  phase   = "http_ratelimit"

  rules = [
    {
      ref         = "website_not_found"
      description = "Challenge clients probing the website for pages that do not exist"
      expression  = "(${local.website_requests}) and not cf.bot_management.verified_bot"
      action      = "log"
      ratelimit = {
        characteristics     = ["cf.colo.id", "ip.src", "cf.bot_management.ja4"]
        period              = 60
        requests_per_period = 20
        mitigation_timeout  = 0
        counting_expression = "(${local.website_requests}) and not cf.bot_management.verified_bot and http.response.code eq 404"
      }
    },
  ]
}
