// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use hyper::Body;
use hyper::Request;
use routerify::ext::RequestExt;
use routerify::Router;
use tracing::field;
use tracing::instrument;
use tracing::Span;

use crate::db::Database;
use crate::util;
use crate::util::ApiResult;
use crate::util::RequestIdExt;

use super::ApiError;
use super::ApiPublishingTask;

pub fn publishing_task_router() -> Router<Body, ApiError> {
  Router::builder()
    .get("/:publishing_task_id", util::json(get_handler))
    .build()
    .unwrap()
}

#[instrument(
  name = "GET /api/publishing_tasks/:publishing_task_id",
  skip(req),
  err,
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
