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
// arbitrary markup that is rendered same-origin on jsr.io. Script execution is
// blocked with a nonce-based `script-src`:
//
// - Fresh generates a fresh nonce per render, stamps it onto every
//   Preact-rendered <script>/<style> element (dark-mode bootstrap, island
//   hydration, particles.js), and exposes it on the Response via
//   `Symbol.for("__freshNonce")`. Injected markup goes through
//   dangerouslySetInnerHTML as a raw string, so it never receives the nonce.
// - `'strict-dynamic'` lets those nonce'd scripts load further scripts
//   (dynamic island imports, the Turnstile loader created by LoginForm via
//   createElement), while parser-inserted injected <script> tags stay blocked.
// - `'self'` is deliberately NOT listed: raw package files are served on this
//   origin (jsr.io/@scope/pkg/...), so allowlisting the origin would let an
//   injected <script src="/@evil/pkg/1.0.0/x.js"> execute attacker-published
//   code. CSP3 browsers ignore host sources when 'strict-dynamic' is present,
//   but it must not be a fallback for older ones either.
// - A `javascript:` iframe executes in a context that inherits this policy and
//   is gated by script-src (not frame-src — the navigation is not a fetch), so
//   the nonce requirement blocks `<iframe src="javascript:...">` as well.
//
// Cloudflare fronts jsr.io and injects its own inline scripts (challenge
// platform / bot management) into HTML responses. Cloudflare propagates the
// nonce from this header onto those scripts, but that must be confirmed in
// production after deploy — a strict script-src blocks any it misses.
//
// `script-src-attr 'none'` blocks all inline event-handler attributes
// (onerror/onclick/...): event handlers in a Preact/Fresh app are attached
// from JS via addEventListener, never as inline HTML attributes.
//
// `frame-src` restricts frames to Cloudflare Turnstile (the only legitimate
// iframe, on the login page), blocking injected frames pointed at other
// origins or `data:` URLs.
const BASE_CSP = [
  "script-src-attr 'none'",
  "object-src 'none'",
  "base-uri 'self'",
  "frame-ancestors 'self'",
  "frame-src https://challenges.cloudflare.com",
];

// Set by Fresh on responses produced by ctx.render(); see
// fresh/src/middlewares/csp.ts (NONCE_SYMBOL). Not part of the public export
// map, but registered in the global symbol registry.
const NONCE_SYMBOL = Symbol.for("__freshNonce");

const securityHeaders = define.middleware(async (ctx) => {
  const resp = await ctx.next();
  const contentType = resp.headers.get("content-type") ?? "";
  if (contentType.includes("text/html")) {
    const nonce = (resp as unknown as Record<symbol, string | undefined>)[
      NONCE_SYMBOL
    ];
    // Every interactive HTML page is produced by ctx.render() and thus
    // carries a nonce; fail closed on any HTML response that is not.
    const scriptSrc = nonce
      ? `script-src 'nonce-${nonce}' 'strict-dynamic'`
      : "script-src 'none'";
    const csp = [...BASE_CSP, scriptSrc];
    resp.headers.set("content-security-policy", csp.join("; "));
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
