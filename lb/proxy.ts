// Copyright 2024 the JSR authors. All rights reserved. MIT license.

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

  const ignoreCache = path === "/login" ||
    path.startsWith("/login/") ||
    path === "/logout" ||
    request.headers.has("Authorization") ||
    request.headers.get("Cookie")?.includes("token=");

  try {
    const response = await cachedFetch(!ignoreCache, backendRequestUrl, {
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

    res.headers.set("Vary", "Cookie,Authorization");

    if (ignoreCache) {
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

export async function proxyToGCS(
  request: Request,
  bucketEndpoint: string | undefined,
  bucketName: string,
  pathRewrite?: (path: string) => string,
): Promise<Response> {
  const url = new URL(request.url);
  let path = url.pathname;
  if (pathRewrite) {
    path = pathRewrite(path);
  }
  path = path.slice(1);

  const gcsUrl = `${
    bucketEndpoint ?? "https://storage.googleapis.com"
  }/${bucketName}/${path}`;

  const headers = new Headers();

  const ifNoneMatch = request.headers.get("If-None-Match");
  if (ifNoneMatch) {
    headers.set("If-None-Match", ifNoneMatch);
  }

  const ifModifiedSince = request.headers.get("If-Modified-Since");
  if (ifModifiedSince) {
    headers.set("If-Modified-Since", ifModifiedSince);
  }

  const range = request.headers.get("Range");
  if (range) {
    headers.set("Range", range);
  }

  const method = request.method === "HEAD" ? "HEAD" : "GET";

  try {
    const response = await cachedFetch(true, gcsUrl, {
      method,
      headers,
      redirect: "follow",
    });

    return new Response(response.body, {
      headers: response.headers,
      status: response.status,
    });
  } catch (error) {
    console.error("GCS proxy error:", error);
    return new Response("Bad Gateway", {
      status: 502,
      headers: {
        "Content-Type": "text/plain",
      },
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
    const cache = await caches.default?.match(req);
    if (cache) {
      return cache;
    }
  }
  const res = await fetch(req);

  if (shouldCache) {
    caches.default?.put(req, res.clone());
  }

  return res;
}
