// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import type { WorkerEnv } from "./types.ts";
import { proxyToCloudRun, proxyToGCS } from "./proxy.ts";
import {
  handleCORSPreflight,
  isCORSPreflight,
  setCORSHeaders,
  setDebugHeaders,
  setSecurityHeaders,
} from "./headers.ts";
import { isBot } from "./bots.ts";
import { trackJSRDownload, trackNPMDownload } from "./analytics.ts";

export type Backend = "api" | "frontend" | "modules" | "npm";
const MODULES = "modules";
const FRONTEND = "frontend";
const API = "api";
const NPM = "npm";

export default {
  async fetch(
    request: Request,
    env: WorkerEnv,
  ): Promise<Response> {
    try {
      const response = await route(request, env);
      return response;
    } catch (error) {
      console.error("LB error:", error);

      return new Response("Internal Server Error", {
        status: 500,
        headers: {
          "Content-Type": "text/plain",
        },
      });
    }
  },
};

export async function route(
  request: Request,
  env: WorkerEnv,
): Promise<Response> {
  const url = new URL(request.url);
  const hostname = url.hostname.toLowerCase();

  if (hostname === env.API_DOMAIN) {
    return await handleAPIRequest(request, env);
  } else if (hostname === env.NPM_DOMAIN) {
    return await handleNPMRequest(request, env);
  } else if (hostname === env.ROOT_DOMAIN) {
    return await handleRootRequest(request, env);
  } else {
    return new Response(`Unknown hostname: ${hostname}`, {
      status: 404,
      headers: {
        "Content-Type": "text/plain",
      },
    });
  }
}

export async function handleAPIRequest(
  request: Request,
  env: WorkerEnv,
  rewritePath: boolean = true,
): Promise<Response> {
  if (isCORSPreflight(request)) {
    return handleCORSPreflight(API);
  }

  const response = await proxyToCloudRun(
    request,
    env.REGISTRY_API_URL,
    rewritePath ? (path) => `/api${path}` : undefined,
  );

  setSecurityHeaders(response, API);
  setCORSHeaders(response, API);
  setDebugHeaders(response, {
    backend: API,
  });

  return response;
}

export async function handleNPMRequest(
  request: Request,
  env: WorkerEnv,
): Promise<Response> {
  if (isCORSPreflight(request)) {
    return handleCORSPreflight(NPM);
  }

  const url = new URL(request.url);
  const response = await proxyToGCS(
    request,
    env.GCS_ENDPOINT,
    env.NPM_BUCKET,
    (path) => {
      if (path === "/" || path === "/-/ping") {
        return "/root.json";
      }
      return path;
    },
  );

  setSecurityHeaders(response, NPM);
  setCORSHeaders(response, NPM);
  setDebugHeaders(response, {
    backend: NPM,
  });

  if ((response.ok || response.status === 304) && request.method === "GET") {
    trackNPMDownload(url.pathname, env);
  }

  return response;
}

/**
 * By default, requests to jsr.io are proxied to the frontend hosted on Cloud
 * Run.
 *
 * GET or HEAD requests to jsr.io/@* are routed to the modules bucket if they
 * do no have an 'Accept' header that starts with 'text/html' and either:
 *  - they do not have a 'Sec-Fetch-Dest' header or the value is 'empty'
 *  - they have a 'Sec-Fetch-Dest' header with value 'image' or 'video' and
 *    a 'Sec-Fetch-Site' with value 'same-origin'
 *
 * Additionally, any requests originating from the Googlebot user agent are
 * punched through to the frontend service, never to the modules bucket.
 *
 * These restrictions are in place to prevent users from accessing hosted files
 * in navigation requests, while allowing access to them (even cross-site) when
 * using `fetch`. We disallow loading resources directly from `<img>` and
 * `<video>` tags (unless they are same-origin, to allow rendering them in
 * markdown previews), to prevent hotlinking.
 *
 * Since jsr.io URLs appear in stack traces, and every character counts, we've
 * introduced this complexity and a potential security risk to avoid the extra
 * two characters. This is instead of a simpler, more secure setup using a
 * subdomain like "p.jsr.io", which would map directly onto a bucket.
 *
 * As an additional security mitigation, we add the strictest possible CSP
 * header to all responses served from the modules bucket. This is done in the
 * backend bucket configuration.
 *
 * WARNING: Exercise extreme caution when modifying this. Untrusted files are
 * stored under the /@ prefix. It's crucial that the browser never loads these
 * untrusted files.
 */
export async function handleRootRequest(
  request: Request,
  env: WorkerEnv,
): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname;

  if (isAPIRoute(path)) {
    return await handleAPIRequest(request, env, false);
  } else if (isBot(request)) {
    return await handleFrontendRoute(request, env, true);
  } else if (path.startsWith("/@")) {
    if (!canAccessModuleFile(request)) {
      return await handleFrontendRoute(request, env, false);
    } else {
      return await handleModuleFileRoute(request, env);
    }
  } else {
    return await handleFrontendRoute(request, env, false);
  }
}

export function canAccessModuleFile(request: Request): boolean {
  const method = request.method;
  const accept = request.headers.get("Accept");
  const secFetchDest = request.headers.get("Sec-Fetch-Dest");

  if (
    (method === "HEAD" || method === "GET") && !accept?.startsWith("text/html")
  ) {
    if (!secFetchDest || secFetchDest === "empty") {
      return true;
    } else if (
      (secFetchDest === "image" || secFetchDest === "video") &&
      request.headers.get("Sec-Fetch-Site") === "same-origin"
    ) {
      return true;
    } else {
      return false;
    }
  } else {
    return false;
  }
}

function isAPIRoute(path: string): boolean {
  return (
    path.startsWith("/api/") ||
    path === "/sitemap.xml" ||
    path === "/sitemap-scopes.xml" ||
    path === "/sitemap-packages.xml" ||
    path === "/login" ||
    path.startsWith("/login/") ||
    path === "/logout"
  );
}

async function handleFrontendRoute(
  request: Request,
  env: WorkerEnv,
  isBot: boolean,
): Promise<Response> {
  const response = await proxyToCloudRun(request, env.REGISTRY_FRONTEND_URL);

  setSecurityHeaders(response, FRONTEND);
  setDebugHeaders(response, {
    backend: FRONTEND,
    isBot,
  });

  return response;
}

async function handleModuleFileRoute(
  request: Request,
  env: WorkerEnv,
): Promise<Response> {
  const url = new URL(request.url);
  const response = await proxyToGCS(
    request,
    env.GCS_ENDPOINT,
    env.MODULES_BUCKET,
  );

  setSecurityHeaders(response, MODULES);
  setCORSHeaders(response, MODULES);
  setDebugHeaders(response, {
    backend: MODULES,
  });

  if ((response.ok || response.status === 304) && request.method === "GET") {
    trackJSRDownload(url.pathname, env);
  }

  return response;
}
