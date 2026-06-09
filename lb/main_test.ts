// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { assertEquals } from "@std/assert";
import { isDocsDiffSourceRoute } from "./main.ts";

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
