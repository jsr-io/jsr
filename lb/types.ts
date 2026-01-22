// Copyright 2024 the JSR authors. All rights reserved. MIT license.

declare global {
  interface AnalyticsEngineDataset {
    writeDataPoint(event?: AnalyticsEngineDataPoint): void;
  }

  interface AnalyticsEngineDataPoint {
    indexes?: [string];
    doubles?: number[];
    blobs?: string[];
  }

  interface CacheStorage {
    default: Cache;
  }

  interface RequestInit {
    cf?: {
      cacheEverything?: boolean;
      cacheKey?: string;
    };
  }
}

export interface WorkerEnv {
  REGISTRY_API_URL: string;

  REGISTRY_FRONTEND_URL: string;

  GCS_ENDPOINT?: string;
  MODULES_BUCKET: string;
  NPM_BUCKET: string;

  ROOT_DOMAIN: string;
  API_DOMAIN: string;
  NPM_DOMAIN: string;

  DOWNLOADS?: AnalyticsEngineDataset;
}
