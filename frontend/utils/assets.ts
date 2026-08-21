// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Reads a file from the frontend's asset tree. In Cloudflare Workers we
// fetch through the `ASSETS` binding (published by `server.ts` via
// `utils/worker_env.ts`); in local Deno dev — where no ASSETS binding
// exists — we fall back to reading from the source tree via
// `node:fs/promises`, which works in both Deno (node compat) and Node.

import { readFile } from "node:fs/promises";
import { workerAssets } from "./worker_env.ts";

// Maps an asset path (URL-space) to its on-disk location relative to the
// frontend root. `/_jsr_docs/<id>.md` → `docs/<id>.md`; everything else
// lives under `static/`.
function diskUrl(path: string): URL {
  const rel = path.startsWith("/_jsr_docs/")
    ? "docs/" + path.slice("/_jsr_docs/".length)
    : "static/" + path.replace(/^\//, "");
  return new URL("../" + rel, import.meta.url);
}

export async function readAsset(path: string): Promise<Uint8Array> {
  const bind = workerAssets();
  if (bind) {
    const resp = await bind.fetch("https://assets.invalid" + path);
    if (!resp.ok) {
      throw Object.assign(
        new Error(`Asset not found: ${path} (${resp.status})`),
        { name: "NotFound" },
      );
    }
    return new Uint8Array(await resp.arrayBuffer());
  }
  return new Uint8Array(await readFile(diskUrl(path)));
}

export async function readAssetText(path: string): Promise<string> {
  return new TextDecoder().decode(await readAsset(path));
}
