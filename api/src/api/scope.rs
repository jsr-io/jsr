// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::sync::OnceLock;

use crate::api::package::package_router;
use crate::emails::EmailArgs;
use crate::emails::EmailSender;
use crate::iam::ReqIamExt;
use crate::RegistryUrl;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use routerify::ext::RequestExt;
use routerify::Router;
use tracing::field;
use tracing::instrument;
use tracing::Span;

use super::errors::map_unique_violation;
use super::errors::ApiError;
use super::types::*;

use crate::auth::lookup_user_by_github_login;
use crate::auth::GithubOauth2Client;
use crate::db::*;
use crate::util;
use crate::util::decode_json;
use crate::util::ApiResult;
use crate::util::RequestIdExt;

pub fn scope_router() -> Router<Body, ApiError> {
  Router::builder()
    .scope("/:scope/packages", package_router())
    .post("/", util::auth(util::json(create_handler)))
    .get("/:scope", util::json(get_handler))
    .patch("/:scope", util::auth(util::json(update_handler)))
    .delete("/:scope", util::auth(delete_handler))
    .get("/:scope/members", util::json(list_members_handler))
    .post(
      "/:scope/members",
      util::auth(util::json(invite_member_handler)),
    )
    .patch(
      "/:scope/members/:member",
      util::auth(util::json(update_member_handler)),
    )
    .delete("/:scope/members/:member", util::auth(delete_member_handler))
    .get(
      "/:scope/invites",
      util::auth(util::json(list_invites_handler)),
    )
    .delete(
      "/:scope/invites/:user_id",
      util::auth(delete_invite_handler),
    )
    .build()
    .unwrap()
}

static RESERVED_SCOPES: OnceLock<std::collections::HashSet<String>> =
  OnceLock::new();

#[instrument(name = "POST /api/scopes", skip(req), err, fields(scope))]
async fn create_handler(mut req: Request<Body>) -> ApiResult<ApiScope> {
  let ApiCreateScopeRequest { scope, description } = decode_json(&mut req).await?;
  Span::current().record("scope", field::display(&scope));
  Span::current().record("description", field::display(description.as_deref().unwrap_or("")));

  let db = req.data::<Database>().unwrap();

  let iam = req.iam();
  let user = iam.check_current_user_access()?;

  // TODO(bartlomieju): this should be done in a transaction and we should check
  // for no of scopes after creating it and if it exceeds the limit rollback.
  // How many scopes has this user created?
  if user.scope_usage >= user.scope_limit.into() {
    return Err(ApiError::ScopeLimitReached);
  }

  let scope_without_hyphens = scope.replace('-', "");

  if db.check_is_bad_word(&scope_without_hyphens).await? {
    return Err(ApiError::ScopeNameNotAllowed);
  }

  let reserved_scopes = RESERVED_SCOPES.get_or_init(|| {
    let reserved_scopes = include_str!("../reserved_scopes.json");
    serde_json::from_str(reserved_scopes).unwrap()
  });

  if reserved_scopes.contains(&scope_without_hyphens) {
    return Err(ApiError::ScopeNameReserved);
  }

  let scope = db
    .create_scope(&user.id, false, &scope, user.id, description)
    .await
    .map_err(|e| map_unique_violation(e, ApiError::ScopeAlreadyExists))?;

  Ok(scope.into())
}

#[instrument(name = "GET /api/scopes/:scope", skip(req), err, fields(scope))]
async fn get_handler(req: Request<Body>) -> ApiResult<ApiScopeOrFullScope> {
  let scope_name = req.param_scope()?;
  Span::current().record("scope", field::display(&scope_name));

  let db = req.data::<Database>().unwrap();
  let scope = db
    .get_scope(&scope_name)
    .await?
    .ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();
  if iam.check_scope_admin_access(&scope.scope).await.is_ok() {
    let user = db
      .get_user_public(scope.creator)
      .await?
      .ok_or(ApiError::ScopeNotFound)?;
    let usage = db.get_scope_usage(&scope.scope).await?;
    Ok(ApiScopeOrFullScope::Full((scope, usage, user).into()))
  } else {
    Ok(ApiScopeOrFullScope::Partial(scope.into()))
  }
}

