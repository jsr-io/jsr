---
title: Using packages
description: Learn how to use JSR packages in your projects.
---

## Quick start

You can use JSR packages from in any runtime that supports ES modules. This
includes **Deno** and **Node.js**. You can also use JSR packages with other
tools that support ES modules, such as **Vite**, **Esbuild**, **Webpack**, and
**Rollup**.

You can use [native `jsr:` imports](#native-jsr-imports) in **Deno**. In tools
that don't yet have native `jsr:` support, like **Node.js**, you can use
[npm compatibility imports](#npm-compatibility-imports).

### Native JSR imports

In **Deno**, JSR is natively supported and can be imported using the `jsr:`
scheme:

```ts
import { printProgress } from "jsr:@luca/flag@1";
```

Here the `jsr:` is followed by the package scope, the package name, a
[semver constraint](#semver-resolution), and optionally an entrypoint.

You can also add JSR packages to your `deno.json`'s `"imports"` section to
simplify your imports:

```json
// deno.json
{
  "imports": {
    "@luca/flag": "jsr:@luca/flag@1"
  }
}
```

You can then import the package using the alias defined in your `deno.json`:

```ts
import { printProgress } from "@luca/flag";
```

### npm compatibility imports

In tools that don't yet have native `jsr:` support, you can use
[JSR's npm compatibility layer](/docs/npm-compatibility) to use JSR packages.
This includes **Node.js**, **Cloudflare Workers**, **Vite**, **Esbuild**,
**Webpack**, and **Rollup**.

You can install JSR packages in existing projects using npm, yarn, or pnpm.
First, add the package to your `package.json` using the `jsr` CLI:

```sh
npx jsr add @luca/flag
```

This will automatically run your preferred package manager to install the
package to your `node_modules`. You can then import JSR packages in your code:

```ts
import { printProgress } from "@luca/flag";
```

> The `.npmrc` file that is created by the `jsr` tool should be checked into
> source control. This enables future calls to `npm install` / `yarn` /
> `pnpm install` to succeed.

[Learn more about the npm compatibility layer](/docs/npm-compatibility).

## Semver resolution

JSR uses semver to resolve package versions. This means that you can use a
semver range in your `jsr:` specifier or `package.json` `"dependencies"` entry,
and then the runtime or package manager will download the latest version that
satisfies all constraints for the package.

If you only care about the major version, you can specify just the major
version:

```ts
import { printProgress } from "jsr:@luca/flag@1";
```

```
npx jsr i @luca/flag@1
```

If you want to use a specific minor version, you can specify the minor version:

```ts
import { printProgress } from "jsr:@luca/flag@1.0";
```

```
npx jsr i @luca/flag@1.0
```

If you want to use a specific patch version, you can specify the patch version:

```ts
import { printProgress } from "jsr:@luca/flag@1.0.0";
```

```
npx jsr i @luca/flag@1.0.0
```

If you want to use at least a specific patch version, but do want to allow\
updates, you can specify the patch version with a `^` prefix:

```ts
import { printProgress } from "jsr:@luca/flag@^1.0.0";
```

```
npx jsr i @luca/flag@^1.0.0
```

Learn more about [SemVer](https://semver.org/).

## Entrypoints

Every package on JSR has one or more entrypoints. An entrypoint is a module that
can be imported by users of the package. The entrypoint is specified in the
`exports` field of the package's config file.
[Learn more about `exports`](/docs/publishing-packages#package-metadata).

If you don't specify an entrypoint in your import, the runtime will use the
package's default entrypoint. For example, when you import `jsr:@luca/flag@1`,
you import the default entrypoint of the `@luca/flag` package.

You can also import other entrypoints from a package. To do this, specify the
entrypoint after the version constraint:

```ts
import { join } from "jsr:@std/path@1/join";
```

If you're using an import map, or a `package.json`, you can specify the
entrypoint after the alias:

```ts
import { join } from "@std/path/join";
```

Files not listed in the `exports` field are not directly importable by users of
the package. However, they can be imported by other modules in the package.

Packages are not required to have a default entrypoint. If a package does not
have a default entrypoint, then you must specify an entrypoint in your import.

## Aliases

Writing the full `jsr:` or `@jsr` import specifier can be tedious. You can
create an alias for a package in your `deno.json` or `package.json` to make this
easier.

When using [native JSR imports](#native-jsr-imports), you can add a line to
[an import map](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/script/type/importmap).
In **Deno** you can do this by adding a line to the `"imports"` section in your
`deno.json`:

```diff
 {
   "imports": {
+    "@luca/flag": "jsr:@luca/flag@1"
   }
 }
```

When using [JSR's npm compatibility imports](#npm-compatibility-imports), for
example in **Node.js**, you can specify an alias in your `package.json`:

```diff
 {
   "type": "module",
   "imports": {
-    "@jsr/luca__flag": "1"
+    "@luca/flag": "npm:@jsr/luca__flag@1"
   }
 }
```

You can then import packages using the aliases defined in the `deno.json` or
`package.json`:

```ts
import { progress } from "@luca/flag";
import { join } from "@std/path";
```
