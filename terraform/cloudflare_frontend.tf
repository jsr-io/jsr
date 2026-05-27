// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The frontend is bundled by `frontend/build.ts` (vite Fresh build + a
// `deno bundle` of `frontend/server.entry.ts`) into:
//   - frontend/_fresh/worker.js  — the script content
//   - frontend/_fresh/client/    — the static-asset tree served by the
//                                  Workers Assets binding (vite's client
//                                  output, plus Fresh-generated CSS from
//                                  _fresh/static/ and frontend/docs/*.md
//                                  under _jsr_docs/).

resource "cloudflare_workers_script" "jsr_frontend" {
  account_id  = var.cloudflare_account_id
  script_name = "${var.gcp_project}-jsr-frontend"
  content     = file("${path.module}/../frontend/_fresh/worker.js")
  main_module = "worker.js"

  compatibility_date  = "2026-05-19"
  compatibility_flags = ["nodejs_compat"]

  observability = {
    enabled = true
    logs = {
      enabled            = true
      invocation_logs    = true
      head_sampling_rate = 0.01
      persist            = true
    }
  }

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
      # Orama search keys are public by design — the frontend ships them to
      # the browser through routes/index.tsx, routes/packages.tsx, and
      # components/Header.tsx. They live as plain_text bindings, not secrets.
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

  lifecycle {
    create_before_destroy = true
  }
}
