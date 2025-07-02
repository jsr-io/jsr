// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use crate::api::ApiError;
use crate::db::*;
use crate::util::ApiResult;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use oauth2::ExtraTokenFields;
use oauth2::StandardRevocableToken;
use oauth2::StandardTokenIntrospectionResponse;
use oauth2::StandardTokenResponse;
use oauth2::TokenResponse;
use oauth2::basic::BasicErrorResponse;
use oauth2::basic::BasicRevocationErrorResponse;
use oauth2::basic::BasicTokenType;
use oauth2::reqwest::async_http_client;
use serde::Deserialize;
use serde::Serialize;
use tracing::instrument;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GithubTokenExtraFields {
  pub refresh_token_expires_in: Option<i64>,
}
impl ExtraTokenFields for GithubTokenExtraFields {}

type GithubTokenResponse =
  StandardTokenResponse<GithubTokenExtraFields, BasicTokenType>;

pub struct Oauth2Client(pub oauth2::Client<
  BasicErrorResponse,
  GithubTokenResponse,
  BasicTokenType,
  StandardTokenIntrospectionResponse<GithubTokenExtraFields, BasicTokenType>,
  StandardRevocableToken,
  BasicRevocationErrorResponse,
>, pub String);

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
  github_oauth2_client: &Oauth2Client,
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
    let res = github_oauth2_client.0
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
pub async fn generate_access_token(
  db: &Database,
  github_oauth2_client: &Oauth2Client,
  res: GithubTokenResponse,
  logged_in_user: Option<&User>,
) -> ApiResult<(String, DateTime<Utc>)> {
  let mut github_identity = new_github_identity_from_oauth_response(res);

  let access_token =
    access_token(db, github_oauth2_client, &mut github_identity).await?;

  let gh = crate::external::github::GitHubUserClient::new(access_token);
  let gh_user = gh.current_user().await?;

  github_identity.github_id = gh_user.id;

  db.upsert_github_identity(github_identity).await?;

  let db_user = if let Some(logged_in_user) = logged_in_user {
    if db.get_user_by_github_id(gh_user.id).await?.is_some() {
      return Err(ApiError::ConnectTakenService);
    } else {
      db.user_set_github_id(logged_in_user.id, Some(gh_user.id))
        .await?
    }
  } else {
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
      gitlab_id: None,
      is_blocked: false,
      is_staff: false,
    };

    db.upsert_user_by_github_id(new_user).await?
  };

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

#[cfg(not(test))]
#[instrument(
  name = "lookup_user_by_github_login",
  skip(db, github_oauth2_client, current_user),
  err
)]
pub async fn lookup_user_by_github_login(
  db: &Database,
  github_oauth2_client: &Oauth2Client,
  current_user: &User,
  github_login: &str,
) -> Result<Option<User>, crate::api::ApiError> {
  let current_gh_user_id = current_user.github_id.ok_or_else(|| {
    tracing::error!("user is not linked to a GitHub account");
    crate::api::ApiError::InternalServerError
  })?;
  let current_github_identity =
    db.get_github_identity(current_gh_user_id).await?;
  let mut new_ghid = current_github_identity.into();
  let access_token =
    access_token(db, github_oauth2_client, &mut new_ghid).await?;
  let Some(user) = crate::external::github::GitHubUserClient::new(access_token)
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
  _github_oauth2_client: &Oauth2Client,
  _current_user: &User,
  github_login: &str,
) -> Result<Option<User>, crate::api::ApiError> {
  let user = match github_login {
    "ry" => db.get_user_by_github_id(101).await?,
    "lucacasonato" => db.get_user_by_github_id(102).await?,
    "crowlkats" => db.get_user_by_github_id(103).await?,
    "bartlomieju" => db.get_user_by_github_id(104).await?,
    _ => None,
  };

  Ok(user)
}

pub fn set_scopes(
  ar: oauth2::AuthorizationRequest,
) -> oauth2::AuthorizationRequest {
  ar.add_scope(oauth2::Scope::new("read:user".to_string()))
    .add_scope(oauth2::Scope::new("user:email".to_string()))
}
