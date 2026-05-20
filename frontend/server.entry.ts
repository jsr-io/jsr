// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Cloudflare Workers entrypoint for the JSR frontend.
//
// The Fresh build emits `_fresh/server.js`, which expects `Deno.*` APIs at
// import time. We install a minimal Deno shim first, stash the worker env
// on globalThis so lazy `env(...)` reads find it, then hand off to Fresh.

import "./shim/deno.ts";
import server from "./_fresh/server.js";

export interface FrontendEnv {
  FRONTEND_ROOT?: string;
  API_ROOT?: string;
  NO_COLOR?: string;
  OTLP_ENDPOINT?: string;

  ORAMA_PACKAGES_PUBLIC_API_KEY?: string;
  ORAMA_PACKAGES_PROJECT_ID?: string;
  ORAMA_SYMBOLS_PUBLIC_API_KEY?: string;
  ORAMA_SYMBOLS_PROJECT_ID?: string;
  ORAMA_DOCS_PUBLIC_API_KEY?: string;
  ORAMA_DOCS_PROJECT_ID?: string;

  PROD_PROXY?: string;

  ASSETS: { fetch: (req: Request | string) => Promise<Response> };
}

// deno-lint-ignore no-explicit-any
const g = globalThis as any;

export default {
  fetch(
    request: Request,
    env: FrontendEnv,
    ctx: {
      waitUntil(p: Promise<unknown>): void;
      passThroughOnException(): void;
    },
  ): Response | Promise<Response> {
    // Stash the env binding once per isolate. Re-assigning each request is
    // cheap and keeps the binding fresh if the runtime swaps it.
    g.__JSR_FRONTEND_ENV = env as unknown as Record<string, string | undefined>;
    g.__JSR_FRONTEND_ASSETS = env.ASSETS;
    g.__JSR_WORKER_CTX = ctx;
    // deno-lint-ignore no-explicit-any
    return (server as any).fetch(request);
  },
};
