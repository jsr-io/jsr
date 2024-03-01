---
title: Native JSR imports
description: In tools that natively support JSR, you can import JSR packages using the `jsr:` scheme.
---

Tools that natively support JSR can import JSR packages using the `jsr:` scheme.
This allows for more efficient installation of JSR packages, and better support
for JSR's features.

Currently, the only tool that natively supports JSR is **Deno**. If you use a
different tool, you can
[use JSR's npm compatibility layer](/docs/npm-compatibility) to use JSR
packages.

> Would you like to add native support for JSR to your tool? Check out
> [the guide for implementing native `jsr:` support](#implementing-in-tools).

## Installing and importing JSR packages

In tools with native JSR support, JSR packages do not need to be explicitly
installed. Instead, you can import them using the `jsr:` scheme in your import
specifier, and the tool will automatically download and cache the package on
first run.

For example, to import the [`@luca/flag`](/@luca/flag) package:

```ts
import { printProgress } from "jsr:@luca/flag";
```

This will automatically download and cache the latest version of the
`@luca/flag` package.

You can also specify version constraints in the import specifier, to import a
specific version of a package:

```ts
// Import a specific patch version
import { printProgress } from "jsr:@luca/flag@1.0.0";

// Import the latest version in a major version range
import { printProgress } from "jsr:@luca/flag@1";

// Import the latest version compatible with a specific version (>= 1.2.3 and < 2.0.0)
import { printProgress } from "jsr:@luca/flag@^1.2.3";

// Import the latest version in a minor version range, greater than the specific version (>= 1.2.3 and < 1.3.0)
import { printProgress } from "jsr:@luca/flag@~1.2.3";
```

If you'd like to import packages without having to write the `jsr:` and version
constraints in your code, you can use
[an import map](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/script/type/importmap)
to map the `jsr:` scheme to a different prefix.

In **Deno** you can do this by adding a line to the `"imports"` section in your
`deno.json`. You can do this manually, or by using the `deno add` command:

```diff
 {
   "imports": {
+    "@luca/flag": "jsr:@luca/flag@1"
   }
 }
```

You can then import packages using the alias defined in the `deno.json`:

```ts
import { printProgress } from "@luca/flag";
```

## Implementing native `jsr:` imports in tools

> **Note**: This section is for tool maintainers who would like to add native
> JSR support to their tool. Are you a user of JSR instead?
> [Go to usage instructions](/docs/using-packages).

**This section is still TODO.** If you'd like to add native JSR support to your
tool, please reach out to us - we'd love to help you get started!
