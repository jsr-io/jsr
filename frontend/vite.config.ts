// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { defineConfig, type Plugin } from "vite";
import { fresh } from "@fresh/plugin-vite";
import tailwindcss from "@tailwindcss/vite";
import { CSS } from "@deno/gfm";
import { copy, ensureDir, walk } from "jsr:@std/fs@^1";
import { dirname, join, relative } from "jsr:@std/path@^1";

const MARKER =
  "/*! During the build process, the @deno/gfm CSS file is injected here. */";

function gfmCss(): Plugin {
  return {
    name: "gfm-css",
    enforce: "pre",
    transform(code, id) {
      if (!/\/gfm\.css(?:$|\?)/.test(id)) return null;
      if (!code.includes(MARKER)) return null;
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

// `workers-og`'s static `.wasm` imports trip Vite's wasm-fallback
// during SSR build. Mark every `.wasm` resolution external so the
// literal `import x from "./foo.wasm"` passes through Vite untouched,
// remember where the original file is so esbuild can pick it up later.
function wasmExternal(): Plugin & { wasmSources: Map<string, string> } {
  const wasmSources = new Map<string, string>();
  return {
    name: "jsr-wasm-external",
    enforce: "pre",
    wasmSources,
    async resolveId(id, importer) {
      if (!id.endsWith(".wasm")) return;
      if (importer) {
        const abs = join(dirname(importer), id);
        try {
          await Deno.stat(abs);
          wasmSources.set(id, abs);
        } catch { /* leave for later resolution */ }
      }
      return { id, external: true };
    },
  };
}

// Bundles `server.ts` (the Cloudflare Worker entry that wraps Fresh's
// `_fresh/server.js`) into a single ESM file at `_fresh/worker.js`,
// plus copies the workers-og `.wasm` files into `_fresh/server/` next
// to the bundled output. The terraform `cloudflare_worker_version`
// resource uploads worker.js + the two .wasm parts as one multi-module
// worker version.
function workerBundle(
  wasm: { wasmSources: Map<string, string> },
): Plugin {
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
        return; // Fresh's SSR env hasn't emitted server.js yet
      }
      done = true;

      // Place .wasm files in `_fresh/server/` so esbuild's resolver
      // finds them at the relative paths server-entry.mjs imports.
      await ensureDir("_fresh/server");
      for (const [rel, src] of wasm.wasmSources) {
        const filename = rel.replace(/^\.\//, "");
        await copy(src, join("_fresh/server", filename), {
          overwrite: true,
        });
      }

      const esbuild = await import("esbuild");
      await esbuild.build({
        entryPoints: ["./server.ts"],
        bundle: true,
        format: "esm",
        platform: "neutral",
        target: "esnext",
        outfile: "_fresh/worker.js",
        // node: imports are provided by the Workers nodejs_compat
        // runtime — keep external.
        external: ["node:*", "*.wasm"],
        // apexcharts (pulled in via the DownloadChart island) has a
        // top-level `window.TreemapSquared = {}` write; aliasing
        // window to globalThis makes that a no-op in the worker.
        banner: { js: "globalThis.window ??= globalThis;" },
        logLevel: "info",
      });
      await esbuild.stop();
    },
  };
}

const wasm = wasmExternal();

export default defineConfig({
  server: {
    port: 8000,
  },
  plugins: [
    fresh(),
    wasm,
    gfmCss(),
    tailwindcss(),
    copyDocs(),
    workerBundle(wasm),
  ],
});
