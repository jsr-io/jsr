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
  REGISTRY_FRONTEND_URL: string;

  ROOT_DOMAIN: string;
  API_DOMAIN: string;
  NPM_DOMAIN: string;

  DOWNLOADS?: AnalyticsEngineDataset;
  NPM_BUCKET: PartialBucket;
  MODULES_BUCKET: PartialBucket;

  // Durable Object binding for Cloudflare Container
  API_CONTAINER: DurableObjectNamespace;

  // API environment variables (passed through to containers)
  DATABASE_URL: string;
  METADATA_STRATEGY: string;
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
  CLOUDFLARE_API_TOKEN?: string;
  CLOUDFLARE_ANALYTICS_DATASET?: string;
}
