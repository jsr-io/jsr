# `jsr-api` Worker (workers-rs)

The Cloudflare Worker (Rust → `wasm32`) that fronts `api.jsr.io`.

This is the lightweight half of the API split described in
[`docs/design/api-service-split.md`](../docs/design/api-service-split.md). Once
the migration sequence completes, this Worker will:

- serve the lightweight CRUD/DB/auth surface directly, reaching Postgres through
  **Cloudflare Hyperdrive** (no `sqlx` — a `wasm32`-compatible driver), and
- **proxy** the heavy/native compute-only paths (publish, docs, source, diff,
  dependency graph, `/tasks/*`) to the Cloud Run **compute** service, which
  keeps `deno_graph`/`deno_doc`/`deno_ast`/`tree-sitter`/`askalono`/`comrak`/
  `jemalloc`.

## Status

**Step 2 of the migration: scaffold only.** This crate currently exposes a
`GET /health` check and returns `501 Not Implemented` for everything else. No
database and no real endpoints are wired up yet; those land one endpoint group
per PR per the design doc's sequence.

## Layout

- `src/lib.rs` — the Worker entrypoint (`#[event(fetch)]`) and route table.
- `Cargo.toml` — a **detached** workspace (its own `[workspace]` + `Cargo.lock`)
  so the repo-root `registry_api` native build never tries to compile this
  `wasm32`-only crate.
- `wrangler.toml` — local-dev config only; production bindings/deploy are
  managed by Terraform in a later infra PR.

## Develop

Build to wasm (what CI gates on):

```sh
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
```

Run locally with the full Worker bundle (requires `worker-build` + `wrangler`):

```sh
cargo install worker-build
npx wrangler dev
```
