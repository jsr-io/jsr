---
title: Troubleshooting
description: Troubleshooting common issues with JSR.
---

When using JSR you may encounter an error. This page will help you to understand
what the error means and how to fix it.

## Publishing errors

These errors may occur when publishing a package to JSR.

### `linkInTarball`

The package being published contains a symlink or hardlink. JSR does not support
symlinks or hardlinks in packages. You can fix this error by removing the
symlink or hardlink from your package, or by
[excluding it](/docs/publishing-packages#ignoring-files) in your `jsr.json` /
`deno.json`.

To find the symlink or hardlink, run the following command in your package
directory:

```sh
# Linux and macOS
find . -type l -o -type h
```

```powershell
# Windows
Get-ChildItem -Recurse | Where-Object { $_.Attributes -match "ReparsePoint" }
```

### `invalidEntryType`

The package tarball contains an entry that is not a regular file or directory.
JSR only supports regular files and directories in package tarballs. You can fix
this error by removing the invalid entry from your package tarball or
[excluding it](/docs/publishing-packages#ignoring-files) in your `jsr.json` /
`deno.json`.

### `invalidPath`

The published package contains a file or directory with a path that JSR does not
allow. The message of the error contains the invalid path, and what is wrong
with it.

Generally, JSR does not allow paths that are invalid on Windows, paths that are
troublesome in URLs, and paths where multiple casings of the same path exist.

You can fix this error by changing the path of the file or directory in your
package to a path that JSR allows, removing the file or directory from your
package, or [excluding it](/docs/publishing-packages#ignoring-files) in your
`jsr.json` / `deno.json`.

Path rules are as follows:

- Less than 155 chars
- The last component (filename) of the path must be less than 95 chars
- Path must not end in a slash
- Must not contain a double slash (`//`)
- Must not contain a `.` or `..` path segment
- No path segment is a Windows reserved name like `CON` or `PRN`
- No path segment ends in a dot (`.`)
- Does not contain a Windows path separator (`\` or `:`)
- Does not contain an invalid Windows path char (`<`, `>`, `"`, `|`, `?`, `*`)
- Does not contain whitespace (`\s`, `\t`, `\n`, `\r`)
- Does not contain chars that have a special meaning in URLs (`%` or `#`)
- Does not contain other chars that are not one of `a-z`, `A-Z`, `0-9`, `$`,
  `(`, `)`, `+`, `-`, `.`, `@`, `[`, `]`, `_`, `{`, `}`, or `~`
- Does not start with `/_dist/`, as this is reserved for the directory JSR emits
  `.js` and .`d.ts` files to when building an npm tarball

### `invalidExternalImport`

The package being published contains an external import that is not allowed by
JSR. JSR only allows external imports that are `jsr:`, `npm:`, `data:`, or
`node:` specifiers.

You can fix this error by removing the external import from your package, or by
replacing it with an external import from a supported source.

### `globalTypeAugmentation`

The package being published contains global type augmentation. This is
disallowed because it introduces a "slow type".
[Learn more about "slow types"](/docs/about-slow-types).

You can fix this error by removing the global type augmentation from your
source.

### `commonJs`

The package being published contains CommonJS code like `require()`. This is
disallowed because JSR is ESM only.

You can fix this error by removing the CommonJS code from your source.

### `bannedTripleSlashDirectives`

The package being published contains a triple slash directive that is not
allowed by JSR. JSR only allows triple slash directives that are
`/// <reference types="..." />` directives.

`/// <reference lib="..." />` directives and
`/// <reference no-default-lib="true" />` are not allowed.

You can fix this error by removing the triple slash directive from your source.

### `bannedImportAssertion`

The package being published contains the legacy "import assertions" syntax,
which is not allowed by JSR. JSR only allows the new "import attributes" syntax.

`import "./data.json" assert { type: "json" };` is not allowed.
`import "./data.json" with { type: "json" };` is allowed.

You can fix this error by updating the import assertion to an import attribute,
by replacing `assert` with `with`.

### `fileTooLarge`

The package being published contains a file that is too large. JSR only allows
files that are less than 4MB in size. You can fix this error by removing the
file from your package or by excluding it in your config file.

[Learn more about limits](/docs/quotas-and-limits#other-limits).

### `packageTooLarge`

The package being published is too large. JSR only allows packages that are less
than 20MB in size. You can fix this error by removing large files from your
package or by excluding them in your config file.

[Learn more about limits](/docs/quotas-and-limits#other-limits).

If you are unable to exclude enough files to get your package under the limit,
[contact support to request a limit increase](/docs/quotas-and-limits).

### `caseInsensitiveDuplicatePath`

The package being published contains a file or directory with a path that is
already used by another file or directory in the package, but with a different
casing. JSR does not allow this because it can cause problems on case
insensitive file systems like NTFS on Windows.

You can fix this error by removing one of the files or directories from your
package, renaming one of the files or directories in your package, or excluding
one of the files or directories in your config file.

### `missingConfigFile`

The package being published does not contain a config file. JSR requires all
packages to contain a config file to read metadata like `exports`.

You can fix this error by adding a config file to your package.

### `invalidConfigFile`

The package being published contains a config file that was not valid JSON(C)
(has syntax errors).

You can fix this error by fixing your config file to be valid JSON(C).

### `configFileNameMismatch`

The package being published contains a config file that has a `name` field that
does not match the name of the package being published. JSR requires the `name`
field of the config file to match the name of the package being published.

You can fix this error by changing the `name` field of your config file to match
the name of the package being published.

### `configFileVersionMismatch`

The package being published contains a config file that has a `version` field
that does not match the version of the package being published. JSR requires the
`version` field of the config file to match the version of the package being
published.

You can fix this error by changing the `version` field of your config file to
match the version of the package being published.

### `configFileExportsInvalid`

The package being published contains a config file that is either missing an
`exports` field, or has an `exports` field that is not valid.
[Learn more about exports](/docs/publishing-packages#package-config-file).

You can fix this error by updating the `exports` field of your config file to be
one of the two valid forms:

```json
{
  "exports": "./mod.ts"
}
```

```json
{
  "exports": {
    ".": "./mod.ts",
    "./greet": "./greet.ts"
  }
}
```

### `graphError`

The package being published references a module that does not exist, or has a
syntax error. JSR requires all modules referenced from the entrypoint to be
valid.

You can fix this error by fixing the module that has the error.

### `docError`

The package being published fails to generate documentation with `deno doc`.
This is likely because the package contains a syntax error.

If you think this is a bug, please contact support at
[help@jsr.io](mailto:help@jsr.io).

### `invalidJsrSpecifier`

The package being published contains a module that references a JSR specifier
that is not valid. JSR specifiers must be in the form
`jsr:@<scope>/<name>@<version>/<path>` or
`jsr:/@<scope>/<name>@<version>/<path>`. You can fix this error by updating the
module to reference a valid JSR specifier.

### `invalidNpmSpecifier`

The package being published contains a module that references an npm specifier
that is not valid. npm specifiers must be in the form
`npm:<name>@<version>/<path>` or `npm:/<name>@<version>/<path>`. You can fix
this error by updating the module to reference a valid npm specifier.

### `actorNotAuthorized`

The package is being published with an access token that does not have
permission to publish to the scope.

This can happen when publishing from GitHub Actions, if the GitHub repository
being published from is not linked to the package you are trying to publish. You
can fix this error by linking the GitHub repository to the package you are
trying to publish in the package settings.

If you are not publishing from GitHub Actions, you can fix this error by using
an access token that has permission to publish to the scope.

### `actorNotScopeMember`

The package is being published with an access token corresponding to a user that
is not a member of the scope.

This can happen when publishing from GitHub Actions if the user that invoked the
Actions workflow is not a member of the scope, if
[publishing is restricted to scope members (default)](/docs/scopes#github-actions-publishing-security).
You can fix this by adding the user to the scope, or by changing the GitHub
Actions security settings on the scope to not require the publishing user to be
a member of the scope.

If you are not publishing from GitHub Actions, you can fix this error by
authenticating as a user that is a member of the scope, or by adding the user to
the scope with at least the "member" role.

## Excluded module error

After filtering files, you may encounter an `excluded-module` error saying that
a module in the package's module graph was excluded from publishing.

This may occur when you've accidentally excluded a module that is used in the
published code (for example, writing `"exclude": ["**/*.ts"]` and then trying to
publish a package with a `mod.ts` export). In this scenario, JSR is preventing
you from accidentally publishing a package that won't work.

To fix the issue, ensure the module mentioned in the error message is not
excluded in `exclude` and/or `publish.exclude` in the config file, or don't
reference it in any code used by your package's exports.

You can find all files that are being included in the package by running the
following command in your package directory:

```sh
npx jsr publish --dry-run
# or
deno publish --dry-run
```
