// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use crate::RegistryUrl;
use crate::api::ApiError;
use crate::db::*;
use crate::iam::ReqIamExt;
use crate::util::ApiResult;
use crate::util::sanitize_redirect_url;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::header;
use oauth2::reqwest::async_http_client;
use oauth2::{AccessToken, RedirectUrl, RefreshToken, StandardRevocableToken};
use routerify::ext::RequestExt;
use routerify_query::RequestQueryExt;
use std::borrow::Cow;
use tracing::Span;
use tracing::instrument;
use url::Url;

pub mod github;
pub mod gitlab;

enum OauthService {
  GitHub,
  GitLab,
}

fn service_param(req: &Request<Body>) -> Result<OauthService, ApiError> {
  let service = crate::util::param(req, "service")?;

  Ok(match service.as_str() {
    "github" => OauthService::GitHub,
    "gitlab" => OauthService::GitLab,
    _ => return Err(ApiError::UnknownLoginService),
  })
}

/// Name of the cookie that binds a login flow to the browser that started it.
/// The login flow has no authenticated user to bind `oauth_state` to (unlike
/// the connect flow), so this cookie is what proves the same browser both
/// initiated and completed the flow, preventing a forced-login (login CSRF).
const LOGIN_CSRF_COOKIE: &str = "oauth_login_csrf";

/// Reads a single cookie value by name from the request's `Cookie` headers.
fn get_cookie<'a>(req: &'a Request<Body>, name: &str) -> Option<&'a str> {
  for header in req.headers().get_all(header::COOKIE) {
    let Ok(header) = header.to_str() else {
      continue;
    };
    for cookie in header.split(';') {
      if let Some((key, value)) = cookie.trim().split_once('=')
        && key == name
      {
        return Some(value);
      }
    }
  }
  None
}

#[instrument(name = "GET /login/:service", skip(req), err, fields(redirect))]
pub async fn login_handler(req: Request<Body>) -> ApiResult<Response<Body>> {
  let service = service_param(&req)?;

  let (pkce_code_challenge, pkce_code_verifier) =
    oauth2::PkceCodeChallenge::new_random_sha256();

  let (auth_url, csrf_token) = (match &service {
    OauthService::GitHub => {
      let github_oauth2_client = req.data::<github::Oauth2Client>().unwrap();
      let authorization_request = github_oauth2_client
        .0
        .authorize_url(oauth2::CsrfToken::new_random);
      github::set_scopes(authorization_request)
    }
    OauthService::GitLab => {
      let gitlab_oauth2_client = req.data::<gitlab::Oauth2Client>().unwrap();
      let authorization_request = gitlab_oauth2_client
        .0
        .authorize_url(oauth2::CsrfToken::new_random);
      gitlab::set_scopes(authorization_request)
    }
  })
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
    // The login flow has no authenticated user yet.
    user_id: None,
  };
  db.insert_oauth_state(new_oauth_state).await?;

  Ok(
    Response::builder()
      .status(StatusCode::TEMPORARY_REDIRECT)
      .header(header::LOCATION, auth_url.as_str())
      // Bind this flow to the current browser: the callback requires the
      // returned `state` to match this cookie, so a `state` minted by an
      // attacker cannot be completed in a victim's browser (login CSRF).
      .header(
        header::SET_COOKIE,
        format!(
          "{LOGIN_CSRF_COOKIE}={}; Max-Age=600; Path=/; SameSite=Lax; HttpOnly",
          csrf_token.secret(),
        ),
      )
      .body(Body::empty())
      .unwrap(),
  )
}

