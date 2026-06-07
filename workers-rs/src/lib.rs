// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! The `api.jsr.io` Cloudflare Worker (workers-rs). Serves the lightweight
//! CRUD/DB/auth surface and proxies heavy/native paths to the Cloud Run compute
//! service. See `docs/design/api-service-split.md`.
//!
//! Routing uses `axum` (via the workers-rs `http` feature) to stay consistent
//! with the compute service's router-based structure.

mod db;

use axum::Json;
use axum::Router;
use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use jsr_types::api::ApiMetrics;
use jsr_types::api::ApiStats;
use serde_json::Value;
use serde_json::json;
use tower_service::Service;
use worker::send::SendWrapper;
use worker::*;

#[event(fetch)]
async fn fetch(
  req: HttpRequest,
  env: Env,
  _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
  Ok(router(env).call(req).await?)
}

fn router(env: Env) -> Router {
  Router::new()
    .route("/health", get(health))
    .route("/api/db_health", get(db_health))
    .route("/api/stats", get(stats))
    .route("/api/metrics", get(metrics))
    // Not-yet-migrated paths are explicitly unimplemented rather than 404.
    .fallback(|| async { (StatusCode::NOT_IMPLEMENTED, "Not Implemented") })
    .layer(Extension(SendWrapper::new(env)))
}

// Worker handlers hold the (`!Send`) Postgres client across awaits;
// `#[worker::send]` marks the resulting futures `Send` so axum accepts them.

async fn health() -> Json<Value> {
  Json(json!({ "service": "jsr-api-worker", "status": "ok" }))
}

#[worker::send]
async fn db_health(
  Extension(env): Extension<SendWrapper<Env>>,
) -> Result<Json<Value>, AppError> {
  let client = db::connect(&env).await?;
  let value = db::ping(&client).await?;
  Ok(Json(
    json!({ "service": "jsr-api-worker", "database": "ok", "select_1": value }),
  ))
}

#[worker::send]
async fn stats(
  Extension(env): Extension<SendWrapper<Env>>,
) -> Result<Json<ApiStats>, AppError> {
  let client = db::connect(&env).await?;
  Ok(Json(db::stats(&client).await?))
}

#[worker::send]
async fn metrics(
  Extension(env): Extension<SendWrapper<Env>>,
) -> Result<Json<ApiMetrics>, AppError> {
  let client = db::connect(&env).await?;
  Ok(Json(db::metrics(&client).await?))
}

// Wraps a `worker::Error` as a 500 response (the error is logged, not exposed).
struct AppError(Error);

impl From<Error> for AppError {
  fn from(err: Error) -> Self {
    Self(err)
  }
}

impl IntoResponse for AppError {
  fn into_response(self) -> axum::response::Response {
    console_error!("request failed: {}", self.0);
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
  }
}
