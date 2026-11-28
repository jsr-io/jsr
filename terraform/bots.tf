// Copyright 2024 the JSR authors. All rights reserved. MIT license.

# WAF and rate-limiting rules for the production zone. Staging is left
# unmanaged here.
#
# The challenge rules only match jsr.io requests whose Accept header starts
# with text/html, which is how browsers and crawlers request pages. The LB
# never serves module files to such requests (see `canAccessModuleFile` in
# lb/main.ts), so module downloads, the Deno CLI, and npm clients aren't
# matched, and api.jsr.io and npm.jsr.io are other hosts. On those requests,
# the rules challenge unverified bots by Bot Management score.
#
# The log rules cover the rest of the website, as routed by the LB, so their
# matches can be measured before anything there is challenged.
locals {
  # The paths the LB serves module files from (`isModuleFilePath` in
  # lb/main.ts). lb/main_test.ts checks that the two agree.
  module_file_path       = "^/@[^/]+/[^/]+/(?:meta[.]json|[0-9][^/]*_meta[.]json|[0-9][^/]*/.*)$"
  scraping_detection_ids = "50331648 50331649"
  # Detection for networks that scrape many zones; see website_scraping_networks.
  scraping_network_detection_id = "50331656"

  # The post-deploy e2e tests fetch pages from GitHub runners, which score as
  # bots, so they send this header to skip the bot rules.
  e2e_exemption = var.e2e_bypass_token == "" ? [] : [
    "not any(http.request.headers[\"x-jsr-e2e\"][*] eq \"${var.e2e_bypass_token}\")",
  ]

  # Fediverse servers fetching link previews. Mastodon 4.2 and earlier send
  # "http.rb/<version> (Mastodon/<version>; ...)".
  fediverse_exemptions = [
    "not http.user_agent contains \"Mastodon/\"",
    "not starts_with(http.user_agent, \"Misskey/\")",
    "not starts_with(http.user_agent, \"Pleroma \")",
    "not starts_with(http.user_agent, \"Akkoma \")",
  ]

  website_page_request = join(" and ", concat([
    "http.host eq \"${var.domain_name}\"",
    "starts_with(lower(http.request.headers[\"accept\"][0]), \"text/html\")",
    "not cf.bot_management.verified_bot",
  ], local.e2e_exemption, local.fediverse_exemptions))

  # Requests the LB routes to the frontend (see `route` in lb/main.ts),
  # excluding the routes crawlers and link unfurlers are expected to fetch:
  # badges, OG images, and sitemaps.
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

  website_unverified = join(" and ", concat([
    "(${local.website_requests})",
    "not cf.bot_management.verified_bot",
  ], local.e2e_exemption, local.fediverse_exemptions))

  # Rate-limiting rules can't read request headers on this zone's plan, so this
  # approximates `website_requests` by path: it leaves out module files for
  # every client, not only for non-browsers.
  website_path_request = join(" and ", [
    "http.host eq \"${var.domain_name}\"",
    "not cf.bot_management.verified_bot",
    "not cf.bot_management.static_resource",
    "not starts_with(http.request.uri.path, \"/api/\")",
    "not starts_with(http.request.uri.path, \"/login/\")",
    "not starts_with(http.request.uri.path, \"/connect/\")",
    "not starts_with(http.request.uri.path, \"/disconnect/\")",
    "not starts_with(http.request.uri.path, \"/badges/\")",
    "not (http.request.uri.path matches \"^/@[^/]+/[^/]+/og$\")",
    "not (http.request.uri.path in {\"/sitemap.xml\" \"/sitemap-scopes.xml\" \"/sitemap-packages.xml\" \"/logout\"})",
    "not (http.request.uri.path matches \"${local.module_file_path}\")",
  ])
}

