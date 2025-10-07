// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use crate::api::ApiError;
use crate::db::*;
use crate::util::ApiResult;
use crate::util::sanitize_redirect_url;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::header;
use oauth2::ExtraTokenFields;
use oauth2::Scope;
use oauth2::StandardRevocableToken;
use oauth2::StandardTokenIntrospectionResponse;
use oauth2::StandardTokenResponse;
use oauth2::TokenResponse;
use oauth2::basic::BasicErrorResponse;
use oauth2::basic::BasicRevocationErrorResponse;
use oauth2::basic::BasicTokenType;
use oauth2::reqwest::async_http_client;
use routerify::ext::RequestExt;
use routerify_query::RequestQueryExt;
use serde::Deserialize;
use serde::Serialize;
use tracing::Span;
use tracing::instrument;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GithubTokenExtraFields {
  pub refresh_token_expires_in: Option<i64>,
}
impl ExtraTokenFields for GithubTokenExtraFields {}

type GithubTokenResponse =
  StandardTokenResponse<GithubTokenExtraFields, BasicTokenType>;

pub type GithubOauth2Client = oauth2::Client<
  BasicErrorResponse,
  GithubTokenResponse,
  BasicTokenType,
  StandardTokenIntrospectionResponse<GithubTokenExtraFields, BasicTokenType>,
  StandardRevocableToken,
  BasicRevocationErrorResponse,
>;

fn new_github_identity_from_oauth_response(
  res: StandardTokenResponse<GithubTokenExtraFields, BasicTokenType>,
) -> NewGithubIdentity {
  let now = Utc::now();
  let access_token = res.access_token().secret().to_string();
  let access_token_expires_in = res.expires_in().unwrap();
  let access_token_expires_at =
    now + Duration::from_std(access_token_expires_in).unwrap();

  let refresh_token = res.refresh_token().map(|t| t.secret().to_string());
  let refresh_token_expires_at = res
    .extra_fields()
    .refresh_token_expires_in
    .map(|s| now + Duration::try_seconds(s).unwrap());

  NewGithubIdentity {
    github_id: 0,
    access_token: Some(access_token),
    access_token_expires_at: Some(access_token_expires_at),
    refresh_token,
    refresh_token_expires_at,
  }
}

#[instrument(skip(db, github_oauth2_client, ghid), err, fields(user.github_id = ghid.github_id))]
pub async fn access_token(
  db: &Database,
  github_oauth2_client: &GithubOauth2Client,
  ghid: &mut NewGithubIdentity,
) -> Result<String, anyhow::Error> {
  let now = Utc::now() + Duration::try_seconds(30).unwrap();

  // If access token is present, and is expired, attempt to refresh it.
  if ghid.access_token.is_some()
    && ghid.access_token_expires_at.is_some()
    && now > ghid.access_token_expires_at.unwrap()
  {
    if ghid.refresh_token.is_none() || ghid.refresh_token_expires_at.is_none() {
      anyhow::bail!(
        "Failed to refresh access token, because no refresh token is present.",
      )
    }

    // If refresh token is expired, error.
    if now > ghid.refresh_token_expires_at.unwrap() {
      anyhow::bail!(
        "Failed to refresh access token, because refresh token is expired."
      )
    }

    // Get new tokens
    let res = github_oauth2_client
      .exchange_refresh_token(&oauth2::RefreshToken::new(
        ghid.refresh_token.clone().unwrap(),
      ))
      .request_async(async_http_client)
      .await?;
    let new_github_identity = new_github_identity_from_oauth_response(res);
    ghid.access_token = new_github_identity.access_token;
    ghid.access_token_expires_at = new_github_identity.access_token_expires_at;
    ghid.refresh_token = new_github_identity.refresh_token;
    ghid.refresh_token_expires_at =
      new_github_identity.refresh_token_expires_at;

    db.upsert_github_identity(ghid.clone()).await?;
  }

  match (ghid.access_token.clone(), ghid.access_token_expires_at) {
    (Some(access_token), Some(access_token_expires_at))
      if now <= access_token_expires_at =>
    {
      Ok(access_token)
    }
    _ => Err(anyhow::anyhow!(
      "Failed to get access token, because no valid credentials are present.",
    )),
  }
}

