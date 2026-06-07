// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! The `api.jsr.io` Cloudflare Worker (workers-rs). Serves the lightweight
//! CRUD/DB/auth surface and proxies heavy/native paths to the Cloud Run compute
//! service. See `docs/design/api-service-split.md`.

mod db;

use worker::*;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
  router().run(req, env).await
}

fn router() -> Router<'static, ()> {
  Router::new()
    .get("/health", |_req, _ctx| health())
    .get_async("/api/db_health", |_req, ctx| async move {
      db_health(&ctx.env).await
    })
    .get_async(
      "/api/stats",
      |_req, ctx| async move { stats(&ctx.env).await },
    )
    .get_async("/api/metrics", |_req, ctx| async move {
      metrics(&ctx.env).await
    })
    // Not-yet-migrated paths are explicitly unimplemented rather than 404.
    .or_else_any_method("/*catchall", |_req, _ctx| {
      Response::error("Not Implemented", 501)
    })
}

fn health() -> Result<Response> {
  Response::from_json(&serde_json::json!({
    "service": "jsr-api-worker",
    "status": "ok",
  }))
}

async fn db_health(env: &Env) -> Result<Response> {
  let client = db::connect(env).await?;
  let value = db::ping(&client).await?;
  Response::from_json(&serde_json::json!({
    "service": "jsr-api-worker",
    "database": "ok",
    "select_1": value,
  }))
}

// `GET /api/stats` — front-page newest/updated/featured package lists.
async fn stats(env: &Env) -> Result<Response> {
  let client = db::connect(env).await?;
  Response::from_json(&db::stats(&client).await?)
}

// `GET /api/metrics` — registry-wide package/version/user counts.
async fn metrics(env: &Env) -> Result<Response> {
  let client = db::connect(env).await?;
  Response::from_json(&db::metrics(&client).await?)
}
