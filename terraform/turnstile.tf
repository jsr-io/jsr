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
  name       = "${var.domain_name} login"
  domains    = [var.domain_name]

  # Show an interactive challenge only to visitors that look suspicious, rather
  # than making every sign-in solve one.
  mode = "managed"
}
