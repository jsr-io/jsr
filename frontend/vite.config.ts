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

// `workers-og`'s static `.wasm` imports trip Vite's wasm-fallback during
// SSR build. Mark every `.wasm` resolution external so the literal
// `import x from "./foo.wasm"` passes through Vite untouched, then copy
// the real files into `_fresh/server/` (next to server-entry.mjs)
// where wrangler looks for them at deploy time.
function wasmExternal(): Plugin {
  const wasmSources = new Map<string, string>();
  return {
    name: "jsr-wasm-external",
    enforce: "pre",
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
    async closeBundle() {
      // Only run after Fresh's SSR build has emitted server-entry.mjs.
      try {
        await Deno.stat("_fresh/server");
      } catch {
        return;
      }
      // Each Fresh route chunk (in `_fresh/server/assets/`) and the
      // main `_fresh/server/server-entry.mjs` independently emit the
      // `import "./foo.wasm"` line, so wrangler needs to find the
      // .wasm next to each importer. Copy into every dir under
      // `_fresh/server/` that holds a .mjs file.
      const dests = new Set<string>();
      for await (
        const entry of walk("_fresh/server", { exts: [".mjs"] })
      ) {
        dests.add(dirname(entry.path));
      }
      for (const [rel, src] of wasmSources) {
        const filename = rel.replace(/^\.\//, "");
        for (const d of dests) {
          await copy(src, join(d, filename), { overwrite: true });
        }
      }
    },
  };
}

export default defineConfig({
  server: {
    port: 8000,
  },
  plugins: [
    fresh(),
    wasmExternal(),
    gfmCss(),
    tailwindcss(),
    copyDocs(),
  ],
});
