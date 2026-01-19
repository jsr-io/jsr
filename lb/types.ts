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

  // Cloudflare Workers Request cf property
  interface Request {
    cf?: IncomingRequestCfProperties;
  }

  interface IncomingRequestCfProperties {
    colo?: string;
    country?: string;
    city?: string;
    continent?: string;
    latitude?: string;
    longitude?: string;
    postalCode?: string;
    metroCode?: string;
    region?: string;
    regionCode?: string;
    timezone?: string;
  }
}

// Worker environment bindings
export interface WorkerEnv {
  // Cloud Run backend URLs
  REGISTRY_API_URL: string;

  // Frontend URLs per region (JSON stringified map)
  REGISTRY_FRONTEND_URLS: string;

  // GCS bucket names
  MODULES_BUCKET: string;
  NPM_BUCKET: string;

  // Domain names
  ROOT_DOMAIN: string;
  API_DOMAIN: string;
  NPM_DOMAIN: string;

  // Feature flags
  ENABLE_CACHE: string;
  ENABLE_BOT_DETECTION: string;

  DOWNLOADS?: AnalyticsEngineDataset;
}
