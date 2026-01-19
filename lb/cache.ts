// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import type { Backend } from "./main.ts";
import type { ProxyCb } from "./proxy.ts";

export interface CacheOptions {
  maxTTL: number;
  serveStale: number;
  bypassOnAuth: boolean;
  segmentByTokenCookie: boolean;
}

export const CACHE_CONFIG: Record<Backend, CacheOptions> = {
  api: {
    maxTTL: 31536000, // 1 year
    serveStale: 600, // 10 minutes
    bypassOnAuth: true,
    segmentByTokenCookie: true,
  },
  frontend: {
    maxTTL: 31536000, // 1 year
    serveStale: 0,
    bypassOnAuth: false,
    segmentByTokenCookie: true,
  },
  modules: {
    maxTTL: 31536000, // 1 year
    serveStale: 0,
    bypassOnAuth: false,
    segmentByTokenCookie: false,
  },
  npm: {
    maxTTL: 31536000, // 1 year
    serveStale: 0,
    bypassOnAuth: false,
    segmentByTokenCookie: false,
  },
};

function parseCacheControl(cacheControl: string | null): {
  maxAge: number;
  sMaxAge: number;
  noCache: boolean;
  noStore: boolean;
  mustRevalidate: boolean;
} {
  if (!cacheControl) {
    return {
      maxAge: 0,
      sMaxAge: 0,
      noCache: false,
      noStore: false,
      mustRevalidate: false,
    };
  }

  const directives = cacheControl.split(",").map((d) => d.trim().toLowerCase());
  const result = {
    maxAge: 0,
    sMaxAge: 0,
    noCache: directives.includes("no-cache"),
    noStore: directives.includes("no-store"),
    mustRevalidate: directives.includes("must-revalidate"),
  };

  for (const directive of directives) {
    if (directive.startsWith("max-age=")) {
      result.maxAge = parseInt(directive.split("=")[1], 10) || 0;
    } else if (directive.startsWith("s-maxage=")) {
      result.sMaxAge = parseInt(directive.split("=")[1], 10) || 0;
    }
  }

  return result;
}

function shouldCache(
  cacheControl: string | null,
): boolean {
  if (!cacheControl) {
    return false;
  }

  const parsed = parseCacheControl(cacheControl);

  if (parsed.noStore || parsed.noCache) {
    return false;
  }

  if (parsed.sMaxAge > 0) {
    return true;
  } else if (parsed.maxAge > 0) {
    return true;
  } else {
    return false;
  }
}

function getEffectiveTTL(
  cacheControl: string | null,
  options: CacheOptions,
): number {
  const parsed = parseCacheControl(cacheControl);

  let ttl = parsed.sMaxAge > 0 ? parsed.sMaxAge : parsed.maxAge;

  ttl = Math.min(ttl, options.maxTTL);

  return ttl;
}

function createCacheKey(request: Request, options: CacheOptions): Request {
  const url = new URL(request.url);

  if (options.segmentByTokenCookie) {
    const cookieHeader = request.headers.get("Cookie");
    if (cookieHeader) {
      const cookies = cookieHeader.split(";").map((c) => c.trim());
      const targetCookie = cookies.find((c) => c.startsWith("token="));
      if (targetCookie) {
        url.searchParams.set("__cache_segment", targetCookie);
      }
    }
  }

  return new Request(url, {
    method: "GET",
    headers: request.headers,
  });
}

export async function handleWithCache(
  request: Request,
  cb: ProxyCb,
  backend: Backend,
): Promise<{ response: Response; cacheStatus: string }> {
  const options = CACHE_CONFIG[backend];

  if (options.bypassOnAuth && request.headers.has("Authorization")) {
    const response = await cb();
    return { response, cacheStatus: "BYPASS" };
  }

  const cache = caches.default;
  const cacheKey = createCacheKey(request, options);
  const cachedResponse = await cache.match(cacheKey);

  if (cachedResponse) {
    const cacheDate = cachedResponse.headers.get("Date");
    const cacheControl = cachedResponse.headers.get("Cache-Control");
    const age = cacheDate
      ? (Date.now() - new Date(cacheDate).getTime()) / 1000
      : 0;
    const ttl = getEffectiveTTL(cacheControl, options);

    if (age > ttl && options.serveStale && age <= (ttl + options.serveStale)) {
      const response = new Response(cachedResponse.body, cachedResponse);
      response.headers.set("X-Cache-Status", "STALE");

      cb().then((freshResponse) => {
        if (shouldCache(freshResponse.headers.get("Cache-Control"))) {
          const ttl = getEffectiveTTL(
            freshResponse.headers.get("Cache-Control"),
            options,
          );
          const cacheResponse = freshResponse.clone();
          const headers = new Headers(cacheResponse.headers);
          headers.set("Cache-Control", `public, max-age=${ttl}`);
          cache.put(
            cacheKey,
            new Response(cacheResponse.body, {
              status: cacheResponse.status,
              headers,
            }),
          );
        }
      });

      return { response, cacheStatus: "STALE" };
    } else if (age <= ttl) {
      const response = new Response(cachedResponse.body, cachedResponse);
      response.headers.set("X-Cache-Status", "HIT");
      return { response, cacheStatus: "HIT" };
    }
  }

  const response = await cb();
  const cacheControl = response.headers.get("Cache-Control");
  if (shouldCache(cacheControl)) {
    const ttl = getEffectiveTTL(cacheControl, options);

    if (ttl > 0) {
      const cacheResponse = response.clone();
      const headers = new Headers(cacheResponse.headers);
      headers.set("Cache-Control", `public, max-age=${ttl}`);
      headers.set("Date", new Date().toUTCString());

      const cacheableResponse = new Response(cacheResponse.body, {
        status: cacheResponse.status,
        statusText: cacheResponse.statusText,
        headers,
      });

      cache.put(cacheKey, cacheableResponse);
    }
  }

  const finalResponse = new Response(response.body, response);
  finalResponse.headers.set("X-Cache-Status", "MISS");
  return { response: finalResponse, cacheStatus: "MISS" };
}
