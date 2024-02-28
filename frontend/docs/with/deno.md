---
title: Using JSR with Deno
description: Learn how to use JSR packages with Deno.
---

[**Deno**](https://deno.land) is a secure runtime for JavaScript and TypeScript.

Deno has [native support for JSR](/docs/using-packages#native-jsr-imports). JSR
packages can be used in Deno by importing them using the `jsr:` scheme:

```ts
import { printProgress } from "jsr:@luca/flag@1";
```

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
