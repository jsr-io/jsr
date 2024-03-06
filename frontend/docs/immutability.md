---
title: Immutability
description: JSR is immutable. This means that once you publish a package version, you can't change it. Learn more about immutability in JSR.
---

JSR is immutable. This means that once you publish a package version, you can't
change it. This is a core design principle of JSR, and is a key part of what
makes JSR reliable and secure.

## Why is JSR immutable?

JSR is immutable for a few reasons:

- **Reliability**: JSR is immutable to ensure that packages are reliable. If
  packages could be changed after they were published, then users could not
  trust that a package would work the same way every time they used it. This
  would make JSR much less useful.

- **Security**: JSR is immutable to ensure that packages are secure. If packages
  could be changed after they were published, then users could not trust that a
  package would not be malicious after they have audited a package previously.

- **Simplicity**: JSR is immutable to ensure that the JSR registry is simple.
  Immutability makes the JSR registry much simpler to implement, which makes it
  easier to maintain and more reliable. It simplifies caching, and makes it
  easier to reason about the registry.

## What does immutability mean for me?

Immutability means that once you publish a package version, you can't change it.
This means that you must be careful when publishing a package version. You
should make sure that the version you are publishing does not contain any
secrets.

If you need to change a package version, you can publish a new version. You
should bump the version number in your config file before publishing a new
version.

## What if I need to change a package version?

You can't change the contents a package version after it has been published.
However, you can publish a new version of your package. You should bump the
version number in your config file before publishing a new version.

## What if I need to delete a package version?

You can't delete a package version after it has been published. However, you can
publish a new version of your package and yank the old version.
[Learn more about yanking.](/docs/packages#yanking-versions)

Note that yanking does not remove the contents of the package version from the
registry. It only superficially hides the version from users in some places.

## I published a secret / personal info by accident, what do I do?

If you accidentally published a secret, you should immediately revoke the
secret.

If you published personal information that you would like to remove from the
registry, please contact us at [help@jsr.io](mailto:help@jsr.io).

Please note that while we'll do our best to scrub the registry of the sensitive
information, we can't guarantee that we'll be able to remove all copies of the
information from the registry and caches immediately. Additionally, users may
have already downloaded the package version and may have a copy of the sensitive
information.