#[instrument(skip(db, github_oauth2_client, res), err)]
async fn generate_access_token(
  db: &Database,
  github_oauth2_client: &GithubOauth2Client,
  res: GithubTokenResponse,
) -> ApiResult<(String, DateTime<Utc>)> {
  let mut github_identity = new_github_identity_from_oauth_response(res);

  let access_token =
    access_token(db, github_oauth2_client, &mut github_identity).await?;

  let gh = crate::github::GitHubUserClient::new(access_token);
  let gh_user = gh.current_user().await?;

  github_identity.github_id = gh_user.id;

  db.upsert_github_identity(github_identity).await?;

  let name = gh_user.name.unwrap_or(gh_user.login);
  let gh_email = match gh_user.email.as_ref() {
    Some(email) => Some(email.clone()), // Email address from public profile.
    None => gh
      .emails()
      .await?
      .into_iter()
      .filter(|e| e.primary && e.verified)
      .map(|e| e.email)
      .next(),
  };

  let new_user = NewUser {
    name: name.as_str(),
    email: gh_email.as_deref(),
    avatar_url: gh_user.avatar_url.as_str(),
    github_id: Some(gh_user.id),
    is_blocked: false,
    is_staff: false,
  };

  let db_user = db.upsert_user_by_github_id(new_user).await?;

  let expires_at = Utc::now() + Duration::try_days(7).unwrap();

  let token_string = crate::token::create_token(
    db,
    db_user.id,
    TokenType::Web,
    None,
    Some(expires_at),
    None,
  )
  .await?;

  Ok((token_string, expires_at))
}

#[instrument(name = "GET /login", skip(req), err, fields(redirect))]
pub async fn login_handler(req: Request<Body>) -> ApiResult<Response<Body>> {
  let (pkce_code_challenge, pkce_code_verifier) =
    oauth2::PkceCodeChallenge::new_random_sha256();
  let github_oauth2_client = req.data::<GithubOauth2Client>().unwrap();
  let authorization_request =
    github_oauth2_client.authorize_url(oauth2::CsrfToken::new_random);

  let (auth_url, csrf_token) = authorization_request
    // Set the desired scopes.
    .add_scope(Scope::new("read:user".to_string()))
    .add_scope(Scope::new("user:email".to_string()))
    // Set the PKCE code challenge.
    .set_pkce_challenge(pkce_code_challenge)
    .url();

  let mut redirect_url = req
    .query("redirect")
    .and_then(|url| urlencoding::decode(url).map(|url| url.into_owned()).ok())
    .unwrap_or("/".to_string());

  redirect_url = sanitize_redirect_url(&redirect_url);

  Span::current().record("redirect", &redirect_url);

  let db = req.data::<Database>().unwrap();
  let new_oauth_state = NewOauthState {
    csrf_token: csrf_token.secret(),
    pkce_code_verifier: pkce_code_verifier.secret(),
    redirect_url: &redirect_url,
  };
  db.insert_oauth_state(new_oauth_state).await?;

  Ok(
    Response::builder()
      .status(StatusCode::TEMPORARY_REDIRECT)
      .header(header::LOCATION, auth_url.as_str())
      .body(Body::empty())
      .unwrap(),
  )
}

#[instrument(name = "GET /login/callback", skip(req), err, fields(state))]
pub async fn login_callback_handler(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  if let Some(err_message) = req.query("error_description") {
    return Err(ApiError::GithubOauthError {
      msg: err_message.to_owned(),
    });
  };

  let code = req
    .query("code")
    .ok_or_else(|| ApiError::MalformedRequest {
      msg: "missing 'code' query parameter".into(),
    })?
    .to_owned();
  let state = req
    .query("state")
    .ok_or_else(|| ApiError::MalformedRequest {
      msg: "missing 'state' query parameter".into(),
    })?;
  let db = req.data::<Database>().unwrap();

  let oauth_state = db
    .get_oauth_state(state)
    .await?
    .ok_or(ApiError::InvalidOauthState)?;

  let github_oauth2_client = req.data::<GithubOauth2Client>().unwrap();
  let res = github_oauth2_client
    .exchange_code(oauth2::AuthorizationCode::new(code))
    .set_pkce_verifier(oauth2::PkceCodeVerifier::new(
      oauth_state.pkce_code_verifier,
    ))
    .request_async(async_http_client)
    .await?;

  db.delete_oauth_state(&oauth_state.csrf_token).await?;

  let (token, expires_at) =
    generate_access_token(db, github_oauth2_client, res).await?;

  let res = Response::builder()
    .status(StatusCode::FOUND)
    .header(header::CONTENT_TYPE, "text/html")
    .header(header::LOCATION, oauth_state.redirect_url)
    .header(
      header::SET_COOKIE,
      format!(
        "token={token}; Expires={}; Path=/; SameSite=Lax; HttpOnly",
        expires_at.to_rfc2822().replace("+0000", "GMT"),
      ),
    )
    .body(Body::empty())
    .unwrap();
  Ok(res)
}

