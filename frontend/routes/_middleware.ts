// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Middleware } from "fresh";
import { deleteCookie, getCookies } from "@std/http/cookie";
import { State } from "../util.ts";
import { API, APIError, path } from "../utils/api.ts";
import { FullUser } from "../utils/api_types.ts";
import { Tracer } from "../utils/tracing.ts";
import { define } from "../util.ts";

export const API_ROOT = process.env.API_ROOT ?? "http://api.jsr.test";

export const tracer = new Tracer();

const tracing = define.middleware(async (ctx) => {
  ctx.state.span = tracer.spanForRequest(ctx.req);
  const attributes: Record<string, string | bigint> = {
    "http.url": ctx.url.href,
    "http.method": ctx.req.method,
    "http.host": ctx.url.host,
  };
  const start = new Date();
  try {
    const resp = await ctx.next();
    resp.headers.set("x-deno-ray", ctx.state.span.traceId);
    attributes["http.status_code"] = BigInt(resp.status);
    return resp;
  } finally {
    const end = new Date();
    ctx.state.span.record(ctx.url.pathname, start, end, attributes, "SERVER");
  }
});

const auth = define.middleware(async (ctx) => {
  const pathname = ctx.url.pathname;
  const interactive = !pathname.startsWith("/_fresh") &&
    !pathname.startsWith("/api") &&
    !ctx.url.searchParams.has("__frsh_c");
  const { token, sudo } = getCookies(ctx.req.headers);
  if (interactive) {
    ctx.state.sudo = sudo === "1";
    ctx.state.api = new API(API_ROOT, {
      token,
      sudo: ctx.state.sudo,
      span: ctx.state.span,
    });
    if (ctx.state.api.hasToken()) {
      ctx.state.userPromise = (async () => {
        const userResp = await ctx.state.api.get<FullUser>(path`/user`);
        if (userResp.ok) {
          return userResp.data;
        } else if (!userResp.ok && userResp.code === "invalidBearerToken") {
          // The token is invalid, so delete it.
          ctx.state.api = new API(API_ROOT, {
            span: ctx.state.span,
            token: null,
          });
          const redirectTarget = `${ctx.url.pathname}${ctx.url.search}`;
          const loginUrl = `/login?redirect=${
            encodeURIComponent(redirectTarget)
          }`;
          const resp = new Response("Re-authenticating, token expired", {
            status: 303,
            headers: { Location: loginUrl },
          });
          deleteCookie(resp.headers, "token", { path: "/" });
          return resp;
        } else {
          throw new APIError(userResp);
        }
      })();
      ctx.state.userPromise.catch(() => {}); // don't trigger unhandled rejection
    } else {
      ctx.state.userPromise = Promise.resolve(null);
    }
    Object.defineProperty(ctx.state, "user", {
      get() {
        throw new Error(
          "'ctx.state.user' may only be used during rendering - use ctx.state.userPromise to get the user object in handlers.",
        );
      },
      configurable: true,
    });
  }
  return await ctx.next();
});

const cache = define.middleware(async (ctx) => {
  const resp = await ctx.next();
  if (ctx.state.api && !ctx.state.api.hasToken() && ctx.state.cacheControl) {
    resp.headers.set("cache-control", ctx.state.cacheControl);
  }
  return resp;
});

// Content-Security-Policy applied to interactive (HTML) responses.
//
// Package documentation embeds HTML rendered from package-controlled symbol
// names. deno_doc does not escape every name, so a malicious package can inject
// markup such as `<img src=x onerror=...>` that is rendered same-origin on
// jsr.io. `script-src-attr 'none'` blocks all inline event-handler attributes
// (onerror/onclick/...), which is the execution vector for that payload. This
// is the right tool for a Preact/Fresh app: event handlers are attached from
// JS via addEventListener, never as inline HTML attributes, and legitimate
// inline <script> elements (dark-mode bootstrap, island hydration) are governed
// by script-src, not script-src-attr, so they keep working.
const CSP = [
  "script-src-attr 'none'",
  "object-src 'none'",
  "base-uri 'self'",
  "frame-ancestors 'self'",
].join("; ");

const securityHeaders = define.middleware(async (ctx) => {
  const resp = await ctx.next();
  const contentType = resp.headers.get("content-type") ?? "";
  if (contentType.includes("text/html")) {
    resp.headers.set("content-security-policy", CSP);
    resp.headers.set("x-content-type-options", "nosniff");
    resp.headers.set("x-frame-options", "SAMEORIGIN");
  }
  return resp;
});

export const handler: Middleware<State>[] = [
  tracing,
  auth,
  cache,
  securityHeaders,
];
