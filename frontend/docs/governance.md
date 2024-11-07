---
title: Governance
description: JSR is a project built for the entire JavaScript ecosystem. This document outlines the current and future governance structure of the project.
---

JSR is not owned by any one person or organization. It is a community-driven
project that is open to all, built for the entire JavaScript ecosystem.

JSR is currently operated by the Deno company. We are currently working on
establishing a governance board to oversee the project, which will then work on
moving the project to a foundation.

## Mission

The mission of the JSR project is to provide a widely adoptable, open,
community-first, modern platform for sharing JavaScript and TypeScript code.

## Governance Board

The governance board is responsible for overseeing the project and ensuring that
it remains true to its mission. The governance board has not yet been formed,
but will consist of community members from various organizations relevant to the
JavaScript ecosystem.

The governance board will be responsible for:

- Overseeing JSR's move to a foundation
- Setting the general direction of the project
- Making decisions on behalf of the project when necessary
- Determining how the governance board is to be elected in the future
- Establishing the moderation group, and determining how it is to be formed in
  the future
- Determining how to review open source contributions to the project, and how to
  add new reviewers to the project

The governance board will initially be formed out of trusted members of the
community who have prior experience with open source projects and open
governance. If you are interested, please email
[ry@deno.com](mailto:ry@deno.com).

## Moderation Group

The moderation group is responsible for setting moderation policy, and ensuring
it is enforced fairly and consistently. The moderation group will be established
after the governance board is formed.

The moderation group will be responsible for:

- Determining what content is and is not allowed on JSR
- Determining what actions should be taken against users who violate the
  moderation policy
- Setting guidelines for how moderation should be carried out
- Ensuring that moderation policy is enforced fairly and consistently
- Handling appeals from users who have been moderated

[Apply to become a member of the moderation group here](https://jsr.io/go/moderator).

## Technical Policy

JSR is implemented as a set of open source projects. The main project is the JSR
registry, which is a server component that manages the JSR database. This also
includes a frontend component that allows users to interact with the registry
through a web interface.

The tool to upload packages to JSR is currently baked into the Deno CLI, with
the `npm:jsr` package being just a thin wrapper around the functionality in the
Deno CLI. This functionality should be extracted into a standalone tool that can
be maintained in the JSR organization.

There is not currently a policy in place to determine how contributions to the
JSR project are accepted. This will be determined by the governance board.
Currently reviews are done by the Deno team.

## Open Working Group Meetings

JSR holds open working group meetings every two weeks on Fridays
([view the public calendar](https://deno.co/jsr-meeting)).

[View video recordings of previous meetings here](https://drive.google.com/drive/folders/1nkV3s4SQP2-lQdy---0oHwHSVCjzhJlR).
