#!/usr/bin/env -S deno run -A --watch
// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import main from "./main.ts";
import type { PartialBucket } from "./types.ts";
import {
  CreateBucketCommand,
  GetObjectCommand,
  HeadObjectCommand,
  type HeadObjectCommandOutput,
  S3Client,
} from "@aws-sdk/client-s3";

const REGISTRY_FRONTEND_URL = Deno.env.get("REGISTRY_FRONTEND_URL") ??
  "http://localhost:8000";
const REGISTRY_API_URL = Deno.env.get("REGISTRY_API_URL") ??
  "http://localhost:8001";
const GCS_ENDPOINT = Deno.env.get("GCS_ENDPOINT") ?? "http://localhost:4080";
const S3_ENDPOINT = Deno.env.get("S3_ENDPOINT") ?? "http://localhost:9000";
const MODULES_BUCKET = Deno.env.get("MODULES_BUCKET") ?? "modules";
const NPM_BUCKET = Deno.env.get("NPM_BUCKET") ?? "npm";
const DOCS_BUCKET = Deno.env.get("DOCS_BUCKET") ?? "docs";
const PUBLISHING_BUCKET = Deno.env.get("PUBLISHING_BUCKET") ?? "publishing";

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

async function createMinioBucket(name: string) {
  try {
    const s3 = new S3Client({
      endpoint: S3_ENDPOINT,
      region: "us-east-1",
      credentials: { accessKeyId: "minioadmin", secretAccessKey: "minioadmin" },
      forcePathStyle: true,
    });

    await s3.send(new CreateBucketCommand({ Bucket: name }));
    return true;
    // deno-lint-ignore no-explicit-any
  } catch (err: any) {
    if (
      err.name === "BucketAlreadyOwnedByYou" ||
      err.name === "BucketAlreadyExists"
    ) {
      return true;
    }

    return false;
  }
}

const bucketCreationInterval = setInterval(async () => {
  let allBucketsCreated = true;
  for (const bucket of [MODULES_BUCKET]) {
    allBucketsCreated &&= await createBucket(bucket);
  }
  for (const bucket of [DOCS_BUCKET, PUBLISHING_BUCKET, NPM_BUCKET]) {
    allBucketsCreated &&= await createMinioBucket(bucket);
  }

  if (allBucketsCreated) {
    console.log("All buckets ready.");
    clearInterval(bucketCreationInterval);
  }
}, 5000);

const s3 = new S3Client({
  endpoint: S3_ENDPOINT,
  region: "us-east-1",
  credentials: { accessKeyId: "minioadmin", secretAccessKey: "minioadmin" },
  forcePathStyle: true,
});

class R2BucketShim implements PartialBucket {
  #bucket: string;

  constructor(bucket: string) {
    this.#bucket = bucket;
  }

  async head(key: string): Promise<R2Object | null> {
    try {
      const res = await s3.send(
        new HeadObjectCommand({ Bucket: this.#bucket, Key: key }),
      );
      return this.#toR2Object(res, key);
    } catch {
      return null;
    }
  }

  get(
    key: string,
    options: R2GetOptions & { onlyIf: R2Conditional | Headers },
  ): Promise<R2ObjectBody | R2Object | null>;
  get(key: string, options?: R2GetOptions): Promise<R2ObjectBody | null>;
  async get(
    key: string,
    options?: R2GetOptions,
  ): Promise<R2ObjectBody | R2Object | null> {
    try {
      const params: {
        Bucket: string;
        Key: string;
        IfNoneMatch?: string;
        IfModifiedSince?: Date;
      } = {
        Bucket: this.#bucket,
        Key: key,
      };
      const onlyIf = options?.onlyIf;
      if (onlyIf && !(onlyIf instanceof Headers)) {
        if (onlyIf.etagMatches) params.IfNoneMatch = onlyIf.etagMatches;
        if (onlyIf.uploadedAfter) params.IfModifiedSince = onlyIf.uploadedAfter;
      }
      const res = await s3.send(new GetObjectCommand(params));
      const body = res.Body!.transformToWebStream();
      const obj = this.#toR2Object(res, key);
      return {
        ...obj,
        body,
        bodyUsed: false,
        arrayBuffer: () => new Response(body).arrayBuffer(),
        bytes: () => new Response(body).bytes(),
        text: () => new Response(body).text(),
        json: <T>() => new Response(body).json() as Promise<T>,
        blob: () => new Response(body).blob(),
      } as R2ObjectBody;
    } catch (err: unknown) {
      if (err instanceof Error && err.name === "NotModified") {
        return this.#toR2Object({
          ETag: "",
          ContentLength: 0,
          ContentType: undefined,
          LastModified: undefined,
        }, key);
      }
      return null;
    }
  }

  #toR2Object(
    res: Pick<
      HeadObjectCommandOutput,
      "ETag" | "ContentLength" | "ContentType" | "LastModified"
    >,
    key: string,
  ): R2Object {
    const etag = res.ETag ?? "";
    const size = res.ContentLength ?? 0;
    const contentType = res.ContentType;
    return {
      key,
      version: "",
      size,
      etag: etag.replace(/^"|"$/g, ""),
      httpEtag: etag,
      checksums: { toJSON: () => ({}) } as R2Checksums,
      uploaded: res.LastModified ?? new Date(),
      httpMetadata: contentType ? { contentType } : undefined,
      customMetadata: undefined,
      range: undefined,
      storageClass: "Standard",
      ssecKeyMd5: undefined,
      writeHttpMetadata(headers: Headers) {
        if (contentType) headers.set("content-type", contentType);
      },
    } as R2Object;
  }
}

function handler(req: Request): Promise<Response> {
  return main.fetch(req, {
    REGISTRY_API_URL,
    REGISTRY_FRONTEND_URL,
    GCS_ENDPOINT,
    MODULES_BUCKET,
    NPM_BUCKET: new R2BucketShim(NPM_BUCKET),
    ROOT_DOMAIN,
    API_DOMAIN,
    NPM_DOMAIN,
  });
}

if (import.meta.main) {
  Deno.serve({ port: PORT, hostname: "0.0.0.0" }, handler);
}
