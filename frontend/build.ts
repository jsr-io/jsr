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

async function run(args: string[]) {
  const cmd = new Deno.Command(Deno.execPath(), {
    args,
    stdout: "inherit",
    stderr: "inherit",
  });
  const { code } = await cmd.output();
  if (code !== 0) Deno.exit(code);
}

async function freshBuild() {
  await run(["run", "-A", join(here, "dev.ts"), "build"]);
}

async function bundleWorker() {
  await run([
    "bundle",
    "--platform=browser",
    "-o",
    join(fresh, "worker.js"),
    join(here, "server.entry.ts"),
  ]);
  await patchBuildIdTopLevelAwait();
  await patchTopLevelAwait();
}

// @fresh/build-id's module init calls `await crypto.subtle.digest(...)` at
// the top level, which workerd rejects ("Disallowed operation called within
// global scope"). The real BUILD_ID is overwritten later by `setBuildId()`
// in _fresh/server.js, so the top-level computation is wasted — strip it.
async function patchBuildIdTopLevelAwait() {
  const file = join(fresh, "worker.js");
  let src = await Deno.readTextFile(file);
  const re =
    /async "deno:https:\/\/jsr\.io\/@fresh\/build-id\/[^"]+"\(\) \{\s*init_hex\(\);[\s\S]*?BUILD_ID = encodeHex\(buildIdHash\);\s*\}/;
  const placeholder =
    `"deno:https://jsr.io/@fresh/build-id/stub"() {\n    init_hex();\n    DENO_DEPLOYMENT_ID = void 0;\n    deploymentId = "ws";\n    BUILD_ID = "0000000000000000000000000000000000000000";\n  }`;
  if (!re.test(src)) {
    console.warn(
      "[build] build-id top-level await pattern not found; bundle may already be patched or Fresh has changed shape",
    );
    return;
  }
  src = src.replace(re, placeholder);
  await Deno.writeTextFile(file, src);
}

// workerd rejects top-level await unless it settles during startup, but the
// bundle's eagerly-initialised route modules call async `init_*()` functions
// that ultimately need the ASSETS binding — which isn't available until the
// fetch handler runs.  Wrap the entire top-level init section in a lazy
// async function that runs once on the first request.
async function patchTopLevelAwait() {
  const file = join(fresh, "worker.js");
  const src = await Deno.readTextFile(file);
  const lines = src.split("\n");

  // 1. Find the first true top-level `await` (column-0, not inside __esm).
  let firstTLA = -1;
  for (let i = 0; i < lines.length; i++) {
    if (/^await\s/.test(lines[i])) {
      firstTLA = i;
      break;
    }
  }
  if (firstTLA === -1) {
    console.warn("[build] no top-level await found; skipping TLA patch");
    return;
  }

  // 2. Find the `export {` line (last one).
  let exportLine = -1;
  for (let i = lines.length - 1; i >= 0; i--) {
    if (/^export\s*\{/.test(lines[i])) {
      exportLine = i;
      break;
    }
  }
  if (exportLine === -1) return;

  // 3. The init section spans [firstTLA, exportLine).
  const initSection = lines.slice(firstTLA, exportLine);

  // 4. Collect var names declared in the init section and strip `var `.
  const varNames: string[] = [];
  const patchedInit = initSection.map((line) => {
    const m = line.match(/^var\s+(\w+)/);
    if (m) {
      varNames.push(m[1]);
      return line.replace(/^var\s+/, "");
    }
    return line;
  });

  // 5. Build the output.
  const prefix = lines.slice(0, firstTLA).join("\n");
  const suffix = lines.slice(exportLine).join("\n");

  const preDecl = varNames.length > 0
    ? "var " + varNames.join(", ") + ";"
    : "";

  const initFn = [
    "var __initPromise;",
    "async function __initApp() {",
    ...patchedInit,
    "}",
  ].join("\n");

  // Replace the original export with a lazy-init fetch wrapper.  The
  // original server_entry_default.fetch stashed env on globalThis then
  // called server_default.fetch — we do the same after ensuring __initApp
  // has run.
  const newExport = [
    "var __g = globalThis;",
    "export default {",
    "  async fetch(request, env, ctx) {",
    "    __g.__JSR_FRONTEND_ENV = env;",
    "    __g.__JSR_FRONTEND_ASSETS = env.ASSETS;",
    "    __g.__JSR_WORKER_CTX = ctx;",
    "    if (!__initPromise) __initPromise = __initApp();",
    "    await __initPromise;",
    "    return server_default.fetch(request);",
    "  }",
    "};",
  ].join("\n");

  const result = [prefix, "", preDecl, "", initFn, "", newExport, ""].join(
    "\n",
  );
  await Deno.writeTextFile(file, result);
  console.log(
    `[build] patched ${varNames.length} var declarations, ` +
      `deferred TLA init (lines ${firstTLA + 1}–${exportLine})`,
  );
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
await bundleWorker();
console.log("Workers build ready at", relative(here, freshStatic));
