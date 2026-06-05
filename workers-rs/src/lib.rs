// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! The `api.jsr.io` Cloudflare Worker (workers-rs).
//!
//! This is the front for the JSR API. Per the API-split design
//! (`docs/design/api-service-split.md`), this Worker will eventually serve the
//! lightweight CRUD/DB/auth surface directly (DB via Cloudflare Hyperdrive) and
//! proxy the heavy/native compute-only paths (publish, docs, source, diff,
//! dependency graph, `/tasks/*`) to the Cloud Run compute service.
//!
//! This file is the **step-2 scaffold**: an empty Worker that builds to
//! `wasm32-unknown-unknown`, with a router skeleton and a health check. No real
//! API endpoints and no database are wired up yet — those land in later,
//! independently-reviewable PRs. Every non-health path returns `501 Not
//! Implemented` for now so the routing surface is explicit.

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
