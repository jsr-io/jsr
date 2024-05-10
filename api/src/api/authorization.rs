// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use base64::Engine;
use chrono::Utc;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use rand::Rng;
use routerify::prelude::RequestExt;
use routerify::Router;
use sha2::Digest;
use url::Url;

use crate::db::Database;
use crate::db::NewAuthorization;
use crate::db::TokenType;
use crate::iam::ReqIamExt;
use crate::token::create_token;
use crate::util;
use crate::util::decode_json;
use crate::util::ApiResult;
use crate::RegistryUrl;

use super::ApiAuthorization;
use super::ApiAuthorizationExchangeRequest;
use super::ApiAuthorizationExchangeResponse;
use super::ApiCreateAuthorizationRequest;
use super::ApiCreateAuthorizationResponse;
use super::ApiError;

pub fn authorization_router() -> Router<Body, ApiError> {
  Router::builder()
    .post("/", util::json(create_authorization))
    .post("/exchange", util::json(exchange_authorization))
    .get("/details/:code", util::json(get_authorization))
    .post("/approve/:code", util::auth(approve_authorization))
    .post("/deny/:code", util::auth(decline_authorization))
    .build()
    .unwrap()
}

async fn create_authorization(
  mut req: Request<Body>,
) -> ApiResult<ApiCreateAuthorizationResponse> {
  let ApiCreateAuthorizationRequest {
    challenge,
    permissions,
  } = decode_json(&mut req).await?;

  let db = req.data::<Database>().unwrap();
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();

  let verification_url = Url::options()
    .base_url(Some(&registry_url))
    .parse("./auth")
    .unwrap();

  let code = create_authorization_code();
  let exchange_token = create_exchange_token();

  let expires_at = Utc::now() + chrono::Duration::try_minutes(10).unwrap();

  let new_authorization = NewAuthorization {
    exchange_token: &exchange_token,
    code: &code,

    challenge: &challenge,
    permissions,

    expires_at,
  };

  let authorization = db.create_authorization(new_authorization).await?;

  Ok(ApiCreateAuthorizationResponse {
    verification_url: verification_url.into(),
    code: authorization.code,
    exchange_token: authorization.exchange_token,
    expires_at: authorization.expires_at,
    poll_interval: 2,
  })
}

// 8 char, upper case, alpha only, no letters that look like numbers, 4 char
// sections split by a dash
const ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";
fn create_authorization_code() -> String {
  let mut rng = rand::thread_rng();
  let mut code = String::with_capacity(8);
  for n in 0..9 {
    if n == 4 {
      code.push('-');
    } else {
      let i = rng.gen_range(0..ALPHABET.len());
      code.push(ALPHABET.chars().nth(i).unwrap());
    }
  }
  code
}

// 40 char hex string
fn create_exchange_token() -> String {
  let mut rng = rand::thread_rng();
  let mut token = String::with_capacity(40);
  for _ in 0..40 {
    let i = rng.gen_range(0..16u8);
    token.push_str(&format!("{:x}", i));
  }
  token
}

async fn exchange_authorization(
  mut req: Request<Body>,
) -> ApiResult<ApiAuthorizationExchangeResponse> {
  let ApiAuthorizationExchangeRequest {
    exchange_token,
    verifier,
  } = decode_json(&mut req).await?;

  let db = req.data::<Database>().unwrap();

  let authorization = db
    .get_authorization_by_exchange_token_and_remove_if_complete(&exchange_token)
    .await?
    .ok_or(ApiError::AuthorizationNotFound)?;

  if authorization.expires_at < Utc::now() {
    return Err(ApiError::AuthorizationExpired);
  }

  let approved = authorization
    .approved
    .ok_or(ApiError::AuthorizationPending)?;

  if !approved {
    return Err(ApiError::AuthorizationDenied);
  }

  let expected_challenge_bytes = sha2::Sha256::digest(verifier.as_bytes());
  let expected_challenge =
    base64::engine::general_purpose::STANDARD.encode(expected_challenge_bytes);

  if authorization.challenge != expected_challenge {
    return Err(ApiError::AuthorizationInvalidVerifier);
  }

  let user_id = authorization.user_id.expect("not pending, so must be set");

  let user = db.get_user(user_id).await?.ok_or(ApiError::UserNotFound)?;

  let expires_at = Utc::now() + chrono::Duration::try_hours(1).unwrap();

  let token = create_token(
    db,
    user_id,
    TokenType::Device,
    None,
    Some(expires_at),
    authorization.permissions,
  )
  .await?;

  Ok(ApiAuthorizationExchangeResponse {
    token,
    user: user.into(),
  })
}

