// Copyright 2024 the JSR authors. All rights reserved. MIT license.

locals {
  module_file_path       = "^/@[^/]+/[^/]+/(?:meta[.]json|[0-9][^/]*_meta[.]json|[0-9][^/]*/.*)$"
  scraping_detection_ids = "50331648 50331649"

  website_requests = <<-EOT
    http.host eq "${var.domain_name}"
    and not cf.bot_management.static_resource
    and not starts_with(http.request.uri.path, "/api/")
    and not starts_with(http.request.uri.path, "/login/")
    and not starts_with(http.request.uri.path, "/connect/")
    and not starts_with(http.request.uri.path, "/disconnect/")
    and not starts_with(http.request.uri.path, "/badges/")
    and not (http.request.uri.path matches "^/@[^/]+/[^/]+/og$")
    and not (http.request.uri.path in {"/sitemap.xml" "/sitemap-scopes.xml" "/sitemap-packages.xml" "/logout"})
    and not (
      http.request.method in {"GET" "HEAD"}
      and http.request.uri.path matches "${local.module_file_path}"
      and not (
        any(lower(http.request.headers.names[*])[*] eq "from")
        and any(starts_with(lower(http.request.headers["from"][*])[*], "googlebot(at)googlebot.com"))
      )
      and not starts_with(lower(http.user_agent), "slack")
      and not starts_with(lower(http.user_agent), "iframely")
      and not starts_with(lower(http.user_agent), "twitter")
      and not starts_with(lower(http.user_agent), "whatsapp")
      and not starts_with(lower(http.user_agent), "mozilla/5.0 (compatible; discordbot")
      and not (
        any(lower(http.request.headers.names[*])[*] eq "accept")
        and any(starts_with(http.request.headers["accept"][*], "text/html"))
      )
      and (
        not any(lower(http.request.headers.names[*])[*] eq "sec-fetch-dest")
        or any(len(http.request.headers["sec-fetch-dest"][*])[*] eq 0)
        or any(http.request.headers["sec-fetch-dest"][*] eq "empty")
        or (
          any(http.request.headers["sec-fetch-dest"][*] in {"image" "video"})
          and any(lower(http.request.headers.names[*])[*] eq "sec-fetch-site")
          and any(http.request.headers["sec-fetch-site"][*] eq "same-origin")
        )
      )
    )
  EOT

  e2e_exemption      = var.e2e_bypass_token == "" ? "" : " and not any(http.request.headers[\"x-jsr-e2e\"][*] eq \"${var.e2e_bypass_token}\")"
  website_unverified = "(${local.website_requests}) and not cf.bot_management.verified_bot${local.e2e_exemption}"
  website_bots       = "${local.website_unverified} and cf.bot_management.score lt 30"
}

resource "cloudflare_bot_management" "zone" {
  zone_id           = var.cloudflare_zone_id
  enable_js         = true
  auto_update_model = true
}

resource "cloudflare_ruleset" "custom_firewall" {
  zone_id = var.cloudflare_zone_id
  name    = "default"
  kind    = "zone"
  phase   = "http_request_firewall_custom"

  rules = [
    {
      ref         = "website_bots"
      description = "Likely bots on the website"
      expression  = local.website_bots
      action      = "log"
    },
    {
      ref         = "website_scrapers"
      description = "Scraping detections on the website"
      expression  = "${local.website_unverified} and any(cf.bot_management.detection_ids[*] in {${local.scraping_detection_ids}})"
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
      description = "Clients probing the website for pages that do not exist"
      expression  = local.website_unverified
      action      = "log"
      ratelimit = {
        characteristics     = ["cf.colo.id", "ip.src", "cf.bot_management.ja4"]
        period              = 60
        requests_per_period = 20
        mitigation_timeout  = 0
        counting_expression = "${local.website_unverified} and http.response.code eq 404"
      }
    },
  ]
}
