// Copyright 2024 the JSR authors. All rights reserved. MIT license.

locals {
  # What the worker runs on each of these is in its `scheduled` handler, in
  # tasks/src/main.rs.
  api_worker_cron_schedules = [
    "*/5 * * * *",  # requeue stuck publishing tasks, sweep pending emails
    "*/15 * * * *", # enqueue missing npm tarballs
    "15 * * * *",   # scrape download counts
    "0 0 * * *",    # clean oauth states
    "0 3 * * *",    # clean 4h download counts
    "0 4 * * *",    # anonymize deleted users
  ]
}

resource "cloudflare_worker" "jsr_api_tasks" {
  account_id = var.cloudflare_account_id
  name       = "${var.gcp_project}-jsr-api-tasks"

  subdomain = {
    enabled          = false
    previews_enabled = false
  }

  observability = {
    enabled = true
    logs = {
      enabled            = true
      invocation_logs    = true
      head_sampling_rate = 1
      persist            = true
    }
    traces = {
      enabled            = true
      head_sampling_rate = 1
      persist            = true
    }
  }
}

resource "cloudflare_worker_version" "jsr_api_tasks" {
  account_id          = var.cloudflare_account_id
  worker_id           = cloudflare_worker.jsr_api_tasks.id
  main_module         = "index.js"
  compatibility_date  = "2026-09-02"
  compatibility_flags = ["nodejs_compat"]

  placement = {
    region = "gcp:us-central1"
  }

  # Analyzing a large package takes more than the default 30s of CPU, and
  # publishing one with tens of thousands of files makes more R2 requests than
  # the default 10,000 subrequests.
  limits = {
    cpu_ms      = 300000
    subrequests = 1000000
  }

  modules  = local.api_worker_modules["tasks"]
  bindings = local.api_worker_bindings
}

resource "cloudflare_workers_deployment" "jsr_api_tasks" {
  # Run the new version only once the database is migrated.
  depends_on = [terraform_data.api_setup]

  account_id  = var.cloudflare_account_id
  script_name = cloudflare_worker.jsr_api_tasks.name
  strategy    = "percentage"
  versions = [{
    percentage = 100
    version_id = cloudflare_worker_version.jsr_api_tasks.id
  }]
}

resource "cloudflare_workers_cron_trigger" "jsr_api_tasks" {
  depends_on = [cloudflare_workers_deployment.jsr_api_tasks]

  account_id  = var.cloudflare_account_id
  script_name = cloudflare_worker.jsr_api_tasks.name
  schedules   = [for schedule in local.api_worker_cron_schedules : { cron = schedule }]
}