async fn get_authorization(req: Request<Body>) -> ApiResult<ApiAuthorization> {
  let db = req.data::<Database>().unwrap();
  let code = req.param("code").unwrap();

  let authorization = db
    .get_authorization_by_code(code)
    .await?
    .ok_or(ApiError::AuthorizationNotFound)?;
  if authorization.approved.is_some() || authorization.expires_at < Utc::now() {
    return Err(ApiError::AuthorizationNotFound);
  }

  Ok(authorization.into())
}

async fn approve_authorization(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let code = req.param("code").unwrap();

  let iam = req.iam();
  let user = iam.check_authorization_approve_access()?;

  let db = req.data::<Database>().unwrap();

  let authorization = db
    .get_authorization_by_code(code)
    .await?
    .ok_or(ApiError::AuthorizationNotFound)?;
  if authorization.approved.is_some() || authorization.expires_at < Utc::now() {
    return Err(ApiError::AuthorizationNotFound);
  }

  let success = db.update_authorization(code, true, user.id).await?;
  if !success {
    return Err(ApiError::AuthorizationNotFound);
  }

  let resp = Response::builder()
    .status(hyper::StatusCode::NO_CONTENT)
    .body(Body::empty())
    .unwrap();
  Ok(resp)
}

async fn decline_authorization(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let code = req.param("code").unwrap();

  let iam = req.iam();
  let user = iam.check_authorization_approve_access()?;

  let db = req.data::<Database>().unwrap();

  let authorization = db
    .get_authorization_by_code(code)
    .await?
    .ok_or(ApiError::AuthorizationNotFound)?;
  if authorization.approved.is_some() || authorization.expires_at < Utc::now() {
    return Err(ApiError::AuthorizationNotFound);
  }

  let success = db.update_authorization(code, false, user.id).await?;
  if !success {
    return Err(ApiError::AuthorizationNotFound);
  }

  let resp = Response::builder()
    .status(hyper::StatusCode::NO_CONTENT)
    .body(Body::empty())
    .unwrap();
  Ok(resp)
}

#[cfg(test)]
mod tests {
  use base64::Engine;
  use chrono::Utc;
  use hyper::Body;
  use hyper::Response;
  use hyper::StatusCode;
  use serde_json::json;
  use sha2::Digest;
  use uuid::Uuid;

  use crate::api::ApiAuthorization;
  use crate::api::ApiAuthorizationExchangeResponse;
  use crate::api::ApiCreateAuthorizationResponse;
  use crate::api::ApiFullUser;
  use crate::db::PackagePublishPermission;
  use crate::db::Permission;
  use crate::db::Permissions;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;

  fn new_verifier_and_challenge() -> (String, String) {
    let verifier = Uuid::new_v4().to_string();
    let challenge = base64::engine::general_purpose::STANDARD
      .encode(sha2::Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
  }

  async fn create_authorization(
    t: &mut TestSetup,
    challenge: &str,
    permissions: Option<Permissions>,
  ) -> Response<Body> {
    t.http()
      .post("/api/authorizations")
      .body_json(json!({
        "challenge": challenge,
        "permissions": permissions,
      }))
      .call()
      .await
      .unwrap()
  }

  async fn exchange(
    t: &mut TestSetup,
    token: &str,
    verifier: &str,
  ) -> Response<Body> {
    t.http()
      .post("/api/authorizations/exchange")
      .body_json(json!({
        "exchangeToken": token,
        "verifier": verifier,
      }))
      .call()
      .await
      .unwrap()
  }

  async fn details(t: &mut TestSetup, code: &str) -> Response<Body> {
    t.http()
      .get(&format!("/api/authorizations/details/{}", code))
      .call()
      .await
      .unwrap()
  }

  #[tokio::test]
  async fn authorization_success() {
    let mut t = TestSetup::new().await;

    let (verifier, challenge) = new_verifier_and_challenge();

    let mut resp = create_authorization(&mut t, &challenge, None).await;
    let auth: ApiCreateAuthorizationResponse = resp.expect_ok().await;
    assert_eq!(auth.verification_url, "http://jsr-tests.test/auth");
    assert_eq!(auth.code.len(), 9);
    assert_eq!(auth.exchange_token.len(), 40);
    assert_eq!(auth.poll_interval, 2);
    assert!(auth.expires_at > Utc::now());

    let mut resp = exchange(&mut t, &auth.exchange_token, &verifier).await;
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "authorizationPending")
      .await;

    let mut resp = details(&mut t, &auth.code).await;
    let auth_details: ApiAuthorization = resp.expect_ok().await;
    assert_eq!(auth_details.code, auth.code);
    assert_eq!(auth_details.expires_at, auth.expires_at);
    assert!(auth_details.permissions.is_none());

    let mut resp = t
      .http()
      .post(&format!("/api/authorizations/approve/{}", auth.code))
      .call()
      .await
      .unwrap();
    resp.expect_ok_no_content().await;

    let mut resp = details(&mut t, &auth.code).await;
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "authorizationNotFound")
      .await;

