// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// deno-lint-ignore-file require-await no-explicit-any

import { assertEquals } from "@std/assert";
import { proxyToBackend, proxyToR2 } from "./proxy.ts";
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

Deno.test("proxyToR2 cache hit returns a fresh, mutable response", async () => {
  // caches.default.match returns responses with immutable headers; callers
  // (setSecurityHeaders etc.) mutate the returned response, so proxyToR2 must
  // re-wrap rather than return the cached object directly.
  const stored = new Response("cached-body", {
    status: 200,
    headers: { "content-type": "application/json" },
  });
  (globalThis as any).caches = {
    default: {
      match: () => Promise.resolve(stored),
      put: () => Promise.resolve(),
    },
  };

  try {
    const req = new Request("https://npm.jsr.io/@jsr/whatever");
    const res = await proxyToR2(req, createFakeBucket({}));

    assertEquals(res !== stored, true); // not the cached object
    res.headers.set("x-test", "1"); // must not throw
    assertEquals(res.headers.get("x-test"), "1");
    assertEquals(await res.text(), "cached-body");
  } finally {
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToR2 namespaces cache keys away from the raw URL", async () => {
  // The frontend caches `/@scope/...` navigations under the raw URL; bucket
  // responses for the same URL must use a distinct key to avoid cross-serving.
  const matchKeys: string[] = [];
  const putKeys: string[] = [];
  (globalThis as any).caches = {
    default: {
      match: (req: Request) => {
        matchKeys.push(req.url);
        return Promise.resolve(undefined);
      },
      put: (req: Request) => {
        putKeys.push(req.url);
        return Promise.resolve();
      },
    },
  };

  try {
    const bucket = createFakeBucket({
      "@jsr/std__yaml": { body: "{}", contentType: "application/json" },
    });
    const res = await proxyToR2(
      new Request("https://npm.jsr.io/@jsr/std__yaml"),
      bucket,
    );

    assertEquals(res.status, 200);
    assertEquals(
      matchKeys[0],
      "https://bucket-cache.jsr.internal/npm.jsr.io/@jsr/std__yaml",
    );
    assertEquals(putKeys[0], matchKeys[0]); // match and put use the same key
  } finally {
    (globalThis as any).caches = { default: undefined };
  }
});

// --- proxyToBackend tests ---

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
  (globalThis as any).fetch = (
    _input: RequestInfo | URL,
    _init?: RequestInit,
  ) => {
    return Promise.resolve(response.clone());
  };
  return () => {
    globalThis.fetch = original;
  };
}

const BACKEND_URL = "https://backend.example.com";

