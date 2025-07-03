// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use crate::api::ApiError;
use crate::db::*;
use crate::util::ApiResult;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use oauth2::EmptyExtraTokenFields;
use oauth2::StandardRevocableToken;
use oauth2::StandardTokenIntrospectionResponse;
use oauth2::StandardTokenResponse;
use oauth2::TokenResponse;
use oauth2::basic::BasicErrorResponse;
use oauth2::basic::BasicRevocationErrorResponse;
use oauth2::basic::BasicTokenType;
use oauth2::reqwest::async_http_client;
use tracing::instrument;

type GitLabTokenResponse =
  StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

pub struct Oauth2Client(
  pub  oauth2::Client<
    BasicErrorResponse,
    GitLabTokenResponse,
    BasicTokenType,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
  >,
);

fn new_gitlab_identity_from_oauth_response(
  res: GitLabTokenResponse,
) -> NewGitlabIdentity {
  let now = Utc::now();
  let access_token = res.access_token().secret().to_string();
  let access_token_expires_in = res.expires_in().unwrap();
  let access_token_expires_at =
    now + Duration::from_std(access_token_expires_in).unwrap();

  let refresh_token = res.refresh_token().map(|t| t.secret().to_string());

  NewGitlabIdentity {
    gitlab_id: 0,
    access_token: Some(access_token),
    access_token_expires_at: Some(access_token_expires_at),
    refresh_token,
  }
}

#[instrument(skip(db, gitlab_oauth2_client, glid), err, fields(user.gitlab_id = glid.gitlab_id))]
pub async fn access_token(
  db: &Database,
  gitlab_oauth2_client: &Oauth2Client,
  glid: &mut NewGitlabIdentity,
) -> Result<String, anyhow::Error> {
  let now = Utc::now() + Duration::try_seconds(30).unwrap();

  // If access token is present, and is expired, attempt to refresh it.
  if glid.access_token.is_some()
    && glid.access_token_expires_at.is_some()
    && now > glid.access_token_expires_at.unwrap()
  {
    if glid.refresh_token.is_none() {
      anyhow::bail!(
        "Failed to refresh access token, because no refresh token is present.",
      )
    }

    // Get new tokens
    let res = gitlab_oauth2_client
      .0
      .exchange_refresh_token(&oauth2::RefreshToken::new(
        glid.refresh_token.clone().unwrap(),
      ))
      .request_async(async_http_client)
      .await?;
    let new_gitlab_identity = new_gitlab_identity_from_oauth_response(res);
    glid.access_token = new_gitlab_identity.access_token;
    glid.access_token_expires_at = new_gitlab_identity.access_token_expires_at;
    glid.refresh_token = new_gitlab_identity.refresh_token;

    db.upsert_gitlab_identity(glid.clone()).await?;
  }

  match (glid.access_token.clone(), glid.access_token_expires_at) {
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

#[instrument(skip(db, gitlab_oauth2_client, res), err)]
pub async fn generate_access_token(
  db: &Database,
  gitlab_oauth2_client: &Oauth2Client,
  res: GitLabTokenResponse,
  logged_in_user: Option<&User>,
) -> ApiResult<(String, DateTime<Utc>)> {
  let mut gitlab_identity = new_gitlab_identity_from_oauth_response(res);

  let access_token =
    access_token(db, gitlab_oauth2_client, &mut gitlab_identity).await?;

  let gl = crate::external::gitlab::GitLabUserClient::new(access_token);
  let gl_user = gl.current_user().await?;

  gitlab_identity.gitlab_id = gl_user.id;

  db.upsert_gitlab_identity(gitlab_identity).await?;

  let db_user = if let Some(logged_in_user) = logged_in_user {
    if db.get_user_by_gitlab_id(gl_user.id).await?.is_some() {
      return Err(ApiError::ConnectTakenService);
    } else {
      db.user_set_gitlab_id(logged_in_user.id, Some(gl_user.id))
        .await?
    }
  } else {
    let name = gl_user.name;
    let gh_email = match gl_user.email.as_ref() {
      Some(email) => Some(email.clone()), // Email address from public profile.
      None => gl
        .emails()
        .await?
        .into_iter()
        .filter(|e| e.confirmed_at.is_some())
        .map(|e| e.email)
        .next(),
    };

    let new_user = NewUser {
      name: name.as_str(),
      email: gh_email.as_deref(),
      avatar_url: gl_user.avatar_url.as_str(),
      github_id: None,
      gitlab_id: Some(gl_user.id),
      is_blocked: false,
      is_staff: false,
    };

    db.upsert_user_by_gitlab_id(new_user).await?
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
  name = "lookup_user_by_gitlab_login",
  skip(db, gitlab_oauth2_client, current_user),
  err
)]
pub async fn lookup_user_by_gitlab_username(
  db: &Database,
  gitlab_oauth2_client: &Oauth2Client,
  current_user: &User,
  gitlab_username: &str,
) -> Result<Option<User>, crate::api::ApiError> {
  let current_gl_user_id = current_user.gitlab_id.ok_or_else(|| {
    tracing::error!("user is not linked to a GitLab account");
    crate::api::ApiError::InternalServerError
  })?;
  let current_gitlab_identity =
    db.get_gitlab_identity(current_gl_user_id).await?;
  let mut new_ghid = current_gitlab_identity.into();
  let access_token =
    access_token(db, gitlab_oauth2_client, &mut new_ghid).await?;
  let Some(user) = crate::external::gitlab::GitLabUserClient::new(access_token)
    .get_user(gitlab_username)
    .await?
  else {
    return Ok(None);
  };
  Ok(db.get_user_by_gitlab_id(user.id).await?)
}

#[cfg(test)]
#[instrument(
  name = "lookup_user_by_gitlab_login",
  skip(db, _gitlab_oauth2_client, _current_user),
  err
)]
pub async fn lookup_user_by_gitlab_username(
  db: &Database,
  _gitlab_oauth2_client: &Oauth2Client,
  _current_user: &User,
  gitlab_username: &str,
) -> Result<Option<User>, crate::api::ApiError> {
  let user = match gitlab_username {
    "ry" => db.get_user_by_gitlab_id(101).await?,
    "lucacasonato" => db.get_user_by_gitlab_id(102).await?,
    "crowlkats" => db.get_user_by_gitlab_id(103).await?,
    "bartlomieju" => db.get_user_by_gitlab_id(104).await?,
    _ => None,
  };

  Ok(user)
}

pub fn set_scopes(
  ar: oauth2::AuthorizationRequest,
) -> oauth2::AuthorizationRequest {
  ar.add_scope(oauth2::Scope::new("read_user".to_string()))
    .add_scope(oauth2::Scope::new("email".to_string()))
}