#[instrument(
  name = "GET /login/callback/:service",
  skip(req),
  err,
  fields(state)
)]
pub async fn login_callback_handler(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let service = service_param(&req)?;

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

  // The login flow has no user to bind `oauth_state` to, so it is instead bound
  // to the browser via the `LOGIN_CSRF_COOKIE` set in `login_handler`. The
  // returned `state` must match that cookie; otherwise this is a `state` the
  // current browser never initiated (a forced-login / login CSRF attempt).
  if get_cookie(&req, LOGIN_CSRF_COOKIE) != Some(state) {
    return Err(ApiError::InvalidOauthState);
  }

  let db = req.data::<Database>().unwrap();

  let oauth_state = db
    .get_oauth_state(state)
    .await?
    .ok_or(ApiError::InvalidOauthState)?;

  let (token, expires_at) = match service {
    OauthService::GitHub => {
      if let Some(err_message) = req.query("error_description") {
        return Err(ApiError::GithubOauthError {
          msg: err_message.to_owned(),
        });
      };

      let github_oauth2_client = req.data::<github::Oauth2Client>().unwrap();
      let res = github_oauth2_client
        .0
        .exchange_code(oauth2::AuthorizationCode::new(code))
        .set_pkce_verifier(oauth2::PkceCodeVerifier::new(
          oauth_state.pkce_code_verifier,
        ))
        .request_async(async_http_client)
        .await?;

      db.delete_oauth_state(&oauth_state.csrf_token).await?;

      github::generate_access_token(db, github_oauth2_client, res, None).await?
    }
    OauthService::GitLab => {
      let gitlab_oauth2_client = req.data::<gitlab::Oauth2Client>().unwrap();
      let res = gitlab_oauth2_client
        .0
        .exchange_code(oauth2::AuthorizationCode::new(code))
        .set_pkce_verifier(oauth2::PkceCodeVerifier::new(
          oauth_state.pkce_code_verifier,
        ))
        .request_async(async_http_client)
        .await?;

      db.delete_oauth_state(&oauth_state.csrf_token).await?;

      gitlab::generate_access_token(db, gitlab_oauth2_client, res, None).await?
    }
  };

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
    // The binding cookie has served its purpose; clear it.
    .header(
      header::SET_COOKIE,
      format!(
        "{LOGIN_CSRF_COOKIE}=; Max-Age=0; Path=/; SameSite=Lax; HttpOnly"
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

#[instrument(name = "GET /connect/:service", skip(req), err, fields(redirect))]
pub async fn connect_handler(req: Request<Body>) -> ApiResult<Response<Body>> {
  let service = service_param(&req)?;

  // Bind the oauth_state to the user that initiated the link, so the callback
  // can reject a forged request that tries to link an identity to a different
  // (victim) account.
  let iam = req.iam();
  let user = iam.check_current_user_access()?;

  let (pkce_code_challenge, pkce_code_verifier) =
    oauth2::PkceCodeChallenge::new_random_sha256();

  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();

  let (auth_url, csrf_token) = (match &service {
    OauthService::GitHub => {
      let github_oauth2_client = req.data::<github::Oauth2Client>().unwrap();
      let authorization_request = github_oauth2_client
        .0
        .authorize_url(oauth2::CsrfToken::new_random);
      github::set_scopes(authorization_request).set_redirect_uri(Cow::Owned(
        RedirectUrl::from_url(
          Url::options()
            .base_url(Some(&registry_url))
            .parse("./connect/callback/github")
            .unwrap(),
        ),
      ))
    }
    OauthService::GitLab => {
      let gitlab_oauth2_client = req.data::<gitlab::Oauth2Client>().unwrap();
      let authorization_request = gitlab_oauth2_client
        .0
        .authorize_url(oauth2::CsrfToken::new_random);
      gitlab::set_scopes(authorization_request).set_redirect_uri(Cow::Owned(
        RedirectUrl::from_url(
          Url::options()
            .base_url(Some(&registry_url))
            .parse("./connect/callback/gitlab")
            .unwrap(),
        ),
      ))
    }
  })
  .set_pkce_challenge(pkce_code_challenge)
  .url();

  let mut redirect_url = req
    .query("redirect")
    .and_then(|url| urlencoding::decode(url).map(|url| url.into_owned()).ok())
    .unwrap_or("/account/settings".to_string());

  redirect_url = sanitize_redirect_url(&redirect_url);

  Span::current().record("redirect", &redirect_url);

  let db = req.data::<Database>().unwrap();
  let new_oauth_state = NewOauthState {
    csrf_token: csrf_token.secret(),
    pkce_code_verifier: pkce_code_verifier.secret(),
    redirect_url: &redirect_url,
    user_id: Some(user.id),
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

#[instrument(
  name = "GET /connect/callback/:service",
  skip(req),
  err,
  fields(state)
)]
pub async fn connect_callback_handler(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let service = service_param(&req)?;

  let iam = req.iam();
  let user = iam.check_current_user_access()?;

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
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();

  let oauth_state = db
    .get_oauth_state(state)
    .await?
    .ok_or(ApiError::InvalidOauthState)?;

  // The state must have been initiated by this same user via `connect_handler`.
  // This prevents an OAuth CSRF where a victim is lured to the callback URL and
  // ends up linking the attacker's identity to their account. A `None` user_id
  // means the state came from the login flow, which must not be usable here.
  if oauth_state.user_id != Some(user.id) {
    return Err(ApiError::InvalidOauthState);
  }

  match service {
    OauthService::GitHub => {
      if let Some(err_message) = req.query("error_description") {
        return Err(ApiError::GithubOauthError {
          msg: err_message.to_owned(),
        });
      };

      let github_oauth2_client = req.data::<github::Oauth2Client>().unwrap();
      let res = github_oauth2_client
        .0
        .exchange_code(oauth2::AuthorizationCode::new(code))
        .set_pkce_verifier(oauth2::PkceCodeVerifier::new(
          oauth_state.pkce_code_verifier,
        ))
        .set_redirect_uri(Cow::Owned(RedirectUrl::from_url(
          Url::options()
            .base_url(Some(&registry_url))
            .parse("./connect/callback/github")
            .unwrap(),
        )))
        .request_async(async_http_client)
        .await?;

      db.delete_oauth_state(&oauth_state.csrf_token).await?;

      github::generate_access_token(db, github_oauth2_client, res, Some(user))
        .await?;
    }
    OauthService::GitLab => {
      let gitlab_oauth2_client = req.data::<gitlab::Oauth2Client>().unwrap();
      let res = gitlab_oauth2_client
        .0
        .exchange_code(oauth2::AuthorizationCode::new(code))
        .set_pkce_verifier(oauth2::PkceCodeVerifier::new(
          oauth_state.pkce_code_verifier,
        ))
        .set_redirect_uri(Cow::Owned(RedirectUrl::from_url(
          Url::options()
            .base_url(Some(&registry_url))
            .parse("./connect/callback/gitlab")
            .unwrap(),
        )))
        .request_async(async_http_client)
        .await?;

      db.delete_oauth_state(&oauth_state.csrf_token).await?;

      gitlab::generate_access_token(db, gitlab_oauth2_client, res, Some(user))
        .await?;
    }
  }

  let res = Response::builder()
    .status(StatusCode::FOUND)
    .header(header::CONTENT_TYPE, "text/html")
    .header(header::LOCATION, oauth_state.redirect_url)
    .body(Body::empty())
    .unwrap();
  Ok(res)
}

#[instrument(name = "GET /disconnect/:service", skip(req), err, fields(state))]
pub async fn disconnect_handler(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let service = service_param(&req)?;

  let iam = req.iam();
  let user = iam.check_current_user_access()?;

  let db = req.data::<Database>().unwrap();

  let ids = [user.github_id.is_some(), user.gitlab_id.is_some()];
  if ids.into_iter().filter(|id| *id).count() == 1 {
    return Err(ApiError::DisconnectLastService);
  }

  match service {
    OauthService::GitHub => {
      if let Some(github_id) = user.github_id {
        db.user_set_github_id(user.id, None).await?;
        let identity = db.delete_github_identity(github_id).await?;
        let github_oauth2_client = req.data::<github::Oauth2Client>().unwrap();
        let github_app_client = crate::external::github::GitHubAppClient::new(
          github_oauth2_client.0.client_id().as_str().to_string(),
          github_oauth2_client.1.clone(),
        );

        github_app_client
          .delete_authorization(identity.access_token.unwrap())
          .await?;
      }
    }
    OauthService::GitLab => {
      if let Some(gitlab_id) = user.gitlab_id {
        db.user_set_gitlab_id(user.id, None).await?;
        let identity = db.delete_gitlab_identity(gitlab_id).await?;
        let gitlab_oauth2_client = req.data::<gitlab::Oauth2Client>().unwrap();
        gitlab_oauth2_client
          .0
          .revoke_token(StandardRevocableToken::RefreshToken(
            RefreshToken::new(identity.refresh_token.unwrap()),
          ))?
          .request_async(async_http_client)
          .await?;
        gitlab_oauth2_client
          .0
          .revoke_token(StandardRevocableToken::AccessToken(AccessToken::new(
            identity.access_token.unwrap(),
          )))?
          .request_async(async_http_client)
          .await?;
      }
    }
  }

  let mut redirect_url = req
    .query("redirect")
    .and_then(|url| urlencoding::decode(url).map(|url| url.into_owned()).ok())
    .unwrap_or("/account/settings".to_string());

  redirect_url = sanitize_redirect_url(&redirect_url);
  Span::current().record("redirect", &redirect_url);

  Ok(
    Response::builder()
      .status(StatusCode::SEE_OTHER)
      .header(header::LOCATION, redirect_url)
      .body(Body::empty())
      .unwrap(),
  )
}

#[cfg(test)]
mod tests {
  use crate::api::ApiFullUser;
  use crate::db::NewOauthState;
  use hyper::StatusCode;
  use serde_json::json;

  //use super::*;
  use crate::util::test::{ApiResultExt, TestSetup};

  // The connect (account-linking) callback must reject any `oauth_state` that
  // was not initiated by the current user via `connect_handler`, before it ever
  // exchanges the code with the identity provider. Otherwise an attacker could
  // lure a victim to the callback and link the attacker's identity to the
  // victim's account (account takeover).
  #[tokio::test]
  async fn connect_callback_rejects_state_bound_to_other_user() {
    let mut t = TestSetup::new().await;

    // Forge a state that belongs to user2, then try to complete it as user1
    // (the default `http()` user).
    let user2_id = t.user2.user.id;
    t.db()
      .insert_oauth_state(NewOauthState {
        csrf_token: "state_bound_to_other",
        pkce_code_verifier: "verifier",
        redirect_url: "/account/settings",
        user_id: Some(user2_id),
      })
      .await
      .unwrap();

    t.http()
      .get("/connect/callback/github?code=somecode&state=state_bound_to_other")
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::BAD_REQUEST, "invalidOauthState")
      .await;
  }

  // A login-flow state carries `user_id = NULL`; it must not be usable to
  // complete a connect callback (which requires a state bound to the user).
  #[tokio::test]
  async fn connect_callback_rejects_login_state() {
    let mut t = TestSetup::new().await;

    t.db()
      .insert_oauth_state(NewOauthState {
        csrf_token: "login_flow_state",
        pkce_code_verifier: "verifier",
        redirect_url: "/",
        user_id: None,
      })
      .await
      .unwrap();

    t.http()
      .get("/connect/callback/github?code=somecode&state=login_flow_state")
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::BAD_REQUEST, "invalidOauthState")
      .await;
  }

  // Initiating a connect flow requires authentication.
  #[tokio::test]
  async fn connect_requires_authentication() {
    let mut t = TestSetup::new().await;

    t.unauthed_http()
      .get("/connect/github")
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::UNAUTHORIZED, "missingAuthentication")
      .await;
  }

  // The login callback is bound to the browser that started the flow via the
  // `oauth_login_csrf` cookie. Without that cookie the returned `state` is one
  // the browser never initiated, so the callback must reject it (login CSRF).
  #[tokio::test]
  async fn login_callback_requires_csrf_cookie() {
    let mut t = TestSetup::new().await;

    t.unauthed_http()
      .get("/login/callback/github?code=somecode&state=somestate")
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::BAD_REQUEST, "invalidOauthState")
      .await;
  }

  // A binding cookie that does not match the returned `state` must also be
  // rejected.
  #[tokio::test]
  async fn login_callback_rejects_mismatched_csrf_cookie() {
    let mut t = TestSetup::new().await;

    t.unauthed_http()
      .get("/login/callback/github?code=somecode&state=somestate")
      .header(
        hyper::header::COOKIE,
        "oauth_login_csrf=different".try_into().unwrap(),
      )
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::BAD_REQUEST, "invalidOauthState")
      .await;
  }

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
