---
title: Using JSR with Deno
description: Learn how to use JSR packages with Deno.
---

[**Deno**](https://deno.com) is a secure runtime for JavaScript and TypeScript.
It has native support for JSR imports using either an import map or `jsr:`
specifiers.

## Using import maps

Deno supports web standard
[import maps](https://docs.deno.com/runtime/manual/basics/import_maps), and a
special `imports` section of the `deno.json` configuration file. You can add JSR
imports manually to `deno.json`, or you can add them using the `deno add`
command.

In this example, we add the most recent version of
[`@luca/flag`](https://jsr.io/@luca/flag) to your project.

```bash
deno add @luca/flag
```

After executing this command, you will have an import map entry in `deno.json`
that looks something like this:

```json
{
  "imports": {
    "@luca/flag": "jsr:@luca/flag@^1.0.1"
  }
}
```

You can then use the module from code like this:

```ts
import { printProgress } from "@luca/flag";
printProgress();
```

## Using JSR specifiers

In Deno, you can use packages without an install step by using `jsr:` specifiers
directly within your source files. In this example, we import the `@luca/flag`
module directly in our code, without needing an import map entry.

```ts
import { printProgress } from "jsr:@luca/flag@^1.0.1";

printProgress();
```

Learn more about [using JSR packages](/docs/using-packages).

## Additional tooling

Deno provides a variety of additional tools to help manage JSR packages.

### Publishing

The `deno publish` command can be used to publish JSR packages. Packages can be
published from a local machine, or from CI.

Learn more about [publishing JSR packages](/docs/publishing-packages).

### Documentation linting and preview

Deno provides a built-in lint rule to ensure that all exported functions and
variables in a JSR package have JSDoc comments.

The documentation linter can be run using the `deno doc --lint` command.

```shell
deno doc --lint
```

Deno can also generate an HTML version of the documentation for your package.
This is a good way to preview how a package's documentation will look on
[jsr.io](https://jsr.io) once it's published.

```shell
deno doc --html
```

### Dependency analysis

The `deno info` command can be used to analyze the dependencies of a JSR
package. This command will display a list of all dependencies, including their
version constraints.

```shell
$ deno info jsr:@std/fs
```
