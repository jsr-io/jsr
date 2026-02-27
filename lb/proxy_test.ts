// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { assertEquals } from "https://deno.land/std@0.224.0/assert/mod.ts";
import { proxyToR2 } from "./proxy.ts";
import type { PartialBucket } from "./types.ts";

/** Minimal in-memory R2 bucket stub for testing. */
function createFakeBucket(
  objects: Record<string, { body: string; contentType?: string }>,
): PartialBucket {
  return {
    head(key: string): Promise<R2Object | null> {
      const obj = objects[key];
      if (!obj) return Promise.resolve(null);
      const size = new TextEncoder().encode(obj.body).byteLength;
      return Promise.resolve({
        key,
        version: "",
        size,
        etag: `"${key}"`,
        httpEtag: `"${key}"`,
        checksums: { toJSON: () => ({}) } as R2Checksums,
        uploaded: new Date(),
        httpMetadata: obj.contentType
          ? { contentType: obj.contentType }
          : undefined,
        customMetadata: undefined,
        range: undefined,
        storageClass: "Standard",
        ssecKeyMd5: undefined,
        writeHttpMetadata(headers: Headers) {
          if (obj.contentType) headers.set("content-type", obj.contentType);
        },
      } as R2Object);
    },

    get(
      key: string,
      _options?: R2GetOptions,
    ): Promise<R2ObjectBody | R2Object | null> {
      const obj = objects[key];
      if (!obj) return Promise.resolve(null);
      const body = new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode(obj.body));
          controller.close();
        },
      });
      const size = new TextEncoder().encode(obj.body).byteLength;
      return Promise.resolve({
        key,
        version: "",
        size,
        etag: `"${key}"`,
        httpEtag: `"${key}"`,
        checksums: { toJSON: () => ({}) } as R2Checksums,
        uploaded: new Date(),
        httpMetadata: obj.contentType
          ? { contentType: obj.contentType }
          : undefined,
        customMetadata: undefined,
        range: undefined,
        storageClass: "Standard",
        ssecKeyMd5: undefined,
        writeHttpMetadata(headers: Headers) {
          if (obj.contentType) headers.set("content-type", obj.contentType);
        },
        body,
        bodyUsed: false,
        arrayBuffer: () => new Response(body).arrayBuffer(),
        bytes: () => new Response(body).bytes(),
        text: () => new Response(body).text(),
        json: <T>() => new Response(body).json() as Promise<T>,
        blob: () => new Response(body).blob(),
      } as R2ObjectBody);
    },
  } as PartialBucket;
}

// Stub caches.default to avoid Cloudflare-specific API errors in tests.
Object.defineProperty(globalThis, "caches", {
  value: { default: undefined },
  writable: true,
  configurable: true,
});

Deno.test("proxyToR2 resolves non-encoded scoped package path", async () => {
  const bucket = createFakeBucket({
    "@jsr/std__yaml": { body: "{}", contentType: "application/json" },
  });

  const request = new Request("https://npm.jsr.io/@jsr/std__yaml");
  const response = await proxyToR2(request, bucket);

  assertEquals(response.status, 200);
  assertEquals(await response.text(), "{}");
});

Deno.test("proxyToR2 resolves URL-encoded scoped package path", async () => {
  const bucket = createFakeBucket({
    "@jsr/std__yaml": { body: "{}", contentType: "application/json" },
  });

  // pnpm sends the URL-encoded form: %2F instead of /
  const request = new Request("https://npm.jsr.io/@jsr%2Fstd__yaml");
  const response = await proxyToR2(request, bucket);

  assertEquals(response.status, 200);
  assertEquals(await response.text(), "{}");
});

Deno.test("proxyToR2 returns 404 for non-existent key", async () => {
  const bucket = createFakeBucket({});

  const request = new Request("https://npm.jsr.io/@jsr/nonexistent");
  const response = await proxyToR2(request, bucket);

  assertEquals(response.status, 404);
});

Deno.test("proxyToR2 applies pathRewrite before decoding", async () => {
  const bucket = createFakeBucket({
    "root.json": { body: "{}", contentType: "application/json" },
  });

  const request = new Request("https://npm.jsr.io/");
  const response = await proxyToR2(request, bucket, (path) => {
    if (path === "/") return "/root.json";
    return path;
  });

  assertEquals(response.status, 200);
  assertEquals(await response.text(), "{}");
});

Deno.test("proxyToR2 handles HEAD requests with URL-encoded path", async () => {
  const bucket = createFakeBucket({
    "@jsr/std__yaml": { body: "{}", contentType: "application/json" },
  });

  const request = new Request("https://npm.jsr.io/@jsr%2Fstd__yaml", {
    method: "HEAD",
  });
  const response = await proxyToR2(request, bucket);

  assertEquals(response.status, 200);
  assertEquals(response.body, null);
});