#[instrument(name = "PATCH /api/scopes/:scope", skip(req), err, fields(scope))]
async fn update_handler(
  mut req: Request<Body>,
) -> ApiResult<ApiScopeOrFullScope> {
  let scope = req.param_scope()?;
  Span::current().record("scope", field::display(&scope));

  let update_req: ApiUpdateScopeRequest = decode_json(&mut req).await?;

  let db = req.data::<Database>().unwrap();

  db.get_scope(&scope).await?.ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();

  let updated_scope = match update_req {
    ApiUpdateScopeRequest::GhActionsVerifyActor(gh_actions_verify_actor) => {
      let (user, sudo) = iam.check_scope_admin_access(&scope).await?;
      db.scope_set_verify_oidc_actor(
        &user.id,
        sudo,
        &scope,
        gh_actions_verify_actor,
      )
      .await?
    }
    ApiUpdateScopeRequest::RequirePublishingFromCI(
      require_publishing_from_ci,
    ) => {
      let (user, sudo) = iam.check_scope_admin_access(&scope).await?;
      db.scope_set_require_publishing_from_ci(
        &user.id,
        sudo,
        &scope,
        require_publishing_from_ci,
      )
      .await?
    }
    ApiUpdateScopeRequest::Description(description) => {
      let (user, sudo) = iam.check_scope_admin_access(&scope).await?;
      db.scope_set_description(&user.id, sudo, &scope, description)
        .await?
    }
  };

  let user = db
    .get_user_public(updated_scope.creator)
    .await?
    .ok_or(ApiError::ScopeNotFound)?;
  let usage = db.get_scope_usage(&updated_scope.scope).await?;

  Ok(ApiScopeOrFullScope::Full(
    (updated_scope, usage, user).into(),
  ))
}

#[instrument(name = "DELETE /api/scopes/:scope", skip(req), err, fields(scop))]
pub async fn delete_handler(req: Request<Body>) -> ApiResult<Response<Body>> {
  let scope = req.param_scope()?;

  let db: &Database = req.data::<Database>().unwrap();

  let _ = db.get_scope(&scope).await?.ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();
  let (user, sudo) = iam.check_scope_admin_access(&scope).await?;

  let deleted = db.delete_scope(&user.id, sudo, &scope).await?;
  if !deleted {
    return Err(ApiError::ScopeNotEmpty);
  }

  let res = Response::builder()
    .status(StatusCode::NO_CONTENT)
    .body(Body::empty())
    .unwrap();
  Ok(res)
}

#[instrument(
  name = "GET /api/scopes/:scope/members",
  skip(req),
  err,
  fields(scope)
)]
async fn list_members_handler(
  req: Request<Body>,
) -> ApiResult<Vec<ApiScopeMember>> {
  let scope = req.param_scope()?;
  Span::current().record("scope", field::display(&scope));

  let db = req.data::<Database>().unwrap();
  let scope_members = db.list_scope_members(&scope).await?;
  if scope_members.is_empty() {
    return Err(ApiError::ScopeNotFound);
  }

  let scope_members = scope_members
    .into_iter()
    .map(ApiScopeMember::from)
    .collect();

  Ok(scope_members)
}

