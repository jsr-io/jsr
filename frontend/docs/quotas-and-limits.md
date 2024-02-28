---
title: Quotas and limits
description: JSR enforces some quotas and limits to ensure that the site is fast and reliable for everyone, and to prevent name squatting.
---

JSR enforces some quotas and limits to ensure that the site is fast and reliable
for everyone, and to prevent name squatting.

Quotas can be increased by contacting JSR support at
[quotas@jsr.io](mailto:quotas@jsr.io). When emailing support, make sure to
include the affected scope / user / package, the quota you would like to
increase, and the reason for the increase.

Don't see quotas as limits. If you need more than the default quota, please
reach out to us and we'll be happy to increase it. Quotas are in place to
prevent abuse, not to prevent you from using JSR to it's fullest.

## Quotas per account

You can find the current usage of these quotas in your
[account settings page](/account/settings).

Every user has a scopes quota. This is the number of scopes that a user can own
at a time. The default quota is 3. This quota exists to prevent name squatting
of scopes. This only applies to scopes that you own, not scopes that someone
else owns that you are a member / admin of.

These quotas can be increased by [contacting jsr support](mailto:quotas@jsr.io).

## Quotas per scope

You can find the current usage of these quotas in the "Settings" tab of your
scope.

Every scope has a packages quota. This is the number of packages that a scope
can contain at any one time. The default quota is 100. This quota counts both
packages with and without versions. This rate limit exists to prevent abuse.

Every scope has a weekly package creation limit. This is the number of packages
that can be created in a scope in a 7 day rolling window. The default rate limit
is 20. This rate limit exists to prevent abuse.

Every scope has a weekly publish attempts limit. This is the number of packages
you can attempt to publish with `jsr publish` / `deno publish` in a scope in a 7
day rolling window. The default rate limit is 1000. This rate limit exists to
prevent abuse.

These quotas can be increased by [contacting jsr support](mailto:quotas@jsr.io).

## Other limits

- The gzipped tarball of an uploaded package must be less than 20MB.
- The sum of all files in a given package version must be less than 20MB.
- No individual file in a package can be larger than 20MB.

These quotas can be increased by [contacting jsr support](mailto:quotas@jsr.io).
