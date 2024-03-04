---
title: Publishing packages
description: Learn how to publish packages to JSR.
---

You can publish most JavaScript and TypeScript code written using ESM modules as
a JSR package. JSR packages are published to [jsr.io](/), and can be imported
from **Deno**, **Node**, and other tools.
[Learn more about using JSR packages](/docs/using-packages).

Both code written originally to use `package.json`, and code written originally
for Deno can be published as a JSR package. JSR supports and encourages
publishing TypeScript source code rather than pairs of `.js` + `.d.ts` files.
This allows JSR to provide more helpful auto-generated documentation, and helps
provide improved auto-completion in editors.

## JSR package rules

All packages uploaded to JSR are automatically processed and verified during
publishing to ensure that all code hosted on JSR abides by a consistent set of
rules. These rules are designed to enable portability across environments. Your
code must follow these rules to be able to be published to JSR.

- **ESM modules only**: JSR packages are published as ESM modules. This means
  that you can only publish modules that use the `import` and `export` keywords.
  You cannot publish CommonJS modules.
- **npm packages are supported**: You can depend on npm packages by either
  specifying them in the `dependencies` of your `package.json`, or by
  referencing them in code using `npm:` specifiers such as
  `import { cloneDeep } from "npm:lodash@4";`.
- **jsr packages are supported**: You can depend on JSR packages by specifying
  them in the `dependencies` of your `package.json`, or by referencing them in
  code using `jsr:` specifiers such as
  `import { encodeBase64 } from "jsr:@std/encoding@1/base64";`
- **`node:` built-ins are supported**: You can import Node.js built-ins using
  the `node:` scheme. For example, you can import the `fs` module using
  `import { readFile } from "node:fs";`. If your package has a `package.json`,
  you can also import Node built-ins with bare specifiers (without the `node:`
  prefix).
- **Simple file names**: File names must be Windows and Unix compatible. This
  means that file names cannot contain characters like `*`, `:`, or `?`. You may
  also not have multiple files with the same name, but different casing.
- **Preferably, no TypeScript "slow types"**: To speed up type checking, support
  documentation generation, and node compatibility, JSR packages should not use
  certain TypeScript types in exported functions, classes, or variables. This is
  enforced by default, but can be opted out of.
  [Learn more about "slow types"](/docs/about-slow-types).
- **Valid cross file imports**: All of the relative imports between modules in
  your package must resolve at publish time. The format of supported specifiers
  depends on whether a `package.json` is in use, and is elaborated below.

## Writing the code

### ESM only

To publish a JSR package, you must first write the code for your package. JSR
packages are written in JavaScript or TypeScript, and are published as ESM
modules.

```ts
// greet.ts
/**
 * Greet a person.
 * @param name The name of the person to greet.
 */
export function greet(name: string) {
  console.log(`Hello, ${name}!`);
}
```

### Relative imports

A package can consist of multiple modules. You can reference other modules in
your package using relative imports. You _should_ use the correct extensions in
the imports -- `./greet.ts` to import the `greet.ts`, rather than `./greet` or
`./greet.js`.

When a `package.json` is present in your package, modules _may_ use "sloppy
imports". With "sloppy imports", you _can_ import files without extensions, or
with a `.js` extension even if the underlying file is `.ts`. You can also use
directory imports with `index.js` resolution.

````ts
// mod.ts
/**
 * A module providing a function to greet people.
 *
 * @example
 * ```ts
 * import { greet } from "@luca/greet";
 *
 * greet("Luca");
 * ```
 *
 * @module
 */

export * from "./greet.ts";
````

### Importing npm packages

You may import npm packages specified in the `"dependencies"` of a
`package.json`, ones specified in an import map or `deno.json`, or ones
specified in source code using `npm:` specifiers.

```json
// package.json
{
  "dependencies": {
    "chalk": "5"
  }
}
```

```ts
// mod.ts
import * as chalk from "chalk";

import * as express from "npm:express@4";
```

### Importing JSR packages

You may import JSR packages specified in the `"dependencies"` of a
`package.json`, ones specified in an import map or `deno.json`, or ones
specified in source code using `jsr:` specifiers.
[Learn more about using JSR packages](/docs/using-packages).

```json
// package.json
{
  "dependencies": {
    "@std/encoding": "npm:@jsr/std__encoding@1"
  }
}
```

```ts
// mod.ts
import * as encoding from "@std/encoding";

import { printProgress } from "jsr:@luca/flag@1";
```

### Importing Node built-ins

You may import Node.js built-ins using the `node:` scheme. If a `package.json`
is present in your package, you may also omit the `node:` scheme.

