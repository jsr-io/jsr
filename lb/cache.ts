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
    const response = new Response(cachedResponse.body, cachedResponse);
    return { response, cacheStatus: "HIT" };
  } else {
    const response = await cb();
    cache.put(cacheKey, response.clone());
    const finalResponse = new Response(response.body, response);
    return { response: finalResponse, cacheStatus: "MISS" };
  }
}
