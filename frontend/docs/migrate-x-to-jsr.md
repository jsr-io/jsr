---
title: Migrating Deno modules from /x to JSR
description:
  Packages currently hosted on deno.land/x can be migrated to JSR. Follow this
  guide to learn how.
---

> **NOTE:** This guide is for Deno users only. If you are using JSR with Node.js
> or in a different runtime environment, this information does not pertain to
> you.

For package authors currently hosting modules on
[deno.land/x](https://deno.land/x), it should be possible to migrate those
modules to JSR with minimal changes. In this guide, we'll describe changes in
using HTTPS imports from Deno, comment on future support plans for deno.land/x,
and describe the process for migration.

## HTTPS modules supported in Deno; _NOT_ in JSR packages

[HTTPS imports](https://docs.deno.com/runtime/manual/basics/modules) will
continue to be supported in Deno. Any code that uses modules hosted on
deno.land/x, deno.land/std, and other URLs will continue to work indefinitely.

However, Deno code on JSR will **NOT** be permitted to use HTTPS imports. JSR
performs deduplication of dependencies based on semantic versions, which is not
possible with HTTPS-imported dependencies. If you attempt to publish code that
contains HTTPS imports to JSR, you will receive an error.

## Future support for deno.land/x

**There are no plans to discontinue or shut down deno.land/x**. Module authors
can continue to publish and update modules there, and Deno code using it will
continue to function.

## Migrating a package from /x to JSR

If you have already published a package on deno.land/x, there is a good chance
it can be migrated to JSR with minimal hassle. Here are the high level steps to
migrate.

### Try using the /x to JSR migration tool

In an attempt to help speed up the migration of existing /x packages to JSR, the
Deno team has created [a utility](https://github.com/denoland/x-to-jsr) to
automate the most common steps required. To use this tool, you will need to have
the most recent version of the Deno CLI installed. Grab the most recent canary
build for your platform with:

```bash
deno upgrade --canary
```

Then, from within your package folder (probably the one with your `deno.json` or
`mod.ts`), execute the following command:

```bash
deno run -Ar jsr:@deno/x-to-jsr
```

This will automatically refactor code where possible, and provide instructions
in your terminal for additional manual steps that may be required. When you've
completed the tasks described by the migration tool,
[follow these instructions](/docs/publishing-packages) to publish your package
to JSR!

### Manually refactor your package

If you would rather migrate your project by hand, here are the high-level steps
often required to do so.

#### 1.) Refactor away from HTTPS imports

Your code may be using HTTPS imports for dependencies on deno.land/x or
deno.land/std. You will need to change how you load these dependencies before
you can publish on JSR.

_Update standard library imports to the `@std` JSR scope_

If you are using HTTPS modules from the standard library, we recommend updating
those dependencies to use the newer [@std scope on JSR](https://jsr.io/@std).
This will be the place to get the latest version of these modules going forward.

_Use `deno vendor` for other dependencies on deno.land/x_

For other dependencies in your project, you can also replace them one by one
with equivalent dependencies on npm or JSR as described above.

If you find that this process would be prohibitively difficult, you also have
the option of using the
[`deno vendor`](https://docs.deno.com/runtime/manual/tools/vendor) command to
download local versions of all your HTTPS dependencies, and store them alongside
your package in source control.

#### 2.) Ensure your library does not contain slow types

In your main package directory, run the following command:

```
deno publish --dry-run
```

This will inform you of any issues within your TypeScript code that may slow
down type checking. [Check out the docs here](/docs/about-slow-types) for more
information on how to fix slow types.

#### 3.) Publish!

When you've completed the tasks above,
[follow these instructions](/docs/publishing-packages) to publish your package
to JSR!
