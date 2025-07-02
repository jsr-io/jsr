#!/usr/bin/env -S deno run -A --watch
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

const FRONTEND_SERVER = "http://localhost:8000";
const API_SERVER = "http://localhost:8001";
const GCS_ENDPOINT = "http://localhost:4080";
const MODULES_BUCKET = "modules";
const NPM_BUCKET = "npm";

const DOMAIN = "jsr.test";
const API_DOMAIN = "api.jsr.test";
const NPM_DOMAIN = "npm.jsr.test";

const PORT = 80;

async function createBucket(name: string) {
  try {
    const resp = await fetch("http://localhost:4080/storage/v1/b", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name }),
    });
    await resp.arrayBuffer();
    return resp.ok || resp.status === 409;
  } catch {
    return false;
  }
}

const bucketCreationInterval = setInterval(async () => {
  let allBucketsCreated = true;
  for (const bucket of [MODULES_BUCKET, "docs", "publishing", NPM_BUCKET]) {
    allBucketsCreated &&= await createBucket(bucket);
  }
  if (allBucketsCreated) {
    console.log("All buckets ready.");
    clearInterval(bucketCreationInterval);
  }
}, 5000);

async function handler(req: Request): Promise<Response> {
  const url = new URL(req.url);

  function redirectRoot() {
    url.hostname = DOMAIN;
    url.port = String(PORT);
    return Response.redirect(url.href, 307);
  }

  switch (url.hostname) {
    case DOMAIN: {
      if (isCDNRequest(req, url)) {
        const file = encodeURIComponent(url.pathname.slice(1));
        const res = await fetch(
          `${GCS_ENDPOINT}/storage/v1/b/${MODULES_BUCKET}/o/${file}?alt=media`,
          { redirect: "manual", method: req.method, headers: req.headers },
        );
        return res;
      }
      if (
        url.pathname.startsWith("/api/") ||
        url.pathname === "/sitemap.xml" ||
        url.pathname === "/sitemap-scopes.xml" ||
        url.pathname === "/sitemap-packages.xml" ||
        url.pathname.startsWith("/login/") ||
        url.pathname.startsWith("/connect/") ||
        url.pathname.startsWith("/disconnect/") ||
        url.pathname === "/logout"
      ) {
        const apiUrl = `${API_SERVER}${url.pathname}${url.search}`;
        const apiRes = await proxy(req, apiUrl);
        return apiRes;
      }
      const frontendUrl = `${FRONTEND_SERVER}${url.pathname}${url.search}`;
      const frontendRes = await proxy(req, frontendUrl);
      return frontendRes;
    }
    case API_DOMAIN: {
      const apiUrl = `${API_SERVER}/api${url.pathname}${url.search}`;
      const apiRes = await proxy(req, apiUrl);
      return apiRes;
    }
    case NPM_DOMAIN: {
      const file = encodeURIComponent(
        decodeURIComponent(url.pathname.slice(1)),
      );
      const res = await fetch(
        `${GCS_ENDPOINT}/storage/v1/b/${NPM_BUCKET}/o/${file}?alt=media`,
        { redirect: "manual" },
      );
      return res;
    }
    default:
      return redirectRoot();
  }
}

export function isCDNRequest(req: Request, url: URL): boolean {
  return (req.method === "HEAD" || req.method === "GET") &&
    url.pathname.startsWith("/@") &&
    !req.headers.get("Accept")?.startsWith("text/html") &&
    (!req.headers.has("Sec-Fetch-Dest") ||
      req.headers.get("Sec-Fetch-Dest") === "empty" ||
      ((req.headers.get("Sec-Fetch-Dest") === "image" ||
        req.headers.get("Sec-Fetch-Dest") === "video") &&
        req.headers.get("Sec-Fetch-Site") === "same-origin"));
}

export async function proxy(req: Request, newUrl: string): Promise<Response> {
  // If the request is WebSocket, proxy it using WebSocket, relaying all
  // messages and closing the connection when one side closes it.
  if (req.headers.get("Upgrade") === "websocket") {
    const { socket, response } = Deno.upgradeWebSocket(req);
    const ws = new WebSocket(newUrl);
    const externalOpen = new Promise<void>((resolve) => {
      ws.onopen = () => resolve();
    });
    const internalOpen = new Promise<void>((resolve) => {
      socket.onopen = () => resolve();
    });

    socket.onmessage = (e) => {
      externalOpen.then(() => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(e.data);
        } else {
          ws.close();
          socket.close();
        }
      });
    };
    ws.onmessage = (e) => {
      internalOpen.then(() => {
        if (socket.readyState === WebSocket.OPEN) {
          socket.send(e.data);
        } else {
          ws.close();
          socket.close();
        }
      });
    };
    ws.onclose = () => socket.close();
    socket.onclose = () => ws.close();

    return response;
  } else {
    // Otherwise, proxy it using fetch.
    return await fetch(newUrl, {
      method: req.method,
      headers: req.headers,
      body: req.body,
      redirect: "manual",
    });
  }
}

if (import.meta.main) {
  Deno.serve({ port: PORT, hostname: "0.0.0.0" }, handler);
}
