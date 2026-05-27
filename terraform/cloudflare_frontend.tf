// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The frontend Cloudflare Worker has WASM dependencies (workers-og →
// satori + resvg-wasm) that need multi-module upload. The
// `cloudflare_workers_script` resource in terraform-provider-cloudflare
// (v5.19.1, the pinned version) only supports single-content uploads,
// so we shell out to `wrangler deploy` from a `null_resource`.
//
// Folding wrangler into the terraform graph (rather than running it as
// a separate CI step) means:
//   * one `terraform apply` runs both, so partial-state windows are
//     bounded by terraform's normal resource ordering;
//   * the LB worker's `service` binding to the frontend can declare a
//     `depends_on` on this resource — wrangler runs before any
//     resource that references the frontend by name;
//   * the trigger hash on the built bundle skips the deploy when
//     nothing changed.

// Hashes of the inputs wrangler bundles. If any of these change, the
// `null_resource` re-runs `wrangler deploy`. Wrangler is idempotent for
// matching content, so an over-trigger is a no-op; an under-trigger
// would silently ship stale code, so prefer the broader hash.
data "archive_file" "frontend_bundle_hash" {
  type        = "zip"
  output_path = "${path.module}/.terraform-cache/frontend-bundle.zip"
  source_dir  = "${path.module}/../frontend/_fresh"
}

resource "null_resource" "jsr_frontend_deploy" {
  triggers = {
    bundle_sha   = data.archive_file.frontend_bundle_hash.output_sha256
    wrangler_sha = filesha256("${path.module}/../frontend/wrangler.jsonc")
    server_ts    = filesha256("${path.module}/../frontend/server.ts")
  }

  provisioner "local-exec" {
    working_dir = "${path.module}/../frontend"
    command     = <<-EOT
      deno run -A npm:wrangler@^4 deploy \
        --name ${var.gcp_project}-jsr-frontend \
        --var ORAMA_PACKAGES_PUBLIC_API_KEY:${var.orama_packages_public_api_key} \
        --var ORAMA_PACKAGES_PROJECT_ID:${var.orama_packages_project_id} \
        --var ORAMA_SYMBOLS_PUBLIC_API_KEY:${var.orama_symbols_public_api_key} \
        --var ORAMA_SYMBOLS_PROJECT_ID:${var.orama_symbols_project_id} \
        --var ORAMA_DOCS_PUBLIC_API_KEY:${var.orama_docs_public_api_key} \
        --var ORAMA_DOCS_PROJECT_ID:${var.orama_docs_project_id} \
        --var FRONTEND_ROOT:https://${var.domain_name} \
        --var API_ROOT:https://${local.api_domain}
    EOT
    environment = {
      CLOUDFLARE_API_TOKEN  = var.cloudflare_api_token
      CLOUDFLARE_ACCOUNT_ID = var.cloudflare_account_id
    }
  }
}
