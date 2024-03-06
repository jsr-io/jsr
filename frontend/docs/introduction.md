---
title: Introduction to JSR
description: JSR is a new modern package registry for JavaScript and TypeScript. It was designed to be fast, simple, and reliable. It is backwards compatible with npm, and natively supports TypeScript.
---

The JavaScript Registry (**JSR**) is a modern package registry for JavaScript
and TypeScript. JSR works with many runtimes (Node.js, Deno, browsers, and more)
and is backwards compatible with npm.
[Learn more about why we built JSR.](/docs/why)

## Using JSR packages

Add a package to your project using one of the commands below. This will add the
most recent version of [`@luca/flag`](https://jsr.io/@luca/flag) to your
project.

```bash
# deno
deno add @luca/flag

# npm (use any of npx, yarn dlx, pnpm dlx, or bunx)
npx jsr add @luca/flag
```

After adding the package, you can import and use it in ES modules like so:

```ts
import { printProgress } from "@luca/flag";

printProgress();
```

In Deno, you can optionally use JSR packages without an install step using
`jsr:` specifiers and Deno's
[native support for JSR](/docs/using-packages#native-jsr-imports).

```ts
import { printProgress } from "jsr:@luca/flag@1";

printProgress();
```

You can find more packages on [jsr.io](https://jsr.io). Each package on the JSR
site also displays documentation, which is automatically generated from the
package's source code.
[Learn more about using packages.](/docs/using-packages)

## Publishing JSR packages

JSR packages are published using the `jsr publish` / `deno publish` command. You
can publish packages from your local machine, or from CI.

First, write your code. JSR packages are written in JavaScript or TypeScript,
and are published as ES modules.

```ts
// mod.ts
/**
 * A module providing a function to greet people.
 * @module
 */

/**
 * Greet a person.
 *
 * @param name The name of the person to greet.
 */
export function greet(name: string) {
  console.log(`Hello, ${name}!`);
}
```

Then, add a config file to your package. This file contains package metadata
like the name, version, and entrypoint(s). The
[`exports` field](/docs/publishing-packages#package-metadata) tells JSR which
modules should be importable by users of your package.

```json
// jsr.json / deno.json
{
  "name": "@luca/greet",
  "version": "1.0.0",
  "exports": "./mod.ts"
}
```

Finally, run `npx jsr publish`, or `deno publish` to publish your package. You
will be prompted to authenticate with JSR, and then your package will be
published.

```
$ npx jsr publish
Visit https://jsr.io/auth?code=ABCD-EFGH to authorize publishing of @luca/greet
Waiting...
Authorization successful. Authenticated as Luca Casonato
Publishing @luca/greet@1.0.0 ...
Successfully published @luca/greet@1.0.0
Visit https://jsr.io/@luca/greet@1.0.0 for details
```

[Learn more about publishing packages.](/docs/publishing-packages)
