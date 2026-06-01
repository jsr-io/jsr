// Copyright 2024 the JSR authors. All rights reserved. MIT license.

/// <reference types="npm:@cloudflare/workers-types" />

import type { ApiContainer } from "./containers.ts";

// `deno.worker` lib declares CacheStorage as an interface while
// `@cloudflare/workers-types` declares it as a class — they can't merge.
declare global {
  interface CacheStorage {
    default: Cache;
  }
}

export type PartialBucket = Pick<R2Bucket, "get" | "head">;

export interface WorkerEnv {
  // The frontend is a sibling Cloudflare Worker, wired up via a service
  // binding rather than an HTTP URL so traffic stays inside Cloudflare.
  FRONTEND: Fetcher;

  // The API server runs as a Cloudflare Container, fronted by a Durable
  // Object namespace. Requests are load-balanced across a fixed set of
  // instances via `getRandom` (see handleAPIRequest in main.ts).
  API_CONTAINER: DurableObjectNamespace<ApiContainer>;

  ROOT_DOMAIN: string;
  API_DOMAIN: string;
  NPM_DOMAIN: string;

  DOWNLOADS?: AnalyticsEngineDataset;
  NPM_BUCKET: PartialBucket;
  MODULES_BUCKET: PartialBucket;

  // Optional: omitted in local dev. Applied only to the frontend route —
  // not modules (R2), the API server, or npm compat. Keeps scrapers from
  // generating cache-miss load on the frontend Worker.
  FRONTEND_RATELIMIT?: RateLimit;

  // API environment variables. These are passed straight through to the
  // ApiContainer (see containers.ts) — the LB worker itself doesn't read
  // them, it only forwards them into the container's process env.
  DATABASE_URL: string;
  DB_CLIENT_CERT?: string;
  DB_CLIENT_KEY?: string;
  DB_ROOT_CERT?: string;
  METADATA_STRATEGY: string;
  GCP_SERVICE_ACCOUNT_KEY?: string;
  PUBLISHING_BUCKET: string;
  MODULES_BUCKET_NAME: string;
  DOCS_BUCKET: string;
  NPM_BUCKET_NAME: string;
  S3_REGION: string;
  S3_ENDPOINT: string;
  S3_ACCESS_KEY: string;
  S3_SECRET_KEY: string;
  GITHUB_CLIENT_ID: string;
  GITHUB_CLIENT_SECRET: string;
  GITLAB_CLIENT_ID: string;
  GITLAB_CLIENT_SECRET: string;
  POSTMARK_TOKEN?: string;
  ORAMA_PACKAGES_PROJECT_ID?: string;
  ORAMA_PACKAGES_PROJECT_KEY?: string;
  ORAMA_PACKAGES_DATA_SOURCE?: string;
  ORAMA_SYMBOLS_PROJECT_ID?: string;
  ORAMA_SYMBOLS_PROJECT_KEY?: string;
  ORAMA_SYMBOLS_DATA_SOURCE?: string;
  REGISTRY_URL: string;
  NPM_URL: string;
  EMAIL_FROM?: string;
  EMAIL_FROM_NAME?: string;
  PUBLISH_QUEUE_ID?: string;
  NPM_TARBALL_BUILD_QUEUE_ID?: string;
  CLOUDFLARE_ACCOUNT_ID: string;
  CLOUDFLARE_ZONE_ID?: string;
  CLOUDFLARE_API_TOKEN?: string;
  CLOUDFLARE_ANALYTICS_DATASET?: string;
  // Telemetry: OTLP/HTTP endpoint (non-secret, from wrangler vars) and the auth
  // header (secret, delivered via `wrangler secret bulk`). Both optional —
  // omitted when telemetry isn't configured for the environment.
  OTLP_ENDPOINT?: string;
  OTLP_HEADERS?: string;
}
