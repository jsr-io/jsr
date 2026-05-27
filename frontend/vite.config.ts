// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { defineConfig, type Plugin } from "vite";
import { fresh } from "@fresh/plugin-vite";
import tailwindcss from "@tailwindcss/vite";
import { CSS } from "@deno/gfm";
import { copy, ensureDir, walk } from "jsr:@std/fs@^1";
import { dirname, join, relative } from "jsr:@std/path@^1";

const MARKER =
  "/*! During the build process, the @deno/gfm CSS file is injected here. */";

function imagescriptUrl(): Plugin {
  return {
    name: "imagescript-url",
    enforce: "pre",
    transform(code, id) {
      const m = id.match(/jsr\.io\/(@matmen\/imagescript\/[^?]+)/);
      if (!m || !code.includes("import.meta.url")) return null;
      const realUrl = `https://jsr.io/${m[1]}`;
      return {
        code: code.replaceAll("import.meta.url", JSON.stringify(realUrl)),
        map: null,
      };
    },
  };
}

function gfmCss(): Plugin {
  return {
    name: "gfm-css",
    enforce: "pre",
    transform(code, id) {
      if (!/\/gfm\.css(?:$|\?)/.test(id)) return null;
      if (!code.includes(MARKER)) return null; // cheap guard
      const injected = CSS.replaceAll("font-size:16px;", "");
      return { code: code.replace(MARKER, injected), map: null };
    },
  };
}

// Mirror `frontend/docs/*.md` into the assets output so the docs route can
// read them via the Workers ASSETS binding at runtime.
function copyDocs(): Plugin {
  return {
    name: "jsr-copy-docs",
    async closeBundle() {
      const src = "docs";
      const dest = "_fresh/client/_jsr_docs";
      for await (const entry of walk(src, { exts: [".md"] })) {
        const out = join(dest, relative(src, entry.path));
        await ensureDir(dirname(out));
        await copy(entry.path, out, { overwrite: true });
      }
    },
  };
}

// Bundles `server.ts` (the Cloudflare Worker entry that imports the Fresh
// `_fresh/server.js`) into a single ESM file at `_fresh/worker.js`. Runs
// after Fresh's SSR environment writes `_fresh/server.js`.
//
// We use esbuild rather than rollup here: when rollup re-bundles Fresh's
// pre-chunked SSR output, it re-orders the route modules ahead of the
// util module that exports `define`, producing a TDZ error at startup
// (`Cannot access 'define$1' before initialization`). esbuild wraps each
// module in a lazy `__esm` initializer so the order doesn't matter.
function workerBundle(): Plugin {
  let isBuild = false;
  let done = false;
  return {
    name: "jsr-worker-bundle",
    enforce: "post",
    configResolved(config) {
      isBuild = config.command === "build";
    },
    async closeBundle() {
      if (!isBuild || done) return;
      try {
        await Deno.stat("_fresh/server.js");
      } catch {
        return; // Fresh's SSR env hasn't written server.js yet
      }
      done = true;
      const esbuild = await import("esbuild");
      await esbuild.build({
        entryPoints: ["./server.ts"],
        bundle: true,
        format: "esm",
        platform: "neutral",
        target: "esnext",
        outfile: "_fresh/worker.js",
        // node: imports are provided by the runtime (Workers
        // nodejs_compat or Deno's node compat) — keep external.
        external: ["node:*"],
        // apexcharts (pulled into Fresh's SSR bundle via the
        // DownloadChart island) has a top-level
        // `window.TreemapSquared = {}` write. Workers don't have a
        // `window` global — point it at globalThis so the assignment is
        // a no-op and module init succeeds. The chart-rendering code
        // that reads `window.X` only runs in the browser, so the
        // polyfill never has to do real work.
        banner: { js: "globalThis.window ??= globalThis;" },
        logLevel: "info",
      });
      await esbuild.stop();
      await deferServerInit("_fresh/worker.js");
    },
  };
}

// Cloudflare workerd rejects async I/O (fetch, setTimeout, getRandomValues)
// at module init. esbuild's bundle has a top-level
// `await init_server_entry()` that pulls Fresh's entire server module
// — including imagescript's WASM `fetch()` calls — into startup.
// Rewrite the trailing 4 lines of the bundle to defer that init to the
// first request (inside the fetch handler, where async I/O is allowed).
async function deferServerInit(file: string) {
  let src = await Deno.readTextFile(file);
  const pattern =
    /await init_server_entry\(\);\s*var (\w+) = \{\s*fetch: (\w+)\.fetch\s*\};/;
  const m = src.match(pattern);
  if (!m) {
    throw new Error(`[workerBundle] could not find init pattern in ${file}`);
  }
  const [, serverDefault, entry] = m;
  const replacement = [
    "var __jsrInitPromise;",
    `var ${serverDefault} = {`,
    "  async fetch(req) {",
    "    if (!__jsrInitPromise) __jsrInitPromise = init_server_entry();",
    "    await __jsrInitPromise;",
    `    return ${entry}.fetch(req);`,
    "  }",
    "};",
  ].join("\n");
  src = src.replace(pattern, replacement);
  await Deno.writeTextFile(file, src);
}

export default defineConfig({
  server: {
    port: 8000,
  },
  plugins: [
    fresh(),
    gfmCss(),
    imagescriptUrl(),
    tailwindcss(),
    copyDocs(),
    workerBundle(),
  ],
});
