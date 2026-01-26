#!/usr/bin/env -S deno run -A --watch
// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import main from "./main.ts";

const REGISTRY_FRONTEND_URL = Deno.env.get("REGISTRY_FRONTEND_URL") ??
  "http://localhost:8000";
const REGISTRY_API_URL = Deno.env.get("REGISTRY_API_URL") ??
  "http://localhost:8001";
const GCS_ENDPOINT = Deno.env.get("GCS_ENDPOINT") ?? "http://localhost:4080";
const MODULES_BUCKET = Deno.env.get("MODULES_BUCKET") ?? "modules";
const NPM_BUCKET = Deno.env.get("NPM_BUCKET") ?? "npm";
const MODULES_PRIVATE_BUCKET = Deno.env.get("MODULES_PRIVATE_BUCKET") ??
  "modules_private";
const NPM_PRIVATE_BUCKET = Deno.env.get("NPM_PRIVATE_BUCKET") ?? "npm_private";

const ROOT_DOMAIN = Deno.env.get("ROOT_DOMAIN") ?? "jsr.test";
const API_DOMAIN = Deno.env.get("API_DOMAIN") ?? "api.jsr.test";
const NPM_DOMAIN = Deno.env.get("NPM_DOMAIN") ?? "npm.jsr.test";

const PORT = 80;

async function createBucket(name: string) {
  try {
    const resp = await fetch(`${GCS_ENDPOINT}/storage/v1/b`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name }),
    });
    await resp.arrayBuffer();
    return resp.ok || resp.status === 409;
  } catch {
    return false;
  }
}

const bucketCreationInterval = setInterval(async () => {
  let allBucketsCreated = true;
  for (
    const bucket of [
      MODULES_BUCKET,
      MODULES_PRIVATE_BUCKET,
      "docs",
      "publishing",
      NPM_BUCKET,
      NPM_PRIVATE_BUCKET,
    ]
  ) {
    allBucketsCreated &&= await createBucket(bucket);
  }
  if (allBucketsCreated) {
    console.log("All buckets ready.");
    clearInterval(bucketCreationInterval);
  }
}, 5000);

function handler(req: Request): Promise<Response> {
  return main.fetch(req, {
    REGISTRY_API_URL,
    REGISTRY_FRONTEND_URL,
    GCS_ENDPOINT,
    MODULES_BUCKET,
    MODULES_PRIVATE_BUCKET,
    NPM_BUCKET,
    NPM_PRIVATE_BUCKET,
    ROOT_DOMAIN,
    API_DOMAIN,
    NPM_DOMAIN,
  });
}

if (import.meta.main) {
  Deno.serve({ port: PORT, hostname: "0.0.0.0" }, handler);
}
