#!/usr/bin/env -S deno run --allow-env --allow-run --allow-read --allow-write
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Delivers the LB worker's secrets to Cloudflare via `wrangler secret bulk`.
//
// The cloudflare Terraform provider can't write secrets to a wrangler-managed
// worker, so this bridges the two: the Terraform-generated secrets (read from
// `terraform output`) and the GitHub-Actions-sourced secrets (from env) are
// bundled and pushed. Terraform still GENERATES/owns its values — this only
// delivers them. The target worker comes from lb/wrangler.json (which Terraform
// renders per environment), so no environment argument is needed here.

async function run(
  cmd: string,
  args: string[],
  opts: Deno.CommandOptions = {},
): Promise<string> {
  const { success, stdout, stderr } = await new Deno.Command(cmd, {
    args,
    stdout: "piped",
    stderr: "piped",
    ...opts,
  }).output();
  if (!success) {
    throw new Error(
      `${cmd} ${args.join(" ")} failed: ${new TextDecoder().decode(stderr)}`,
    );
  }
  return new TextDecoder().decode(stdout);
}

// Precondition: terraform must already be initialized against the target
// environment's state (CI does this in the preceding `terraform init`/apply
// steps). Running this outside that flow will fail to read the outputs.
const tfOutput = (name: string) =>
  run("terraform", ["-chdir=terraform", "output", "-raw", name]);

function envVar(name: string): string {
  const v = Deno.env.get(name);
  if (!v) throw new Error(`missing required env var ${name}`);
  return v;
}

// TF-generated values (owned by terraform) + GitHub-sourced values.
const secrets: Record<string, string> = {
  DATABASE_URL: await tfOutput("lb_database_url"),
  GCP_SERVICE_ACCOUNT_KEY: await tfOutput("lb_gcp_service_account_key"),
  S3_ACCESS_KEY: await tfOutput("lb_s3_access_key"),
  S3_SECRET_KEY: await tfOutput("lb_s3_secret_key"),
  DB_CLIENT_CERT: await tfOutput("lb_db_client_cert"),
  DB_CLIENT_KEY: await tfOutput("lb_db_client_key"),
  DB_ROOT_CERT: await tfOutput("lb_db_root_cert"),
  // Empty when telemetry isn't configured for this environment; filtered out
  // below so we don't push an empty OTLP_HEADERS secret.
  OTLP_HEADERS: await tfOutput("lb_otlp_headers"),
  GITHUB_CLIENT_SECRET: envVar("GITHUB_CLIENT_SECRET"),
  GITLAB_CLIENT_SECRET: envVar("GITLAB_CLIENT_SECRET"),
  POSTMARK_TOKEN: envVar("POSTMARK_TOKEN"),
  ORAMA_PACKAGES_PROJECT_KEY: envVar("ORAMA_PACKAGES_PROJECT_KEY"),
  ORAMA_PACKAGES_PROJECT_ID: envVar("ORAMA_PACKAGES_PROJECT_ID"),
  ORAMA_PACKAGES_DATA_SOURCE: envVar("ORAMA_PACKAGES_DATA_SOURCE"),
  ORAMA_SYMBOLS_PROJECT_KEY: envVar("ORAMA_SYMBOLS_PROJECT_KEY"),
  ORAMA_SYMBOLS_PROJECT_ID: envVar("ORAMA_SYMBOLS_PROJECT_ID"),
  ORAMA_SYMBOLS_DATA_SOURCE: envVar("ORAMA_SYMBOLS_DATA_SOURCE"),
  CLOUDFLARE_API_TOKEN: envVar("CLOUDFLARE_API_TOKEN"),
};

// Drop empty values so optional secrets (e.g. OTLP_HEADERS when telemetry is
// disabled) aren't pushed as blank secrets.
const nonEmptySecrets = Object.fromEntries(
  Object.entries(secrets).filter(([, v]) => v !== ""),
);

const file = await Deno.makeTempFile({ suffix: ".json" });
try {
  await Deno.writeTextFile(file, JSON.stringify(nonEmptySecrets));
  const { success } = await new Deno.Command("wrangler", {
    args: ["secret", "bulk", file],
    cwd: "lb",
    stdout: "inherit",
    stderr: "inherit",
  }).output();
  if (!success) Deno.exit(1);
} finally {
  await Deno.remove(file);
}
