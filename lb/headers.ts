// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import type { Backend } from "./main.ts";

const SECURITY_HEADERS: Record<Backend, Record<string, string>> = {
  modules: {
    "Content-Security-Policy":
      "default-src 'none'; script-src 'none'; style-src 'none'; img-src 'none'; font-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; frame-ancestors 'none'; sandbox; form-action 'none';",
    "X-Robots-Tag": "noindex",
    "X-Content-Type-Options": "nosniff",
    "Cross-Origin-Resource-Policy": "cross-origin",
  },
  npm: {
    "Content-Security-Policy":
      "default-src 'none'; script-src 'none'; style-src 'none'; img-src 'none'; font-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; frame-ancestors 'none'; sandbox; form-action 'none';",
    "X-Robots-Tag": "noindex",
    "X-Content-Type-Options": "nosniff",
    "Cross-Origin-Resource-Policy": "cross-origin",
  },
  api: {
    "X-Robots-Tag": "noindex",
    "X-Content-Type-Options": "nosniff",
    "Cross-Origin-Resource-Policy": "cross-origin",
  },
  frontend: {
    "X-Content-Type-Options": "nosniff",
  },
};

type BackendForCors = Exclude<Backend, "frontend">;
interface CORSConfig {
  allowOrigins: string;
  allowMethods: string[];
  allowHeaders: string[];
  exposeHeaders: string;
  maxAge: number;
}
const CORS_CONFIG: Record<BackendForCors, CORSConfig> = {
  api: {
    allowOrigins: "*",
    allowMethods: ["HEAD", "GET", "POST", "PUT", "PATCH", "DELETE"],
    allowHeaders: ["Authorization", "X-Cloud-Trace-Context", "Content-Type"],
    exposeHeaders: "*",
    maxAge: 3600,
  },
  npm: {
    allowOrigins: "*",
    allowMethods: ["HEAD", "GET"],
    allowHeaders: [
      "Authorization",
      "X-Cloud-Trace-Context",
      "npm-command",
      "npm-scope",
      "npm-session",
      "user-agent",
    ],
    exposeHeaders: "*",
    maxAge: 3600,
  },
  modules: {
    allowOrigins: "*",
    allowMethods: ["HEAD", "GET"],
    allowHeaders: ["Authorization", "X-Cloud-Trace-Context"],
    exposeHeaders: "*",
    maxAge: 3600,
  },
};

export function setSecurityHeaders(
  response: Response,
  backend: "api" | "frontend" | "modules" | "npm",
) {
  const securityHeaders = SECURITY_HEADERS[backend];

  for (const [key, value] of Object.entries(securityHeaders)) {
    response.headers.set(key, value);
  }

  if (!response.headers.has("Access-Control-Allow-Origin")) {
    response.headers.set("Access-Control-Allow-Origin", "*");
  }
  if (!response.headers.has("Access-Control-Expose-Headers")) {
    response.headers.set("Access-Control-Expose-Headers", "*");
  }
}

export function setDebugHeaders(
  response: Response,
  metadata: {
    backend: Backend;
    cacheStatus: string;
    isBot?: boolean;
    version?: string;
  },
) {
  response.headers.set("X-JSR-Backend", metadata.backend);
  response.headers.set("X-JSR-Cache-Status", metadata.cacheStatus);
  if (metadata.isBot !== undefined) {
    response.headers.set(
      "X-JSR-Bot-Detected",
      metadata.isBot ? "true" : "false",
    );
  }
  if (metadata.version) {
    response.headers.set("X-JSR-Worker-Version", metadata.version);
  }
}

export function handleCORSPreflight(
  backend: BackendForCors,
): Response {
  const config = CORS_CONFIG[backend];

  return new Response(null, {
    status: 204,
    headers: {
      "Access-Control-Allow-Origin": config.allowOrigins,
      "Access-Control-Allow-Methods": config.allowMethods.join(", "),
      "Access-Control-Allow-Headers": config.allowHeaders.join(", "),
      "Access-Control-Max-Age": config.maxAge.toString(),
      "Vary": "Origin",
    },
  });
}

export function setCORSHeaders(
  response: Response,
  backend: BackendForCors,
) {
  const config = CORS_CONFIG[backend];

  response.headers.set("Access-Control-Allow-Origin", config.allowOrigins);
  response.headers.set("Access-Control-Expose-Headers", config.exposeHeaders);

  const existingVary = response.headers.get("Vary");
  if (existingVary) {
    if (!existingVary.includes("Origin")) {
      response.headers.set("Vary", `${existingVary}, Origin`);
    }
  } else {
    response.headers.set("Vary", "Origin");
  }
}

export function isCORSPreflight(request: Request): boolean {
  return (
    request.method === "OPTIONS" &&
    request.headers.has("Origin") &&
    request.headers.has("Access-Control-Request-Method")
  );
}
