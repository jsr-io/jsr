---
title: Why JSR?
description:
  JSR is a modern package registry for the JavaScript ecosystem. Here's why
  we built it - to be TypeScript-first and ESM-only, to work across all
  JavaScript runtimes, and to be fast, simple, and secure.
---

The incredible success of Node.js has been driven in large part by the success
of [npm](https://www.npmjs.com/). With 2 million (going on 3 million) packages,
npm is likely the most successful package manager and registry in history. The
JavaScript community should look on this accomplishment with pride.

So why build JSR when npm exists? Because the world today is not the same as it
was when npm was originally introduced:

- **ECMAScript modules have arrived as a standard**. The web platform has now
  adopted
  [ESM](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Modules)
  as the module format of choice, superseding CommonJS.
- **There are more JavaScript runtimes than just Node.js and browsers**. With
  the emergence of Deno, Bun, workerd, and other new JavaScript environments, a
  Node.js-centric package registry no longer makes sense for the entire JS
  ecosystem.
- **TypeScript has emerged as a de facto standard**. TypeScript, as a superset
  of JavaScript and test bed for the latest ECMAScript features, has emerged as
  a default choice for non-trivial JavaScript codebases. A modern registry
  should be designed with TypeScript in mind.

In addition to these shifting requirements, there are also opportunities to
improve on the developer experience, performance, reliability, and security of
npm. JSR was created to address these new requirements and take on these
opportunities.

Here are a few reasons why we think you should consider using JSR.

## Native TypeScript support

JSR was designed with TypeScript support in mind. TypeScript source files are
published directly to JSR. Platforms (like Deno) that
[natively support TypeScript](/docs/using-packages#native-jsr-imports) can use
these files directly.

For other environments (like Node.js) that lack native TypeScript support, JSR
will transpile your source code to JavaScript, and distribute your modules with
`.d.ts` files to support TypeScript tooling for Node.js projects. No additional
configuration or build steps are required on the side of module authors.

JSR will also generate reference documentation for your packages from TypeScript
source code, providing rich online documentation that you can maintain alongside
your code.

## ECMAScript modules only

The web standard for JavaScript modules is
[ESM](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Modules). A
modern package registry should rally around this standard and shift the
community in this direction. For this reason, JSR was designed for ESM only.

## Cross-runtime support

The goal of JSR is to work everywhere JavaScript works, and to provide a
runtime-agnostic registry for JavaScript and TypeScript code. Today, JSR works
with Deno and other npm environments that populate a `node_modules`. This means
that Node.js, Bun, Cloudflare Workers, and other projects that manage
dependencies with a `package.json` can interoperate with JSR as well.

We intend to expand our support of bundlers and other runtimes as time goes on,
and document the APIs and techniques for doing so.

## JSR is a superset of npm

The npm registry has been incredibly successful thanks to the contributions of
developers worldwide. We want JSR to build on this success, not fork it. JSR is
a superset of npm, much as TypeScript is a superset of JavaScript.

JSR is designed to
[interoperate with npm-based projects and packages](/docs/npm-compatibility).
You can use JSR packages in any runtime environment that uses a `node_modules`
folder. JSR modules can import dependencies from npm.

## Outstanding developer experience

JSR has many features aimed at helping module publishers become more productive,
including but not limited to:

- [Easy publishing](/docs/publishing-packages) with a single command - the CLI
  will walk you through the rest
- Automatic API documentation generation from source code
- Zero-config
  [publishing from GitHub Actions](/docs/publishing-packages#publishing-from-github-actions)
- Automatic inclusion of `.d.ts` files for Node.js/npm distribution
- Automated guidance on TypeScript best practices that will keep your code
  loading as fast as possible.
- Much more

## Fast, secure, and reliable

JSR is intended to be secure, fast, and flexible, and also work well in
resource-constrained environments.

- JSR uses a global CDN to serve packages, and uses local caching and high
  parallelism to speed up downloads.
- JSR package uploads are immutable, so you can trust that packages will never
  change after downloading them or disappear underneath you.
- JSR package downloads are efficient, downloading only the exact files you are
  importing.
- JSR uses OIDC-based authentication for publishing packages from CI, and uses a
  tokenless interactive authentication flow for publishing from a local machine.

## JSR is still evolving

Your feedback will be critical to the success of JSR. If you have any ideas or
feedback on how JSR could work better for your use case, please let us know on
[Discord](https://discord.gg/hMqvhAn9xG).

Ready to try JSR yourself? [Get started now](/docs/introduction).
