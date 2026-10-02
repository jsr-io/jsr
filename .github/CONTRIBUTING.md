# Contributing Guidelines

Thank you for your interest in contributing to JSR!

This repository contains the source code for the JSR registry - the frontend
(https://jsr.io), the management API (https://api.jsr.io), and the load balancer
worker. If you want to fix a bug or add a feature to the registry itself, this
is the repository to contribute to.

If you are looking for information on how to _use_ the registry, see
https://jsr.io/docs.

## Getting started

The [README](../README.md) explains how to set up a local development
environment. You can run just the frontend against the production API, or the
entire stack (frontend + API + database) locally.

For a high level overview of how the system fits together, see
[architecture.md](../architecture.md).

## Before opening a pull request

Make sure that:

1. Code is formatted: `deno fmt` and `cargo fmt --all`
2. Lints pass: `deno task lint` and
   `cargo clippy --all-targets --all-features -- -D warnings`
3. Tests pass: `deno task test` (in `frontend/`) and `cargo test`
4. New `.ts`, `.tsx`, `.rs`, and `.tf` files have a license header - run
   `deno task lint:license:fix` to add missing ones
5. If you changed SQL queries, run `deno task sqlx:prepare` and commit the
   changes in `api/.sqlx`

## Submitting a pull request

- PR titles must follow
  [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/) (for
  example `fix: correct package score calculation`). This is enforced by CI.
- Link the issue your PR closes, or describe what you changed and why.
- Frontend changes should include screenshots or a recording. If colors are
  affected, include both light and dark mode.
- Backend changes should include tests.

The pull request template will remind you of these when you open a PR.

## Security issues

Please do not report security vulnerabilities through public issues. See
[SECURITY.md](./SECURITY.md) for how to report them.
