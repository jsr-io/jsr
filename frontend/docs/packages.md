---
title: Packages
description: Packages are collections of JavaScript or TypeScript code published by an author to JSR. Learn how to create and manage packages.
---

Packages are the core of JSR. Packages are collections of JavaScript or
TypeScript code published by an author to the JSR site. Packages can be imported
by other users using `jsr:` imports.
[Learn more about using JSR](/docs/using-packages).

Packages are contained by scopes. Scopes are collections of packages published
by an author to the JSR site. Scopes are similar to npm organizations or GitHub
accounts. [Learn more about scopes](/docs/scopes).

Packages have a name. Package names are unique within a scope - no two packages
in the same scope can have the same name. Package names must be between 2 and 20
characters long, and can only contain lowercase letters, numbers, and hyphens.
They cannot start with a hyphen or digit.

Packages can have a description. The description is a short blurb about the
package that is displayed on the package page and in search results.
Descriptions should summarize what the package does for potential users. The
description can be up to 250 characters long. The description can be updated
from the "Settings" tab on the package page.

Packages can be created at [jsr.io/new](/new). Packages are always created in a
scope, so a scope must be created before creating a package.

## Linked GitHub repository

Packages can have a linked GitHub repository. This repository is shown to users
of a package and can be used to link to the source code and issue tracker for
the package.

The GitHub repository shown on the package page is verified by JSR to be
administered by the package author. This prevents package authors from linking
to repositories they do not own.

A GitHub repository can be linked to a package from the "Settings" tab on the
package page. Only scope admins can link a GitHub repository to a package. Scope
admins can also unlink a GitHub repository from the same page. To link a
repository, one must be an admin of the repository on GitHub.

Linking a GitHub repository also enables tokenless publishing from GitHub
Actions using OIDC.
[Learn more about publishing from GitHub Actions](/docs/publishing-packages#publishing-from-github-actions).

## Deleting a package

A package can be deleted from the "Settings" tab on the package page. Only scope
admins can delete a package.

Packages can only be deleted if they have no published versions. If a package
has published versions, it can not be deleted.
[Learn more about registry immutability](/docs/immutability).

## Versions

Code in a package is published as a version. A version is a snapshot of the
package's code at a point in time. Versions are immutable - once a version is
published, it cannot be changed or deleted. This ensures that packages are
reliable and that users can trust that a package will not change out from under
them. [Learn more about registry immutability](/docs/immutability).

Versions are published using the `jsr publish` or `deno publish` command.
[Learn more about publishing packages](/docs/publishing-packages).

Versions must be valid [SemVer](https://semver.org/) versions. We recommend that
packages follow semantic versioning policies. These work as follows:

- If you make a breaking change, bump the major version.
- If you add a feature in a backwards compatible manner, bump the minor version.
- If you fix a bug in a backwards compatible manner, bump the patch version.

To publish a new version of a package, you must bump the version in your config
file before running `jsr publish` or `deno publish`.

### Yanking versions

Package versions can not be deleted. However, sometimes you may want to prevent
users from using a specific version of your package, for example because it
contains a critical bug. In this case you can "yank" the version.

Yanking a version does multiple things:

- Semver resolution ignores the version when resolving dependencies.
- It shows a warning on the package page that the version has been yanked.
- It prevents the version from being shown in search results.
- It marks the version as yanked in the package's version list.
- Yanked versions are not considered when determining the latest version of a
  package.

Yanking a version does not prevent users from downloading that version
explicitly, viewing documentation, or viewing the source code. Particularly, if
a user has already downloaded the version, have it in their lock file, or are
explicitly specifying the version in their import, they can still use it.

To yank a version, head to the "Versions" tab on the package page and click the
"Yank" button next to the version you want to yank. Only scope admins can yank
versions. Versions can be unyanked by clicking the "Unyank" button next to the
version on the same page.

To illustrate the difference between deleting and yanking a version, consider
the following scenario:

- You publish version `1.0.0` of your package.
- You publish version `1.0.1` of your package.
- You discover a critical bug in version `1.0.1` and want to prevent users from
  using it.
- You yank version `1.0.1`.

At this point, users can still use version `1.0.1` if they have already
downloaded it, or if they explicitly specify the version in their import like
so:

```ts
import { foo } from "jsr:foo@1.0.1";
```

However, if a user does not have version `1.0.1` downloaded, and does not
explicitly specify the version in their import, they will get version `1.0.0`
for the following import:

```ts
import { foo } from "jsr:foo@1";
```

## Documentation

Documentation for a package is automatically generated from the package's source
code. Documentation is generated for all exported functions, classes, and
variables in the package using
[JSDoc comments](https://jsdoc.app/about-getting-started).

JSR uses `deno doc` to generate documentation. This means that one can preview
how a package's documentation will look on JSR, by running `deno doc --html`
locally. This will generate HTML files with very similar looking documentation
to what is shown on the JSR site.

The "Overview" tab on the package page shows the module doc of the default
entrypoint of the package. If the package does not have a default entrypoint, or
the default entrypoint does not have a module doc, then the "Overview" tab will
show the README of the package instead. If no README is present, then the
"Overview" tab will only show the package outline in the sidebar.

The sidebar at the base of the package page contains links to all exports from
the default entrypoint of the package, and links to all other entrypoints in the
package. Clicking these links will show the documentation for the specific
symbol or entrypoint selected.

Providing useful and relevant documentation for a package is important to make
it successful. Users will often look at the documentation of a package before
deciding to use it. We recommend that you write documentation for all exported
functions, classes, and variables in your package.
[Learn more about writing documentation](/docs/writing-docs).

## Publishing

Packages are published using the `jsr publish` or `deno publish` command.
[Learn more about publishing packages](/docs/publishing-packages).

The "Publish" tab on the package page contains helpful information about
publishing packages, like an outline for the config file, and instructions for
publishing from GitHub Actions.