#[instrument(
  name = "POST /api/scopes/:scope/members",
  skip(req),
  err,
  fields(scope, github_login)
)]
async fn invite_member_handler(
  mut req: Request<Body>,
) -> ApiResult<ApiScopeInvite> {
  let scope = req.param_scope()?;
  Span::current().record("scope", field::display(&scope));

  let invite = decode_json::<ApiAddScopeMemberRequest>(&mut req).await?;

  let db = req.data::<Database>().unwrap();
  let github_oauth2_client = req.data::<GithubOauth2Client>().unwrap();

  db.get_scope(&scope).await?.ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();
  let (current_user, sudo) = iam.check_scope_admin_access(&scope).await?;

  let new_user = match invite {
    ApiAddScopeMemberRequest::GithubLogin(github_login) => {
      lookup_user_by_github_login(
        db,
        github_oauth2_client,
        current_user,
        &github_login,
      )
      .await?
      .ok_or(ApiError::UserNotFound)?
    }
    ApiAddScopeMemberRequest::Id(id) => {
      db.get_user(id).await?.ok_or(ApiError::UserNotFound)?
    }
  };

  if db.get_scope_member(&scope, new_user.id).await?.is_some() {
    return Err(ApiError::AlreadyScopeMember);
  }

  let scope_invite = db
    .add_scope_invite(
      &current_user.id,
      sudo,
      NewScopeInvite {
        scope: &scope,
        target_user_id: new_user.id,
        requesting_user_id: current_user.id,
      },
    )
    .await
    .map_err(|e| map_unique_violation(e, ApiError::AlreadyInvited))?;

  if let Some(ref email) = new_user.email {
    let email_sender = req.data::<Option<EmailSender>>().unwrap();
    let registry_url = req.data::<RegistryUrl>().unwrap();
    if let Some(email_sender) = email_sender {
      let email_args = EmailArgs::ScopeInvite {
        name: Cow::Borrowed(&new_user.name),
        scope: Cow::Borrowed(&scope),
        registry_url: Cow::Borrowed(registry_url.0.as_str()),
        registry_name: Cow::Borrowed(&email_sender.from_name),
        support_email: Cow::Borrowed(&email_sender.from),
        inviter_name: Cow::Borrowed(&current_user.name),
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

  Ok(
    (
      scope_invite,
      UserPublic::from(new_user),
      UserPublic::from(current_user.to_owned()),
    )
      .into(),
  )
}

#[instrument(
  name = "GET /api/scopes/:scope/members/:member",
  skip(req),
  err,
  fields(scope, member)
)]
async fn get_member_handler(req: Request<Body>) -> ApiResult<ApiScopeMember> {
  let scope = req.param_scope()?;
  let member_id = req.param_uuid("member")?;
  Span::current().record("scope", field::display(&scope));
  Span::current().record("member", field::display(&member_id));

  let db = req.data::<Database>().unwrap();

  let user = db
    .get_user_public(member_id)
    .await?
    .ok_or(ApiError::UserNotFound)?;
  let scope_member = db
    .get_scope_member(&scope, member_id)
    .await?
    .ok_or(ApiError::ScopeMemberNotFound)?;

  Ok((scope_member, user).into())
}

#[instrument(
  name = "PATCH /api/scopes/:scope/members/:member",
  skip(req),
  err,
  fields(scope, member)
)]
async fn update_member_handler(
  mut req: Request<Body>,
) -> ApiResult<ApiScopeMember> {
  let scope = req.param_scope()?;
  let member_id = req.param_uuid("member")?;
  Span::current().record("scope", field::display(&scope));
  Span::current().record("member", field::display(&member_id));

  let ApiUpdateScopeMemberRequest { is_admin } = decode_json(&mut req).await?;

  let db = req.data::<Database>().unwrap();

  db.get_scope(&scope).await?.ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();
  let (user, sudo) = iam.check_scope_admin_access(&scope).await?;

  let res = db
    .update_scope_member_role(&user.id, sudo, &scope, member_id, is_admin)
    .await?;

  let scope_member = match res {
    ScopeMemberUpdateResult::Ok(scope_member) => scope_member,
    ScopeMemberUpdateResult::TargetIsLastTransferableAdmin => {
      return Err(ApiError::NoScopeOwnerAvailable)
    }
    ScopeMemberUpdateResult::TargetIsLastAdmin => {
      return Err(ApiError::ScopeMustHaveAdmin)
    }
    ScopeMemberUpdateResult::TargetNotMember => {
      return Err(ApiError::ScopeMemberNotFound)
    }
  };

  let user = db
    .get_user_public(scope_member.user_id)
    .await?
    .ok_or(ApiError::InternalServerError)?;

  Ok((scope_member, user).into())
}

#[instrument(
  name = "DELETE /api/scopes/:scope/members/:member",
  skip(req),
  err,
  fields(scope, member)
)]
pub async fn delete_member_handler(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let scope = req.param_scope()?;
  let member_id = req.param_uuid("member")?;
  Span::current().record("scope", field::display(&scope));
  Span::current().record("member", field::display(&member_id));

  let db = req.data::<Database>().unwrap();

  db.get_scope(&scope).await?.ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();
  iam
    .check_scope_member_delete_access(&scope, member_id)
    .await?;

  let res = db.delete_scope_member(&scope, member_id).await?;
  match res {
    ScopeMemberUpdateResult::Ok(_) => {}
    ScopeMemberUpdateResult::TargetIsLastTransferableAdmin => {
      return Err(ApiError::NoScopeOwnerAvailable)
    }
    ScopeMemberUpdateResult::TargetIsLastAdmin => {
      return Err(ApiError::ScopeMustHaveAdmin)
    }
    ScopeMemberUpdateResult::TargetNotMember => {
      return Err(ApiError::ScopeMemberNotFound)
    }
  };

  let resp = Response::builder()
    .status(StatusCode::NO_CONTENT)
    .body(Body::empty())
    .unwrap();
  Ok(resp)
}

#[instrument(
  name = "GET /api/scopes/:scope/invites",
  skip(req),
  err,
  fields(scope)
)]
pub async fn list_invites_handler(
  req: Request<Body>,
) -> ApiResult<Vec<ApiScopeInvite>> {
  let scope = req.param_scope()?;
  Span::current().record("scope", field::display(&scope));

  let db = req.data::<Database>().unwrap();

  db.get_scope(&scope).await?.ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();
  iam.check_scope_admin_access(&scope).await?;

  let scope_invites = db.get_scope_invites_by_scope(&scope).await?;

  let scope_invites = scope_invites
    .into_iter()
    .map(ApiScopeInvite::from)
    .collect();

  Ok(scope_invites)
}

