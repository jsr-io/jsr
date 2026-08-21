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
// the write still completes deterministically. A failed write (sync throw or
// async rejection) is logged and swallowed — caching is best-effort and must
// never break serving the response.
async function persistCacheWrite(
  ctx: ExecutionCtx | undefined,
  cache: Cache,
  key: Request,
  response: Response,
): Promise<void> {
  let write: Promise<unknown>;
  try {
    write = cache.put(key, response);
  } catch (error) {
    console.error("cache write failed:", error);
    return;
  }
  const guarded = write.catch((error) => {
    console.error("cache write failed:", error);
  });
  if (ctx) {
    ctx.waitUntil(guarded);
  } else {
    await guarded;
  }
}

// Cache key for a bucket (R2) response. `caches.default` is shared across all
// backends, and a `/@scope/...` URL is served as EITHER a module file (bucket,
// JSON) or an HTML page (frontend) depending on request headers — keying both
// on the raw URL cross-serves HTML for module files (and vice versa). Bucket
// entries are namespaced under a synthetic, non-routable host (which no real
// request can ever target, so it can't be poisoned) keyed by the original host
// + path so module and npm buckets also stay distinct.
function bucketCacheKey(rawUrl: string): Request {
  const u = new URL(rawUrl);
  return new Request(
    `https://bucket-cache.jsr.internal/${u.host}${u.pathname}${u.search}`,
    { method: "GET" },
  );
}

// Response header the API sets on routes whose body does not depend on the
// requesting identity (no permission/member/sudo branch — e.g. docs/diff). It
// lets the lb serve such responses from its shared (URL-keyed) cache even to
// authenticated callers, instead of bypassing the cache whenever auth is
// present. Stripped from client responses in proxyToBackend.
const SHARED_CACHE_HEADER = "x-jsr-cache-shared";

// True when a response may be served from the shared cache to any caller
// regardless of auth. Requires the API's explicit marker AND a `public`
// Cache-Control, so an accidentally-`private` response can never be shared even
// if the marker leaks onto it.
function isIdentityIndependent(res: Response): boolean {
  if (res.headers.get(SHARED_CACHE_HEADER) !== "1") return false;
  return (res.headers.get("Cache-Control") ?? "").includes("public");
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

  // Cache key is the PUBLIC URL (inbound origin + proxied path), never the
  // backend origin. CDN purges target the public jsr.io / api.jsr.io URLs (see
  // `package_api_cache_urls` in the API), so an entry keyed under the Cloud Run
  // backend host could never be evicted on publish — which froze package
  // metadata (`latestVersion`/version counts) for the full, now-30-day TTL. The
  // proxied `path` already matches the purge URLs' path on both hosts, so this
  // makes the existing purges land. For a service-binding (frontend) backend
  // this equals `backendRequestUrl`, so frontend caching is unchanged.
  const cacheKeyUrl = new URL(path + url.search, url.origin).toString();

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
  const isLoginPath = originalPath === "/login" ||
    originalPath.startsWith("/login/") ||
    originalPath === "/logout";
  const isAuthenticated = request.headers.has("Authorization") ||
    (request.headers.get("Cookie")?.includes("token=") ?? false);

  // Caching is allowed for safe methods outside the login flow. Authenticated
  // requests are additionally *restricted to identity-independent responses*
  // (see cachedFetch): they may only read/write cache entries the API marked as
  // viewer-independent (docs/diff), so a viewer-specific response is never
  // shared across users, while those endpoints still get cached for everyone.
  const allowCache = !isLoginPath &&
    (request.method === "GET" || request.method === "HEAD");

  try {
    const fetcher = isUrlBackend
      ? (req: Request) => fetch(req)
      : (req: Request) => backend.fetch(req);
    const response = await cachedFetch(
      allowCache,
      isAuthenticated,
      fetcher,
      backendRequestUrl,
      {
        method: request.method,
        headers,
        body: request.body,
        redirect: "manual",
      },
      ctx,
      cacheKeyUrl,
    );

    const shared = isIdentityIndependent(response);

    const res = new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    });

    // The shared marker is internal to lb↔API; never expose it to clients.
    res.headers.delete(SHARED_CACHE_HEADER);

    // Identity-independent responses don't vary by auth (so downstream may share
    // them); everything else does. Don't clobber a Vary the backend set.
    if (shared) {
      if (!res.headers.has("Vary")) res.headers.set("Vary", "Accept-Encoding");
    } else {
      res.headers.set("Vary", "Cookie, Authorization");
    }

    // Mark uncacheable responses `private, no-store` so nothing downstream
    // stores them: anything not a cacheable safe method (POST, login flow), and
    // any authenticated request to a viewer-specific (non-shared) response.
    // Identity-independent responses keep their public Cache-Control so they
    // stay cacheable for everyone.
    if (!allowCache || (isAuthenticated && !shared)) {
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

  const cacheKey = bucketCacheKey(request.url);
  let cached: Response | undefined;
  try {
    cached = await caches.default?.match(cacheKey);
  } catch (error) {
    // A corrupt/unreadable cache entry must never break the request.
    console.error("R2 cache match error:", error);
  }
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
        await persistCacheWrite(ctx, cache, cacheKey, response.clone());
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
  allowCache: boolean,
  // When true (authenticated request), the cache may only be read from / written
  // to for identity-independent responses, so a viewer-specific response is
  // never served to, or stored by, an authenticated caller.
  restrictToShared: boolean,
  fetcher: (req: Request) => Promise<Response>,
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  ctx: ExecutionCtx | undefined,
  // The PUBLIC URL to key the cache under (see proxyToBackend) — kept separate
  // from the backend fetch URL so CDN purges, which target public URLs, can
  // evict these entries.
  cacheKeyUrl: string,
): Promise<Response> {
  const req = new Request(input, init);
  const cacheKey = new Request(cacheKeyUrl, { method: "GET" });

  if (allowCache) {
    const cached = await caches.default?.match(cacheKey);
    if (cached && (!restrictToShared || isIdentityIndependent(cached))) {
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

  // Only cache responses the origin explicitly marked cacheable: a `max-age` or
  // `s-maxage` directive, and never `private`/`no-store`. This applies to both
  // 200s and (negatively-cached) 404s. Previously an unmarked 200 was cached by
  // default, which silently cached dynamic endpoints that forgot to opt out —
  // e.g. the publish-status poll (`util::json`, no `Cache-Control`), pinning a
  // stale "pending"/"processing" status so `deno publish` hung until the entry
  // expired. Endpoints meant to be cached go through the API's `cache*`
  // wrappers, which always set an explicit `public, max-age=…, s-maxage=…`.
  const cacheControl = res.headers.get("Cache-Control") ?? "";
  const explicitlyUncacheable = cacheControl.includes("private") ||
    cacheControl.includes("no-store");
  const hasCacheableDirective = cacheControl.includes("max-age") ||
    cacheControl.includes("s-maxage");
  const cacheable = (res.ok || res.status === 404) &&
    !explicitlyUncacheable && hasCacheableDirective;
  // An authenticated request may only write an identity-independent response —
  // a viewer-specific authed response must never land in the shared cache.
  const writable = cacheable &&
    (!restrictToShared || isIdentityIndependent(res));
  const cache = caches.default;
  if (cache && allowCache && req.method === "GET" && writable) {
    // `waitUntil` (or await in tests) so the write isn't dropped when the
    // invocation ends — the cause of the lb caching nothing in production.
    await persistCacheWrite(ctx, cache, cacheKey, res.clone());
  }

  return res;
}
