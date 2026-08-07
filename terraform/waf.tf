// Copyright 2024 the JSR authors. All rights reserved. MIT license.

# WAF rules that keep scrapers off the frontend without ever touching the raw
# module serving path.
#
# The constraint that shapes everything here: every WAF phase runs BEFORE
# Workers, so the WAF cannot see what `route()` in lb/main.ts decided. It has to
# re-derive "frontend vs. R2" from the request alone.
#
# Rather than mirror `isModuleFilePath()` exactly — two copies of that regex
# would drift, and a drift that made this copy NARROWER would rate-limit
# `deno add` — the skip rule below is deliberately WIDER than the Worker's.
# Every path the Worker serves from R2 also matches `waf_re_module_shaped`, but
# not the reverse. Drift therefore fails toward leaving frontend traffic
# unprotected (a cost problem) and never toward challenging an install (an
# outage).
#
# WARNING: `waf_re_module_shaped` MUST stay a strict superset of
# `isModuleFilePath()` in lb/main.ts. Narrowing it breaks `deno add` globally.
# lb/waf_test.ts enforces this invariant in CI against a generated corpus.

locals {
  # Superset of isModuleFilePath() in lb/main.ts. That regex requires more
  # structure after the version segment (`\d[^/]*/.*`, `\d[^/]*_meta\.json`);
  # this only asks that the third segment be `meta.json` or start with a digit,
  # so anything the Worker calls a module file also matches here.
  waf_re_module_shaped = "^/@[^/]+/[^/]+/(meta\\.json|[0-9])"

  # Mirrors isAPIRoute() in lb/main.ts — these proxy to the registry API, not
  # the frontend, and /login/ is an OAuth redirect that must never be
  # challenged. The login page has its own captcha (see turnstile.tf).
  waf_re_api_route = "^/(api/|login/|connect/|disconnect/|logout$|sitemap(-scopes|-packages)?\\.xml$)"

  # The expensive renders scrapers walk symbol-by-symbol. The package segment
  # may carry an `@version` suffix, hence the second [^/]+.
  #
  # Source pages (`/@scope/pkg/1.2.3/mod.ts`) are covered too, but not by this
  # regex — they are module-shaped and indistinguishable from a raw module fetch
  # by path alone. The skip rule below does that separation for us: it removes
  # every module fetch from the rate limiting phase by reading Accept, so any
  # module-shaped path still arriving here can only be a source-view render.
  # The rule reuses waf_re_module_shaped to pick them up.
  waf_re_docs_route = "^/@[^/]+/[^/]+/(doc|diff)(/|$)"

  # Static assets (frontend/_fresh/client — /assets/, /fonts/, /images/, plus
  # root files like /favicon.ico). Cloudflare counts these toward rate limits
  # even when served from cache, and one page load pulls a dozen, so counting
  # them would throttle ordinary browsing rather than scraping.
  waf_re_static_asset = "\\.(js|mjs|css|map|svg|png|jpe?g|webp|avif|gif|ico|woff2?|ttf|txt|xml)$"

  # Mirrors `!accept?.startsWith("text/html")` in canAccessModuleFile(). Both
  # sides read the raw header and compare case-sensitively. A request with no
  # Accept header yields an empty array, so any() is false — matching the
  # Worker, where a null Accept also permits R2.
  #
  # One known divergence: the Worker's headers.get() joins REPEATED Accept
  # headers into a single comma-separated string and tests only its start,
  # while [*] here tests each value. A client sending `text/html` as a second
  # Accept header is served R2 by the Worker but not exempted here — the safe
  # direction (it can be challenged, not mis-served), and no real client sends
  # repeated Accept headers.
  waf_expr_accept_html = "any(starts_with(http.request.headers[\"accept\"][*], \"text/html\"))"

  # Mirrors the Sec-Fetch-Dest branch of canAccessModuleFile(): the Worker
  # serves R2 only when the header is absent, `empty`, or an image/video
  # subresource load from jsr.io itself. Without this clause the Accept check
  # alone would exempt a request the Worker routes to the FRONTEND — e.g.
  # `Accept: */*` + `Sec-Fetch-Dest: document` on a module-shaped path is an
  # expensive source-view render, and a scraper could walk those unthrottled
  # with one header.
  #
  # Sec-Fetch-* headers are forbidden request headers — browsers set them and
  # scripts cannot, and CLIs (`deno add`, npm) never send them — so for module
  # fetches this clause is satisfied via absence and cannot throttle installs.
  waf_expr_sec_fetch_dest_ok = "(not any(http.request.headers.names[*] == \"sec-fetch-dest\") or any(http.request.headers[\"sec-fetch-dest\"][*] == \"empty\") or ((any(http.request.headers[\"sec-fetch-dest\"][*] == \"image\") or any(http.request.headers[\"sec-fetch-dest\"][*] == \"video\")) and any(http.request.headers[\"sec-fetch-site\"][*] == \"same-origin\")))"

  # Mirrors the user-agent patterns of BOT_PATTERNS in lb/bots.ts. The Worker
  # checks isBot() BEFORE the module-file branch, so a request carrying one of
  # these headers is a frontend render even on a module-shaped path with a
  # non-HTML Accept — without this clause it would inherit the exemption, and
  # `User-Agent: Slack` would be a one-header ticket to walk source views
  # unthrottled.
  #
  # Unlike waf_re_module_shaped, this must mirror lb/bots.ts EXACTLY rather
  # than be a superset: over-matching here un-exempts real module fetches, so
  # a CLI whose UA happened to match would have its installs rate-limited.
  # lb/waf_test.ts enforces parity in both directions against isBot().
  #
  # (?i:...) is the scoped case-insensitivity form, which both the Rust regex
  # crate (Cloudflare's engine) and the test's JS RegExp accept; bots.ts uses
  # the /i flag.
  waf_re_bot_user_agent = "^(?i:Slack|Iframely|Twitter|WhatsApp|Mozilla/5\\.0 \\(compatible; Discordbot)"

  # Mirrors the `from` header pattern in BOT_PATTERNS — Googlebot detection.
  waf_re_bot_from = "^(?i:googlebot\\(at\\)googlebot\\.com)"

  waf_expr_lb_bot = "(http.user_agent matches \"${local.waf_re_bot_user_agent}\" or any(http.request.headers[\"from\"][*] matches \"${local.waf_re_bot_from}\"))"

  # api.<domain> and npm.<domain> share the LB Worker but are pure R2/API
  # surfaces with no frontend to scrape, so rate limiting is apex-only. They
  # still need SBFM handling of their own — see skip_cli_hosts_sbfm below.
  waf_expr_host = "http.host eq \"${var.domain_name}\""
}

