// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { assert, assertEquals } from "@std/assert";
import { isModuleFilePath } from "./main.ts";

// terraform/waf.tf re-derives "module fetch vs. frontend page" at the edge
// with `waf_re_module_shaped`, which must stay a strict SUPERSET of
// `isModuleFilePath()`: every path the Worker serves from R2 must be exempt
// from the WAF's rate limiting, SBFM, and Managed Rules phases. A WAF copy
// narrower than the Worker's rate-limits `deno add` globally; a wider one
// merely leaves some frontend traffic unprotected.
//
// Cloudflare evaluates the pattern with the Rust regex crate; for the
// character classes and literals used here its semantics match RegExp.

const wafTf = await Deno.readTextFile(
  new URL("../terraform/waf.tf", import.meta.url),
);

function extractLocal(name: string): RegExp {
  const match = wafTf.match(new RegExp(`${name}\\s*=\\s*"(.*)"`));
  if (!match) throw new Error(`${name} not found in terraform/waf.tf`);
  // HCL string escaping: `\\` in the file is a single `\` in the value.
  return new RegExp(match[1].replaceAll("\\\\", "\\"));
}

const moduleShaped = extractLocal("waf_re_module_shaped");
const docsRoute = extractLocal("waf_re_docs_route");

Deno.test("waf_re_module_shaped is a superset of isModuleFilePath", () => {
  const scopes = ["std", "luca", "scope-with-dashes", "x"];
  const packages = ["fs", "pkg", "pkg.dotted", "foo_bar"];
  const versions = [
    "1.0.0",
    "0.213.1",
    "1.2.3-beta.1",
    "2.0.0-rc.1+build.5",
    "10.20.30",
  ];
  const files = [
    "mod.ts",
    "deep/nested/file.js",
    "README.md",
    "union_select.ts",
    ".gitignore",
    "meta.json",
  ];

  const paths: string[] = [];
  for (const scope of scopes) {
    for (const pkg of packages) {
      paths.push(`/@${scope}/${pkg}/meta.json`);
      for (const version of versions) {
        paths.push(`/@${scope}/${pkg}/${version}_meta.json`);
        for (const file of files) {
          paths.push(`/@${scope}/${pkg}/${version}/${file}`);
        }
      }
    }
  }

  for (const path of paths) {
    assert(isModuleFilePath(path), `corpus path is not a module file: ${path}`);
    assert(
      moduleShaped.test(path),
      `R2-served path is not exempt at the WAF: ${path}`,
    );
  }
});

Deno.test("frontend routes stay outside the module exemption", () => {
  for (
    const path of [
      "/",
      "/packages",
      "/@scope",
      "/@scope/pkg",
      "/@scope/pkg/versions",
      "/@scope/pkg/score",
      "/@scope/pkg/dependencies",
      "/@scope/pkg/doc",
      "/@scope/pkg/doc/~/Foo",
      "/@scope/pkg@1.2.3/doc",
      "/@scope/pkg/diff/1.0.0...2.0.0",
    ]
  ) {
    assertEquals(
      moduleShaped.test(path),
      false,
      `frontend route wrongly exempt at the WAF: ${path}`,
    );
  }
});

Deno.test("waf_re_docs_route matches the pages the LB used to throttle", () => {
  for (
    const path of [
      "/@scope/pkg/doc",
      "/@scope/pkg/doc/",
      "/@scope/pkg/doc/all_symbols",
      "/@scope/pkg/doc/~/Foo",
      "/@scope/pkg/doc/mod.ts/~/Foo.bar",
      "/@scope/pkg@1.2.3/doc",
      "/@scope/pkg@1.2.3/doc/~/Foo",
      "/@scope/pkg/diff/1.0.0...2.0.0",
      "/@scope/pkg/diff/1.0.0...2.0.0/all_symbols",
      "/@scope/pkg/diff/1.0.0...2.0.0/~/Foo",
    ]
  ) {
    assert(docsRoute.test(path), path);
  }
  // Source pages (/@scope/pkg/1.2.3/mod.ts) are deliberately absent here: the
  // rate limiting rule picks them up via waf_re_module_shaped instead.
  for (
    const path of [
      "/",
      "/@scope/pkg",
      "/@scope/pkg/versions",
      "/@scope/pkg/meta.json",
      "/api/scopes/scope/packages/pkg/versions/1.2.3/docs",
    ]
  ) {
    assertEquals(docsRoute.test(path), false, path);
  }
});
