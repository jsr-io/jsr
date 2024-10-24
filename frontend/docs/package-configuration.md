---
description: JSR packages are configured using a jsr.json file.
---

JSR packages need to contain a configuration file that specifies the name,
version, and exports of a package. This file should be named `jsr.json` or
`jsr.jsonc`. When using Deno, all properties of the `jsr.json` configuration
file can instead be placed in the
[`deno.json`](https://docs.deno.com/runtime/manual/getting_started/configuration_file).

```json
// jsr.json / deno.json
{
  "name": "@luca/greet",
  "version": "1.0.0",
  "exports": "./mod.ts"
}
```

### `name`

The `name` field is the name of your package, prefixed with a JSR scope.
[Learn more about scope and package names](/docs/publishing-packages#creating-a-scope-and-package).

### `version`

The `version` field is the version of your package, which must be a valid
[SemVer](https://semver.org/) version. You must increment the version of your
package every time you publish a new version.
[Learn more about package versions](/docs/packages#versions).

### `exports`

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
you only have a single entrypoint in your package. It is semantically equivalent
to specifying a default entrypoint in the object form.

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

### `include` and `exclude`

You can also use the `include` and `exclude` options to include and exclude
files during publishing. When using a `deno.json`, you can use `publish.include`
and `publish.exclude` to include and exclude files only for publishing, rather
than for all Deno subcommands.
[Learn more about filtering files](/docs/publishing-packages#filtering-files).

## JSON Schema

A JSON schema file is available for editors to provide autocompletion. The file
is versioned and available at: https://jsr.io/schema/config-file.v1.json
