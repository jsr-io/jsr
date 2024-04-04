// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Middleware } from "$fresh";
import { deleteCookie, getCookies } from "$std/http/cookie.ts";
import { State } from "../util.ts";
import { API, path } from "../utils/api.ts";
import { FullUser } from "../utils/api_types.ts";
import { Tracer } from "../utils/tracing.ts";

export const API_ROOT = Deno.env.get("API_ROOT") ?? "http://api.jsr.test";

export const tracer = new Tracer();

const tracing: Middleware<State> = async (ctx) => {
  ctx.state.span = tracer.spanForRequest(ctx.req, ctx.destination);
  const url = new URL(ctx.req.url);
  const attributes: Record<string, string | bigint> = {
    "http.url": url.href,
    "http.method": ctx.req.method,
    "http.host": url.host,
  };
  const start = new Date();
  try {
    const resp = await ctx.next();
    resp.headers.set("x-deno-ray", ctx.state.span.traceId);
    attributes["http.status_code"] = BigInt(resp.status);
    return resp;
  } finally {
    const end = new Date();
    ctx.state.span.record(url.pathname, start, end, attributes);
  }
};

const auth: Middleware<State> = async (ctx) => {
  console.log("auth middleware");
  const url = new URL(ctx.req.url);
  const interactive = true ||
    (ctx.destination === "route" || ctx.destination === "notFound") &&
      !(url.pathname === "/gfm.css" || url.pathname === "/_frsh/client.js.map");
  const { token, sudo } = getCookies(ctx.req.headers);
  console.log(token, sudo);
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
          const redirectTarget = `${url.pathname}${url.search}`;
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
          throw userResp;
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
};

export const handler: Middleware<State>[] = auth;
