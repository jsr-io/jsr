// Copyright 2024 the JSR authors. All rights reserved. MIT license.

# Custom WAF rules for the production zone. Staging is left unmanaged here.
#
# The bot rules only match jsr.io requests whose Accept header starts with
# text/html, which is how browsers and crawlers request pages. The LB never
# serves module files to such requests (see `canAccessModuleFile` in
# lb/main.ts), so module downloads, the Deno CLI, and npm clients aren't
# matched, and api.jsr.io and npm.jsr.io are other hosts. On those requests,
# the rules challenge unverified bots by Bot Management score.
locals {
  website_page_request = join(" and ", [
    "http.host eq \"${var.domain_name}\"",
    "starts_with(lower(http.request.headers[\"accept\"][0]), \"text/html\")",
    "not cf.bot_management.verified_bot",
    # The post-deploy e2e tests fetch pages with Deno's default user agent from
    # GitHub runners, which score as bots.
    "not starts_with(http.user_agent, \"Deno/\")",
    # Fediverse servers fetching link previews. Mastodon 4.2 and earlier send
    # "http.rb/<version> (Mastodon/<version>; ...)".
    "not http.user_agent contains \"Mastodon/\"",
    "not starts_with(http.user_agent, \"Misskey/\")",
    "not starts_with(http.user_agent, \"Pleroma \")",
    "not starts_with(http.user_agent, \"Akkoma \")",
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

  rules = [
    {
      # Rule refs keep the rule IDs stable across edits; Security Events filters
      # by rule ID. These are the IDs the rules had in the dashboard.
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
  ]

  lifecycle {
    prevent_destroy = true
  }
}

# The production zone's entrypoint ruleset was created in the dashboard, and a
# zone can only have one per phase, so it's imported rather than created. To
# stop managing it, use a `removed` block with `destroy = false`; deleting this
# file would delete every custom rule in the zone.
import {
  for_each = var.production ? { "0" = var.cloudflare_custom_firewall_ruleset_id } : {}
  to       = cloudflare_ruleset.custom_firewall[0]
  id       = "zones/${var.cloudflare_zone_id}/${each.value}"
}
