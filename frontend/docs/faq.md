---
title: Frequently Asked Questions
description: Answers to frequently asked questions about JSR.
---

### What is JSR?

JSR is a new package registry for JavaScript. It is a free alternative to npm,
that is designed to work well with all JavaScript tools and runtimes, like
Node.js, Deno, Bun, Vite, and more.

[Learn more about why we built JSR.](/docs/why)

### How do I use JSR?

If you want to consume packages, read the [Using packages](/docs/using-packages)
guide.

If you want to publish packages, read the
[Publishing packages](/docs/publishing-packages) guide.

### Is JSR a package manager like `npm`, `yarn`, or `pnpm`?

No. JSR is a package registry (the server storing the packages), not a package
manager (the tool that installs packages). You can use existing package managers
like `npm`, `yarn`, or `pnpm` with JSR.

### Is JSR open source?

Yes. JSR is open source and licensed under the MIT License. You can find the
source code on [GitHub](https://github.com/jsr-io/jsr).

### How is JSR different from npm?

JSR is designed to be a superset of npm, allowing existing tools that use npm to
seamlessly work with JSR. JSR is designed to be better than npm:

- Automatic documentation generation
- Package scoring
- Native TypeScript support
- No need for build steps, which leads to better user experience for package
  publishers
- Secure, token-less publishing for resistance against supply chain attacks
- and more... [Learn more about why we built JSR.](/docs/why)

### How is JSR funded? / Do I have to pay for JSR?

JSR is designed to be a public good for the JavaScript community, and will thus
always be free to use.

Currently hosting bills for JSR are paid for by the
[Deno Company](https://deno.com). In the future, JSR may be funded by
alternative means, like sponsorships, donations, or a foundation. We expect that
the Deno Company will be able to continue paying for JSR's hosting bills for the
foreseeable future - JSR is designed to be very cheap to run.

### Can I delete a package from JSR?

Source code published to JSR can not be deleted.
[Learn more about immutability.](/docs/immutability)

You can "yank" a version of a package, which will hide the version from the
default view, but it will still be available to users who depend on it.
[Learn more about yanking.](/docs/packages#yanking-versions)

You may delete a package if it has no published versions.
[Learn more about deleting empty packages.](/docs/packages#deleting-a-package)

### Why are there quotas for scopes and package versions on JSR?

To prevent abuse, JSR has quotas for scopes and package versions. These quotas
are designed to be generous and should not affect your normal usage. If you need
a quota increase, please reach out to us at quotas@jsr.io - we will happily
increase your quota if you run into it.

### What is JSR's policy on name squatting?

We disallow name squatting on JSR. We define name squatting as the act of
registering a scope / package name with no intention of using it, or to prevent
someone with a legitimate use from using it (e.g. to sell it to them).

[Learn more about the JSR usage policy](/docs/usage-policy).

### How do I report a security vulnerability in JSR?

Please follow the Deno Company's
[security policy](https://docs.deno.com/deploy/manual/security) to report
security vulnerabilities in JSR infrastructure.

You may also report security vulnerabilities in individual packages to the
package's scope owner. If the scope owner is unresponsive, please reach out to
us at security@jsr.io.

### How do I report a bug in JSR?

> During the open beta, please email us at help@jsr.io, or chat in the `#jsr`
> channel on the Deno Discord (https://discord.gg/deno).

Please open an issue on the JSR GitHub repository at
[jsr-io/jsr](https://github.com/jsr-io/jsr).

### Why does JSR ask to "Act on your behalf" when I log in with GitHub?

When signing in with GitHub, GitHub presents you with a screen that asks for
your permission to authorize JSR. This screen includes a list of resources that
JSR can access (only your email addresses and public profile information).

This screen also contains the text "Act on your behalf" which suggests that JSR
can perform arbitrary actions on your behalf. **This is misleading, and not
true.** JSR can act on your behalf only to read any resources you have granted
it access to (your email addresses and public profile information). It cannot
perform arbitrary actions on your behalf.

JSR can not modify your GitHub account, create repositories, create comments, or
do anything else that would be considered "acting on your behalf". GitHub
presents this "Act on your behalf" screen to all GitHub Apps, regardless of what
permissions they actually request.

You can read more about this in
[the GitHub documentation](https://docs.github.com/en/apps/using-github-apps/authorizing-github-apps#about-github-apps-acting-on-your-behalf)
and in
[this discussion thread](https://github.com/orgs/community/discussions/37117) on
the official GitHub Discussions feedback forum.

## Can I filter packages by compatible runtime in the search?

Yes! You can filter the package search to only return packages that are
compatible with one or more runtimes, by using the `runtime` filter in the
search query.

To filter for packages that are compatible with Deno, you can use the query
`runtime:deno`. To filter for packages that are compatible with Node.js, you can
use the query `runtime:node`. You can also combine these filters, for example
`runtime:deno runtime:browsers` will return packages that are compatible with
both Deno and web browsers. The possible values for the `runtime` filter are
`deno`, `node`, `browsers`, `workerd` (Cloudflare Workers), and `bun`.

## Can I filter packages by scope in the search?

Yes! You can specify `scope:` in the search query to filter packages by scope.
To only return packages in the `std` scope, you can use the query `scope:std`.

## Is "JSR" an initialism for "JavaScript Registry"?

Yes.