# Custom rules run before rate limiting, Super Bot Fight Mode, and Managed
# Rules, and `skip` is non-terminating when it only names phases — so this one
# rule carves raw module fetches out of all three and evaluation continues.
#
# This is also why the rate limiting rules below can stay blunt: on this plan
# their expressions cannot read request headers, so they could never separate a
# source-view page from the identical raw module path themselves. That
# header-aware decision happens here, once.
resource "cloudflare_ruleset" "waf_custom" {
  zone_id = var.cloudflare_zone_id
  name    = "jsr custom rules"
  kind    = "zone"
  phase   = "http_request_firewall_custom"

  rules = [
    {
      ref         = "skip_module_serving"
      description = "Exempt raw module fetches from rate limiting, SBFM, and Managed Rules"
      enabled     = true
      action      = "skip"

      # A module fetch is a GET or HEAD for a module-shaped path that is not
      # asking for HTML. `deno add` sends Accept: */* (or nothing); a browser
      # navigating to the same path to read the source view sends text/html and
      # so falls through to the rules below, which is correct — that request
      # renders on the frontend.
      #
      # The method, Accept, and Sec-Fetch-Dest checks together mirror
      # canAccessModuleFile(), and the bot clause mirrors the isBot() branch
      # that runs before it: the Worker only ever serves R2 for a non-bot GET
      # or HEAD with a non-HTML Accept and a CLI/fetch-shaped Sec-Fetch-Dest,
      # so any request failing one of these lands on the frontend and keeps
      # full WAF coverage rather than inheriting this exemption.
      expression = "${local.waf_expr_host} and http.request.method in {\"GET\" \"HEAD\"} and (http.request.uri.path matches \"${local.waf_re_module_shaped}\") and not ${local.waf_expr_accept_html} and ${local.waf_expr_sec_fetch_dest_ok} and not ${local.waf_expr_lb_bot}"

      action_parameters = {
        # http_request_sbfm: Super Bot Fight Mode's only knob on this plan is
        # "Definitely automated", and a CLI with a fixed JA3/JA4, no JS engine,
        # and no cookies scores 1 — exactly that bucket. Without this skip,
        # flipping that toggle in the dashboard would break every `deno add`
        # instantly. This makes the toggle survivable.
        #
        # http_ratelimit: installs must never be throttled.
        #
        # http_request_firewall_managed: Managed Rules inspect the URI for
        # injection patterns, but these paths are attacker-chosen package file
        # names served as static bytes from R2 under a strict CSP. A published
        # file named `union_select.ts` should not be able to trip OWASP and
        # break its own package's installs; the rules protect nothing here.
        phases = ["http_ratelimit", "http_request_sbfm", "http_request_firewall_managed"]
      }

      # Surface skips in Security Events, so it stays visible how much traffic
      # takes this path and whether the expression is matching as intended.
      logging = {
        enabled = true
      }
    },
    {
      ref         = "skip_cli_hosts_sbfm"
      description = "Exempt the CLI-only hosts (api, npm) from Super Bot Fight Mode"
      enabled     = true
      action      = "skip"

      # api.<domain> and npm.<domain> serve exclusively programmatic clients —
      # `deno publish`, the npm CLI — with a fixed JA3/JA4, no JS engine, and no
      # cookies, so SBFM scores them 1 ("definitely automated") just like the
      # module fetches skipped above. SBFM is zone-wide, so without this rule
      # the skip above makes the toggle survivable for `deno add` while
      # flipping it would still break every npm-compat install and publish.
      #
      # Managed Rules stay active here deliberately: api.<domain> fronts the
      # registry API, a real dynamic backend worth OWASP coverage, and npm
      # paths are built from constrained package names that cannot trip it.
      # No rate limiting rule matches these hosts, so that phase needs no skip.
      expression = "http.host in {\"${local.api_domain}\" \"${local.npm_domain}\"}"

      action_parameters = {
        phases = ["http_request_sbfm"]
      }

      logging = {
        enabled = true
      }
    }
  ]
}

