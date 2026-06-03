// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The frontend Cloudflare Worker, deployed as three resources that
// model the modern Workers "versions and deployments" lifecycle:
//
//   1. `cloudflare_worker.jsr_frontend` — the worker shell (name,
//      tags). Stable across deploys.
//   2. `cloudflare_worker_version.jsr_frontend` — an *immutable*
//      version made of multiple modules (the bundled JS + the .wasm
//      parts pulled in by workers-og + the Workers Assets directory).
//      The resource's `id` is the version id. It RequiresReplace on
//      any content change, so a new build naturally creates a new
//      version, with a new id, on the next apply.
//   3. `cloudflare_workers_deployment.jsr_frontend` — pins 100% of
//      traffic to the current version id. Updating it is the
//      promotion. If a downstream resource fails before this
//      resource updates, the new version sits unpromoted and the
//      previous one keeps serving — clean rollback.
//
// All bindings, vars, and assets are managed by terraform directly,
// so the worker reuses the same `var.*` and tfvars values terraform
// uses everywhere else.

resource "cloudflare_worker" "jsr_frontend" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-jsr-frontend"

  observability = {
    enabled            = true
    head_sampling_rate = 0.01
    logs = {
      enabled         = true
      invocation_logs = true
      persist         = true
      # Export logs (console output + invocation logs) to the named dashboard
      # destination. null = keep them in Cloudflare's dashboard only.
      destinations = var.otlp_logs_destination != "" ? [var.otlp_logs_destination] : null
    }
    # Cloudflare's automatic request tracing, exported to the named dashboard
    # destination (OTLP endpoint + auth). null = no external trace export.
    traces = var.otlp_traces_destination != "" ? {
      enabled            = true
      head_sampling_rate = 0.01
      persist            = true
      destinations       = [var.otlp_traces_destination]
    } : null
  }
}

resource "cloudflare_worker_version" "jsr_frontend" {
  account_id          = var.cloudflare_account_id
  worker_id           = cloudflare_worker.jsr_frontend.id
  main_module         = "worker.js"
  compatibility_date  = "2026-05-19"
  compatibility_flags = ["nodejs_compat"]

  modules = [
    {
      name         = "worker.js"
      content_file = "${path.module}/../frontend/_fresh/worker.js"
      content_type = "application/javascript+module"
    },
    {
      name         = "yoga-ZMNYPE6Z.wasm"
      content_file = "${path.module}/../frontend/_fresh/server/yoga-ZMNYPE6Z.wasm"
      content_type = "application/wasm"
    },
    {
      name         = "resvg-LFIOYO65.wasm"
      content_file = "${path.module}/../frontend/_fresh/server/resvg-LFIOYO65.wasm"
      content_type = "application/wasm"
    },
  ]

  assets = {
    directory = "${path.module}/../frontend/_fresh/client"
    config = {
      html_handling      = "none"
      not_found_handling = "none"
    }
  }

  bindings = [
    {
      type = "assets"
      name = "ASSETS"
      }, {
      # Service binding to the LB worker. The frontend's API requests
      # are routed through this binding (see utils/api.ts) instead of
      # `fetch("https://api.<zone>/…")`, which Cloudflare bypasses for
      # same-zone subrequests from a Worker — that path skips the LB
      # entirely and 525s on direct TLS to origin.
      type    = "service"
      name    = "LB"
      service = "${var.gcp_project}-jsr-lb"
      }, {
      type = "plain_text"
      name = "FRONTEND_ROOT"
      text = "https://${var.domain_name}"
      }, {
      type = "plain_text"
      name = "API_ROOT"
      text = "https://${local.api_domain}"
      }, {
      type = "plain_text"
      name = "NO_COLOR"
      text = "true"
      }, {
      # Orama search keys are public by design — the frontend ships them
      # to the browser. They live as plain_text bindings, not secrets.
      type = "plain_text"
      name = "ORAMA_PACKAGES_PUBLIC_API_KEY"
      text = var.orama_packages_public_api_key
      }, {
      type = "plain_text"
      name = "ORAMA_PACKAGES_PROJECT_ID"
      text = var.orama_packages_project_id
      }, {
      type = "plain_text"
      name = "ORAMA_SYMBOLS_PUBLIC_API_KEY"
      text = var.orama_symbols_public_api_key
      }, {
      type = "plain_text"
      name = "ORAMA_SYMBOLS_PROJECT_ID"
      text = var.orama_symbols_project_id
      }, {
      type = "plain_text"
      name = "ORAMA_DOCS_PUBLIC_API_KEY"
      text = var.orama_docs_public_api_key
      }, {
      type = "plain_text"
      name = "ORAMA_DOCS_PROJECT_ID"
      text = var.orama_docs_project_id
    }
  ]
}

resource "cloudflare_workers_deployment" "jsr_frontend" {
  account_id  = var.cloudflare_account_id
  script_name = cloudflare_worker.jsr_frontend.name
  strategy    = "percentage"
  versions = [{
    percentage = 100
    version_id = cloudflare_worker_version.jsr_frontend.id
  }]
}
