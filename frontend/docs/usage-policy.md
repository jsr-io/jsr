---
description: This document describes the usage policy for JSR.
---

_Last updated: March 6th, 2024_

JSR is a public registry service for the JavaScript and TypeScript ecosystem. It
is designed to be a reliable and secure place to publish and discover packages.
To ensure that JSR remains a reliable and secure place for everyone, we have a
usage policy that all users must follow to use the service.

We reserve the right to suspend or terminate access to JSR for any account that
violates this usage policy.

## Package contents and metadata

### Acceptable use

JSR may only be used to publish and consume packages that consist of source code
destined to be executed in a runtime that supports JavaScript or TypeScript.

It is acceptable to serve package contents directly to browsers for development
purposes.

It is acceptable to publish tooling that is designed to analyze or identify
malware.

### Unacceptable use

Packages must not grossly misrepresent their purpose or functionality to deceive
users. This includes, but is not limited to, packages that are designed to
collect user data without consent, to perform malicious actions, to impersonate
other packages, or to violate the privacy or security of users.

Malware, adware, or any other software that is designed to harm users or their
devices is not allowed on JSR.

It is not acceptable to use JSR as a CDN for serving assets directly to web
applications in a browser, except for development purposes. JSR is not a general
purpose file hosting service.

JSR may not be used to distribute images, videos, or other documents that are
not directly related to JavaScript or TypeScript packages.

Content that is considered illegal based on the laws of the United States of
America is not permitted on JSR.

### Examples

- You MAY use JSR to publish a package that provides a JavaScript library for
  performing calculations.
- You MAY use JSR to publish a package that provides a TypeScript library for
  working with dates.
- You MAY use JSR to publish a package that contains SVG icons that are used by
  a JavaScript library.
- You MAY include a PNG image of the package logo in the package, to display it
  in the package README.
- You MAY NOT use JSR to publish a package that contains an MP4 file of a movie.
- You MAY NOT use JSR to publish a package that contains many pictures of cats.
- You MAY NOT use JSR to publish a package that scrapes the /etc/passwd file
  from the user's system and uploads it to a remote server, unless this is the
  intended purpose of the package and this is made clear to the user.
- You MAY NOT use JSR to publish a package that contains a virus.
- You MAY use JSR to publish a package that contains a tool for analyzing
  malware, as long as the tool is not itself malware.

## Scope names

JSR organizes packages into scopes. A scope is a collection of packages that are
related to each other in some way. For example, the `@deno` scope contains
packages that are related to the Deno runtime.

### Name guidelines

Scope names should generally be one of:

- A personal username (e.g. `@ry`).
- An organization name (e.g. `@deno`).
- A project name (e.g. `@fresh`).

Project names should not be too generic. For example, `@ai` is a very generic
name and is likely to be used by many different projects. This is not a good
scope name.

For most packages, you should register them under a personal username or an
organization name. If you are publishing a package that is part of a larger
project, you can register it under the project name (e.g. `@vite/plugin-node`).

The enforcement of this policy is at the discretion of the JSR moderators. There
is unfortunately not a clear cut rule for what is too generic, but we will do
our best to be reasonable and fair. If you are unsure if a scope name is too
generic, please reach out to us at [help@jsr.io](mailto:help@jsr.io).

### Scope name squatting

JSR disallows name squatting of scope names. We define name squatting as the act
of registering a scope / package name with no intention of using it, or to
prevent someone with a legitimate use from using it (e.g. to sell it to them).

We will be reasonable in our enforcement of name squatting policies. We
understand that sometimes people reserve names with the intention of using them,
but then never get around to it. We will always reach out to the current scope
owner to hear their side of the story, and come to a reasonable resolution for
all parties involved.

### Sale of scope names

Scope names may not be registered with the intention of selling them to others.
Selling a scope name is expressly prohibited and will result in the suspension
of the account that registered the scope name.

If you are contacted by someone who is attempting to sell you a scope name, or
buy a scope name from you, please report this to us at
[help@jsr.io](mailto:help@jsr.io).

It is ok to offer a user a small gratuity after a scope name has exchanged
hands, such as some stickers or a T-shirt, but this occurring must never be a
condition of the scope name transfer.

### Trademark and copyright

We reserve the right to reclaim scope names that violate copyright, trademark,
or other laws.

## Reporting violations

If you believe that a package on JSR violates this usage policy, please open a
ticket via the "Report Package" button located on the package page.

## Changes to this policy

We may change this usage policy at any time. We will notify users of any changes
to this policy by updating the "Last updated" date at the top of this document.
We encourage users to review this policy regularly to stay informed about
acceptable usage of JSR.
