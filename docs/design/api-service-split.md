# Design: split the API server into a Workers-rs front + a Cloud Run compute service

Status: **Approved** — boundary, DB approach, and migration sequence signed off
in review (see "Resolved decisions"). Landing the sequence one small PR at a
time, starting with the `jsr_types` extraction (step 1, no behavior change).

## Motivation

`api/` is a single Rust binary (hyper + routerify) that serves all of
`api.jsr.io`. The bulk of its surface is lightweight CRUD/DB/auth work, but it
is welded to a handful of heavy, native-only crates — `deno_graph`, `deno_doc`,
`deno_ast`, the `tree-sitter-*` parsers, `askalono`, `comrak`, and `jemalloc` —
pulled in by the publish pipeline and the rendering endpoints. Those crates
cannot compile to or run on `wasm32` Cloudflare Workers.

The frontend was just moved onto a standalone Cloudflare Worker (#1402), and the
`lb/` Worker already fronts `api.jsr.io`. We want to move the lightweight API
surface onto a **Workers-rs** service for the same reasons the frontend moved
(edge latency, autoscaling, cost), while keeping the heavy native work on Cloud
Run.

This doc proposes the split, the shared-crate extraction, the DB story
(Hyperdrive + a wasm-compatible Postgres driver), the request routing, where
`/tasks/*` lands, the infra changes, and a PR-by-PR migration sequence.

**Nothing in here is built yet.** The intent is to agree the boundary and the
sequence first, then land it in small, independently reviewable, green PRs.

## Guiding principle: split by crate dependency, not by read/write

An endpoint lives on **compute** if and only if its handler (transitively)
touches one of the heavy/native crates. Everything else goes to **workers-rs**.
This is deliberately _not_ a read-vs-write split — several compute endpoints are
GET reads (docs, source, diff) and several worker endpoints are writes (most
CRUD).

## Endpoint inventory

Routes are registered in `api/src/main.rs` (`main_router`), `api/src/api/mod.rs`
(`api_router`, mounted under `/api`), `api/src/tasks.rs` (`tasks_router`, under
`/tasks`), `api/src/sitemap.rs`, and `api/src/auth/mod.rs`.

### Stays on **compute** (Cloud Run) — heavy/native crates

| Method | Path                                                          | Handler                                       | Heavy dependency                                                                                                                            |
| ------ | ------------------------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| POST   | `/api/scopes/:scope/packages/:package/versions/:version`      | `package::version_publish_handler`            | `tarball.rs`→`analysis.rs` (`process_tarball`/`analyze_package`): `deno_graph`, `deno_ast`, `deno_doc`, `tree_sitter`, `comrak`, `askalono` |
| GET    | `/api/scopes/:scope/packages/:package/versions/:version/docs` | `package::get_docs_handler`                   | `docs.rs`: `deno_doc`, `comrak`, `tree_sitter`                                                                                              |
| GET    | `…/versions/:version/docs/search`                             | `package::get_docs_search_handler`            | `deno_doc`                                                                                                                                  |
| GET    | `…/versions/:version/docs/search_structured`                  | `package::get_docs_search_structured_handler` | `deno_doc`                                                                                                                                  |
| GET    | `…/versions/:version/source`                                  | `package::get_source_handler`                 | `tree_sitter` (syntax highlight), `comrak`                                                                                                  |
| GET    | `…/packages/:package/diff/:old_version/:new_version`          | `package::get_diff_handler`                   | `deno_doc`                                                                                                                                  |
| GET    | `…/versions/:version/dependencies/graph`                      | `package::get_dependencies_graph_handler`     | `deno_graph`                                                                                                                                |
| POST   | `/api/scopes/:scope/packages`                                 | `package::create_handler`                     | config validation via `deno_ast`/`deno_graph` (see open question Q1)                                                                        |
| GET    | `/api/debug/mem_stats`, `/api/debug/mem_dump`                 | `jemalloc_profiling::*`                       | `jemalloc`                                                                                                                                  |
| POST   | `/tasks/publish`                                              | `publish::publish_handler`                    | full publish pipeline (all heavy crates)                                                                                                    |
| POST   | `/tasks/npm_tarball_build`                                    | `tasks::npm_tarball_build_handler`            | `analysis.rs` / `npm/tarball.rs` rebuild                                                                                                    |
| POST   | `/tasks/npm_tarball_enqueue`                                  | `tasks::npm_tarball_enqueue_handler`          | enqueues into the heavy build queue                                                                                                         |
| POST   | `/tasks/scrape_download_counts`                               | `tasks::*`                                    | runs on compute today; cron-driven, internal-only (see Q3)                                                                                  |
| POST   | `/tasks/clean_oauth_states`                                   | `tasks::*`                                    | internal-only cron                                                                                                                          |
| POST   | `/tasks/clean_download_counts_4h`                             | `tasks::*`                                    | internal-only cron                                                                                                                          |

The render endpoints (docs/source/diff/graph) are GET reads but stay on compute
purely because they invoke `deno_doc`/`deno_graph`/`tree_sitter`/`comrak` at
request time.

### Moves to **workers-rs**

Package/scope/user/authorization CRUD + listings, metadata GETs, search,
downloads, dependents, dependency listings (the non-graph one), sitemaps, and
the OAuth flows. None of these touch a heavy crate.

- **Global**: `GET /api/metrics`, `GET /api/packages`, `GET /api/stats`,
  `GET /api/.well-known/openapi`, `GET /api/publish_status/:id`,
  `GET /api/publishing_tasks/:id`.
- **Scopes**: full CRUD on `/api/scopes/:scope`, members, invites (`scope.rs` —
  `create/get/update/delete/list_members/invite_member/…`).
- **Packages (non-heavy)**: `GET …/packages`,
  `GET/PATCH/DELETE …/packages/:package`, `GET …/versions`, `GET …/dependents`,
  `GET …/downloads`, `GET …/score`, `GET …/publishing_tasks`,
  `GET …/versions/:version` (metadata), `PATCH/DELETE …/versions/:version`,
  `GET …/versions/:version/tarball` (R2/redirect),
  `POST …/versions/:version/provenance`, `GET …/versions/:version/dependencies`
  (the listing, **not** the graph).
- **Users**: `/api/user/*` (self), `/api/users/:id*`.
- **Authorizations** (device flow): `/api/authorizations/*`.
- **Tickets**: `/api/tickets/*`.
- **Admin**: `/api/admin/*` (pure DB).
- **Sitemaps**: `/sitemap.xml`, `/sitemap-scopes.xml`, `/sitemap-packages.xml`.
- **OAuth**: `/login/:service`, `/login/callback/:service`, `/logout`,
  `/connect/:service`, `/connect/callback/:service`, `/disconnect/:service`.

> Note: `POST /api/scopes/:scope/packages` (create package) is listed on compute
> pending Q1 — if package creation does not actually require a heavy crate (it
> may only do name/config validation), it moves to workers-rs.

## Shared-crate extraction & avoiding `sqlx` on the Worker

Today everything lives in the `registry_api` crate. The Worker must not pull in
`sqlx` (`sqlx-postgres` does not build for `wasm32` — it needs native
sockets/TLS and a tokio runtime) nor any heavy crate. `api/src/db/models.rs` is
heavily sqlx-coupled (~100 references: `FromRow`, `sqlx::Type`,
`Decode`/`Encode` impls), but the **data shapes themselves** are plain structs
over `chrono`/`uuid`/`serde`, and the **wire types** in `api/src/api/types.rs`
(the `Api*` structs + their `From<dbmodel>` conversions) have no sqlx dependency
at all.

Proposed crate layout:

```
crates/
  jsr_types/        # NEW. wasm-safe. no sqlx, no heavy crates.
    - id newtypes (from api/src/ids.rs: ScopeName, PackageName, Version, …)
    - plain model structs (User, Scope, Package, PackageVersion, …) WITHOUT
      the sqlx FromRow/Type/Encode/Decode impls
    - the Api* wire types + From<model> conversions (from api/src/api/types.rs)
    - shared error enums / request+response DTOs
api/                # the compute service (unchanged crate name `registry_api`)
    - depends on jsr_types; keeps sqlx, keeps all heavy crates
    - the sqlx FromRow/Type impls move next to the queries (db/) and are
      implemented ON the jsr_types structs via a thin newtype or feature flag
workers-rs/         # NEW. the api.jsr.io front. depends on jsr_types only.
```

How the sqlx impls are handled without leaking sqlx into `jsr_types`:

- **Option A (preferred): `sqlx` feature on `jsr_types`.** The structs live in
  `jsr_types`; the `FromRow`/`Type`/`Encode`/`Decode` impls are gated behind a
  default-off `sqlx` feature. `api/` enables it; `workers-rs/` does not. The
  orphan rule is satisfied because the impls live in the same crate as the
  structs. wasm builds never compile the sqlx code path.
- **Option B: newtype wrappers in `api/`.** Keep structs sqlx-free in
  `jsr_types`; define `#[derive(FromRow)]` row structs in `api/db/` that
  `Into<jsr_types::X>`. More boilerplate; avoids any sqlx mention in the shared
  crate's Cargo manifest even as an optional dep.

Recommend **Option A** — it keeps the conversions in one place and lets `api/`
keep its existing query code essentially untouched.

The Worker does **not** reuse `api/src/db/database.rs` (137 KB of sqlx
`query!`/`query_as!` macros). Instead it gets its own small DB module that ports
**only the queries the moved endpoints need**, against the wasm-compatible
driver below. We port queries incrementally, one endpoint group per PR, so we
never carry a giant untested rewrite.

## Database: Hyperdrive + a wasm-compatible Postgres driver

The Postgres instance is Cloud SQL on a private VPC IP, reached today by Cloud
Run over a VPC connector (`terraform/db.tf`, `local.postgres_url`). A Worker has
no VPC route, so it reaches Postgres through **Cloudflare Hyperdrive**, which
holds the pooled connection to origin and exposes a binding to the Worker.

Driver choice (the Worker side must be `wasm32`-clean):

- **`sqlx` — rejected.** Does not build for `wasm32`.
- **`tokio-postgres` over the Hyperdrive `Socket` — proposed.** workers-rs
  exposes the Hyperdrive binding as `env.hyperdrive("HYPERDRIVE")`, whose
  `.connect()` returns a `worker::Socket` (a TCP stream to the Hyperdrive
  endpoint). `tokio-postgres` can speak the Postgres wire protocol over any
  `AsyncRead + AsyncWrite` via `Config::connect_raw`, so we wrap the
  `worker::Socket` and hand it to `tokio-postgres`. Hyperdrive terminates TLS to
  origin, so the Worker→Hyperdrive hop is plaintext over the socket (no rustls
  in wasm needed). This is the established workers-rs + Hyperdrive + Postgres
  pattern.

Query style on the Worker: hand-written SQL with `client.query`/`query_opt`,
mapping rows into the `jsr_types` structs (a small manual or macro-light
mapper). We deliberately give up sqlx's compile-time query checking on the
Worker side; compute keeps it. To guard against drift we keep the existing
`api/` integration tests (which exercise the same SQL through sqlx) and add
parity tests for the ported queries.

Local dev: Hyperdrive supports a local connection string (`wrangler dev` with a
`localConnectionString`), so the Worker can hit the docker-compose Postgres
without Hyperdrive in the loop.

## Request routing: workers-rs as the front

Today: `lb/` Worker (`<project>-jsr-lb`) fronts every hostname. For `api.jsr.io`
it proxies to the Cloud Run API URL, rewriting `/x` → `/api/x` (`lb/main.ts`
`handleAPIRequest`, `isAPIRoute`). `jsr.io` goes to the frontend Worker /
modules R2 / npm R2.

Proposed:

- A **new `api` Worker** (`<project>-jsr-api`, workers-rs) becomes the origin
  for `api.jsr.io`. It owns the routing table: if a path is a worker-owned
  endpoint it handles it locally (DB via Hyperdrive); if it's a compute-only
  path (publish POST, docs/source/diff/graph, `/tasks/*`) it **proxies to the
  Cloud Run compute service**.
- The **`lb/` Worker stays** and is **not** broken. The cleanest seam that
  matches the frontend pattern (#1402): `lb` keeps fronting `api.jsr.io` at the
  edge (CORS, security headers, analytics, cache) but its `api` backend becomes
  a **service binding to the new `api` Worker** instead of the Cloud Run URL —
  exactly how `lb` already service-binds the `frontend` Worker. The path rewrite
  that prepends `/api` moves into / is mirrored by the new Worker's router. The
  `api` Worker then service-binds (or `fetch`es over the public Cloud Run URL
  for) the compute service for the proxied paths.
  - Rationale for keeping `lb` in front rather than pointing the DNS route
    straight at the new Worker: `lb` centralizes header/CORS/analytics/cache
    policy across all hostnames and avoids the same-zone-subrequest 525 issue
    documented in #1402. We reuse it rather than reimplement it.
- The compute service keeps running with `--api --tasks=false` (it still serves
  the `/api/...` compute routes and is invoked by the `api` Worker), and the
  tasks service keeps `--tasks --api=false`.

```
  api.jsr.io
      │
┌─────▼─────┐   service binding
│ lb Worker │──────────────┐
└───────────┘              │
                      ┌────▼─────────┐  worker-owned paths
                      │  api Worker  │───────────────► Hyperdrive ─► Postgres
                      │ (workers-rs) │
                      └────┬─────────┘  compute-only paths
                           │ (publish, docs, source, diff, graph)
                    ┌──────▼─────────────┐
                    │ Cloud Run compute  │ (registry-api, heavy crates)
                    └────────────────────┘
```

## `/tasks/*` and GCP Cloud Tasks queueing

`/tasks/publish` and `/tasks/npm_tarball_build` are invoked by **GCP Cloud
Tasks** (`terraform/queues.tf`) targeting the internal-only `registry-api-tasks`
Cloud Run service, authenticated with an OIDC token. The enqueue side lives in
`api/src/gcp.rs` + `task_queue.rs` and runs from request handlers (e.g. publish)
and crons.

This boundary is unchanged by the split:

- The tasks Cloud Run service (`--tasks`) and the GCP queues stay exactly as-is.
  Cloud Tasks → Cloud Run with OIDC is the right tool for durable, retried,
  long-running native work and has no Worker equivalent we want here.
- The **enqueue** path: today the publish handler enqueues a publishing task.
  Since the publish POST stays on compute, the enqueue stays on compute too — no
  change. For any worker-owned endpoint that needs to enqueue a task (none
  identified yet), the Worker would call an internal compute endpoint rather
  than signing GCP OIDC tokens at the edge.
- Crons (`scrape_download_counts`, `clean_oauth_states`,
  `clean_download_counts_4h`) stay on compute/tasks (`terraform/scheduler.tf`).

## Deploy / infra changes

Model the new Worker on `terraform/cloudflare_frontend.tf` (the
`cloudflare_worker` + immutable `cloudflare_worker_version` + 100%
`cloudflare_workers_deployment` triple) and the CI wrangler build (`ci.yml`):

- **New `terraform/cloudflare_api.tf`**: `cloudflare_worker`
  `<project>-jsr-api`, a `cloudflare_worker_version` whose `main_module` is the
  built workers-rs wasm bundle, bindings for `HYPERDRIVE`, the compute service
  URL / service binding, and the OAuth + Orama config the moved handlers need; a
  deployment pinning 100%.
- **New `cloudflare_hyperdrive_config`** pointing at the Cloud SQL Postgres
  (origin host/port/db/user, password from secret). Per the resolved decision
  (Q4) connectivity uses the **public IP + required client certificate (mTLS)**
  model from #1406 — Hyperdrive carries the client cert/key + instance CA; no
  authorized-networks allowlist and no Cloudflare Tunnel.
- **`lb`**: change the `api` backend from the Cloud Run URL var to a service
  binding to the new `api` Worker (mirrors the existing `frontend`/`LB` binding
  wiring).
- **Build**: workers-rs builds with `worker-build` (wasm-pack under the hood) to
  a wasm module + JS shim; wire a `deno task`/CI step like the frontend's
  `vite build` → `_fresh/worker.js`. The artifact feeds the
  `cloudflare_worker_version` `content_file`.
- `cloud_run_api.tf` stays; the compute service keeps its env, secrets, VPC
  connector, and CDN backend. We may later trim its CDN/edge config since `lb` +
  the `api` Worker now sit in front.

## Migration sequence (small, independently reviewable PRs)

Each PR builds green (compute `cargo build`/tests stay passing) and is shippable
on its own. No endpoint actually moves traffic until the cutover PRs late in the
sequence, behind the existing two-service Cloud Run setup.

1. **Extract `jsr_types` crate (types only).** Move `ids.rs`, the plain model
   structs, and `api/types.rs` wire types + conversions into `crates/jsr_types`
   with a default-off `sqlx` feature carrying the `FromRow`/`Type` impls. `api/`
   depends on it with the feature on. Pure refactor, no behavior change.
   _Verify: `jsr_types` builds for `wasm32-unknown-unknown` with the feature
   off._
2. **Scaffold the `workers-rs` crate.** Empty Worker that builds to wasm, a
   router skeleton, health check, CI build step. No real endpoints, no DB.
3. **Hyperdrive + DB connectivity spike.** `tokio-postgres` over the Hyperdrive
   `Socket`; one trivial read (e.g. `GET /api/stats` or a `SELECT 1`). Land the
   Hyperdrive terraform + local dev wiring. Proves the DB story end-to-end.
4. **Move read-only metadata GETs + the compute proxy.** `GET /api/stats`,
   `/api/metrics`, `/api/packages`, `/api/users/:id`, package/scope/version
   metadata GETs. Port their queries; the Worker's fallback **proxies everything
   else to compute** so it can front 100% of the surface while only a few routes
   are served locally. Worker still not yet fronting prod.
5. **Routing cutover (early, incremental).** Point `lb`'s `api` backend at the
   new Worker (service binding); the Worker serves its migrated routes and
   proxies the compute-only paths (and not-yet-migrated routes) to Cloud Run.
   Ship behind a **percentage rollout** via the deployment resource, starting
   low and ramping. This goes **before** the remaining endpoint migrations
   (per review): doing the one prod-routing change early — while the Worker is
   almost entirely a transparent proxy — keeps the blast radius minimal and
   avoids a big-bang cutover after everything has moved. Rolled back by dialing
   the rollout to 0% / repointing the `lb` binding.
6. **Move sitemaps + downloads + dependents + dependency listing.**
7. **Move scope/package/version CRUD (writes).** auth middleware on the Worker
   side; token + session validation.
8. **Move user/authorization/tickets/admin + OAuth `/login/*` flows.**
9. **Cleanup.** Trim compute to only the routes it still serves; remove dead
   edge config; docs.

Steps 1–3 are infrastructure with no traffic impact and land first. Step 4
stands up the Worker as a transparent proxy with a couple of routes served
locally. Step 5 is the single prod-routing change and is done **early**, behind
a percentage rollout, so the risky cutover happens once with a near-empty Worker
rather than as a big-bang at the end. Steps 6–8 then move one endpoint group at
a time **behind the already-live Worker**, each individually revertable (the
proxy keeps serving any route until it is moved).

## Resolved decisions

These were open questions during review; resolved in the PR discussion.

- **Q1 — package create.** `POST /api/scopes/:scope/packages` only validates
  names/config and touches **no heavy crate**, so it lives on the **workers-rs**
  side. (It moves with the scope/package CRUD group in step 7.)
- **Q2 — lost sqlx compile-time checking.** Accepted as proposed: **parity
  tests** plus keeping compute's compile-time-checked queries as the source of
  truth. No separate codegen guard.
- **Q3 — cron `/tasks/*` handlers.** Stay on compute/tasks **as-is for now**;
  not in scope for this split.
- **Q4 — Hyperdrive → Cloud SQL connectivity.** Use the same model as the
  Cloudflare Container migration (#1406): the DB stays reachable on its
  **public IP** with a **required client certificate (mTLS,
  `TRUSTED_CLIENT_CERTIFICATE_REQUIRED` / `verify-ca`)** as the access boundary,
  rather than an authorized-networks allowlist or a Cloudflare Tunnel.
  Hyperdrive is configured with the client cert/key + instance CA. No `net.tf`
  VPC-tunnel work.
- **Q5 — Worker → compute invocation.** Keep the **compute service public
  behind `lb`** for now (it is already public today); do **not** add a
  Worker-held invoker credential / authenticated `fetch` in this iteration.
  Revisit hardening separately if/when desired.