```ts
// mod.ts
import { readFileSync } from "node:fs";

export function readJsonFile(path: string) {
  return JSON.parse(readFileSync(path, "utf8"));
}
```

### Dependency manifest

You may use a dependency manifest like a `package.json`, or an
[import map](https://docs.deno.com/runtime/manual/basics/import_maps) (like the
`deno.json` file) to simplify your imports. During publishing, `jsr publish` /
`deno publish` will automatically rewrite the specifiers in your source code to
fully qualified specifiers that do not require an import map / `package.json`
anymore.

```json
// import_map.json / deno.json
{
  "imports": {
    "@luca/greet": "jsr:@luca/greet@1",
    "@std/path": "jsr:@std/path@1"
  }
}
```

```ts
// mod.ts
export { greet } from "@luca/greet";
export { join } from "@std/path";
```

### Preventing slow types

When writing TypeScript, you should ensure that your code does not use "slow
types" that prevent JSR from generating documentation, generating type
declarations for the npm compatibility layer, and speeding up type checking for
consumers of your package.
[Learn more about "slow types"](/docs/about-slow-types).

> You may temporarily bypass this restriction by publishing with the
> `--allow-slow-types` flag. This will cause type checking to be significantly
> slower for all of your users. Additionally, documentation generation and node
> compatibility will suffer. Consider fixing the slow types to avoid these
> drawbacks rather than using this flag.

## Package config file

After you have written your code, you must add a config file to your package.
This file contains package metadata like the name, version, and entrypoint(s).
This file should be named `jsr.json`. Deno users can also include the required
JSR properties in their `deno.json` to avoid having to create another file.

```json
// jsr.json / deno.json
{
  "name": "@luca/greet",
  "version": "1.0.0",
  "exports": "./mod.ts"
}
```

