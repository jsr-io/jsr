// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { Container } from "@cloudflare/containers";
import type { WorkerEnv } from "./types.ts";

export function apiEnvVars(env: WorkerEnv): Record<string, string> {
  return {
    PORT: "8001",
    NO_COLOR: "true",
    DATABASE_URL: env.DATABASE_URL,
    METADATA_STRATEGY: env.METADATA_STRATEGY,
    GCS_ENDPOINT: env.GCS_ENDPOINT ?? "",
    PUBLISHING_BUCKET: env.PUBLISHING_BUCKET,
    MODULES_BUCKET: env.MODULES_BUCKET_NAME,
    DOCS_BUCKET: env.DOCS_BUCKET,
    NPM_BUCKET: env.NPM_BUCKET_NAME,
    S3_REGION: env.S3_REGION,
    S3_ENDPOINT: env.S3_ENDPOINT,
    S3_ACCESS_KEY: env.S3_ACCESS_KEY,
    S3_SECRET_KEY: env.S3_SECRET_KEY,
    GITHUB_CLIENT_ID: env.GITHUB_CLIENT_ID,
    GITHUB_CLIENT_SECRET: env.GITHUB_CLIENT_SECRET,
    POSTMARK_TOKEN: env.POSTMARK_TOKEN ?? "",
    ORAMA_PACKAGES_PROJECT_ID: env.ORAMA_PACKAGES_PROJECT_ID ?? "",
    ORAMA_PACKAGES_PROJECT_KEY: env.ORAMA_PACKAGES_PROJECT_KEY ?? "",
    ORAMA_PACKAGES_DATA_SOURCE: env.ORAMA_PACKAGES_DATA_SOURCE ?? "",
    ORAMA_SYMBOLS_PROJECT_ID: env.ORAMA_SYMBOLS_PROJECT_ID ?? "",
    ORAMA_SYMBOLS_PROJECT_KEY: env.ORAMA_SYMBOLS_PROJECT_KEY ?? "",
    ORAMA_SYMBOLS_DATA_SOURCE: env.ORAMA_SYMBOLS_DATA_SOURCE ?? "",
    REGISTRY_URL: env.REGISTRY_URL,
    NPM_URL: env.NPM_URL,
    EMAIL_FROM: env.EMAIL_FROM ?? "",
    EMAIL_FROM_NAME: env.EMAIL_FROM_NAME ?? "",
    PUBLISH_QUEUE_ID: env.PUBLISH_QUEUE_ID ?? "",
    NPM_TARBALL_BUILD_QUEUE_ID: env.NPM_TARBALL_BUILD_QUEUE_ID ?? "",
    CLOUDFLARE_ACCOUNT_ID: env.CLOUDFLARE_ACCOUNT_ID,
    CLOUDFLARE_API_TOKEN: env.CLOUDFLARE_API_TOKEN ?? "",
    CLOUDFLARE_ANALYTICS_DATASET: env.CLOUDFLARE_ANALYTICS_DATASET ?? "",
  };
}

export class ApiContainer extends Container {
  override defaultPort = 8001;
  override sleepAfter = "5m";
  args = ["--api", "--tasks=false", "--database_pool_size=4"];
}
