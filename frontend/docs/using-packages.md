---
title: Using packages
description: Learn how to use JSR packages in your projects.
---

You can use JSR packages in any runtime that supports ES modules, like Deno,
Node.js, Bun, Cloudflare Workers, etc. You can also use JSR packages with
bundlers that support ES modules, such as **Vite**, **esbuild**, **Webpack**,
and **Rollup**.

## Adding a package

You can add a JSR package to your project using any of the commands below - the
example below will add the most recent version of
[`@luca/cases`](https://jsr.io/@luca/cases) to your project.

```bash
# deno, pnpm 10.9+, and yarn 4.9+ with first class JSR support
deno add jsr:@luca/cases
pnpm add jsr:@luca/cases
yarn add jsr:@luca/cases

# npm, bun, and older versions of yarn or pnpm
npx jsr add @luca/cases
bunx jsr add @luca/cases
yarn dlx jsr add @luca/cases
pnpm dlx jsr add @luca/cases
```

If you're using Deno, the `deno add` command will add an
[import map](https://docs.deno.com/runtime/manual/basics/import_maps) entry for
the JSR module you specified in a `deno.json(c)` file. That entry will look
something like this:

```json
{
  "imports": {
    "@luca/cases": "jsr:@luca/cases@^1.0.1"
  }
}
```

For npm compatible package managers, the dependency will be added to your
`package.json` file, and the package will be installed to your `node_modules`
directory.

For npm, older versions of pnpm and Yarn, and for Bun, the `jsr` command will
additionally add a `.npmrc` file to your project root with the necessary config
to use JSR with npm. Learn more about
[JSR's npm compatibility layer](/docs/npm-compatibility).

After installation with any npm compatible package manager, your package.json
will contain a dependency entry that looks like one of these configurations:

```json
// pnpm 10.9+ and yarn 4.9+
{
  "dependencies": {
    "@luca/cases": "jsr:@luca/cases@^1.0.1"
  }
}
```

or

```json
// npm, bun, and older versions of yarn or pnpm
{
  "dependencies": {
    "@luca/cases": "npm:@jsr/luca__cases@^1.0.1"
  }
}
```

In npm, Bun, and older versions of yarn or pnpm the dependency configuration
makes use of a special custom scope called `@jsr`, which is configured for you
in `.npmrc`:

```
@jsr:registry=https://npm.jsr.io
```

> **NOTE:** You should check the new `.npmrc` file into source control - it will
> be needed to install updates to JSR modules.

Once your package has been added, you can use it in your ES module code the same
way across runtimes:

```ts
import { camelCase } from "@luca/cases";

camelCase("hello world"); // "helloWorld"
```

### Importing with `jsr:` specifiers

In Deno, you can use packages without an install step by using `jsr:` specifiers
directly within your source files. In this example, we import the `@luca/cases`
module directly in our code, without needing an import map entry.

```ts
import { camelCase } from "jsr:@luca/cases@^1.0.1";

camelCase("hello world"); // "helloWorld"
```

Here the `jsr:` specifier is followed by the package scope, the package name, a
[semver constraint](#semver-resolution), and optionally an entrypoint.

## Semver resolution

JSR uses semantic versioning to resolve package versions. This means that you
can use a semver range in your `jsr:` specifier, import map, or `package.json`
`"dependencies"` entry, and then the runtime or package manager will download
the latest version that satisfies all constraints for the package.

If you only care about the major version, you can specify just the major
version:

```bash
# deno, pnpm 10.9+ and yarn 4.9+
deno add jsr:@luca/cases@1
pnpm add jsr:@luca/cases@1
yarn add jsr:@luca/cases@1

# npm (and bun, and older versions of yarn or pnpm)
npx jsr add @luca/cases@1
```

If you want to use a specific minor version, you can specify the minor version:

```bash
# deno, pnpm 10.9+ and yarn 4.9+
deno add jsr:@luca/cases@1.0
pnpm add jsr:@luca/cases@1.0
yarn add jsr:@luca/cases@1.0

# npm (and bun, and older versions of yarn or pnpm)
npx jsr add @luca/cases@1.0
```

If you want to use a specific patch version, you can specify the patch version:

```bash
# deno, pnpm 10.9+ and yarn 4.9+
deno add jsr:@luca/cases@1.0.1
pnpm add jsr:@luca/cases@1.0.1
yarn add jsr:@luca/cases@1.0.1

# npm (and bun, and older versions of yarn or pnpm)
npx jsr add @luca/cases@1.0.1
```

If you want to use at least a specific patch version, but do want to allow\
updates, you can specify the patch version with a `^` prefix:

```bash
# deno, pnpm 10.9+ and yarn 4.9+
deno add jsr:@luca/cases@^1.0.1
pnpm add jsr:@luca/cases@^1.0.1
yarn add jsr:@luca/cases@^1.0.1

# npm (and bun, and older versions of yarn or pnpm)
npx jsr add @luca/cases@^1.0.1
```

Learn more about semantic versioning [here](https://semver.org/).

## Entrypoints

Every package on JSR has one or more entrypoints. An entrypoint is a module that
can be imported by users of the package. The entrypoint is specified in the
`exports` field of the package's config file.
[Learn more about `exports`.](/docs/publishing-packages#package-metadata)

If you don't specify an entrypoint in your import, the runtime will use the
package's default entrypoint. For example, when you import `jsr:@luca/cases@1`,
you import the default entrypoint of the `@luca/cases` package.

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