    let mut resp = exchange(&mut t, &auth.exchange_token, &verifier).await;
    let token: ApiAuthorizationExchangeResponse = resp.expect_ok().await;
    assert!(token.token.starts_with("jsrd_"));
    assert_eq!(token.user.id, t.user1.user.id);

    let mut resp = t
      .http()
      .get("/api/user")
      .token(Some(&token.token))
      .call()
      .await
      .unwrap();
    let user: ApiFullUser = resp.expect_ok().await;
    assert_eq!(user.id, t.user1.user.id);
  }

  #[tokio::test]
  async fn authorization_with_permissions() {
    let mut t = TestSetup::new().await;

    let (verifier, challenge) = new_verifier_and_challenge();

    let permissions = Permissions(vec![Permission::PackagePublish(
      PackagePublishPermission::Version {
        scope: t.scope.scope.clone(),
        package: "test".try_into().unwrap(),
        version: "1.0.0".try_into().unwrap(),
        tarball_hash: "sha256-1234567890".into(),
      },
    )]);

    let mut resp =
      create_authorization(&mut t, &challenge, Some(permissions)).await;
    let auth: ApiCreateAuthorizationResponse = resp.expect_ok().await;
    assert_eq!(auth.verification_url, "http://jsr-tests.test/auth");
    assert_eq!(auth.code.len(), 9);
    assert_eq!(auth.exchange_token.len(), 40);
    assert_eq!(auth.poll_interval, 2);
    assert!(auth.expires_at > Utc::now());

    let mut resp = exchange(&mut t, &auth.exchange_token, &verifier).await;
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "authorizationPending")
      .await;

    let mut resp = details(&mut t, &auth.code).await;
    let auth_details: ApiAuthorization = resp.expect_ok().await;
    assert_eq!(auth_details.code, auth.code);
    assert_eq!(auth_details.expires_at, auth.expires_at);
    assert!(!auth_details.permissions.unwrap().0.is_empty());

    let mut resp = t
      .http()
      .post(&format!("/api/authorizations/approve/{}", auth.code))
      .call()
      .await
      .unwrap();
    resp.expect_ok_no_content().await;

    let mut resp = details(&mut t, &auth.code).await;
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "authorizationNotFound")
      .await;

    let mut resp = exchange(&mut t, &auth.exchange_token, &verifier).await;
    let token: ApiAuthorizationExchangeResponse = resp.expect_ok().await;
    assert!(token.token.starts_with("jsrd_"));
    assert_eq!(token.user.id, t.user1.user.id);

    let mut resp = t
      .http()
      .get("/api/user")
      .token(Some(&token.token))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "missingPermission")
      .await;
  }

  #[tokio::test]
  async fn authorization_deny() {
    let mut t = TestSetup::new().await;

    let (verifier, challenge) = new_verifier_and_challenge();

    let mut resp = create_authorization(&mut t, &challenge, None).await;
    let auth: ApiCreateAuthorizationResponse = resp.expect_ok().await;
    assert_eq!(auth.verification_url, "http://jsr-tests.test/auth");
    assert_eq!(auth.code.len(), 9);
    assert_eq!(auth.exchange_token.len(), 40);
    assert_eq!(auth.poll_interval, 2);
    assert!(auth.expires_at > Utc::now());

    let mut resp = exchange(&mut t, &auth.exchange_token, &verifier).await;
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "authorizationPending")
      .await;

    let mut resp = details(&mut t, &auth.code).await;
    let auth_details: ApiAuthorization = resp.expect_ok().await;
    assert_eq!(auth_details.code, auth.code);
    assert_eq!(auth_details.expires_at, auth.expires_at);
    assert!(auth_details.permissions.is_none());

    let mut resp = t
      .http()
      .post(&format!("/api/authorizations/deny/{}", auth.code))
      .call()
      .await
      .unwrap();
    resp.expect_ok_no_content().await;

    let mut resp = details(&mut t, &auth.code).await;
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "authorizationNotFound")
      .await;

    let mut resp = exchange(&mut t, &auth.exchange_token, &verifier).await;
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "authorizationDenied")
      .await;
  }
}
