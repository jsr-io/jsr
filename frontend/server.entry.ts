// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Cloudflare Workers entrypoint for the JSR frontend.
//
// Fresh's bundled server defensively references `Deno.stat`, `Deno.open`,
// and `Deno.readTextFileSync`. They only fire from Fresh's
// `ProdBuildCache.readFile` (when its `staticFiles` Map has an entry) and
// from its dev-only `getCodeFrame` — Workers Assets serves the matching
// paths upstream before the worker runs, so in practice these never
// execute. But the bundle still needs `Deno` to be a defined identifier;
// we install a tiny stub that throws `NotFound` so those defensive paths
// degrade to "no static file" if they ever do fire.

const g = globalThis as {
  Deno?: unknown;
  __JSR_FRONTEND_ASSETS?: unknown;
};

if (typeof g.Deno === "undefined") {
  const notFound = () =>
    Object.assign(new Error("file system unavailable in Workers"), {
      name: "NotFound",
    });
  g.Deno = {
    stat: () => {
      throw notFound();
    },
    open: () => {
      throw notFound();
    },
    readTextFileSync: () => {
      throw notFound();
    },
    build: { os: "linux", arch: "x86_64" },
  };
}

import server from "./_fresh/server.js";

export interface FrontendEnv {
  ASSETS: { fetch: (req: Request | string) => Promise<Response> };
}

export default {
  fetch(
    request: Request,
    env: FrontendEnv,
    _ctx: {
      waitUntil(p: Promise<unknown>): void;
      passThroughOnException(): void;
    },
  ): Response | Promise<Response> {
    g.__JSR_FRONTEND_ASSETS = env.ASSETS;
    // deno-lint-ignore no-explicit-any
    return (server as any).fetch(request);
  },
};
