// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import {
  assertEquals,
  assertNotEquals,
} from "https://deno.land/std@0.224.0/assert/mod.ts";
import { proxyToCloudRun, proxyToR2 } from "./proxy.ts";
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

// --- proxyToCloudRun tests ---

/** In-memory Cache stub that records put/match calls for assertions. */
function createFakeCache(): Cache & {
  store: Map<string, Response>;
  putCalls: string[];
  matchCalls: string[];
} {
  const store = new Map<string, Response>();
  const putCalls: string[] = [];
  const matchCalls: string[] = [];

  return {
    store,
    putCalls,
    matchCalls,
    async match(request: RequestInfo | URL): Promise<Response | undefined> {
      const url = typeof request === "string"
        ? request
        : request instanceof URL
        ? request.toString()
        : request.url;
      matchCalls.push(url);
      const cached = store.get(url);
      return cached ? cached.clone() : undefined;
    },
    async put(request: RequestInfo | URL, response: Response): Promise<void> {
      const url = typeof request === "string"
        ? request
        : request instanceof URL
        ? request.toString()
        : request.url;
      putCalls.push(url);
      store.set(url, response.clone());
    },
    async delete(_request: RequestInfo | URL): Promise<boolean> {
      return false;
    },
  } as Cache & {
    store: Map<string, Response>;
    putCalls: string[];
    matchCalls: string[];
  };
}

function setupFetchStub(
  response: Response,
): () => void {
  const original = globalThis.fetch;
  // deno-lint-ignore no-explicit-any
  (globalThis as any).fetch = (_input: RequestInfo | URL, _init?: RequestInit) => {
    return Promise.resolve(response.clone());
  };
  return () => {
    globalThis.fetch = original;
  };
}

const BACKEND_URL = "https://backend.example.com";

Deno.test("proxyToCloudRun caches anonymous GET with URL-only key", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response('{"ok":true}', {
      status: 200,
      headers: { "Cache-Control": "public, max-age=30, s-maxage=300" },
    }),
  );

  try {
    const request = new Request("https://jsr.io/api/packages", {
      method: "GET",
    });
    const response = await proxyToCloudRun(request, BACKEND_URL);

    assertEquals(response.status, 200);
    assertEquals(cache.putCalls.length, 1);
    // Cache key should be the backend URL, not include client headers
    assertEquals(cache.putCalls[0], `${BACKEND_URL}/api/packages`);
    // Cacheable responses should NOT have Vary: Cookie, Authorization
    assertEquals(response.headers.has("Vary"), false);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToCloudRun serves cached response on second GET", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  let fetchCount = 0;
  const original = globalThis.fetch;
  // deno-lint-ignore no-explicit-any
  (globalThis as any).fetch = (_input: RequestInfo | URL, _init?: RequestInit) => {
    fetchCount++;
    return Promise.resolve(
      new Response('{"ok":true}', {
        status: 200,
        headers: { "Cache-Control": "public, max-age=30, s-maxage=300" },
      }),
    );
  };

  try {
    const req1 = new Request("https://jsr.io/api/packages", { method: "GET" });
    await proxyToCloudRun(req1, BACKEND_URL);
    assertEquals(fetchCount, 1);

    const req2 = new Request("https://jsr.io/api/packages", {
      method: "GET",
      headers: { "Cookie": "other=value", "CF-Connecting-IP": "1.2.3.4" },
    });
    const res2 = await proxyToCloudRun(req2, BACKEND_URL);
    // Should be served from cache — fetch not called again
    assertEquals(fetchCount, 1);
    assertEquals(res2.status, 200);
  } finally {
    globalThis.fetch = original;
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToCloudRun skips cache for authenticated requests", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response('{"user":"me"}', {
      status: 200,
      headers: { "Cache-Control": "private, max-age=300" },
    }),
  );

  try {
    const request = new Request("https://jsr.io/api/user", {
      method: "GET",
      headers: { "Authorization": "Bearer token123" },
    });
    const response = await proxyToCloudRun(request, BACKEND_URL);

    assertEquals(response.status, 200);
    // Should not cache authenticated requests
    assertEquals(cache.putCalls.length, 0);
    assertEquals(cache.matchCalls.length, 0);
    // Should set private headers
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
    assertEquals(response.headers.get("Vary"), "Cookie, Authorization");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToCloudRun skips cache for cookie-authenticated requests", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response('{"user":"me"}', { status: 200 }),
  );

  try {
    const request = new Request("https://jsr.io/api/user", {
      method: "GET",
      headers: { "Cookie": "token=abc123" },
    });
    const response = await proxyToCloudRun(request, BACKEND_URL);

    assertEquals(cache.putCalls.length, 0);
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToCloudRun skips cache for POST requests", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response('{"created":true}', { status: 201 }),
  );

  try {
    const request = new Request("https://jsr.io/api/packages", {
      method: "POST",
      body: "{}",
    });
    const response = await proxyToCloudRun(request, BACKEND_URL);

    assertEquals(response.status, 201);
    assertEquals(cache.putCalls.length, 0);
    assertEquals(cache.matchCalls.length, 0);
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToCloudRun does not cache responses with private Cache-Control", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response('{"ok":true}', {
      status: 200,
      headers: { "Cache-Control": "private, max-age=60" },
    }),
  );

  try {
    const request = new Request("https://jsr.io/api/data", { method: "GET" });
    await proxyToCloudRun(request, BACKEND_URL);

    // Cache was checked but response was not stored due to private directive
    assertEquals(cache.matchCalls.length, 1);
    assertEquals(cache.putCalls.length, 0);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToCloudRun does not cache non-2xx responses", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response("Not Found", { status: 404 }),
  );

  try {
    const request = new Request("https://jsr.io/api/missing", {
      method: "GET",
    });
    const response = await proxyToCloudRun(request, BACKEND_URL);

    assertEquals(response.status, 404);
    assertEquals(cache.putCalls.length, 0);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToCloudRun serves HEAD from cached GET response", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  let fetchCount = 0;
  const original = globalThis.fetch;
  // deno-lint-ignore no-explicit-any
  (globalThis as any).fetch = (_input: RequestInfo | URL, _init?: RequestInit) => {
    fetchCount++;
    return Promise.resolve(
      new Response('{"ok":true}', {
        status: 200,
        headers: { "Cache-Control": "public, max-age=30, s-maxage=300" },
      }),
    );
  };

  try {
    // First, populate cache with a GET
    const getReq = new Request("https://jsr.io/api/packages", {
      method: "GET",
    });
    await proxyToCloudRun(getReq, BACKEND_URL);
    assertEquals(fetchCount, 1);

    // HEAD should be served from cache without hitting origin
    const headReq = new Request("https://jsr.io/api/packages", {
      method: "HEAD",
    });
    const headRes = await proxyToCloudRun(headReq, BACKEND_URL);
    assertEquals(fetchCount, 1); // No additional fetch
    assertEquals(headRes.status, 200);
    assertEquals(headRes.body, null);
  } finally {
    globalThis.fetch = original;
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToCloudRun skips cache for login paths", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response("", { status: 302, headers: { Location: "/callback" } }),
  );

  try {
    const request = new Request("https://jsr.io/login/callback?code=abc", {
      method: "GET",
    });
    const response = await proxyToCloudRun(request, BACKEND_URL);

    assertEquals(cache.putCalls.length, 0);
    assertEquals(cache.matchCalls.length, 0);
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});
