// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The frontend Cloudflare Worker has WASM dependencies (workers-og →
// satori + resvg-wasm) that need multi-module upload. The
// `cloudflare_workers_script` resource in terraform-provider-cloudflare
// (v5.19.1) only supports single-content uploads, so the *upload* of
// each new immutable version is done by CI via
// `wrangler versions upload` (see .github/workflows/ci.yml).
//
// What terraform owns here is the *promotion*: the worker's deployment
// resource pins 100% of traffic to a specific version id, passed in
// via `var.frontend_version_id`. Splitting upload from promotion means:
//
//   * the new version is already live on Cloudflare's edge before
//     terraform touches anything — if `terraform apply` fails at this
//     resource or any downstream resource, the old version keeps
//     serving traffic. No partial state where the worker is half-
//     deployed.
//   * rolling back is just `terraform apply` with the previous version
//     id; the immutable artifact is still on Cloudflare.

resource "cloudflare_workers_deployment" "jsr_frontend" {
  account_id  = var.cloudflare_account_id
  script_name = "${var.gcp_project}-jsr-frontend"
  strategy    = "percentage"
  versions = [{
    percentage = 100
    version_id = var.frontend_version_id
  }]
}
