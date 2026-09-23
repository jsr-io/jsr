// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { assertEquals } from "@std/assert";
import { isDocsDiffSourceRoute, route } from "./main.ts";
import type { PartialBucket, WorkerEnv } from "./types.ts";

Deno.test("isDocsDiffSourceRoute matches doc pages", () => {
  for (
    const path of [
      "/@scope/pkg/doc",
      "/@scope/pkg/doc/",
      "/@scope/pkg/doc/all_symbols",
      "/@scope/pkg/doc/~/Foo",
      "/@scope/pkg/doc/mod.ts/~/Foo.bar",
      "/@scope/pkg@1.2.3/doc",
      "/@scope/pkg@1.2.3/doc/~/Foo",
    ]
  ) {
    assertEquals(isDocsDiffSourceRoute(path), true, path);
  }
});

Deno.test("isDocsDiffSourceRoute matches diff pages", () => {
  for (
    const path of [
      "/@scope/pkg/diff/1.0.0...2.0.0",
      "/@scope/pkg/diff/1.0.0...2.0.0/all_symbols",
      "/@scope/pkg/diff/1.0.0...2.0.0/~/Foo",
      "/@scope/pkg/diff/...2.0.0/mod.ts/~/Foo",
    ]
  ) {
    assertEquals(isDocsDiffSourceRoute(path), true, path);
  }
});

Deno.test("isDocsDiffSourceRoute matches source pages", () => {
  for (
    const path of [
      "/@scope/pkg/1.2.3",
      "/@scope/pkg/1.2.3/mod.ts",
      "/@scope/pkg/1.2.3/src/foo.ts",
      "/@scope/pkg/1.2.3-beta.1/mod.ts",
    ]
  ) {
    assertEquals(isDocsDiffSourceRoute(path), true, path);
  }
});

Deno.test("isDocsDiffSourceRoute ignores other routes", () => {
  for (
    const path of [
      "/",
      "/@scope",
      "/@scope/pkg",
      "/@scope/pkg/score",
      "/@scope/pkg/versions",
      "/@scope/pkg/dependencies",
      "/@scope/pkg/meta.json",
      "/api/scopes/scope/packages/pkg/versions/1.2.3/docs",
      "/packages",
    ]
  ) {
    assertEquals(isDocsDiffSourceRoute(path), false, path);
  }
});

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
    REGISTRY_API_URL: "https://api.invalid/",
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