resource "cloudflare_ruleset" "custom_firewall" {
  count = var.production ? 1 : 0

  zone_id = var.cloudflare_zone_id
  # name, kind, phase, and zone_id force replacement, which would delete every
  # rule in the zone until the new ruleset is created.
  name  = "default"
  kind  = "zone"
  phase = "http_request_firewall_custom"

  # Log rules come first: log doesn't stop rule evaluation, so they also see
  # the requests the challenge rules below match.
  rules = [
    {
      # Rule refs keep the rule IDs stable across edits; Security Events filters
      # by rule ID.
      ref         = "website_scrapers"
      description = "Scraping detections on website requests"
      action      = "log"
      enabled     = true
      expression  = "(${local.website_unverified} and any(cf.bot_management.detection_ids[*] in {${local.scraping_detection_ids}}))"
    },
    {
      ref         = "website_other_bots"
      description = "Likely bots on website requests without an HTML Accept header"
      action      = "log"
      enabled     = true
      expression  = "(${local.website_unverified} and not any(starts_with(lower(http.request.headers[\"accept\"][*])[*], \"text/html\")) and cf.bot_management.score lt 30)"
    },
    {
      # The IDs the challenge rules had in the dashboard.
      ref         = "75b22afd9b55498d83342e016d550ec4"
      description = "Bots rendering frontend pages"
      action      = "managed_challenge"
      enabled     = true
      # Lightpanda is a headless browser whose crawl overloaded the database on
      # 2026-11-28. Its requests score 13-34, so it's matched by user agent and
      # TLS fingerprint as well as by score.
      expression = "(${local.website_page_request} and (cf.bot_management.score lt 10 or http.user_agent contains \"Lightpanda\" or cf.bot_management.ja4 eq \"t13d1710h2_5b57614c22b0_95ca0cbbc74b\"))"
    },
    {
      ref         = "794a507f1f1243c9ac301f4c06b90f39"
      description = "Likely bots rendering frontend pages"
      action      = "managed_challenge"
      enabled     = true
      expression  = "(${local.website_page_request} and cf.bot_management.score ge 10 and cf.bot_management.score lt 30)"
    },
    {
      ref         = "website_scraping_networks"
      description = "Scraping networks rendering frontend pages"
      action      = "managed_challenge"
      enabled     = true
      # Bot Management detection 50331656 flags networks that scrape many
      # zones. On 2026-11-28 it caught a crawl of doc and diff pages from 1,000+
      # proxy and VPN IPs that scored 30 or more, so the rules above missed it.
      expression = "(${local.website_page_request} and any(cf.bot_management.detection_ids[*] eq ${local.scraping_network_detection_id}))"
    },
  ]

  lifecycle {
    prevent_destroy = true
  }
}

resource "cloudflare_ruleset" "ratelimit" {
  count = var.production ? 1 : 0

  zone_id = var.cloudflare_zone_id
  name    = "default"
  kind    = "zone"
  phase   = "http_ratelimit"

  rules = [
    {
      ref         = "website_not_found"
      description = "Clients requesting many website pages that don't exist"
      action      = "log"
      enabled     = true
      expression  = local.website_path_request
      ratelimit = {
        # This zone's plan doesn't allow cf.bot_management.ja4 or ip.src.asnum
        # as characteristics.
        characteristics     = ["cf.colo.id", "ip.src"]
        period              = 60
        requests_per_period = 20
        # Must be 0 or at least the period, and 0 (throttling) isn't available
        # on this zone's plan.
        mitigation_timeout  = 60
        counting_expression = "(${local.website_path_request} and http.response.code eq 404)"
      }
    },
  ]

  lifecycle {
    prevent_destroy = true
  }
}

# The production zone's entrypoint rulesets were created by hand, and a zone
# can only have one per phase, so they're imported rather than created. To stop
# managing one, use a `removed` block with `destroy = false`; deleting its
# resource would delete every rule in that phase.
import {
  for_each = var.production ? { "0" = var.cloudflare_custom_firewall_ruleset_id } : {}
  to       = cloudflare_ruleset.custom_firewall[0]
  id       = "zones/${var.cloudflare_zone_id}/${each.value}"
}

import {
  for_each = var.production ? { "0" = var.cloudflare_ratelimit_ruleset_id } : {}
  to       = cloudflare_ruleset.ratelimit[0]
  id       = "zones/${var.cloudflare_zone_id}/${each.value}"
}
