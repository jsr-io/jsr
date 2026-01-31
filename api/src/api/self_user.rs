// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use routerify::Router;
use routerify::prelude::RequestExt;
use tracing::Span;
use tracing::field;
use tracing::instrument;

use std::borrow::Cow;

use crate::RegistryUrl;
use crate::db::PackagePublishPermission;
use crate::db::Permission;
use crate::db::TokenType;
use crate::db::UserPublic;
use crate::db::{Database, PackageReadPermission};
use crate::emails::EmailArgs;
use crate::emails::EmailSender;
use crate::iam::ReqIamExt;
use crate::util;
use crate::util::ApiResult;
use crate::util::RequestIdExt;
use crate::util::decode_json;

use super::ApiCreateTokenRequest;
use super::ApiCreatedToken;
use super::ApiError;
use super::ApiFullUser;
use super::ApiScope;
use super::ApiScopeInvite;
use super::ApiScopeMember;
use super::ApiTicket;
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
    .delete("/tokens/:id", util::auth(delete_token))
    .get("/tickets", util::auth(util::json(list_tickets)))
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
  Span::current().record("scope", field::display(&scope));

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
  Span::current().record("scope", field::display(&scope));

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
  Span::current().record("scope", field::display(&scope));

  let iam = req.iam();
  let current_user = iam.check_current_user_access()?;

  let db = req.data::<Database>().unwrap();

  db.delete_scope_invite(&current_user.id, false, &current_user.id, &scope)
    .await?;

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

  if let Some(permissions) = permissions.as_ref()
    && permissions.0.len() != 1
  {
    return Err(ApiError::MalformedRequest {
      msg: "permissions must contain exactly one element".into(),
    });
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
      let permissions = if let Some(permissions) = &token.permissions {
        match &permissions.0[0] {
          Permission::PackagePublish(PackagePublishPermission::Full {}) => {
            Cow::Borrowed("Publish new versions to any package in any scope")
          }
          Permission::PackagePublish(PackagePublishPermission::Scope {
            scope,
          }) => Cow::Owned(format!(
            "Publish new versions to any package in the @{} scope",
            scope
          )),
          Permission::PackagePublish(PackagePublishPermission::Package {
            scope,
            package,
          }) => Cow::Owned(format!(
            "Publish new versions of the @{}/{} package",
            scope, package
          )),
          Permission::PackagePublish(PackagePublishPermission::Version {
            scope,
            package,
            version,
            ..
          }) => Cow::Owned(format!(
            "Publish the {} version of the @{}/{} package",
            version, scope, package
          )),
          Permission::PackageRead(PackageReadPermission::Package {
            scope,
            package,
          }) => Cow::Owned(format!(
            "Read the private @{}/{} package",
            scope, package
          )),
          Permission::PackageRead(PackageReadPermission::Scope { scope }) => {
            Cow::Owned(format!(
              "Read any private package of the @{} scope",
              scope
            ))
          }
          Permission::PackageRead(PackageReadPermission::Full {}) => {
            Cow::Borrowed("Read any private package in any scope")
          }
        }
      } else {
        Cow::Borrowed("Full account access")
      };

      let expiry = token
        .expires_at
        .map(|e| Cow::Owned(e.to_string()))
        .unwrap_or_else(|| Cow::Borrowed("never"));

      let email_args = EmailArgs::PersonalAccessToken {
        token_description: Cow::Borrowed(token.description.as_ref().unwrap()),
        token_permissions: permissions,
        token_expiry: expiry,
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
async fn delete_token(req: Request<Body>) -> Result<Response<Body>, ApiError> {
  let id = req.param_uuid("id")?;

  let iam = req.iam();
  let user = iam.check_authorization_approve_access()?;

  let db = req.data::<Database>().unwrap();

  if !db.delete_token(user.id, id).await? {
    return Err(ApiError::TokenNotFound);
  };

  let resp = Response::builder()
    .status(hyper::StatusCode::NO_CONTENT)
    .body(Body::empty())
    .unwrap();
  Ok(resp)
}

#[instrument(name = "GET /api/user/tickets", skip(req), err)]
pub async fn list_tickets(req: Request<Body>) -> ApiResult<Vec<ApiTicket>> {
  let iam = req.iam();
  let current_user = iam.check_current_user_access()?;

  let db = req.data::<Database>().unwrap();

  let tickets = db.list_tickets_for_user(current_user.id).await?;
  Ok(tickets.into_iter().map(|scope| scope.into()).collect())
}

#[cfg(test)]
mod tests {
  use hyper::StatusCode;
  use serde_json::json;

  use crate::api::ApiCreatedToken;
  use crate::api::ApiFullUser;
  use crate::api::ApiToken;
  use crate::api::ApiTokenType;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;

  #[tokio::test]
  async fn list_tokens() {
    let mut t = TestSetup::new().await;

    let tokens: Vec<ApiToken> = t
      .http()
      .get("/api/user/tokens")
      .call()
      .await
      .unwrap()
      .expect_ok()
      .await;

    assert_eq!(tokens.len(), 1);
    assert!(
      matches!(tokens[0].r#type, ApiTokenType::Web),
      "{:?}",
      tokens[0].r#type
    );
  }

  #[tokio::test]
  async fn create_and_delete_token() {
    let mut t = TestSetup::new().await;

    let token: ApiCreatedToken = t
      .http()
      .post("/api/user/tokens")
      .body_json(json!({
        "description": "test token",
        "expires_at": null,
        "permissions": null
      }))
      .call()
      .await
      .unwrap()
      .expect_ok()
      .await;

    let secret = token.secret;

    let user: ApiFullUser = t
      .http()
      .get("/api/user")
      .token(Some(&secret))
      .call()
      .await
      .unwrap()
      .expect_ok()
      .await;
    assert_eq!(user.id, t.user1.user.id);

    let token = token.token;
    assert_eq!(token.description.unwrap(), "test token");
    assert!(
      matches!(token.r#type, ApiTokenType::Personal),
      "{:?}",
      token.r#type
    );

    let tokens: Vec<ApiToken> = t
      .http()
      .get("/api/user/tokens")
      .call()
      .await
      .unwrap()
      .expect_ok()
      .await;
    assert_eq!(tokens.len(), 2);
    assert!(
      matches!(tokens[0].r#type, ApiTokenType::Personal),
      "{:?}",
      tokens[1].r#type
    );
    assert!(
      matches!(tokens[1].r#type, ApiTokenType::Web),
      "{:?}",
      tokens[0].r#type
    );

    // can't create another token with this token
    t.http()
      .post("/api/user/tokens")
      .token(Some(&secret))
      .body_json(json!({
        "description": "test token",
        "expires_at": null,
        "permissions": null
      }))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::FORBIDDEN, "credentialNotInteractive")
      .await;

    t.http()
      .delete(format!("/api/user/tokens/{}", token.id))
      .call()
      .await
      .unwrap()
      .expect_ok_no_content()
      .await;

    let tokens: Vec<ApiToken> = t
      .http()
      .get("/api/user/tokens")
      .call()
      .await
      .unwrap()
      .expect_ok()
      .await;

    assert_eq!(tokens.len(), 1);
    assert!(
      matches!(tokens[0].r#type, ApiTokenType::Web),
      "{:?}",
      tokens[0].r#type
    );

    // can't delete the token again
    t.http()
      .delete(format!("/api/user/tokens/{}", token.id))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::NOT_FOUND, "tokenNotFound")
      .await;

    // can't use the token anymore
    t.http()
      .get("/api/user")
      .token(Some(&secret))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::UNAUTHORIZED, "invalidBearerToken")
      .await;
  }
}
