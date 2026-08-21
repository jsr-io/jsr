// Copyright 2024 the JSR authors. All rights reserved. MIT license.

# The captcha guarding the login page. Terraform mints the widget, so the two
# keys it yields always come as a matched pair: `sitekey` is public and is
# handed to the frontend worker (see cloudflare_frontend.tf), while `secret` is
# what the API uses to verify the response token, and so goes to Secrets Manager
# (see secrets.tf).
#
# `domains` is the frontend origin, where the widget is embedded — not the api
# subdomain it submits to. Turnstile checks the token against the hostname that
# rendered it.
resource "cloudflare_turnstile_widget" "login" {
  account_id = var.cloudflare_account_id

  # Staging and prod share one Cloudflare account, so the name — which is only
  # a label in the dashboard — namespaces by environment to tell the two
  # widgets apart. `domains` is what actually scopes the widget.
  name    = "${var.production ? "prod" : "staging"} login"
  domains = [var.domain_name]

  # Cloudflare decides, per visitor, between clearing them silently and asking
  # them to tick a box. The other two modes never prompt, which sounds nicer but
  # leaves a visitor the passive signals cannot clear — on Tor, a VPN, or a
  # hardened browser — with no way to prove themselves and so no way to sign in.
  # The occasional checkbox is the price of that escape hatch.
  mode = "managed"
}
