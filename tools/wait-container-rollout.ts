#!/usr/bin/env -S deno run --allow-env --allow-net
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Blocks until the Cloudflare Container application backing the LB worker has
// finished rolling out a new image, so CI can wait for the rollout to complete
// before promoting the worker deployment (see .github/workflows/ci.yml).
//
// Cloudflare has no `wrangler containers rollout` subcommand, so this polls the
// REST API directly:
//   GET /accounts/{account_id}/containers/applications
// finds the application, then reads its rollout state. A rollout is "done" when
// there is no in-progress rollout and every instance is on the target version
// (the rollout object, when present, reports `status: "completed"`).
//
// Usage:
//   CLOUDFLARE_API_TOKEN=... CLOUDFLARE_ACCOUNT_ID=... \
//     deno run --allow-env --allow-net tools/wait-container-rollout.ts \
//     --app <application-name-or-substring> [--timeout 1200] [--interval 15]

import { parseArgs } from "jsr:@std/cli@^1";

const API_BASE = "https://api.cloudflare.com/client/v4";

const args = parseArgs(Deno.args, {
  string: ["app", "timeout", "interval"],
});

const accountId = Deno.env.get("CLOUDFLARE_ACCOUNT_ID");
const apiToken = Deno.env.get("CLOUDFLARE_API_TOKEN");
// The container application Cloudflare creates for the LB worker is named after
// the worker script. Match by substring so callers can pass the bare worker
// name (e.g. "jsr-lb") regardless of the project prefix.
const appQuery = args.app ?? Deno.env.get("CONTAINER_APP_NAME") ?? "jsr-lb";
const timeoutSecs = Number(args.timeout ?? "1200");
const intervalSecs = Number(args.interval ?? "15");

if (!accountId || !apiToken) {
  console.error(
    "CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN must be set.",
  );
  Deno.exit(1);
}

interface RolloutProgress {
  total_steps?: number;
  current_step?: number;
  updated_instances?: number;
  total_instances?: number;
}

interface Rollout {
  id?: string;
  status?: string; // pending | progressing | completed | reverted | replaced
  progress?: RolloutProgress;
}

interface ContainerApplication {
  id: string;
  name: string;
  health?: { instances?: Record<string, number> };
  active_rollout_id?: string | null;
  rollout?: Rollout;
  instances?: number;
}

async function cf<T>(path: string): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`, {
    headers: {
      Authorization: `Bearer ${apiToken}`,
      "Content-Type": "application/json",
    },
  });
  const body = await resp.json() as {
    success?: boolean;
    errors?: unknown;
    result?: unknown;
  };
  if (!resp.ok || body.success === false) {
    throw new Error(
      `Cloudflare API ${path} failed: ${resp.status} ${
        JSON.stringify(body.errors ?? body)
      }`,
    );
  }
  return body.result as T;
}

async function findApplication(): Promise<ContainerApplication> {
  const apps = await cf<ContainerApplication[]>(
    `/accounts/${accountId}/containers/applications`,
  );
  const matches = apps.filter((a) => a.name.includes(appQuery));
  if (matches.length === 0) {
    throw new Error(
      `No container application matching "${appQuery}". Found: ${
        apps.map((a) => a.name).join(", ") || "(none)"
      }`,
    );
  }
  if (matches.length > 1) {
    throw new Error(
      `Multiple container applications match "${appQuery}": ${
        matches.map((a) => a.name).join(", ")
      }. Pass a more specific --app.`,
    );
  }
  return matches[0];
}

// Returns true when the application has no rollout in progress.
function isRolledOut(app: ContainerApplication): boolean {
  const rollout = app.rollout;
  // An explicit rollout object is the most reliable signal.
  if (rollout?.status) {
    const p = rollout.progress;
    const progress = p
      ? ` (${p.updated_instances ?? "?"}/${
        p.total_instances ?? "?"
      } instances,` +
        ` step ${p.current_step ?? "?"}/${p.total_steps ?? "?"})`
      : "";
    console.log(`  rollout status: ${rollout.status}${progress}`);
    if (rollout.status === "completed") return true;
    if (rollout.status === "reverted" || rollout.status === "replaced") {
      throw new Error(`Rollout ended in state "${rollout.status}".`);
    }
    return false;
  }
  // Otherwise fall back to the active-rollout pointer: no active rollout means
  // nothing is in progress.
  if (app.active_rollout_id) {
    console.log(`  active rollout in progress: ${app.active_rollout_id}`);
    return false;
  }
  console.log("  no active rollout");
  return true;
}

// Stop promptly if CI cancels the job, instead of polling until --timeout.
Deno.addSignalListener("SIGTERM", () => Deno.exit(143));

const deadline = Date.now() + timeoutSecs * 1000;
console.log(
  `Waiting for container rollout of "${appQuery}" ` +
    `(timeout ${timeoutSecs}s, poll every ${intervalSecs}s)...`,
);

while (true) {
  try {
    // Re-resolve each poll: on a brand-new deploy the application may not exist
    // yet (findApplication throws / the API 404s), so we tolerate that and keep
    // polling until it appears or we time out.
    const found = await findApplication();
    const app = await cf<ContainerApplication>(
      `/accounts/${accountId}/containers/applications/${found.id}`,
    );
    if (isRolledOut(app)) {
      console.log("Container rollout complete.");
      Deno.exit(0);
    }
  } catch (err) {
    console.log(`  application not ready yet: ${(err as Error).message}`);
  }
  if (Date.now() >= deadline) {
    console.error(`Timed out after ${timeoutSecs}s waiting for rollout.`);
    Deno.exit(1);
  }
  await new Promise((r) => setTimeout(r, intervalSecs * 1000));
}
