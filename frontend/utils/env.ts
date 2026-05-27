// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Cloudflare Workers binds env per-request (via the fetch handler), so
// reading environment variables at module-init time returns nothing. This
// helper defers the lookup until call time — by which point the worker
// entry has stashed the binding on globalThis.
//
// During `deno serve`/`deno task dev` local execution there is no
// globalThis binding and we fall back to `Deno.env.get`.

declare global {
  // eslint-disable-next-line no-var
  var __JSR_FRONTEND_ENV: Record<string, string | undefined> | undefined;
  // eslint-disable-next-line no-var
  var __JSR_FRONTEND_ASSETS: {
    fetch: (req: Request | string) => Promise<Response>;
  } | undefined;
}

export function env(name: string): string | undefined {
  const fromWorker = globalThis.__JSR_FRONTEND_ENV?.[name];
  if (fromWorker !== undefined) return fromWorker;
  if (typeof Deno !== "undefined" && Deno.env?.get) {
    return Deno.env.get(name);
  }
  return undefined;
}

export function assets(): {
  fetch: (req: Request | string) => Promise<Response>;
} | undefined {
  return globalThis.__JSR_FRONTEND_ASSETS;
}