#[instrument(name = "GET /logout", skip(req), err, fields(redirect))]
pub async fn logout_handler(req: Request<Body>) -> ApiResult<Response<Body>> {
  let mut redirect_url = req
    .query("redirect")
    .and_then(|url| urlencoding::decode(url).map(|url| url.into_owned()).ok())
    .unwrap_or("/".to_string());

  redirect_url = sanitize_redirect_url(&redirect_url);
  Span::current().record("redirect", &redirect_url);

  Ok(
    Response::builder()
      .status(StatusCode::SEE_OTHER)
      .header(
        header::SET_COOKIE,
        r#"token=""; Max-Age=0; Path=/; SameSite=Lax; HttpOnly"#,
      )
      .header(header::LOCATION, redirect_url)
      .body(Body::empty())
      .unwrap(),
  )
}

#[cfg(not(test))]
#[instrument(
  name = "lookup_user_by_github_login",
  skip(db, github_oauth2_client, current_user),
  err
)]
pub async fn lookup_user_by_github_login(
  db: &Database,
  github_oauth2_client: &GithubOauth2Client,
  current_user: &User,
  github_login: &str,
) -> Result<Option<User>, ApiError> {
  let current_gh_user_id = current_user.github_id.ok_or_else(|| {
    tracing::error!("user is not linked to a GitHub account");
    ApiError::InternalServerError
  })?;
  let current_github_identity =
    db.get_github_identity(current_gh_user_id).await?;
  let mut new_ghid = current_github_identity.into();
  let access_token =
    access_token(db, github_oauth2_client, &mut new_ghid).await?;
  let Some(user) = crate::github::GitHubUserClient::new(access_token)
    .get_user(github_login)
    .await?
  else {
    return Ok(None);
  };
  Ok(db.get_user_by_github_id(user.id).await?)
}

#[cfg(test)]
#[instrument(
  name = "lookup_user_by_github_login",
  skip(db, _github_oauth2_client, _current_user),
  err
)]
pub async fn lookup_user_by_github_login(
  db: &Database,
  _github_oauth2_client: &GithubOauth2Client,
  _current_user: &User,
  github_login: &str,
) -> Result<Option<User>, ApiError> {
  let user = match github_login {
    "ry" => db.get_user_by_github_id(101).await?,
    "lucacasonato" => db.get_user_by_github_id(102).await?,
    "crowlkats" => db.get_user_by_github_id(103).await?,
    "bartlomieju" => db.get_user_by_github_id(104).await?,
    _ => None,
  };

  Ok(user)
}

#[cfg(test)]
mod tests {
  use crate::api::ApiFullUser;
  use hyper::StatusCode;
  use serde_json::json;

  //use super::*;
  use crate::util::test::{ApiResultExt, TestSetup};

  #[tokio::test]
  async fn user_admin_api() {
    let mut t = TestSetup::new().await;
    let mock_user_id: uuid::Uuid =
      "00000000-0000-0000-0000-000000000000".try_into().unwrap();

    let user = t.db().get_user(mock_user_id).await.unwrap().unwrap();
    assert!(!user.is_staff);
    assert!(!user.is_blocked);

    let token = t.staff_user.token.clone();
    let resp = t
      .http()
      .patch(format!("/api/admin/users/{}", mock_user_id))
      .body_json(json!({
        "isStaff": true
      }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();

    eprintln!("resp status {}", resp.status());
    assert!(resp.status().is_success());
    let user = t.db().get_user(mock_user_id).await.unwrap().unwrap();
    assert!(user.is_staff);
    assert!(!user.is_blocked);

    // Try again without authorization header
    let resp = t
      .http()
      .patch(format!("/api/admin/users/{}", mock_user_id))
      .body_json(json!({
        "isStaff": true
      }))
      .token(None)
      .call()
      .await
      .unwrap();
    assert_eq!(resp.status(), hyper::StatusCode::UNAUTHORIZED);

    // Turn off admin, turn on blocked, update scope limit

    let resp = t
      .http()
      .patch(format!("/api/admin/users/{}", mock_user_id))
      .body_json(json!({
        "isStaff": false,
        "isBlocked": true,
        "scopeLimit": 30,
      }))
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    assert!(resp.status().is_success());
    let user = t.db().get_user(mock_user_id).await.unwrap().unwrap();
    assert!(!user.is_staff);
    assert!(user.is_blocked);
    assert_eq!(user.scope_limit, 30);
  }

  #[tokio::test]
  async fn blocked() {
    let mut t = TestSetup::new().await;

    let path = format!("/api/admin/users/{}", t.user1.user.id);
    let token = t.staff_user.token.clone();
    let resp = t
      .http()
      .patch(path)
      .token(Some(&token))
      .body_json(json!({
        "isBlocked": true,
      }))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiFullUser>()
      .await;
    assert!(resp.is_blocked);

    let resp = t
      .http()
      .get("/api/user")
      .call()
      .await
      .unwrap()
      .expect_err(StatusCode::FORBIDDEN)
      .await;
    assert_eq!(resp.code, "blocked");
  }
}
