// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use routerify::prelude::RequestExt;
use routerify::Router;
use tracing::field;
use tracing::instrument;
use tracing::Span;

use crate::db::Database;
use crate::db::UserPublic;
use crate::iam::ReqIamExt;
use crate::util;
use crate::util::ApiResult;
use crate::util::RequestIdExt;

use super::ApiError;
use super::ApiFullUser;
use super::ApiScope;
use super::ApiScopeInvite;
use super::ApiScopeMember;

pub fn self_user_router() -> Router<Body, ApiError> {
  Router::builder()
    .get("/", util::auth(util::json(get_handler)))
    .get("/scopes", util::auth(util::json(list_scopes_handler)))
    .get("/member/:scope", util::auth(util::json(get_member_handler)))
    .get("/invites", util::auth(util::json(list_invites_handler)))
    .post(
      "/invites/:scope",
      util::auth(util::json(accept_invite_handler)),
    )
    .delete("/invites/:scope", util::auth(decline_invite_handler))
    .build()
    .unwrap()
}

#[instrument(name = "GET /api/user", skip(req), err)]
pub async fn get_handler(req: Request<Body>) -> ApiResult<ApiFullUser> {
  let iam = req.iam();
  let current_user = iam.check_current_user_access()?.to_owned();
  Ok(current_user.into())
}

#[instrument(name = "GET /api/user/scopes", skip(req), err)]
pub async fn list_scopes_handler(
  req: Request<Body>,
) -> ApiResult<Vec<ApiScope>> {
  let iam = req.iam();
  let current_user = iam.check_current_user_access()?;

  let db = req.data::<Database>().unwrap();
  let scopes = db.get_member_scopes_by_user(&current_user.id).await?;

  Ok(scopes.into_iter().map(ApiScope::from).collect())
}

#[instrument(
  name = "GET /api/user/member/:scope",
  skip(req),
  err,
  fields(scope)
)]
pub async fn get_member_handler(
  req: Request<Body>,
) -> ApiResult<ApiScopeMember> {
  let scope = req.param_scope()?;
  Span::current().record("scope", &field::display(&scope));

  let iam = req.iam();
  let current_user = iam.check_current_user_access()?;

  let db = req.data::<Database>().unwrap();

  let scope_member = db
    .get_scope_member(&scope, current_user.id)
    .await?
    .ok_or(ApiError::ScopeMemberNotFound)?;

  Ok((scope_member, UserPublic::from(current_user.clone())).into())
}

#[instrument(name = "GET /api/user/invites", skip(req), err)]
pub async fn list_invites_handler(
  req: Request<Body>,
) -> ApiResult<Vec<ApiScopeInvite>> {
  let iam = req.iam();
  let current_user = iam.check_current_user_access()?;

  let db = req.data::<Database>().unwrap();

  let scope_invites = db.get_scope_invites_by_user(&current_user.id).await?;

  let scope_invites = scope_invites
    .into_iter()
    .map(ApiScopeInvite::from)
    .collect();

  Ok(scope_invites)
}

#[instrument(
  name = "POST /api/user/invites/:scope",
  skip(req),
  err,
  fields(scope)
)]
pub async fn accept_invite_handler(
  req: Request<Body>,
) -> ApiResult<ApiScopeMember> {
  let scope = req.param_scope()?;
  Span::current().record("scope", &field::display(&scope));

  let iam = req.iam();
  let current_user = iam.check_current_user_access()?.to_owned();

  let db = req.data::<Database>().unwrap();

  let member = db
    .accept_scope_invite(&current_user.id, &scope)
    .await?
    .ok_or(ApiError::ScopeInviteNotFound)?;

  Ok((member, UserPublic::from(current_user)).into())
}

#[instrument(
  name = "DELETE /api/user/invites/:scope",
  skip(req),
  err,
  fields(scope)
)]
pub async fn decline_invite_handler(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let scope = req.param_scope()?;
  Span::current().record("scope", &field::display(&scope));

  let iam = req.iam();
  let current_user = iam.check_current_user_access()?;

  let db = req.data::<Database>().unwrap();

  db.delete_scope_invite(&current_user.id, &scope).await?;

  let resp = Response::builder()
    .status(StatusCode::NO_CONTENT)
    .body(Body::empty())
    .unwrap();
  Ok(resp)
}