# Rate limiting escalates to a Managed Challenge rather than a block: a
# false-positive scraper verdict against a real reader is then recoverable in
# one click instead of a dead 403. Challenges work here because these rules live
# on the apex zone the browser is actually visiting, so cf_clearance is scoped
# to a host the browser will send it back to.
#
# An under-limit request increments the counter of EVERY rule whose expression
# matches — a doc page counts toward both rules below, exactly as the old
# stacked Worker limits did. Ordering still matters: rules evaluate in order
# and a challenge is terminating, so when both counters are exceeded the
# stricter doc/diff verdict is the one that fires.
#
# The two rules are exactly the production plan's cap on this phase. The
# staging zone's plan caps it at ONE rule (the apply fails with "exceeded the
# maximum number of rules in the phase http_ratelimit: 2 out of 1"), so staging
# deploys only the first — the doc/diff/source rule, which encodes the
# interesting classification and so still validates the expressions and the
# managed_challenge/mitigation_timeout combination against the real API.
resource "cloudflare_ruleset" "waf_ratelimit" {
  zone_id = var.cloudflare_zone_id
  name    = "jsr rate limiting"
  kind    = "zone"
  phase   = "http_ratelimit"

  rules = concat([
    {
      ref         = "ratelimit_docs_diff_source"
      description = "Throttle doc, diff, and source page scraping"
      enabled     = true
      action      = "managed_challenge"

      # Source pages are counted here even though their file extensions look
      # like static assets to the general rule below — a package file named
      # `mod.js` is an expensive render, unlike /assets/main.js.
      #
      # cf.client.bot is verified by Cloudflare against ASN and reverse DNS,
      # unlike the spoofable User-Agent list in lb/bots.ts — `User-Agent: Slack`
      # is a free bypass there today, but cannot forge this.
      #
      # Exempting verified bots is a deliberate change from the Worker limits,
      # which counted them too: a managed challenge is unsolvable by a crawler,
      # so throttling Googlebot here would read as a site outage to the indexer.
      expression = "${local.waf_expr_host} and ((http.request.uri.path matches \"${local.waf_re_docs_route}\") or (http.request.uri.path matches \"${local.waf_re_module_shaped}\")) and not cf.client.bot"

      ratelimit = {
        # cf.colo.id is mandatory on every rate limiting rule, and makes the
        # counter per-datacenter rather than global.
        characteristics     = ["ip.src", "cf.colo.id"]
        period              = 60
        requests_per_period = 15

        # Challenge actions on this plan always throttle for the counting period
        # and reject any other duration, so this must be 0.
        mitigation_timeout = 0
      }
    }
    ], var.production ? [
    {
      ref         = "ratelimit_frontend"
      description = "Throttle general frontend scraping"
      enabled     = true

      action = "managed_challenge"

      # Module fetches never reach this rule — the skip rule above removes them
      # from the phase entirely — so this only has to fence off the frontend
      # routes that are not worth counting:
      #   - API routes, which proxy elsewhere and include the login redirect
      #   - /badges/, fetched server-side by GitHub's image proxy on behalf of
      #     every reader of every README; those few IPs would blow any per-IP
      #     limit and take every badge on GitHub down with them
      #   - static assets, per waf_re_static_asset above
      expression = "${local.waf_expr_host} and not (http.request.uri.path matches \"${local.waf_re_api_route}\") and not starts_with(http.request.uri.path, \"/badges/\") and not (http.request.uri.path matches \"${local.waf_re_static_asset}\") and not cf.client.bot"

      ratelimit = {
        characteristics     = ["ip.src", "cf.colo.id"]
        period              = 60
        requests_per_period = 300
        mitigation_timeout  = 0
      }
    }
  ] : [])
}