The `name` field is the name of your package, prefixed with a JSR scope.
[Learn more about scope and package names](#creating-a-scope-and-package).

The `version` field is the version of your package. This field must be a valid
[SemVer](https://semver.org/) version. You must increment the version of your
package every time you publish a new version.
[Learn more about package versions](/docs/packages#versions).

The `exports` field tells JSR which modules should be importable by users of
your package. The `exports` field can either be specified as a single string, or
as an object mapping entrypoint names to paths in your package.

```json
// jsr.json / deno.json
{
  "name": "@luca/greet",
  "version": "1.0.0",
  "exports": {
    ".": "./mod.ts",
    "./greet": "./greet.ts"
  }
}
```

In the above example, the `exports` field is an object. The `.` entrypoint is
the default entrypoint for the package. The `./greet` entrypoint is a named
entrypoint. With this entrypoint can import the `greet.ts` module using
`import { greet } from "@luca/greet/greet";` and the `mod.ts` module using
`import { greet } from "@luca/greet";`.

You can also specify the `exports` field as a single string. This is useful if
you only have a single entrypoint in your package. This is semantically
equivalent to specifying a default entrypoint in the object form.

```diff
// deno.json
{
  "name": "@luca/greet",
  "version": "1.0.0",
- "exports": {
-   ".": "./mod.ts"
- }
+ "exports": "./mod.ts"
}
```

You can also use the `include` and `exclude` options to include and exclude
files during publishing. If using a `deno.json`, you can use `publish.include`
and `publish.exclude` to include and exclude files only for publishing, rather
than for all Deno subcommands.
[Learn more about ignoring files](#ignoring-files).

## Creating a scope and package

JSR is organized into scopes. A scope is a collection of packages. Scopes are
similar to npm organizations. Scopes are prefixed with an `@` symbol, and are
followed by a name. For example, `@luca` is a scope.

You can create a scope at [jsr.io/new](/new). Scopes names must be between 2 and
20 characters long, and can only contain lowercase letters, numbers, and
hyphens. You can only create a scope if the name is not already taken. Scope
names that are very similar to existing scope names -- for example ones that
only differ by a hyphen -- are prohibited.
[Learn more about scopes](/docs/scopes).

After you have created a scope, you can create a package in that scope. You can
create a package at [jsr.io/new](/new). Package names must be between 2 and 20
characters long, and can only contain lowercase letters, numbers, and hyphens.
You can only create a package if the name is not already taken. Package names
that are very similar to existing package names -- for example ones that only
differ by a hyphen -- are prohibited.
[Learn more about packages](/docs/packages).

## Verifying your package

To publish packages, including performing a dry run to confirm your package
meets all JSR rules, involves using `jsr publish` or `deno publish`. The syntax
for both commands is broadly identical. Depending on your tool, you can invoke
the publish command as follows.

```shell
# deno
deno publish
# npm
npx jsr publish
# yarn
yarn dlx jsr publish
# pnpm
pnpm dlx jsr publish
```

You can run `jsr publish` with the `--dry-run` flag to perform all publish
verification that would happen during a real publish. This will print out a list
of files that will be published, but stop short of actually publishing to the
registry.

```shell
# deno
$ deno publish --dry-run
# npm
$ npx jsr publish --dry-run
# yarn
yarn dlx jsr publish --dry-run
# pnpm
pnpm dlx jsr publish --dry-run
```

## Publishing from your local machine

You can publish packages from your local machine using either `jsr publish` or
`deno publish` command.

Authentication will happen via your browser, so you do not need to provide any
credentials to the CLI.

Enter the root directory of your package (containing the `jsr.json` /
`deno.json` file), and run `jsr publish`.

```shell
# deno
$ deno publish
# npm
$ npx jsr publish
# yarn
yarn dlx jsr publish
# pnpm
pnpm dlx jsr publish
```

When you run `jsr publish`, the CLI will open your browser to approve
publishing. You will be prompted to log in with your JSR account if you are not
already logged in. After you have logged in, you will be prompted to grant the
CLI access to publish the specific package you are trying to publish. Click
"Allow" to grant the CLI access.

The CLI will now upload your package to the JSR registry. After the upload is
complete, the CLI will output the URL of where you can view the package on the
JSR site.

During publishing, both the JSR CLI and the JSR server will run many checks
against your package to ensure that it is valid. If any of these checks fail,
the CLI will output an error message. You must fix these errors before you can
attempt publishing again.
[Learn more about troubleshooting publishing errors](/docs/troubleshooting#publishing-errors).

## Publishing from GitHub Actions

JSR has first class support for publishing packages from GitHub Actions. This
allows you to publish packages automatically from CI without you having to
configure any secrets or authentication.

To publish from GitHub Actions, you must first link your package to your GitHub
repository from your package settings in JSR. To do this, go to the settings tab
of your package on [jsr.io](/), enter your GitHub repository name, and click
"Link".

After you have linked your package to your GitHub repository, you can publish
from GitHub Actions. To do this, create a workflow file in your repository, for
example at `.github/workflows/publish.yml`. In this workflow file, you can
create a job that publishes your package using the `jsr publish` command.

```yaml
# .github/workflows/publish.yml

name: Publish

on:
  push:
    branches:
      - main

jobs:
  publish:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      id-token: write # The OIDC ID token is used for authentication with JSR.    
    steps:
      - uses: actions/checkout@v4
      - run: npx jsr publish
```

This workflow will run every time you push to the `main` branch of your
repository. It will publish your package to JSR, and will automatically use the
correct version number based on the version in your `jsr.json` file.
`jsr publish` will not attempt to publish if the version specified in your
`jsr.json` file is already published to JSR.

## Ignoring files

`jsr publish` will ignore files that are listed in a `.gitignore` file in the
root of your package. Additionally, you can specify the `exclude` and `include`
fields in your `jsr.json` / `deno.json` file to ignore or include specific
files.

For example, you may have a package that has a `.gitignore` file with the
following contents:

```gitignore
.DS_Store
dist/
```

In this case any files in the `dist/` directory, and any files named `.DS_Store`
will be ignored when publishing.

This may however be inconvenient if you want to publish the `dist/` directory,
because you have `"exports"` pointing to it (or a subdirectory of it). In this
case, you can use the `include` field in your `jsr.json` / `deno.json` file to
include the `dist/` directory anyway.

```json
// jsr.json
{
  "name": "@luca/greet",
  "version": "1.0.0",
  "exports": "./dist/mod.ts",
  "include": ["dist/**"]
}
```

In this case, the `dist/` directory will be included when publishing, even
though it is listed in the `.gitignore` file.

When using Deno, the `exclude` and `include` options in `deno.json` are used for
many other Deno subcommands as well, such as `deno test` and `deno bundle`. You
can use `publish.include` and `publish.exclude` in your `deno.json` file to
specify options that only apply to `deno publish`.

```json
// deno.json
{
  "name": "@luca/greet",
  "version": "1.0.0",
  "exports": "./dist/mod.ts",
  "publish": {
    "include": ["dist/**"]
  }
}
```

To ignore all files, and only selectively include certain files, you can specify
a glob that matches all files in the `exclude` option:

```json
// jsr.json
{
  "name": "@luca/greet",
  "version": "1.0.0",
  "exports": "./dist/mod.ts",
  "include": ["dist/**"],
  "exclude": ["**"]
}
```
