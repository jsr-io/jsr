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
it remains true to its mission. You can read more on it
[here](/docs/governance/board).

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

## Office Hours

JSR holds office hour meetings every two weeks on Thursdays
([view the public calendar](https://deno.co/jsr-meeting)).

[View video recordings of previous meetings on YouTube](https://www.youtube.com/@jsr-io/videos).
