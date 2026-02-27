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

  REGISTRY_FRONTEND_URL: string;

  GCS_ENDPOINT?: string;
  MODULES_BUCKET: string;

  ROOT_DOMAIN: string;
  API_DOMAIN: string;
  NPM_DOMAIN: string;

  DOWNLOADS?: AnalyticsEngineDataset;
  NPM_BUCKET: PartialBucket;
}
