// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! The `api.jsr.io` Cloudflare Worker (workers-rs).
//!
//! This is the front for the JSR API. Per the API-split design
//! (`docs/design/api-service-split.md`), this Worker will eventually serve the
//! lightweight CRUD/DB/auth surface directly (DB via Cloudflare Hyperdrive) and
//! proxy the heavy/native compute-only paths (publish, docs, source, diff,
//! dependency graph, `/tasks/*`) to the Cloud Run compute service.
//!
//! On top of the step-2 scaffold (a Worker that builds to
//! `wasm32-unknown-unknown` with a router skeleton and a health check), this
//! adds the **step-3 DB connectivity spike**: `GET /api/db_health` opens a
//! Postgres connection through Cloudflare Hyperdrive and runs a trivial
//! `SELECT 1` (see [`db`]). It proves the wasm database story end-to-end. Real
//! API endpoints still land in later, independently-reviewable PRs; every other
//! path returns `501 Not Implemented` so the not-yet-migrated surface is
//! explicit.

mod db;

use worker::*;

/// Worker entrypoint. All `api.jsr.io` requests enter here once the routing
/// cutover lands; today this Worker is not yet fronting any traffic.
#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
  router().run(req, env).await
}

/// The Worker's route table. Endpoint groups are added here one PR at a time as
/// they move off the Cloud Run compute service (see the migration sequence in
/// the design doc).
fn router() -> Router<'static, ()> {
  Router::new()
    .get("/health", |_req, _ctx| health())
    // DB connectivity spike: opens a Hyperdrive→Postgres connection and runs
    // `SELECT 1`. This is the first route that touches the database; the real
    // read/write endpoint groups build on this connection path.
    .get_async("/api/db_health", |_req, ctx| async move {
      db_health(&ctx.env).await
    })
    // Catch-all: until the real endpoints land, every other path is explicitly
    // unimplemented rather than 404, to make the not-yet-migrated surface
    // obvious.
    .or_else_any_method("/*catchall", |_req, _ctx| {
      Response::error("Not Implemented", 501)
    })
}

/// Liveness probe. Returns `200` with a small JSON body so uptime checks and
/// the load balancer can tell the Worker is up.
fn health() -> Result<Response> {
  Response::from_json(&serde_json::json!({
    "service": "jsr-api-worker",
    "status": "ok",
  }))
}

/// Readiness probe for the database path: connects to Postgres through
/// Hyperdrive and runs `SELECT 1`. Returns `200` with the echoed value on
/// success, `500` otherwise. Confirms the Hyperdrive binding, the wasm
/// `tokio-postgres` driver, and the Worker→Hyperdrive→Postgres hop all work.
async fn db_health(env: &Env) -> Result<Response> {
  let client = db::connect(env).await?;
  let value = db::ping(&client).await?;
  Response::from_json(&serde_json::json!({
    "service": "jsr-api-worker",
    "database": "ok",
    "select_1": value,
  }))
}
