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

  const backendRequest = new Request(backendRequestUrl, {
    method: request.method,
    headers,
    body: request.body,
    redirect: "manual",
  });

  try {
    let x;
    let cfOptions: RequestInit["cf"];
    if (
      url.pathname === "/login" || url.pathname.startsWith("/login/") ||
      url.pathname === "/logout"
    ) {
      x = "path";
      cfOptions = undefined;
    } else if (
      request.headers.has("Authorization") ||
      request.headers.get("Cookie")?.includes("token=")
    ) {
      x = "auth";
      cfOptions = {
        //cacheEverything: false,
        //cacheKey: `${backendRequestUrl.toString()}:authed`,
      };
    } else {
      x = "none";
      cfOptions = { cacheEverything: true };
    }

    const response = await fetch(backendRequest, { cf: cfOptions });

    // For auth redirects with Set-Cookie, use an HTML redirect instead of 302.
    // This fixes a browser quirk where SameSite=Lax cookies aren't sent on
    // redirects that are part of a cross-site redirect chain (OAuth flow).
    const isRedirect = response.status >= 300 && response.status < 400;
    const hasSetCookie = response.headers.has("set-cookie");
    const location = response.headers.get("location");

    if (isRedirect && hasSetCookie && location) {
      const html = `<!DOCTYPE html>
<html>
<head>
  <meta http-equiv="refresh" content="0;url=${location}">
  <script>window.location.href = ${JSON.stringify(location)};</script>
</head>
<body>Redirecting...</body>
</html>`;

      const newHeaders = new Headers();
      newHeaders.set("Content-Type", "text/html;charset=UTF-8");
      // Preserve the Set-Cookie header
      response.headers.forEach((value, key) => {
        if (key.toLowerCase() === "set-cookie") {
          newHeaders.append(key, value);
        }
      });

      return new Response(html, {
        status: 200,
        headers: newHeaders,
      });
    }

    // Cloudflare Workers: explicitly build headers to ensure Set-Cookie passes through
    const newHeaders = new Headers();
    response.headers.forEach((value, key) => {
      newHeaders.append(key, value);
    });

    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: newHeaders,
    });
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
    const response = await fetch(gcsUrl, {
      method,
      headers,
      redirect: "follow",
      cf: {
        cacheEverything: true,
      },
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
