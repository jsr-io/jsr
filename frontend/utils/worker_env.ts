// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Cloudflare Workers env bindings, injected by `server.ts` (the worker
// entry) at isolate boot and read by utilities running inside Fresh
// route handlers. `server.ts` and the Fresh server bundle
// (`_fresh/server.js`) are produced by two separate bundlers (esbuild
// and vite respectively), so module-level state set in one bundle
// isn't visible from the other — globalThis is the only truly shared
// scope. Keeping all of the globalThis casts in this one file isolates
// the side-channel rather than scattering it across consumers.

interface Fetcher {
  fetch: typeof fetch;
}

const KEY = "__jsr_worker_env";

interface WorkerEnv {
  assets?: Fetcher;
  lb?: Fetcher;
}

function slot(): { [k: string]: WorkerEnv | undefined } {
  return globalThis as unknown as { [k: string]: WorkerEnv | undefined };
}

export function setWorkerEnv(env: WorkerEnv): void {
  slot()[KEY] = env;
}

export function workerAssets(): Fetcher | undefined {
  return slot()[KEY]?.assets;
}

export function workerLb(): Fetcher | undefined {
  return slot()[KEY]?.lb;
}
