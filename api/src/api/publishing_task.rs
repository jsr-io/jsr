// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::router::RequestExt;
use axum::Router;
use axum::body::Body;
use axum::routing::get;
use hyper::Request;
use tracing::Span;
use tracing::field;
use tracing::instrument;

use crate::db::Database;
use crate::util;
use crate::util::ApiResult;
use crate::util::RequestIdExt;

use super::ApiError;
use super::ApiPublishingTask;

pub fn publishing_task_router() -> Router {
  Router::new()
    // Never cache: `deno publish` polls this for live status, and a cached
    // non-terminal status would make it hang until the entry expired.
    .route(
      "/{publishing_task_id}",
      get(util::no_store(util::json(get_handler))),
    )
}

#[instrument(
  name = "GET /api/publishing_tasks/:publishing_task_id",
  skip(req),
  fields(publishing_task_id)
)]
pub async fn get_handler(req: Request<Body>) -> ApiResult<ApiPublishingTask> {
  let publishing_task_id = req.param_uuid("publishing_task_id")?;
  Span::current()
    .record("publishing_task_id", field::display(&publishing_task_id));

  let db = req.data::<Database>().unwrap();

  let publishing_task = db
    .get_publishing_task(publishing_task_id)
    .await?
    .ok_or(ApiError::PublishNotFound)?;

  Ok(publishing_task.into())
}
