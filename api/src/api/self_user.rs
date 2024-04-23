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

use std::borrow::Cow;

use crate::db::Database;
use crate::db::TokenType;
use crate::db::UserPublic;
use crate::emails::EmailArgs;
use crate::emails::EmailSender;
use crate::iam::ReqIamExt;
use crate::util;
use crate::util::decode_json;
use crate::util::ApiResult;
use crate::util::RequestIdExt;
use crate::RegistryUrl;

use super::ApiCreateTokenRequest;
use super::ApiCreatedToken;
use super::ApiError;
use super::ApiFullUser;
use super::ApiScope;
use super::ApiScopeInvite;
use super::ApiScopeMember;
use super::ApiToken;

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
    .get("/tokens", util::auth(util::json(list_tokens)))
    .post("/tokens", util::auth(util::json(create_token)))
    .delete("/tokens/:id", util::auth(util::json(delete_token)))
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

#[instrument("GET /api/user/tokens")]
async fn list_tokens(req: Request<Body>) -> Result<Vec<ApiToken>, ApiError> {
  let iam = req.iam();
  let user = iam.check_current_user_access()?;

  let db = req.data::<Database>().unwrap();

  let tokens = db.list_tokens(user.id).await?;

  Ok(tokens.into_iter().map(ApiToken::from).collect())
}

#[instrument("POST /api/user/tokens")]
async fn create_token(
  mut req: Request<Body>,
) -> Result<ApiCreatedToken, ApiError> {
  let ApiCreateTokenRequest {
    description,
    expires_at,
    permissions,
  } = decode_json(&mut req).await?;

  let description = description.trim().replace('\n', " ").replace('\r', "");
  if description.is_empty() {
    return Err(ApiError::MalformedRequest {
      msg: "description must not be empty".into(),
    });
  }
  if description.len() > 250 {
    return Err(ApiError::MalformedRequest {
      msg: "description must not be longer than 250 characters".into(),
    });
  }
  if description.contains(|c: char| c.is_control()) {
    return Err(ApiError::MalformedRequest {
      msg: "description must not contain control characters".into(),
    });
  }

  if let Some(permissions) = permissions.as_ref() {
    if permissions.0.len() != 1 {
      return Err(ApiError::MalformedRequest {
        msg: "permissions must contain exactly one element".into(),
      });
    }
  }

  let iam = req.iam();
  let user = iam.check_authorization_approve_access()?;

  let db = req.data::<Database>().unwrap();

  let secret = crate::token::create_token(
    db,
    user.id,
    TokenType::Personal,
    Some(description),
    expires_at,
    permissions,
  )
  .await?;

  let hash = crate::token::hash(&secret);
  let token = db.get_token_by_hash(&hash).await?.unwrap();

  if let Some(ref email) = user.email {
    let email_sender = req.data::<Option<EmailSender>>().unwrap();
    let registry_url = req.data::<RegistryUrl>().unwrap();
    if let Some(email_sender) = email_sender {
      let email_args = EmailArgs::PersonalAccessToken {
        name: Cow::Borrowed(&user.name),
        registry_url: Cow::Borrowed(registry_url.0.as_str()),
        registry_name: Cow::Borrowed(&email_sender.from_name),
        support_email: Cow::Borrowed(&email_sender.from),
      };
      email_sender
        .send(email.clone(), email_args)
        .await
        .map_err(|e| {
          tracing::error!("failed to send email: {:?}", e);
          ApiError::InternalServerError
        })?;
    }
  }

  Ok(ApiCreatedToken {
    token: token.into(),
    secret,
  })
}

#[instrument("DELETE /api/user/tokens/:id")]
async fn delete_token(req: Request<Body>) -> Result<(), ApiError> {
  let id = req.param_uuid("id")?;

  let iam = req.iam();
  let user = iam.check_authorization_approve_access()?;

  let db = req.data::<Database>().unwrap();

  if !db.delete_token(user.id, id).await? {
    return Err(ApiError::TokenNotFound);
  };

  Ok(())
}
