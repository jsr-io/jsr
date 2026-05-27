// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Polyfills the subset of the `Deno` global needed by the frontend at
// runtime so the Fresh build output can run inside the Cloudflare Workers
// runtime. Only filesystem and env reads are shimmed — anything else
// genuinely requiring Deno (subprocess, openKv, ...) is not used at runtime
// by this app.
//
// The shim is installed by importing this module from `server.entry.ts`
// before any other Fresh code runs. It is a no-op when the real `Deno`
// global is already present (i.e. `deno task dev`).

import { assets } from "../utils/env.ts";

// deno-lint-ignore no-explicit-any
const g = globalThis as any;

function assetPath(p: string | URL): string {
  if (p instanceof URL) {
    if (p.protocol === "file:") {
      // file:///…/frontend/docs/<id>.md → /_jsr_docs/<id>.md
      // (build copies the docs tree to _fresh/assets/_jsr_docs/)
      const docsMatch = p.pathname.match(/\/docs\/(.+\.md)$/);
      if (docsMatch) return "/_jsr_docs/" + docsMatch[1];
      // file:///…/frontend/static/<path> → /<path>
      const staticMatch = p.pathname.match(/\/static\/(.+)$/);
      if (staticMatch) return "/" + staticMatch[1];
    }
    return p.pathname;
  }
  let s = String(p).replace(/^\.\//, "/");
  if (!s.startsWith("/")) s = "/" + s;
  // ./static/foo -> /foo  (the assets binding root holds the merged
  // _fresh/{static,client} + frontend/static tree at URL-space layout)
  s = s.replace(/^\/static\//, "/");
  // The Fresh ProdBuildCache reads files at paths like
  // `<root>/_fresh/client/assets/X.js` or `<root>/_fresh/static/styles.css`
  // — strip the `_fresh/{client,static}/` prefix so they hit the merged
  // assets binding.
  s = s.replace(/^.*?\/_fresh\/(?:client|static)\//, "/");
  return s;
}

function notFound(p: string | URL): Error {
  const err = new Error(`No such file or directory: ${String(p)}`);
  (err as { name: string }).name = "NotFound";
  return err;
}

async function readAsset(path: string | URL): Promise<Response> {
  const bind = assets();
  if (!bind) {
    throw new Error(
      `Workers ASSETS binding is not available; cannot read ${String(path)}`,
    );
  }
  const url = "https://assets.invalid" + assetPath(path);
  const resp = await bind.fetch(url);
  if (resp.status === 404) throw notFound(url);
  if (!resp.ok) {
    throw new Error(`Failed to read asset ${url}: ${resp.status}`);
  }
  return resp;
}

if (typeof g.Deno === "undefined") {
  g.Deno = {
    env: {
      get(name: string): string | undefined {
        return g.__JSR_FRONTEND_ENV?.[name];
      },
      has(name: string): boolean {
        return g.__JSR_FRONTEND_ENV?.[name] !== undefined;
      },
      toObject(): Record<string, string> {
        return { ...(g.__JSR_FRONTEND_ENV ?? {}) };
      },
    },
    async readFile(path: string | URL): Promise<Uint8Array> {
      const resp = await readAsset(path);
      return new Uint8Array(await resp.arrayBuffer());
    },
    async readTextFile(path: string | URL): Promise<string> {
      const resp = await readAsset(path);
      return await resp.text();
    },
    async stat(
      path: string | URL,
    ): Promise<{ size: number; isFile: boolean }> {
      const bind = assets();
      if (!bind) throw notFound(path);
      const resp = await bind.fetch(
        new Request("https://assets.invalid" + assetPath(path), {
          method: "HEAD",
        }),
      );
      if (!resp.ok) throw notFound(path);
      const len = resp.headers.get("content-length");
      return { size: len ? parseInt(len, 10) : 0, isFile: true };
    },
    async open(path: string | URL): Promise<{
      readable: ReadableStream<Uint8Array>;
      close(): void;
    }> {
      const resp = await readAsset(path);
      return { readable: resp.body!, close() {} };
    },
    args: [] as string[],
    build: { os: "linux", arch: "x86_64" } as { os: string; arch: string },
    inspect(value: unknown): string {
      try {
        return JSON.stringify(value, (_k, v) => {
          if (v instanceof Error) {
            return { name: v.name, message: v.message, stack: v.stack };
          }
          return v;
        }, 2) ?? String(value);
      } catch {
        return String(value);
      }
    },
  };
}