#[instrument(
  name = "DELETE /api/scopes/:scope/invites/:user_id",
  skip(req),
  err,
  fields(scope, user_id)
)]
pub async fn delete_invite_handler(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let scope = req.param_scope()?;
  let user_id = req.param_uuid("user_id")?;
  Span::current().record("scope", field::display(&scope));
  Span::current().record("user_id", field::display(&user_id));

  let db = req.data::<Database>().unwrap();

  db.get_scope(&scope).await?.ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();
  let (user, sudo) = iam.check_scope_admin_access(&scope).await?;

  db.delete_scope_invite(&user.id, sudo, &user_id, &scope)
    .await?;

  let resp = Response::builder()
    .status(StatusCode::NO_CONTENT)
    .body(Body::empty())
    .unwrap();
  Ok(resp)
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use crate::ids::PackageName;
  use crate::ids::ScopeName;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;
  use serde_json::json;
  use uuid::Uuid;

  #[tokio::test]
  async fn scope_get_create() {
    let mut t = TestSetup::new().await;

    let mut resp = t.http().get("/api/scopes/scope1").call().await.unwrap();
    let err = resp.expect_err(StatusCode::NOT_FOUND).await;
    assert_eq!(err.code, "scopeNotFound");

    let mut resp = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "scope1" }))
      .call()
      .await
      .unwrap();
    let scope: ApiScope = resp.expect_ok().await;
    assert_eq!(scope.scope.to_string(), "scope1");

    let mut resp = t.http().get("/api/scopes/scope1").call().await.unwrap();
    let scope: ApiScope = resp.expect_ok().await;
    assert_eq!(scope.scope.to_string(), "scope1");

    // duplicate scope name
    let mut resp = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "scope1" }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::CONFLICT, "scopeAlreadyExists")
      .await;

    // duplicate scope name (only difference is dashes)
    let mut resp = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "scop-e1" }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::CONFLICT, "scopeAlreadyExists")
      .await;

    // invalid name
    let mut resp = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "scope 1" }))
      .call()
      .await
      .unwrap();
    let err = resp.expect_err(StatusCode::BAD_REQUEST).await;
    assert_eq!(err.code, "malformedRequest");

    // disallowed name
    let mut resp = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "somebadword" }))
      .call()
      .await
      .unwrap();
    let err = resp.expect_err(StatusCode::BAD_REQUEST).await;
    assert_eq!(err.code, "scopeNameNotAllowed");

    // reserved name
    let mut resp = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "react" }))
      .call()
      .await
      .unwrap();
    let err = resp.expect_err(StatusCode::BAD_REQUEST).await;
    assert_eq!(err.code, "scopeNameReserved");
  }

  #[tokio::test]
  async fn scope_limit() {
    let mut t = TestSetup::new().await;

    let previously_created_scopes = t
      .db()
      .list_scopes_created_by_user(t.user1.user.id)
      .await
      .unwrap()
      .len();

    // In TestSetup a scope has already been created for this user.
    assert_eq!(previously_created_scopes, 1);

    // Now create two more
    let mut resp = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "scope1" }))
      .call()
      .await
      .unwrap();
    resp.expect_ok::<ApiScope>().await;
    let mut resp: Response<Body> = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "scope2" }))
      .call()
      .await
      .unwrap();
    resp.expect_ok::<ApiScope>().await;

    // Yet another one should fail due to limit.
    let mut resp: Response<Body> = t
      .http()
      .post("/api/scopes")
      .body_json(json!({ "scope": "scope3" }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "scopeLimitReached")
      .await;
  }

  #[tokio::test]
  async fn scope_update_gh_oidc_settings() {
    let mut t = TestSetup::new().await;

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &t.scope.scope,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let path = format!("/api/scopes/{}", t.scope.scope);
    let token = t.user2.token.clone();
    let mut resp = t
      .http()
      .patch(&path)
      .body_json(json!({ "ghActionsVerifyActor": true }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    let token = t.user1.token.clone();
    let mut resp = t
      .http()
      .patch(&path)
      .body_json(json!({ "ghActionsVerifyActor": true }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    let scope = resp.expect_ok::<ApiFullScope>().await;
    assert!(scope.gh_actions_verify_actor);

    let mut resp = t
      .http()
      .patch(&path)
      .body_json(json!({ "ghActionsVerifyActor": false }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    let scope = resp.expect_ok::<ApiFullScope>().await;
    assert!(!scope.gh_actions_verify_actor);
  }

  #[tokio::test]
  async fn scope_update_require_publishing_from_ci() {
    let mut t = TestSetup::new().await;

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &t.scope.scope,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let path = format!("/api/scopes/{}", t.scope.scope);
    let token = t.user2.token.clone();
    let mut resp = t
      .http()
      .patch(&path)
      .body_json(json!({ "requirePublishingFromCI": true }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    let token = t.user1.token.clone();
    let mut resp = t
      .http()
      .patch(&path)
      .body_json(json!({ "requirePublishingFromCI": true }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    let scope = resp.expect_ok::<ApiFullScope>().await;
    assert!(scope.require_publishing_from_ci);

    let mut resp = t
      .http()
      .patch(&path)
      .body_json(json!({ "requirePublishingFromCI": false }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    let scope = resp.expect_ok::<ApiFullScope>().await;
    assert!(!scope.require_publishing_from_ci);
  }

  async fn list_members(t: &mut TestSetup) -> Vec<ApiScopeMember> {
    // list
    let mut resp = t
      .http()
      .get("/api/scopes/scope1/members")
      .call()
      .await
      .unwrap();
    resp.expect_ok().await
  }

  async fn add_member(
    t: &mut TestSetup,
    token: String,
    sudo: bool,
    github_login: String,
  ) -> Response<Body> {
    let body = json!({ "githubLogin": github_login });
    t.http()
      .post("/api/scopes/scope1/members")
      .body_json(body)
      .token(Some(&token))
      .sudo(sudo)
      .call()
      .await
      .unwrap()
  }

  async fn accept_invite(t: &mut TestSetup, token: String) -> Response<Body> {
    t.http()
      .post("/api/user/invites/scope1")
      .token(Some(&token))
      .call()
      .await
      .unwrap()
  }

  #[tokio::test]
  async fn scope_members_add_as_admin() {
    // - admin adds themselves
    // - admin adds user
    // - admin adds member

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();
    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 1);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);

    // user1 (admin) adds themselves
    let github_login = t.user1.github_name.clone();
    let token = t.user1.token.clone();
    add_member(&mut t, token, false, github_login)
      .await
      .expect_err_code(StatusCode::BAD_REQUEST, "alreadyScopeMember")
      .await;

    // user1 (admin) adds user2
    let github_login = t.user2.github_name.clone();
    let token = t.user1.token.clone();
    let mut resp = add_member(&mut t, token, false, github_login).await;
    let invite: ApiScopeInvite = resp.expect_ok().await;
    assert_eq!(invite.scope.to_string(), "scope1");
    assert_eq!(invite.target_user.id, t.user2.user.id);
    assert_eq!(invite.requesting_user.id, t.user1.user.id);

    // accept invite from wrong account fails
    let token = t.user3.token.clone();
    accept_invite(&mut t, token)
      .await
      .expect_err_code(StatusCode::NOT_FOUND, "scopeInviteNotFound")
      .await;

    // accept invite from invited account works
    let token = t.user2.token.clone();
    let mut resp = accept_invite(&mut t, token).await;
    let member: ApiScopeMember = resp.expect_ok().await;
    assert_eq!(member.scope.to_string(), "scope1");
    assert_eq!(member.user.id, t.user2.user.id);

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user1 (admin) adds user2 again
    let github_login = t.user2.github_name.clone();
    let token = t.user1.token.clone();
    add_member(&mut t, token, false, github_login)
      .await
      .expect_err_code(StatusCode::BAD_REQUEST, "alreadyScopeMember")
      .await;
  }

  #[tokio::test]
  async fn scope_members_add_as_member() {
    // - member adds themselves
    // - member adds user

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user2 (member) adds themselves
    let github_login = t.user2.github_name.clone();
    let token = t.user2.token.clone();
    add_member(&mut t, token, false, github_login)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    // user2 (member) adds user3
    let github_login = t.user3.github_name.clone();
    let token = t.user2.token.clone();
    add_member(&mut t, token, false, github_login)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
  }

  #[tokio::test]
  async fn scope_members_add_as_user() {
    // - user adds member
    // - user adds themselves

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user3 adds user2 (member)
    let github_login = t.user2.github_name.clone();
    let token = t.user3.token.clone();
    add_member(&mut t, token, false, github_login)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 adds user3
    let github_login = t.user2.github_name.clone();
    let token = t.user3.token.clone();
    add_member(&mut t, token, false, github_login)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
  }

  #[tokio::test]
  async fn scope_members_add_as_staff() {
    // - staff adds user (no sudo)
    // - staff adds user (sudo)

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 1);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);

    // staff adds user2 without sudo
    let github_login = t.user2.github_name.clone();
    let token = t.staff_user.token.clone();
    add_member(&mut t, token, false, github_login)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // staff adds user2
    let github_login = t.user2.github_name.clone();
    let token = t.staff_user.token.clone();
    add_member(&mut t, token, true, github_login)
      .await
      .expect_ok::<ApiScopeInvite>()
      .await;

    let token2 = t.user2.token.clone();
    accept_invite(&mut t, token2)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
  }

  async fn update_member_permission(
    t: &mut TestSetup,
    token: String,
    sudo: bool,
    user_id: Uuid,
    is_admin: bool,
  ) -> Response<Body> {
    let body = json!({ "isAdmin": is_admin });
    t.http()
      .patch(format!("/api/scopes/scope1/members/{}", user_id))
      .body_json(body)
      .token(Some(&token))
      .sudo(sudo)
      .call()
      .await
      .unwrap()
  }

  #[tokio::test]
  async fn scope_member_upgrade_permission_as_admin() {
    // - admin upgrade themselves
    // - admin upgrade member
    // - admin upgrade user

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user1 (admin) upgrades themselves
    let user_id = t.user1.user.id;
    let token = t.user1.token.clone();
    let member = update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;
    assert!(member.is_admin);

    // user1 (admin) upgrades user2 (member)
    let user_id = t.user2.user.id;
    let token = t.user1.token.clone();
    let member = update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;
    assert!(member.is_admin);

    // user1 (admin) upgrades user3 (not scope member)
    let user_id = t.user3.user.id;
    let token = t.user1.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::NOT_FOUND, "scopeMemberNotFound")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_downgrade_permission_as_admin() {
    // - admin downgrades themselves (last)
    // - admin downgrades member
    // - admin downgrades user
    // - admin downgrades themselves

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user1 (admin) downgrades user2 (member)
    let user_id = t.user2.user.id;
    let token = t.user1.token.clone();
    let member = update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;
    assert!(!member.is_admin);

    // user2 (admin) downgrades user3 (not scope member)
    let user_id = t.user3.user.id;
    let token = t.user1.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::NOT_FOUND, "scopeMemberNotFound")
      .await;

    // user1 (admin) downgrades themselves
    let user_id = t.user1.user.id;
    let token = t.user1.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::BAD_REQUEST, "scopeMustHaveAdmin")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_downgrade_permission_as_admin_not_last1() {
    // - admin downgrades themselves (not-last)

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: true,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user1 (admin) downgrades themselves
    let user_id = t.user1.user.id;
    let token = t.user1.token.clone();
    let member = update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;
    assert!(!member.is_admin);

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(!members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_downgrade_permission_as_admin_not_last2() {
    // - admin downgrades admin (not-last)

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: true,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user1 (admin) downgrades user2 (admin)
    let user_id = t.user2.user.id;
    let token = t.user1.token.clone();
    let member = update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;
    assert!(!member.is_admin);

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_upgrade_permission_as_member() {
    // - member upgrades admin
    // - member upgrades themselves
    // - member upgrades user

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user2 (member) upgrades user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.user2.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    // user2 (member) upgrades themselves
    let user_id = t.user2.user.id;
    let token = t.user2.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    // user2 (member) upgrades user3 (user)
    let user_id = t.user3.user.id;
    let token = t.user2.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_downgrade_permission_as_member() {
    // - member downgrades admin
    // - member downgrades themselves
    // - member downgrades user

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user2 (member) downgrades user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.user2.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    // user2 (member) downgrades themselves
    let user_id = t.user2.user.id;
    let token = t.user2.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    // user2 (member) downgrades user3 (not scope member)
    let user_id = t.user3.user.id;
    let token = t.user2.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_upgrade_permission_as_user() {
    // - user upgrades admin
    // - user upgrades member
    // - user upgrades themselves

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user3 (user) upgrades user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.user3.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 (user) upgrades user2 (member)
    let user_id = t.user2.user.id;
    let token = t.user3.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 (user) upgrades themselves
    let user_id = t.user3.user.id;
    let token = t.user3.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 (user) upgrades admin_user (not scope member)
    let user_id = t.staff_user.user.id;
    let token = t.user3.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_downgrade_permission_as_user() {
    // - user downgrades admin
    // - user downgrades member
    // - user downgrades themselves

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user3 (user) downgrades user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.user3.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 (user) upgrades user2 (member)
    let user_id = t.user2.user.id;
    let token = t.user3.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 (user) upgrades themselves
    let user_id = t.user3.user.id;
    let token = t.user3.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 (user) upgrades admin_user (not scope member)
    let user_id = t.staff_user.user.id;
    let token = t.user3.token.clone();
    update_member_permission(&mut t, token, false, user_id, false)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_upgrade_permission_as_staff() {
    // - staff upgrades admin (sudo)
    // - staff upgrades member (sudo)
    // - staff upgrades user (sudo)

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // staff upgrades user1 (admin) without sudo
    let user_id = t.user1.user.id;
    let token = t.staff_user.token.clone();
    update_member_permission(&mut t, token, false, user_id, true)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // staff upgrades user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.staff_user.token.clone();
    let member = update_member_permission(&mut t, token, true, user_id, true)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;
    assert!(member.is_admin);

    // staff upgrades user2 (member)
    let user_id = t.user2.user.id;
    let token = t.staff_user.token.clone();
    let member = update_member_permission(&mut t, token, true, user_id, true)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;
    assert!(member.is_admin);

    // staff upgrades user3 (not scope member)
    let user_id = t.user3.user.id;
    let token = t.staff_user.token.clone();
    update_member_permission(&mut t, token, true, user_id, true)
      .await
      .expect_err_code(StatusCode::NOT_FOUND, "scopeMemberNotFound")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_member_downgrade_permission_as_staff() {
    // - staff downgrades admin (sudo)
    // - staff downgrades member (sudo)
    // - staff downgrades user (sudo)

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // staff downgrades user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.staff_user.token.clone();
    update_member_permission(&mut t, token, true, user_id, false)
      .await
      .expect_err_code(StatusCode::BAD_REQUEST, "scopeMustHaveAdmin")
      .await;

    // staff downgrades user2 (member)
    let user_id = t.user2.user.id;
    let token = t.staff_user.token.clone();
    let member = update_member_permission(&mut t, token, true, user_id, false)
      .await
      .expect_ok::<ApiScopeMember>()
      .await;
    assert!(!member.is_admin);

    // staff downgrades user3 (not scope member)
    let user_id = t.user3.user.id;
    let token = t.staff_user.token.clone();
    update_member_permission(&mut t, token, true, user_id, false)
      .await
      .expect_err_code(StatusCode::NOT_FOUND, "scopeMemberNotFound")
      .await;
  }

  async fn remove_member(
    t: &mut TestSetup,
    token: String,
    sudo: bool,
    user_id: Uuid,
  ) -> Response<Body> {
    t.http()
      .delete(format!("/api/scopes/scope1/members/{}", user_id))
      .token(Some(&token))
      .sudo(sudo)
      .call()
      .await
      .unwrap()
  }

  #[tokio::test]
  async fn scope_members_remove_as_admin() {
    // - admin removes themselves (last)
    // - admin removes member
    // - admin removes user

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user1 (admin) removes themselves
    let user_id = t.user1.user.id;
    let token = t.user1.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::BAD_REQUEST, "scopeMustHaveAdmin")
      .await;

    // user1 (admin) removes user2 (member)
    let user_id = t.user2.user.id;
    let token = t.user1.token.clone();
    let resp = remove_member(&mut t, token, false, user_id).await;
    assert!(resp.status().is_success());

    // user1 (owner) removes user3 (not scope member)
    let user_id = t.user3.user.id;
    let token = t.user1.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::NOT_FOUND, "scopeMemberNotFound")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].user.id, t.user1.user.id);
  }

  #[tokio::test]
  async fn scope_members_remove_as_admin_not_last1() {
    // - admin removes themselves (not-last)

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: true,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user1 (admin) removes themselves
    let user_id = t.user1.user.id;
    let token = t.user1.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_ok_no_content()
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 1);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_members_remove_as_admin_not_last2() {
    // - admin removes admin (not-last)

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: true,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user1 (admin) removes user2 (admin)
    let user_id = t.user2.user.id;
    let token = t.user1.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_ok_no_content()
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 1);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
  }

  #[tokio::test]
  async fn scope_members_remove_as_member() {
    // - member removes admin
    // - member removes member
    // - member removes user

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user2 (member) removes user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.user2.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    // user2 (member) removes user3 (not scope member)
    let user_id = t.user3.user.id;
    let token = t.user2.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;

    // user2 (member) removes themselves
    let user_id = t.user2.user.id;
    let token = t.user2.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_ok_no_content()
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].user.id, t.user1.user.id);
  }

  #[tokio::test]
  async fn scope_members_remove_as_user() {
    // - user removes admin
    // - user removes member
    // - user removes themselves

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(!members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);

    // user3 (not scope owner) removes user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.user3.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 (not scope owner) removes user2 (member)
    let user_id = t.user2.user.id;
    let token = t.user3.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // user3 (not scope owner) removes themselves
    let user_id = t.user3.user.id;
    let token = t.user3.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert_eq!(members[1].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_members_remove_as_staff() {
    // - staff removes admin (not-last) (not sudo)
    // - staff removes admin (not-last) (sudo)
    // - staff removes admin (last) (sudo)
    // - staff removes member (sudo)
    // - staff removes user (sudo)

    let mut t = TestSetup::new().await;

    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: true,
      })
      .await
      .unwrap();

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user3.user.id,
        is_admin: false,
      })
      .await
      .unwrap();

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 3);
    assert!(members[0].is_admin);
    assert_eq!(members[0].user.id, t.user1.user.id);
    assert!(members[1].is_admin);
    assert_eq!(members[1].user.id, t.user2.user.id);
    assert!(!members[2].is_admin);
    assert_eq!(members[2].user.id, t.user3.user.id);

    // admin_user (staff) removes user1 (admin) without sudo
    let user_id = t.user1.user.id;
    let token = t.staff_user.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    // admin_user (staff) removes user1 (admin)
    let user_id = t.user1.user.id;
    let token = t.staff_user.token.clone();
    remove_member(&mut t, token, true, user_id)
      .await
      .expect_ok_no_content()
      .await;

    // admin_user (staff) removes user2 (admin)
    let user_id = t.user2.user.id;
    let token = t.staff_user.token.clone();
    remove_member(&mut t, token, true, user_id)
      .await
      .expect_err_code(StatusCode::BAD_REQUEST, "scopeMustHaveAdmin")
      .await;

    // admin_user (staff) removes user3 (member)
    let user_id = t.user3.user.id;
    let token = t.staff_user.token.clone();
    remove_member(&mut t, token, true, user_id)
      .await
      .expect_ok_no_content()
      .await;

    // admin_user (staff) removes admin_user (not scope member)
    let user_id = t.staff_user.user.id;
    let token = t.staff_user.token.clone();
    remove_member(&mut t, token, true, user_id)
      .await
      .expect_err_code(StatusCode::NOT_FOUND, "scopeMemberNotFound")
      .await;

    let members = list_members(&mut t).await;
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].user.id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_delete() {
    let mut t = TestSetup::new().await;

    // create scope
    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    let url = format!("/api/scopes/{}", scope_name);
    let mut resp = t.http().delete(&url).call().await.unwrap();
    resp.expect_ok_no_content().await;

    let mut resp = t.http().get(&url).call().await.unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "scopeNotFound")
      .await;
  }

  #[tokio::test]
  async fn scope_delete_non_admin() {
    let mut t = TestSetup::new().await;

    // create scope
    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();
    t.db()
      .add_scope_invite(
        &t.user1.user.id,
        false,
        NewScopeInvite {
          target_user_id: t.user2.user.id,
          requesting_user_id: t.user1.user.id,
          scope: &scope_name,
        },
      )
      .await
      .unwrap();
    t.db()
      .accept_scope_invite(&t.user2.user.id, &scope_name)
      .await
      .unwrap();

    let url = format!("/api/scopes/{}", scope_name);
    let token = t.user2.token.clone();
    let mut resp = t
      .http()
      .delete(&url)
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;
  }

  #[tokio::test]
  async fn scope_delete_non_member() {
    let mut t = TestSetup::new().await;

    // create scope
    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    let url = format!("/api/scopes/{}", scope_name);
    let token = t.user3.token.clone();
    let mut resp = t
      .http()
      .delete(&url)
      .token(Some(&token))
      .call()
      .await
      .unwrap();

    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;
  }

  #[tokio::test]
  async fn scope_delete_not_found() {
    let mut t = TestSetup::new().await;

    let url = "/api/scopes/scope42";
    let mut resp = t.http().delete(url).call().await.unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "scopeNotFound")
      .await;
  }

  #[tokio::test]
  async fn scope_delete_not_empty() {
    let mut t = TestSetup::new().await;

    // create scope and package
    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();
    let name = PackageName::new("foo".to_owned()).unwrap();
    t.ephemeral_database
      .create_package(&scope_name, &name)
      .await
      .unwrap();

    let url = format!("/api/scopes/{}", scope_name);
    let mut resp = t.http().delete(&url).call().await.unwrap();
    resp
      .expect_err_code(StatusCode::CONFLICT, "scopeNotEmpty")
      .await;
  }

  #[tokio::test]
  async fn scope_delete_with_pending_invite() {
    let mut t = TestSetup::new().await;

    // create scope and package
    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();
    t.db()
      .add_scope_invite(
        &t.user1.user.id,
        false,
        NewScopeInvite {
          target_user_id: t.user3.user.id,
          requesting_user_id: t.user1.user.id,
          scope: &scope_name,
        },
      )
      .await
      .unwrap();

    let url = format!("/api/scopes/{}", scope_name);
    let mut resp = t.http().delete(&url).call().await.unwrap();
    resp.expect_ok_no_content().await;
  }

  #[tokio::test]
  async fn scope_transfer() {
    let mut t = TestSetup::new().await;

    // create scope
    let scope_name = ScopeName::try_from("scope1").unwrap();
    t.db()
      .create_scope(&t.user1.user.id, false, &scope_name, t.user1.user.id)
      .await
      .unwrap();

    for i in 0..3 {
      let scope_name = ScopeName::try_from(format!("temp{i}")).unwrap();
      t.db()
        .create_scope(&t.user2.user.id, false, &scope_name, t.user2.user.id)
        .await
        .unwrap();
    }

    t.db()
      .add_user_to_scope(NewScopeMember {
        scope: &scope_name,
        user_id: t.user2.user.id,
        is_admin: true,
      })
      .await
      .unwrap();

    let user_id = t.user1.user.id;
    let token = t.user1.token.clone();
    remove_member(&mut t, token, false, user_id)
      .await
      .expect_err_code(StatusCode::BAD_REQUEST, "noScopeOwnerAvailable")
      .await;
  }
}
