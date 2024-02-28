---
title: Migrating Deno modules from /x to JSR
description:
  Packages currently hosted on deno.land/x can be migrated to JSR. Follow this
  guide to learn how.
---

> **NOTE:** This guide is for Deno users only. If you are using JSR with Node or
> in a different runtime environment, this information does not pertain to you.

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

### 1.) Refactor away from HTTPS imports

Your code may be using HTTPS imports for dependencies on deno.land/x or
deno.land/std. You will need to change how you load these dependencies before
you can publish on JSR.

**Update standard library imports to the `@std` JSR scope**

If you are using HTTPS modules from the standard library, we recommend updating
those dependencies to use the newer [@std scope on JSR](https://jsr.io/@std).
This will be the place to get the latest version of these modules going forward.

**Use `deno vendor` for other dependencies on deno.land/x**

For other dependencies in your project, you can also replace them one by one
with equivalent dependencies on npm or JSR as described above.

If you find that this process would be prohibitively difficult, you also have
the option of using the
[`deno vendor`](https://docs.deno.com/runtime/manual/tools/vendor) command to
download local versions of all your HTTPS dependencies, and store them alongside
your package in source control.

### 2.) Ensure your library does not contain slow types

In your main package directory, run the following command:

```
deno publish --dry-run
```

This will inform you of any issues within your TypeScript code that may slow
down type checking. [Check out the docs here](/docs/about-slow-types) for more
information on how to fix slow types.

### 3.) Publish!

When you've completed the tasks above,
[follow these instructions](/docs/publishing-packages) to publish your package
to JSR!
