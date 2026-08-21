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

**Step 4 of the migration: first read-only metadata GETs.** This crate exposes:

- `GET /health` — liveness check.
- `GET /api/db_health` — Hyperdrive connectivity check (`SELECT 1`).
- `GET /api/stats` — front-page newest/updated/featured package lists.
- `GET /api/metrics` — registry-wide package/version/user counts.

Everything else still returns `501 Not Implemented`; the remaining endpoint
groups land one PR at a time per the design doc's sequence. The Worker is not
yet fronting prod traffic.

The `/api/stats` and `/api/metrics` handlers reach Postgres through Hyperdrive
(`tokio-postgres`, no `sqlx`) and serialize the **same** `jsr_types::api` wire
structs the compute service uses, so the JSON is byte-identical by construction.

## Layout

- `src/lib.rs` — the Worker entrypoint (`#[event(fetch)]`) and the `axum` router
  (via the workers-rs `http` feature), matching the compute service's
  router-based structure.
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

Run locally with the full Worker bundle (requires `worker-build` + `wrangler`).
The Hyperdrive binding's local connection string comes from an environment
variable (no values are committed):

```sh
cargo install worker-build
export WRANGLER_HYPERDRIVE_LOCAL_CONNECTION_STRING_HYPERDRIVE="postgres://user:password@localhost:5432/registry"
npx wrangler dev
```
