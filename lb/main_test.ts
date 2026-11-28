// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { assertEquals } from "@std/assert";
import { isModuleFilePath, route } from "./main.ts";
import type { PartialBucket, WorkerEnv } from "./types.ts";

// The lb keys its bucket cache entries under a reserved path prefix on the
// public origin (see BUCKET_CACHE_PREFIX). Nothing is served from there, and
// `route` must reject such requests before reaching a backend — otherwise a
// crafted request could plant a frontend response under a bucket cache key.
Deno.test("route rejects the reserved bucket-cache namespace", async () => {
  const unreachable = (what: string) => () => {
    throw new Error(`${what} must not be consulted`);
  };
  const bucket = {
    get: unreachable("bucket"),
    head: unreachable("bucket"),
  } as unknown as PartialBucket;
  const env: WorkerEnv = {
    API: { fetch: unreachable("api") } as unknown as Fetcher,
    FRONTEND: { fetch: unreachable("frontend") } as unknown as Fetcher,
    ROOT_DOMAIN: "jsr.io",
    API_DOMAIN: "api.jsr.io",
    NPM_DOMAIN: "npm.jsr.io",
    NPM_BUCKET: bucket,
    MODULES_BUCKET: bucket,
  };

  for (
    const url of [
      "https://jsr.io/__bucket-cache",
      "https://jsr.io/__bucket-cache/@scope/pkg/meta.json",
      "https://npm.jsr.io/__bucket-cache/@jsr/scope__pkg",
      "https://api.jsr.io/__bucket-cache/api/scopes/scope",
    ]
  ) {
    const res = await route(new Request(url), env);
    assertEquals(res.status, 404, url);
  }

  // A path that merely starts with the same characters is a normal route: it
  // still reaches the frontend, which this env makes throw — surfacing as the
  // proxy's 502 rather than the 404 above.
  const res = await route(
    new Request("https://jsr.io/__bucket-cache-not-really"),
    env,
  );
  assertEquals(res.status, 502);
});

Deno.test("the WAF website rules exempt the same module paths as the lb", async () => {
  const tf = await Deno.readTextFile(
    new URL("../terraform/bots.tf", import.meta.url),
  );
  const waf = new RegExp(tf.match(/module_file_path\s*=\s*"([^"]+)"/)![1]);
  for (
    const path of [
      "/@std/assert/meta.json",
      "/@std/assert/1.0.0_meta.json",
      "/@std/assert/1.0.0-beta.1_meta.json",
      "/@std/assert/1.0.0/mod.ts",
      "/@std/assert/1.0.0/src/equal.ts",
      "/@std/assert/1.0.0",
      "/@std/assert",
      "/@std/assert/doc",
      "/@std/assert/doc/~/assertEquals",
      "/@std/assert@1.0.0/doc",
      "/@std/assert/versions",
      "/@std/assert/og",
      "/@std/assert/diff/1.0.0...1.0.1",
      "/@std",
      "/packages",
    ]
  ) {
    assertEquals(waf.test(path), isModuleFilePath(path), path);
  }
});
