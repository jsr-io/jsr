// Copyright 2024 the JSR authors. All rights reserved. MIT license.

declare global {
  // Cloudflare Workers ExecutionContext
  interface ExecutionContext {
    waitUntil(promise: Promise<unknown>): void;
    passThroughOnException(): void;
  }

  // Cloudflare Workers Analytics Engine
  interface AnalyticsEngineDataset {
    writeDataPoint(event?: AnalyticsEngineDataPoint): void;
  }

  interface AnalyticsEngineDataPoint {
    indexes?: [string];
    doubles?: number[];
    blobs?: string[];
  }

  // Cloudflare Workers Cache API
  interface CacheStorage {
    default: Cache;
  }

  interface RequestInit {
    cf?: {
      cacheEverything?: boolean;
    };
  }
}

export interface WorkerEnv {
  REGISTRY_API_URL: string;

  REGISTRY_FRONTEND_URL: string;

  MODULES_BUCKET: string;
  NPM_BUCKET: string;

  ROOT_DOMAIN: string;
  API_DOMAIN: string;
  NPM_DOMAIN: string;

  ENABLE_CACHE: string;
  ENABLE_BOT_DETECTION: string;

  DOWNLOADS?: AnalyticsEngineDataset;
}
