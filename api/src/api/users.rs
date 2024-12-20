// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use hyper::Body;
use hyper::Request;
use routerify::prelude::RequestExt;
use routerify::Router;
use tracing::field;
use tracing::instrument;
use tracing::Span;

use crate::db::Database;
use crate::util;
use crate::util::ApiResult;
use crate::util::RequestIdExt;

use super::ApiError;
use super::ApiScope;
use super::ApiUser;

pub fn users_router() -> Router<Body, ApiError> {
  Router::builder()
    .get("/:id", util::json(get_handler))
    .get("/:id/scopes", util::json(get_scopes_handler))
    .build()
    .unwrap()
}

#[instrument(name = "GET /api/users/:id", skip(req), err, fields(id))]
pub async fn get_handler(req: Request<Body>) -> ApiResult<ApiUser> {
  let id = req.param_uuid("id")?;
  Span::current().record("id", &field::display(id));

  let db = req.data::<Database>().unwrap();
  let user = db
    .get_user_public(id)
    .await?
    .ok_or(ApiError::UserNotFound)?;

  Ok(user.into())
}

#[instrument(name = "GET /api/users/:id/scopes", skip(req), err, fields(id))]
pub async fn get_scopes_handler(
  req: Request<Body>,
) -> ApiResult<Vec<ApiScope>> {
  let id = req.param_uuid("id")?;
  Span::current().record("id", &field::display(id));

  let db = req.data::<Database>().unwrap();
  db.get_user_public(id)
    .await?
    .ok_or(ApiError::UserNotFound)?;

  let scopes = db.get_member_scopes_by_user(&id).await?;

  Ok(scopes.into_iter().map(ApiScope::from).collect())
}
