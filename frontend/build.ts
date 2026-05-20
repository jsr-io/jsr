#!/usr/bin/env -S deno run -A
// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//
// Wraps the Fresh build so the output directory is also a valid Cloudflare
// Workers assets root. We:
//   1. Run the Fresh build (dev.ts build).
//   2. Mirror frontend/static/* into _fresh/static/* so the same URL paths
//      hit either Fresh's generated files or the original tree.
//   3. Mirror frontend/docs/*.md into _fresh/static/_jsr_docs/<id>.md so the
//      docs route's `Deno.readTextFile` calls can be satisfied by the
//      Workers ASSETS binding (see frontend/shim/deno.ts).

import { walk } from "jsr:@std/fs@^1/walk";
import { ensureDir } from "jsr:@std/fs@^1/ensure-dir";
import { copy } from "jsr:@std/fs@^1/copy";
import { dirname, join, relative } from "jsr:@std/path@^1";

const here = new URL(".", import.meta.url).pathname;
const fresh = join(here, "_fresh");
const freshStatic = join(fresh, "static");

async function freshBuild() {
  const cmd = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", join(here, "dev.ts"), "build"],
    stdout: "inherit",
    stderr: "inherit",
  });
  const { code } = await cmd.output();
  if (code !== 0) Deno.exit(code);
}

async function mirrorStatic() {
  const src = join(here, "static");
  for await (const entry of walk(src, { includeDirs: false })) {
    const rel = relative(src, entry.path);
    const dest = join(freshStatic, rel);
    await ensureDir(dirname(dest));
    await copy(entry.path, dest, { overwrite: true });
  }
}

async function mirrorDocs() {
  const src = join(here, "docs");
  const dest = join(freshStatic, "_jsr_docs");
  for await (const entry of walk(src, { includeDirs: false, exts: [".md"] })) {
    const rel = relative(src, entry.path);
    const out = join(dest, rel);
    await ensureDir(dirname(out));
    await copy(entry.path, out, { overwrite: true });
  }
}

await freshBuild();
await mirrorStatic();
await mirrorDocs();
console.log("Workers build ready at", relative(here, freshStatic));
