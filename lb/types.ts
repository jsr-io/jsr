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
      cacheTtl?: number;
      cacheKey?: string;
    };
  }

  interface KVNamespace {
    get(key: string): Promise<string | null>;
    put(
      key: string,
      value: string,
      options?: { expirationTtl?: number },
    ): Promise<void>;
    delete(key: string): Promise<void>;
  }
}

export interface WorkerEnv {
  REGISTRY_API_URL: string;

  REGISTRY_FRONTEND_URL: string;

  GCS_ENDPOINT?: string;
  MODULES_BUCKET: string;
  MODULES_PRIVATE_BUCKET: string;
  NPM_BUCKET: string;
  NPM_PRIVATE_BUCKET: string;

  ROOT_DOMAIN: string;
  API_DOMAIN: string;
  NPM_DOMAIN: string;

  DOWNLOADS?: AnalyticsEngineDataset;
  PRIVATE_PACKAGES_KV?: KVNamespace;
}
