// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Cloudflare Workers entrypoint. Workers only pass `env` to this fetch
// handler, not into Fresh's route handlers — so we stash the ASSETS
// binding on globalThis where `utils/assets.ts` can reach it from
// inside routes (og.ts, docs/[...id].tsx), then delegate to Fresh.
import server from "./_fresh/server.js";

type AssetsFetcher = { fetch(req: Request | string): Promise<Response> };

export default {
  fetch(request: Request, env: { ASSETS: AssetsFetcher }) {
    (globalThis as { __JSR_FRONTEND_ASSETS?: AssetsFetcher })
      .__JSR_FRONTEND_ASSETS = env.ASSETS;
    return server.fetch(request);
  },
};
