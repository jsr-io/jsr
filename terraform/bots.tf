// Copyright 2024 the JSR authors. All rights reserved. MIT license.

# Custom WAF rules for the production zone. Staging is left unmanaged here.
#
# The bot rules only match requests that render a frontend page: the LB sends a
# request to the frontend only when its Accept header starts with text/html
# (see lb/main.ts). Module downloads, the Deno CLI, npm clients, and API
# requests never send that, so they're never challenged. On page renders, the
# rules challenge unverified bots by Bot Management score.
locals {
  website_page_request = join(" and ", [
    "http.host eq \"${var.domain_name}\"",
    "starts_with(lower(http.request.headers[\"accept\"][0]), \"text/html\")",
    "not cf.bot_management.verified_bot",
    # The post-deploy e2e tests fetch pages with Deno's default user agent from
    # GitHub runners, which score as bots.
    "not starts_with(http.user_agent, \"Deno/\")",
    # Fediverse servers fetching link previews.
    "not starts_with(http.user_agent, \"Mastodon/\")",
    "not starts_with(http.user_agent, \"Misskey/\")",
    "not starts_with(http.user_agent, \"Pleroma \")",
    "not starts_with(http.user_agent, \"Akkoma \")",
  ])
}

resource "cloudflare_ruleset" "custom_firewall" {
  count = var.production ? 1 : 0

  zone_id = var.cloudflare_zone_id
  name    = "default"
  kind    = "zone"
  phase   = "http_request_firewall_custom"

  rules = [
    {
      description = "Singapore/China/Vietnam scrape"
      action      = "managed_challenge"
      enabled     = true
      expression  = "(ip.src.asnum eq 136907 and ip.src.country eq \"SG\") or (ip.src.asnum eq 212238 and ip.src.country eq \"SG\") or (ip.src.asnum eq 132203 and ip.src.country eq \"SG\") or (ip.src.asnum eq 55967) or (ip.src.asnum eq 45899)"
    },
    {
      description = "Bots rendering frontend pages"
      action      = "managed_challenge"
      enabled     = true
      # Lightpanda is a headless browser whose crawl overloaded the database on
      # 2026-11-28. Its requests score 13-34, so it's matched by user agent and
      # TLS fingerprint as well as by score.
      expression = "(${local.website_page_request} and (cf.bot_management.score lt 10 or http.user_agent contains \"Lightpanda\" or cf.bot_management.ja4 eq \"t13d1710h2_5b57614c22b0_95ca0cbbc74b\"))"
    },
    {
      description = "Likely bots rendering frontend pages"
      action      = "managed_challenge"
      enabled     = true
      expression  = "(${local.website_page_request} and cf.bot_management.score ge 10 and cf.bot_management.score lt 30)"
    },
  ]
}

# The production zone's entrypoint ruleset was created in the dashboard, and a
# zone can only have one per phase, so it's imported rather than created.
import {
  for_each = var.production ? { "0" = var.cloudflare_custom_firewall_ruleset_id } : {}
  to       = cloudflare_ruleset.custom_firewall[tonumber(each.key)]
  id       = "zones/${var.cloudflare_zone_id}/${each.value}"
}
