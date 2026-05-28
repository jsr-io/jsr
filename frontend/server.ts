// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Cloudflare Workers entrypoint. Stashes the ASSETS binding on
// globalThis where `utils/assets.ts` can reach it from inside Fresh
// routes, and patches `fetch` so requests to the api/npm subdomains
// go through the LB worker via a service binding — Cloudflare bypasses
// `workers_route` for same-zone subrequests from a Worker, so a plain
// `fetch("https://api.<domain>/...")` would skip the LB and try to
// reach the origin (Google Cloud LB) directly, where TLS fails (525).
import server from "./_fresh/server.js";

type Fetcher = { fetch(req: Request | string): Promise<Response> };
interface WorkerEnv {
  ASSETS: Fetcher;
  LB: Fetcher;
  API_ROOT: string;
}

export default {
  fetch(request: Request, env: WorkerEnv) {
    (globalThis as { __JSR_FRONTEND_ASSETS?: Fetcher })
      .__JSR_FRONTEND_ASSETS = env.ASSETS;

    // Route subrequests to our own zone (api.*, npm.*) through the
    // LB worker via the service binding. Anything else (deno.com, etc.)
    // uses normal fetch. The set of hosts we tunnel comes from the
    // API_ROOT / NPM hosts — derived once per isolate.
    const tunneled = tunneledHosts(env);
    const originalFetch = globalThis.fetch;
    if (!(originalFetch as { __jsr_patched?: boolean }).__jsr_patched) {
      const patched: typeof fetch = (input, init) => {
        const url = input instanceof Request
          ? input.url
          : input instanceof URL
          ? input.href
          : String(input);
        const host = safeHostname(url);
        if (host && tunneled.has(host)) {
          return env.LB.fetch(new Request(input as Request | string, init));
        }
        return originalFetch(input as Request | string | URL, init);
      };
      (patched as { __jsr_patched?: boolean }).__jsr_patched = true;
      globalThis.fetch = patched;
    }

    return server.fetch(request);
  },
};

function tunneledHosts(env: WorkerEnv): Set<string> {
  const set = new Set<string>();
  const apiHost = safeHostname(env.API_ROOT);
  if (apiHost) {
    set.add(apiHost);
    // npm.<root>.* mirrors the same registration shape — derive from
    // api.<root>.* by swapping the subdomain.
    set.add(apiHost.replace(/^api\./, "npm."));
  }
  return set;
}

function safeHostname(s: string): string | undefined {
  try {
    return new URL(s).hostname.toLowerCase();
  } catch {
    return undefined;
  }
}
