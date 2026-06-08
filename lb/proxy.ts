// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import type { PartialBucket } from "./types.ts";

// Minimal slice of Cloudflare's `ExecutionContext` we depend on. Cache writes
// (`Cache.put`) finish *after* the response is returned to the client; without
// registering them via `waitUntil`, the Workers runtime tears the invocation
// down first and silently drops the write — so nothing ever gets cached. See
// https://developers.cloudflare.com/workers/runtime-apis/cache/#put
export interface ExecutionCtx {
  waitUntil(promise: Promise<unknown>): void;
}

// Persist a cache write so it survives past the response. With an execution
// context we hand it to `waitUntil`; without one (unit tests) we await it so
// the write still completes deterministically.
async function persistCacheWrite(
  ctx: ExecutionCtx | undefined,
  write: Promise<unknown>,
): Promise<void> {
  if (ctx) {
    ctx.waitUntil(write);
  } else {
    await write;
  }
}

// Proxies an inbound request to a backend. The backend can be either an
// HTTP URL (Cloud Run API) or a service-binding Fetcher (frontend Worker).
// In both cases the caller receives the same cache + header semantics.
export async function proxyToBackend(
  request: Request,
  backend: string | { fetch: (req: Request) => Promise<Response> },
  pathRewrite?: (path: string) => string,
  ctx?: ExecutionCtx,
): Promise<Response> {
  const url = new URL(request.url);
  let path = url.pathname;
  if (pathRewrite) {
    path = pathRewrite(path);
  }

  const isUrlBackend = typeof backend === "string";
  const backendRequestUrl = isUrlBackend
    ? new URL(path + url.search, backend)
    : new URL(path + url.search, url.origin);

  const headers = new Headers(request.headers);
  if (isUrlBackend) {
    headers.set("Host", new URL(backend).host);
  }

  const clientIP = request.headers.get("CF-Connecting-IP");
  if (clientIP) {
    const existingForwarded = headers.get("X-Forwarded-For");
    headers.set(
      "X-Forwarded-For",
      existingForwarded ? `${existingForwarded}, ${clientIP}` : clientIP,
    );
  }

  headers.set("X-Forwarded-Proto", url.protocol.slice(0, -1));
  headers.set("X-Forwarded-Host", url.host);

  const originalPath = url.pathname;
  const ignoreCache = originalPath === "/login" ||
    originalPath.startsWith("/login/") ||
    originalPath === "/logout" ||
    request.headers.has("Authorization") ||
    request.headers.get("Cookie")?.includes("token=");

  const shouldCache = !ignoreCache &&
    (request.method === "GET" || request.method === "HEAD");

  try {
    const fetcher = isUrlBackend
      ? (req: Request) => fetch(req)
      : (req: Request) => backend.fetch(req);
    const response = await cachedFetch(
      shouldCache,
      fetcher,
      backendRequestUrl,
      {
        method: request.method,
        headers,
        body: request.body,
        redirect: "manual",
      },
      ctx,
    );

    const res = new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    });

    res.headers.set("Vary", "Cookie, Authorization");

    if (!shouldCache) {
      res.headers.set("Cache-Control", "private, no-store");
    }

    return res;
  } catch (error) {
    console.error("Backend proxy error:", error);
    return new Response("Bad Gateway", {
      status: 502,
      headers: {
        "Content-Type": "text/plain",
      },
    });
  }
}

export async function proxyToR2(
  request: Request,
  bucket: PartialBucket,
  pathRewrite?: (path: string) => string,
  ctx?: ExecutionCtx,
): Promise<Response> {
  const url = new URL(request.url);
  let path = url.pathname;
  if (pathRewrite) {
    path = pathRewrite(path);
  }
  const key = decodeURIComponent(path.slice(1));

  const cacheKey = new Request(request.url, { method: "GET" });
  const cached = await caches.default?.match(cacheKey);
  if (cached) {
    if (request.method === "HEAD") {
      return new Response(null, {
        headers: cached.headers,
        status: cached.status,
      });
    }
    // Re-wrap: responses from `caches.default.match` have immutable headers,
    // and callers (e.g. setSecurityHeaders) mutate the returned response's
    // headers — mutating the cached response directly throws.
    return new Response(cached.body, {
      headers: cached.headers,
      status: cached.status,
    });
  }

  try {
    if (request.method === "HEAD") {
      const object = await bucket.head(key);
      if (!object) {
        return new Response(null, { status: 404 });
      }
      const headers = new Headers();
      object.writeHttpMetadata(headers);
      headers.set("etag", object.httpEtag);
      headers.set("content-length", object.size.toString());
      return new Response(null, { headers });
    } else {
      const object = await bucket.get(key, {
        onlyIf: request.headers,
      });

      if (!object) {
        return new Response("404 - Not Found", { status: 404 });
      }

      const headers = new Headers();
      object.writeHttpMetadata(headers);
      headers.set("etag", object.httpEtag);
      headers.set("content-length", object.size.toString());

      if (!("body" in object)) {
        return new Response(null, { status: 304, headers });
      }

      const response = new Response(object.body, { headers });
      const cache = caches.default;
      if (cache) {
        await persistCacheWrite(ctx, cache.put(cacheKey, response.clone()));
      }
      return response;
    }
  } catch (error) {
    console.error("R2 proxy error:", error);
    return new Response("Bad Gateway", {
      status: 502,
      headers: { "Content-Type": "text/plain" },
    });
  }
}

async function cachedFetch(
  shouldCache: boolean,
  fetcher: (req: Request) => Promise<Response>,
  input: RequestInfo | URL,
  init?: RequestInit,
  ctx?: ExecutionCtx,
): Promise<Response> {
  const req = new Request(input, init);

  if (shouldCache) {
    const cacheKey = new Request(req.url, { method: "GET" });
    const cached = await caches.default?.match(cacheKey);
    if (cached) {
      if (req.method === "HEAD") {
        return new Response(null, {
          headers: cached.headers,
          status: cached.status,
        });
      }
      return cached;
    }
  }

  const res = await fetcher(req);

  const cache = caches.default;
  if (cache && shouldCache && req.method === "GET" && res.ok) {
    const cacheControl = res.headers.get("Cache-Control") ?? "";
    if (
      !cacheControl.includes("private") &&
      !cacheControl.includes("no-store")
    ) {
      const cacheKey = new Request(req.url, { method: "GET" });
      // `waitUntil` (or await in tests) so the write isn't dropped when the
      // invocation ends — the cause of the lb caching nothing in production.
      await persistCacheWrite(ctx, cache.put(cacheKey, res.clone()));
    }
  }

  return res;
}
