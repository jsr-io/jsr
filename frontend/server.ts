// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Cloudflare Workers entrypoint. Workers only pass `env` to this fetch
// handler, not into Fresh's route handlers — we publish the bindings
// the frontend code needs via `utils/worker_env.ts`, then delegate to
// the Fresh app. See that module for why this is a globalThis hop
// rather than a regular module-level setter.
import server from "./_fresh/server.js";
import { setWorkerEnv } from "./utils/worker_env.ts";

interface Fetcher {
  fetch: typeof fetch;
}

export default {
  fetch(request: Request, env: { ASSETS: Fetcher; LB: Fetcher }) {
    setWorkerEnv({ assets: env.ASSETS, lb: env.LB });
    return server.fetch(request);
  },
};
