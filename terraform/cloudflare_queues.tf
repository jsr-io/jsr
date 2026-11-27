// Copyright 2024 the JSR authors. All rights reserved. MIT license.

locals {
  api_queues = {
    publish = {
      binding = "PUBLISH_QUEUE"
      // Must be above MAX_PUBLISH_ATTEMPTS in tasks/src/publish.rs, which is
      // where a task that keeps failing is recorded as failed.
      max_retries     = 30
      retry_delay     = 30
      max_concurrency = 30
    }
    npm-tarball-build = {
      binding         = "NPM_TARBALL_BUILD_QUEUE"
      max_retries     = 30
      retry_delay     = 30
      max_concurrency = 30
    }

    // Delivers outgoing email. Rows in `email_deliveries` are the durable
    // record; this queue is what actually hands each one to Postmark, so a
    // transient Postmark failure retries here instead of failing the request
    // that queued it.
    email = {
      binding = "EMAIL_QUEUE"
      // The handler abandons a delivery after MAX_EMAIL_ATTEMPTS of its own,
      // so this bound only needs to cover transient failures.
      max_retries     = 10
      retry_delay     = 300
      max_concurrency = 10
    }
  }
}

resource "cloudflare_queue" "api" {
  for_each = local.api_queues

  account_id = var.cloudflare_account_id
  queue_name = "${var.gcp_project}-${each.key}"
}

resource "cloudflare_queue_consumer" "api" {
  for_each = local.api_queues

  depends_on = [cloudflare_workers_deployment.jsr_api_tasks]

  account_id  = var.cloudflare_account_id
  queue_id    = cloudflare_queue.api[each.key].queue_id
  type        = "worker"
  script_name = cloudflare_worker.jsr_api_tasks.name

  settings = {
    // One task per invocation, so each gets the invocation's full CPU and
    // memory limits and one failing doesn't hold up another.
    batch_size      = 1
    max_retries     = each.value.max_retries
    retry_delay     = each.value.retry_delay
    max_concurrency = each.value.max_concurrency
  }
}
