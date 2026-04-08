// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import type { PartialBucket } from "./types.ts";

export async function proxyToCloudRun(
  request: Request,
  backendUrl: string,
  pathRewrite?: (path: string) => string,
): Promise<Response> {
  const url = new URL(request.url);
  let path = url.pathname;
  if (pathRewrite) {
    path = pathRewrite(path);
  }

  const backendRequestUrl = new URL(path + url.search, backendUrl);

  const headers = new Headers(request.headers);
  headers.set("Host", new URL(backendUrl).host);

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
    const response = await cachedFetch(shouldCache, backendRequestUrl, {
      method: request.method,
      headers,
      body: request.body,
      redirect: "manual",
    });

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
    console.error("Cloud Run proxy error:", error);
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
    return cached;
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
      caches.default?.put(cacheKey, response.clone());
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
  input: RequestInfo | URL,
  init?: RequestInit,
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

  const res = await fetch(req);

  if (shouldCache && req.method === "GET" && res.ok) {
    const cacheControl = res.headers.get("Cache-Control") ?? "";
    if (
      !cacheControl.includes("private") &&
      !cacheControl.includes("no-store")
    ) {
      const cacheKey = new Request(req.url, { method: "GET" });
      caches.default?.put(cacheKey, res.clone());
    }
  }

  return res;
}
