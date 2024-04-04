---
title: Other Registries
description: A Comparison of Different JavaScript Registries
---

There are several platforms besides JSR where you can share JavaScript
libraries. In this section, we will highlight the distinctions between various
options.

## npm

npm is the primary and widely recognized platform for sharing JavaScript
libraries. It was initially developed alongside Node.js in the early 2010s.

JSR is designed to complement npm, not replace it. JSR allows packages to
reference npm packages using `npm:` specifiers.

JSR packages can also be used in tools that don't yet natively support JSR,
[by using JSR's npm compatibility layer](/docs/npm-compatibility).

We created JSR to address specific issues in the npm ecosystem:

- **Native TypeScript Support**: JSR doesn't require transpilation of TypeScript
  code before publishing. It's explicitly built to support TypeScript features
  like "go-to-definition," avoiding unnecessary encounters with declaration
  files (d.ts).
- **ESM Syntax**: JSR promotes modern ECMAScript module (ESM) syntax over
  CommonJS, enabling simplified code structures.
- **Better Constraints**: JSR enforces various constraints that enhance
  portability between Unix and Windows platforms, such as path length
  restrictions and disallowing certain file names.

## deno.land/x

JSR and deno.land/x share common origins, but JSR is designed to be compatible
with various runtimes and bundlers.

deno.land/x serves as a repository for hosting source code accessible via HTTPS.
JSR was created to address several concerns related to deno.land/x:

- **Semver Enforcement**: Deno.land/x does not enforce Semantic Versioning
  (semver), leading to challenges in deduplicating dependencies. Consequently,
  multiple versions of the same library may appear in the module graph.
- **Reliability**: Deno.land/x lacks self-contained links, including links to
  potentially unreliable servers that may have gone offline since the code was
  published. This can undermine trust in the long-term reliability of
  deno.land/x libraries.
- **TypeScript Performance**: While Deno and deno.land/x offer native TypeScript
  support, there are performance issues when using HTTPS modules from
  deno.land/x. The type checker may continue to analyze code beyond a user's
  control, impacting performance.

## esm.sh

Esm.sh is a platform that serves npm packages over HTTPS. Before Deno provided
native npm support, this was the preferred method for including npm dependencies
in Deno programs.

In contrast to esm.sh, JSR does not serve npm packages directly. JSR operates as
an independent registry that allows npm dependencies while primarily focusing on
pure JSR code.

Similarities with esm.sh:

- **ESM Modules Only**: Both esm.sh and JSR exclusively provide ESM modules, not
  CommonJS.
- **Direct File Access**: Like esm.sh, JSR offers direct access to individual
  package files via HTTPS.

## unpkg.com

unpkg.com shares similarities with esm.sh by allowing access to individual files
within npm modules over HTTPS. However, it provides fewer features compared to
esm.sh.