Deno.test("proxyToBackend caches anonymous GET with URL-only key", async () => {
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
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(response.status, 200);
    assertEquals(cache.putCalls.length, 1);
    // Cache key should be the backend URL, not include client headers
    assertEquals(cache.putCalls[0], `${BACKEND_URL}/api/packages`);
    // Vary is set on all responses so browsers re-fetch when auth changes
    assertEquals(response.headers.get("Vary"), "Cookie, Authorization");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend registers the cache write with ctx.waitUntil", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  // Without this, the Workers runtime tears the invocation down before the
  // async Cache.put completes and the write is silently dropped — the bug that
  // left the lb caching nothing in production.
  const waited: Promise<unknown>[] = [];
  const ctx = { waitUntil: (p: Promise<unknown>) => waited.push(p) };

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
    await proxyToBackend(request, BACKEND_URL, undefined, ctx);

    assertEquals(waited.length, 1);
    await Promise.all(waited);
    assertEquals(cache.putCalls.length, 1);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend serves cached response on second GET", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  let fetchCount = 0;
  const original = globalThis.fetch;
  (globalThis as any).fetch = (
    _input: RequestInfo | URL,
    _init?: RequestInit,
  ) => {
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
    await proxyToBackend(req1, BACKEND_URL);
    assertEquals(fetchCount, 1);

    const req2 = new Request("https://jsr.io/api/packages", {
      method: "GET",
      headers: { "Cookie": "other=value", "CF-Connecting-IP": "1.2.3.4" },
    });
    const res2 = await proxyToBackend(req2, BACKEND_URL);
    // Should be served from cache — fetch not called again
    assertEquals(fetchCount, 1);
    assertEquals(res2.status, 200);
  } finally {
    globalThis.fetch = original;
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend skips cache for authenticated requests", async () => {
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
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(response.status, 200);
    // The cache is consulted (it may hold an identity-independent entry), but a
    // viewer-specific (non-shared) response is neither served nor stored for an
    // authenticated caller.
    assertEquals(cache.matchCalls.length, 1);
    assertEquals(cache.putCalls.length, 0);
    // Should set private headers
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
    assertEquals(response.headers.get("Vary"), "Cookie, Authorization");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend skips cache for cookie-authenticated requests", async () => {
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
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(cache.putCalls.length, 0);
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend skips cache for POST requests", async () => {
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
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(response.status, 201);
    assertEquals(cache.putCalls.length, 0);
    assertEquals(cache.matchCalls.length, 0);
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend does not cache responses with private Cache-Control", async () => {
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
    await proxyToBackend(request, BACKEND_URL);

    // Cache was checked but response was not stored due to private directive
    assertEquals(cache.matchCalls.length, 1);
    assertEquals(cache.putCalls.length, 0);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend does not cache 404s without a cacheable directive", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response("Not Found", { status: 404 }),
  );

  try {
    const request = new Request("https://jsr.io/api/missing", {
      method: "GET",
    });
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(response.status, 404);
    assertEquals(cache.putCalls.length, 0);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend negatively caches 404s with a public TTL", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  // The API stamps a short public TTL on 404s from cached routes (docs/diff for
  // a missing symbol/version), so repeated misses are served from cache instead
  // of hammering the origin.
  const restore = setupFetchStub(
    new Response("Not Found", {
      status: 404,
      headers: {
        "Cache-Control": "public, max-age=30, s-maxage=60",
      },
    }),
  );

  try {
    const request = new Request("https://jsr.io/api/missing", {
      method: "GET",
    });
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(response.status, 404);
    assertEquals(cache.putCalls.length, 1);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend does not cache 404s with no-store", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  // Authenticated 404s carry `private, no-store` and must never be stored.
  const restore = setupFetchStub(
    new Response("Not Found", {
      status: 404,
      headers: { "Cache-Control": "private, no-store" },
    }),
  );

  try {
    const request = new Request("https://jsr.io/api/missing", {
      method: "GET",
    });
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(response.status, 404);
    assertEquals(cache.putCalls.length, 0);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend serves HEAD from cached GET response", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  let fetchCount = 0;
  const original = globalThis.fetch;
  (globalThis as any).fetch = (
    _input: RequestInfo | URL,
    _init?: RequestInit,
  ) => {
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
    await proxyToBackend(getReq, BACKEND_URL);
    assertEquals(fetchCount, 1);

    // HEAD should be served from cache without hitting origin
    const headReq = new Request("https://jsr.io/api/packages", {
      method: "HEAD",
    });
    const headRes = await proxyToBackend(headReq, BACKEND_URL);
    assertEquals(fetchCount, 1); // No additional fetch
    assertEquals(headRes.status, 200);
    assertEquals(headRes.body, null);
  } finally {
    globalThis.fetch = original;
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend skips cache for login paths", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response("", { status: 302, headers: { Location: "/callback" } }),
  );

  try {
    const request = new Request("https://jsr.io/login/callback?code=abc", {
      method: "GET",
    });
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(cache.putCalls.length, 0);
    assertEquals(cache.matchCalls.length, 0);
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend skips cache for login paths even with pathRewrite", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  const restore = setupFetchStub(
    new Response("", { status: 302, headers: { Location: "/callback" } }),
  );

  try {
    // Simulates api.jsr.io/login where pathRewrite prepends /api
    const request = new Request("https://api.jsr.io/login", {
      method: "GET",
    });
    const response = await proxyToBackend(
      request,
      BACKEND_URL,
      (path) => `/api${path}`,
    );

    assertEquals(cache.putCalls.length, 0);
    assertEquals(cache.matchCalls.length, 0);
    assertEquals(response.headers.get("Cache-Control"), "private, no-store");
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

const DOCS_URL = "https://jsr.io/api/scopes/std/packages/x/versions/1.0.0/docs";

Deno.test("proxyToBackend caches an identity-independent response for an authenticated caller", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  // The API marks docs/diff (viewer-independent) responses as shared and keeps
  // them `public` even when authenticated.
  const restore = setupFetchStub(
    new Response('{"docs":true}', {
      status: 200,
      headers: {
        "Cache-Control": "public, max-age=30, s-maxage=300",
        "x-jsr-cache-shared": "1",
      },
    }),
  );

  try {
    const request = new Request(DOCS_URL, {
      method: "GET",
      headers: { "Authorization": "Bearer token123" },
    });
    const response = await proxyToBackend(request, BACKEND_URL);

    assertEquals(response.status, 200);
    // Stored despite the request being authenticated.
    assertEquals(cache.putCalls.length, 1);
    // Public Cache-Control preserved; not auth-varying; internal marker stripped.
    assertEquals(
      response.headers.get("Cache-Control"),
      "public, max-age=30, s-maxage=300",
    );
    assertEquals(response.headers.get("Vary"), "Accept-Encoding");
    assertEquals(response.headers.get("x-jsr-cache-shared"), null);
  } finally {
    restore();
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend serves an authenticated caller from a cached shared entry", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  let fetchCount = 0;
  const original = globalThis.fetch;
  (globalThis as any).fetch = () => {
    fetchCount++;
    return Promise.resolve(
      new Response('{"docs":true}', {
        status: 200,
        headers: {
          "Cache-Control": "public, max-age=30, s-maxage=300",
          "x-jsr-cache-shared": "1",
        },
      }),
    );
  };

  try {
    // Anonymous request fills the shared cache.
    await proxyToBackend(new Request(DOCS_URL, { method: "GET" }), BACKEND_URL);
    assertEquals(fetchCount, 1);

    // Authenticated request is served from that shared entry — no origin hit.
    const res = await proxyToBackend(
      new Request(DOCS_URL, {
        method: "GET",
        headers: { "Authorization": "Bearer token123" },
      }),
      BACKEND_URL,
    );
    assertEquals(fetchCount, 1);
    assertEquals(res.status, 200);
    assertEquals(res.headers.get("x-jsr-cache-shared"), null);
  } finally {
    globalThis.fetch = original;
    (globalThis as any).caches = { default: undefined };
  }
});

Deno.test("proxyToBackend never serves an authenticated caller a viewer-specific cached entry", async () => {
  const cache = createFakeCache();
  (globalThis as any).caches = { default: cache };

  // Origin returns a per-viewer (private) response to the authenticated caller,
  // and a cacheable anonymous (public, unmarked) response otherwise — modelling
  // a viewer-dependent endpoint like scope `get`.
  let fetchCount = 0;
  const original = globalThis.fetch;
  (globalThis as any).fetch = (input: RequestInfo | URL) => {
    fetchCount++;
    const authed = input instanceof Request &&
      input.headers.has("Authorization");
    return Promise.resolve(
      authed
        ? new Response('{"full":true}', {
          status: 200,
          headers: { "Cache-Control": "private, max-age=300" },
        })
        : new Response('{"partial":true}', {
          status: 200,
          headers: { "Cache-Control": "public, max-age=300" },
        }),
    );
  };

  const url = "https://jsr.io/api/scopes/std";
  try {
    // Anonymous request caches the viewer-dependent (unmarked) entry.
    await proxyToBackend(new Request(url, { method: "GET" }), BACKEND_URL);
    assertEquals(fetchCount, 1);
    assertEquals(cache.putCalls.length, 1);

    // Authenticated request must NOT be served that entry — it goes to origin
    // and gets its own private view, which is never stored.
    const res = await proxyToBackend(
      new Request(url, {
        method: "GET",
        headers: { "Authorization": "Bearer token123" },
      }),
      BACKEND_URL,
    );
    assertEquals(fetchCount, 2);
    assertEquals(await res.text(), '{"full":true}');
    assertEquals(res.headers.get("Cache-Control"), "private, no-store");
    assertEquals(cache.putCalls.length, 1);
  } finally {
    globalThis.fetch = original;
    (globalThis as any).caches = { default: undefined };
  }
});
