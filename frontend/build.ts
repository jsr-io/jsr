#!/usr/bin/env -S deno run -A
// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//
// Wraps the vite Fresh build so the output is also a deployable Cloudflare
// Worker plus a Workers Assets directory. We:
//   1. Run `vite build` (produces _fresh/server.js + _fresh/{client,static}/).
//   2. Build a merged `_fresh/assets/` tree that mirrors the URL-space layout
//      Fresh expects (client/static merged, plus frontend/static/ and
//      frontend/docs/*.md under /_jsr_docs/).
//   3. `deno bundle` `server.entry.ts` → `_fresh/worker.js`.

import { walk } from "jsr:@std/fs@^1/walk";
import { ensureDir } from "jsr:@std/fs@^1/ensure-dir";
import { copy } from "jsr:@std/fs@^1/copy";
import { dirname, join, relative } from "jsr:@std/path@^1";

const here = new URL(".", import.meta.url).pathname;
const fresh = join(here, "_fresh");
const assetsDir = join(fresh, "assets");

async function run(args: string[]) {
  const cmd = new Deno.Command(Deno.execPath(), {
    args,
    stdout: "inherit",
    stderr: "inherit",
    cwd: here,
  });
  const { code } = await cmd.output();
  if (code !== 0) Deno.exit(code);
}

async function viteBuild() {
  await run(["task", "build:fresh"]);
}

async function bundleWorker() {
  await run([
    "bundle",
    "--platform=browser",
    "-o",
    join(fresh, "worker.js"),
    join(here, "server.entry.ts"),
  ]);
}

async function mirror(src: string, dest: string, exts?: string[]) {
  for await (
    const entry of walk(src, { includeDirs: false, exts })
  ) {
    const rel = relative(src, entry.path);
    const out = join(dest, rel);
    await ensureDir(dirname(out));
    await copy(entry.path, out, { overwrite: true });
  }
}

await viteBuild();
await ensureDir(assetsDir);
// Order matters: Fresh-generated CSS (`_fresh/static/`) and the original
// `frontend/static/` tree (mirrored to `_fresh/client/`) overlap on paths
// like `/styles.css`; the Fresh-generated ones must win, so they go last.
await mirror(join(here, "static"), assetsDir);
await mirror(join(fresh, "client"), assetsDir);
await mirror(join(fresh, "static"), assetsDir);
await mirror(join(here, "docs"), join(assetsDir, "_jsr_docs"), [".md"]);
await bundleWorker();
console.log("Workers build ready at", relative(here, fresh));
