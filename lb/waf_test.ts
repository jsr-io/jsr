// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { assert, assertEquals } from "@std/assert";
import { isModuleFilePath } from "./main.ts";
import { isBot } from "./bots.ts";

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
const botUserAgent = extractLocal("waf_re_bot_user_agent");
const botFrom = extractLocal("waf_re_bot_from");

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

// The skip rule in waf.tf carries a `not <bot>` clause because the Worker
// checks isBot() BEFORE the module-file branch — a request with a bot header
// is a frontend render even on a module-shaped path, and must not inherit the
// module exemption. Unlike waf_re_module_shaped this mirror must be EXACT in
// both directions: a narrower WAF copy re-opens the spoofed-bot scraping
// bypass, a wider one rate-limits real module fetches from any client whose
// headers it over-matches.
Deno.test("waf bot clause agrees with isBot() in both directions", () => {
  const cases: [string, Record<string, string>][] = [
    // Everything isBot() flags, including case variance from its /i flag.
    ["Slack link unfurler", {
      "User-Agent":
        "Slackbot-LinkExpanding 1.0 (+https://api.slack.com/robots)",
    }],
    ["Slack image proxy", { "User-Agent": "slack-imgproxy 149" }],
    ["Iframely (Notion previews)", {
      "User-Agent": "Iframely/1.3.1 (+https://iframely.com/docs/about)",
    }],
    ["Twitter card fetcher", { "User-Agent": "Twitterbot/1.0" }],
    ["WhatsApp preview", { "User-Agent": "WhatsApp/2.23.20.0" }],
    ["Discord unfurler", {
      "User-Agent":
        "Mozilla/5.0 (compatible; Discordbot/2.0; +https://discordapp.com)",
    }],
    ["Googlebot from header", { "From": "googlebot(at)googlebot.com" }],
    ["Googlebot from header, cased", {
      "From": "GoogleBot(at)googlebot.com",
      "User-Agent": "Mozilla/5.0 (compatible; Googlebot/2.1)",
    }],
    // Module-fetching clients that must keep the exemption — a WAF bot clause
    // matching any of these rate-limits installs.
    ["deno", { "User-Agent": "Deno/2.1.4" }],
    ["npm", {
      "User-Agent": "npm/10.8.2 node/v22.6.0 darwin arm64 workspaces/false",
    }],
    ["bun", { "User-Agent": "Bun/1.1.20" }],
    ["pnpm", { "User-Agent": "pnpm/9.6.0" }],
    ["curl", { "User-Agent": "curl/8.6.0" }],
    ["no headers at all", {}],
    ["browser", {
      "User-Agent":
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    }],
    // Googlebot's real crawler UA: bots.ts only detects it via the From
    // header, so both sides must agree this UA alone is not a bot.
    ["Googlebot UA without From", {
      "User-Agent":
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
    }],
    ["unrelated From header", { "From": "someone@example.com" }],
  ];

  for (const [name, headers] of cases) {
    const request = new Request("https://jsr.io/@scope/pkg/1.2.3/mod.ts", {
      headers,
    });
    const wafSaysBot = botUserAgent.test(headers["User-Agent"] ?? "") ||
      botFrom.test(headers["From"] ?? "");
    assertEquals(
      wafSaysBot,
      isBot(request),
      `WAF bot clause and isBot() disagree for: ${name}`,
    );
  }
});
