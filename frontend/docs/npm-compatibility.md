---
title: npm compatibility
description: JSR packages can be used in tools that don't yet natively support JSR, by using JSR's npm compatibility layer.
---

Tools that use `npm` and `node_modules` can use JSR through JSR's npm
compatibility layer. Examples of such tools are, **Node.js**, **Cloudflare
Workers**, **Vite**, **Esbuild**, **Webpack**, and **Rollup**.

> If you are using **Deno**, you
> [can use `jsr:` imports](/docs/using-packages#native-jsr-imports).

With this compatibility layer, all JSR packages can be installed using existing
npm-compatible package managers like `npm`, `yarn`, and `pnpm`. This allows you
to use JSR packages in any tool that supports npm packages using `node_modules`.
When using this compatibility layer, you can install JSR packages using
`npm install` / `yarn add` / `pnpm install` as usual. The JSR packages will be
downloaded from jsr.io and installed in your `node_modules` directory.

## Installing and using JSR packages

You can add JSR packages to your project using the `jsr` CLI:

```sh
npx jsr add @luca/flag
```

This will add the `@luca/flag` package to your `package.json` file, and install
it to your `node_modules` directory using your preferred package manager.

The package manager to use will be automatically detected based on the presence
of a `package.lock`, `yarn.lock` or `pnpm-lock.yaml` file in your project. You
can also specify the package manager to use explicitly using the `--npm`,
`--yarn`, or `--pnpm` flags to the `jsr add` command.

> You should check the `.npmrc` file that is created into source control. This
> enables future calls to `npm install` / `yarn` / `pnpm install` to succeed.

You can then use the JSR package from your code:

```ts
import { printProgress } from "@luca/flag";
```

> Note: Due to limitations of `npm` and `yarn`, they may sometimes install
> duplicate copies of your JSR dependencies. This can lead to larger
> `node_modules` directories than necessary, and for some packages it can lead
> to unexpected behavior. We recommend using `pnpm` for the best experience.

## Limitations

The JSR npm compatibility layer is not a perfect replacement for native JSR
support. There are some limitations:

- You can not use `jsr:` specifiers to import JSR packages.
- Unlike with native JSR imports, you are not directly importing TypeScript
  code. Instead JSR transpiles the TypeScript code to JavaScript before it is
  installed into your `node_modules` directory. This generally means that your
  editor experience will suffer, because "Go to definition" and other features
  will link to transpiled JavaScript code, or to generated `.d.ts` files.
- Install times will generally be slower than with native JSR support, because
  npm will download files even if they are not used in your project.
- You can not publish JSR packages using the npm compatibility layer, using
  `npm publish`. You can only publish JSR packages using `jsr publish` or
  `deno publish`.

## Advanced setup

The JSR npm compatibility layer works by making all JSR packages available under
the special `@jsr` npm scope. The `@jsr` npm scope is not a real npm scope, and
you cannot publish packages to it. You can only use it to install jsr packages
from npm.

The `@jsr` npm scope is served from the JSR registry at `https://npm.jsr.io`.
This means that you need to configure your package manager to use this registry
to install JSR packages. When adding packages with the `jsr` CLI, this is done
automatically.

Instead of using the `jsr` CLI to install JSR packages, you can also manually
configure your package manager to support installing JSR packages.

To do this, create an `.npmrc` file, and add the following lines to it:

```
@jsr:registry=https://npm.jsr.io
```

This instructs your package manager to load all packages in the `@jsr` scope
from the JSR backend instead of npm.

You can also configure your package manager to support JSR in all projects on
your machine, by creating a `.npmrc` file in your home directory with the same
content.

You can then manually install JSR packages using `npm install` / `yarn add` /
`pnpm install` as usual, using the special `@jsr` npm scope:

```sh
npm install @jsr/luca__flag@1 # installs the @luca/flag package from JSR
yarn add @jsr/luca__flag@1 # installs the @luca/flag package from JSR
pnpm install @jsr/luca__flag@1 # installs the @luca/flag package from JSR
```

The name following the `@jsr/` scope is the name of the JSR package you want to
install. This name is the same as the package name you'd use with `jsr:`
imports, except that the `@` prefixing the scope is removed, and the `/` between
the scope and the name is replaced with `__`.

For example, the `jsr:@luca/flag` package is available at `@jsr/luca__flag`.

You can then import JSR packages using the `@jsr` scope in your code:

```ts
import { printProgress } from "@jsr/luca__flag";
```

If you'd like to import packages without having to specify the `@jsr/`, you can
update the `dependencies` object in your `package.json`:

```diff
 // package.json
 {
   "type": "module",
   "dependencies": {
-    "@jsr/luca__flag": "1"
+    "@luca/flag": "npm:@jsr/luca__flag@1"
   }
 }
```

You can then import the package using the name defined in your `package.json`:

```ts
import { printProgress } from "@luca/flag";
```

## Technical details

The `@jsr` npm scope is a special scope that is used to map JSR packages to npm
packages for systems that do not natively support JSR. The `@jsr` npm scope is
not a real npm scope, and you cannot publish packages to it. You can only use it
to install JSR packages from npm.

`@jsr` packages are not served from the npm registry at
`https://registry.npmjs.org`. Instead, they are served from the JSR registry at
`https://npm.jsr.io`. This endpoint implements the npm registry API. For example
to get the metadata for the `@jsr/luca__flag` package, you can send a `GET`
request to `https://npm.jsr.io/@jsr/luca__flag`.

This endpoint serves npm compatible tarballs for `@jsr` packages. These tarballs
are generated by JSR, and contain all source code reachable from the entrypoint
of the package. This source code is transpiled to JavaScript, and TypeScript
type declarations (`.d.ts` files) are generated for all TypeScript files. The
tarball also contains a `package.json` file that contains the `exports` field
from the original `jsr.json` / `deno.json` file.

Yanked versions of packages are not advertised in the package version manifest
of the npm registry endpoint. Tarballs for yanked versions are still available
even when a version is yanked, which means that tools that have a reference to a
yanked version in a lockfile will still be able to install that version.

The specific tarballs advertised for a given version of a package may change
over time, even if the version itself is not changed. This is because the JSR
registry may re-generate npm compatible tarballs for a package version to fix
compatibility issues with npm or improve the transpile output in the generated
tarball. We refer to this as the "revision" of a tarball. The revision of a
tarball is not advertised in the npm registry endpoint, but it is included in
the URL of the tarball itself and is included in the `package.json` file in the
tarball at the `_jsr_revision` field. The revision of a tarball is not
considered part of the package version, and does not affect semver resolution.

However, tarball URLs are immutable. Tools that have a reference to a specific
tarball URL will always be able to download that exact tarball. When a new
revision of a tarball is generated, the old tarball is not deleted and will
continue to be available at the same URL. The new tarball will be available at a
new URL that includes the new revision.

Because the tarball URL is included in package manager lock files, running
`npm i` / `yarn` / `pnpm i` will never accidentally download a new revision of
the tarball.
