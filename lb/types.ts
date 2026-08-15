// Copyright 2024 the JSR authors. All rights reserved. MIT license.

/// <reference types="npm:@cloudflare/workers-types" />

// `deno.worker` lib declares CacheStorage as an interface while
// `@cloudflare/workers-types` declares it as a class — they can't merge.
declare global {
  interface CacheStorage {
    default: Cache;
  }
}

export type PartialBucket = Pick<R2Bucket, "get" | "head">;

export interface WorkerEnv {
  REGISTRY_API_URL: string;

  // The frontend is a sibling Cloudflare Worker, wired up via a service
  // binding rather than an HTTP URL so traffic stays inside Cloudflare.
  FRONTEND: Fetcher;

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

  // Optional: omitted in local dev. Stricter per-IP limit applied only to the
  // doc, diff, and source package pages (see isDocsDiffSourceRoute), the
  // expensive-to-render routes scrapers walk symbol-by-symbol.
  DOCS_RATELIMIT?: RateLimit;
}
