// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::fmt::Display;
use std::str::FromStr;

use crate::api::ApiError;
use crate::db::Database;
use crate::db::Permissions;
use crate::external::oidc;
use crate::external::oidc::OidcProvider;
use crate::external::oidc::OidcProviderKind;
use crate::iam::IamInfo;
use crate::iam::Principal;
use crate::util::ApiResult;
use crate::util::shared_http_client;
use hyper::StatusCode;
use serde::Deserialize;
use serde::Deserializer;
use tracing::Span;
use tracing::field;
use tracing::instrument;

pub struct GitHubUserClient {
  access_token: String,
}

impl GitHubUserClient {
  pub fn new(access_token: String) -> Self {
    Self { access_token }
  }

  async fn request(
    &self,
    path: &str,
  ) -> Result<reqwest::Response, anyhow::Error> {
    let response = shared_http_client()
      .get(format!("https://api.github.com{}", path))
      .bearer_auth(&self.access_token)
      .send()
      .await?;
    Ok(response)
  }

  #[instrument(name = "GitHubUserClient::current_user", skip(self), err)]
  pub async fn current_user(&self) -> Result<User, anyhow::Error> {
    let res = self.request("/user").await?;
    let status = res.status();
    if status.is_success() {
      Ok(res.json().await?)
    } else {
      let response = res.text().await?;
      Err(anyhow::anyhow!(
        "failed to get current user (status {status}): {response}"
      ))
    }
  }

  #[instrument(name = "GitHubUserClient::emails", skip(self), err)]
  pub async fn emails(&self) -> Result<Vec<Email>, anyhow::Error> {
    let res = self.request("/user/emails").await?;
    let status = res.status();
    if status.is_success() {
      Ok(res.json().await?)
    } else {
      let response = res.text().await?;
      Err(anyhow::anyhow!(
        "failed to get user emails (status {status}): {response}"
      ))
    }
  }

  #[cfg(not(test))]
  #[instrument(name = "GitHubUserClient::get_user", skip(self), err)]
  pub async fn get_user(
    &self,
    name: &str,
  ) -> Result<Option<User>, anyhow::Error> {
    let name = super::sanitize_url_part(name);
    let res = self.request(&format!("/users/{name}")).await?;
    let status = res.status();
    if status == StatusCode::NOT_FOUND {
      Ok(None)
    } else if status.is_success() {
      Ok(Some(res.json().await?))
    } else {
      let response = res.text().await?;
      Err(anyhow::anyhow!(
        "failed to get user '{name}' (status {status}): {response}"
      ))
    }
  }

  #[instrument(name = "GitHubUserClient::get_repo", skip(self), err)]
  pub async fn get_repo(
    &self,
    owner: &str,
    name: &str,
  ) -> Result<Option<Repository>, anyhow::Error> {
    let owner = super::sanitize_url_part(owner);
    let name = super::sanitize_url_part(name);
    let res = self.request(&format!("/repos/{owner}/{name}")).await?;
    let status = res.status();
    if status == StatusCode::NOT_FOUND {
      return Ok(None);
    } else if !status.is_success() {
      let response = res.text().await?;
      return Err(anyhow::anyhow!(
        "failed to get repository '{owner}/{name}' (status {status}): {response}",
      ));
    }
    let repo: Repository = res.json().await?;
    Ok(Some(repo))
  }
}

pub struct GitHubAppClient {
  id: String,
  secret: String,
}

impl GitHubAppClient {
  pub fn new(client_id: String, client_secret: String) -> Self {
    Self {
      id: client_id,
      secret: client_secret,
    }
  }

  #[instrument(name = "GitHubAppClient::delete_authorization", skip(self), err)]
  pub async fn delete_authorization(
    &self,
    access_token: String,
  ) -> Result<(), anyhow::Error> {
    let res = shared_http_client()
      .delete(format!(
        "https://api.github.com/applications/{}/grant",
        self.id
      ))
      .basic_auth(&self.id, Some(&self.secret))
      .json(&serde_json::json!({ "access_token": access_token }))
      .send()
      .await?;

    let status = res.status();
    if status.is_success() {
      Ok(())
    } else {
      let response = res.text().await?;
      Err(anyhow::anyhow!(
        "failed to delete authorization (status {status}): {response}"
      ))
    }
  }
}

#[derive(Debug, Deserialize, Clone, Eq, PartialEq)]
pub struct User {
  pub id: i64,
  pub login: String,
  pub name: Option<String>,
  pub avatar_url: String,
  pub created_at: Option<chrono::DateTime<chrono::Utc>>,
  pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct Email {
  pub email: String,
  pub primary: bool,
  pub verified: bool,
}

#[derive(Deserialize)]
pub struct Repository {
  pub id: i64,
  pub name: String,
  pub owner: RepositoryOwner,
  pub visibility: String,
  pub permissions: RepositoryPermissions,
}

#[derive(Deserialize)]
pub struct RepositoryOwner {
  pub login: String,
}

#[derive(Deserialize)]
pub struct RepositoryPermissions {
  pub push: bool,
}

fn deserialize_number_from_string<'de, T, D>(
  deserializer: D,
) -> Result<T, D::Error>
where
  D: Deserializer<'de>,
  T: FromStr + serde::Deserialize<'de>,
  <T as FromStr>::Err: Display,
{
  #[derive(Deserialize)]
  #[serde(untagged)]
  enum StringOrInt<T> {
    String(String),
    Number(T),
  }

  match StringOrInt::<T>::deserialize(deserializer)? {
    StringOrInt::String(s) => s.parse::<T>().map_err(serde::de::Error::custom),
    StringOrInt::Number(i) => Ok(i),
  }
}

pub static GITHUB_OIDC_ISSUER: &str =
  "https://token.actions.githubusercontent.com";

/// Build the provider configuration for a validated GitHub Actions issuer.
/// The issuer is per-token: GitHub Enterprise Cloud enterprises with the
/// "unique OIDC issuer URL" setting get an enterprise-scoped issuer that
/// serves its JWKS from its own path.
fn oidc_provider(issuer: String) -> OidcProvider {
  OidcProvider {
    kind: OidcProviderKind::GitHub,
    jwks_url: format!("{issuer}/.well-known/jwks"),
    issuer,
  }
}

#[derive(Debug, Deserialize, Clone)]
pub struct GitHubClaims {
  #[serde(deserialize_with = "deserialize_number_from_string")]
  pub repository_id: i64,
  #[serde(deserialize_with = "deserialize_number_from_string")]
  pub actor_id: i64,
  pub aud: String,
}

/// Validate that `iss` is a GitHub Actions OIDC issuer: either the shared
/// issuer, or an enterprise-scoped one of the form
/// `https://token.actions.githubusercontent.com/<enterpriseSlug>`, which GitHub
/// Enterprise Cloud uses when the "unique OIDC issuer URL" setting is enabled.
/// The slug charset check also keeps the JWKS fetch below pinned to GitHub's
/// domain.
fn validate_oidc_issuer(iss: &str) -> ApiResult<()> {
  if iss == GITHUB_OIDC_ISSUER {
    return Ok(());
  }
  if let Some(slug) = iss.strip_prefix(GITHUB_OIDC_ISSUER)
    && let Some(slug) = slug.strip_prefix('/')
    && !slug.is_empty()
    && slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
  {
    return Ok(());
  }
  Err(ApiError::InvalidOidcToken {
    msg: format!("invalid issuer: {iss}").into(),
  })
}

/// Extract the `iss` claim from a JWT without verifying its signature. The
/// value must only be used to select the JWKS endpoint; `verify_oidc_token`
/// re-validates it under the token signature.
fn extract_unverified_issuer(token: &str) -> ApiResult<String> {
  use base64::Engine as _;
  use base64::prelude::BASE64_URL_SAFE_NO_PAD;

  #[derive(Deserialize)]
  struct UnverifiedClaims {
    iss: String,
  }

  let payload = token.split('.').nth(1).ok_or(ApiError::InvalidOidcToken {
    msg: "malformed token".into(),
  })?;
  let payload = BASE64_URL_SAFE_NO_PAD.decode(payload).map_err(|err| {
    ApiError::InvalidOidcToken {
      msg: format!("failed to decode claims: {err}").into(),
    }
  })?;
  let claims: UnverifiedClaims =
    serde_json::from_slice(&payload).map_err(|err| {
      ApiError::InvalidOidcToken {
        msg: format!("failed to parse claims: {err}").into(),
      }
    })?;
  Ok(claims.iss)
}

#[instrument(name = "github::verify_oidc_token", err, skip(token))]
pub async fn verify_oidc_token(token: &str) -> ApiResult<GitHubClaims> {
  // The issuer is needed before verification to know which JWKS endpoint to
  // fetch: enterprise-scoped issuers serve their keys from their own path.
  let issuer = extract_unverified_issuer(token)?;
  validate_oidc_issuer(&issuer)?;
  oidc::verify_token::<GitHubClaims>(&oidc_provider(issuer), token).await
}

/// The `aud` claim from a GitHub Actions OIDC token issued for JSR carries
/// JSON-encoded fine-grained publishing permissions. Other providers use a
/// plain-string aud and surface their own `aud`-shaped types.
#[derive(Clone, Debug, Deserialize)]
pub struct GithubOidcTokenAud {
  pub permissions: Permissions,
}

/// Verify a GitHub Actions OIDC token and turn it into an [`IamInfo`].
/// Records `repo.id` and (if the OIDC actor maps to a known JSR user)
/// `user.id` on the provided span.
#[instrument(name = "github::build_iam_info", err, skip(db, token, span))]
pub async fn build_iam_info(
  db: &Database,
  token: &str,
  span: &Span,
) -> ApiResult<IamInfo> {
  let claims = verify_oidc_token(token).await?;
  span.record("repo.id", field::display(claims.repository_id));

  let aud: GithubOidcTokenAud =
    serde_json::from_str(&claims.aud).map_err(|err| {
      ApiError::InvalidOidcToken {
        msg: format!("failed to parse 'aud': {err}").into(),
      }
    })?;

  let user = db.get_user_by_github_id(claims.actor_id).await?;
  if let Some(user) = &user {
    span.record("user.id", field::display(user.id));
  }

  Ok(IamInfo {
    principal: Principal::OidcCi {
      provider: OidcProviderKind::GitHub,
      repository_external_id: claims.repository_id.to_string(),
      user,
    },
    permissions: Some(aud.permissions),
    interactive: false,
    sudo: false,
  })
}

#[cfg(test)]
mod tests {
  use super::GITHUB_OIDC_ISSUER;
  use super::extract_unverified_issuer;
  use super::validate_oidc_issuer;
  use base64::Engine as _;
  use base64::prelude::BASE64_URL_SAFE_NO_PAD;

  #[test]
  fn validate_oidc_issuer_accepts_github_issuers() {
    validate_oidc_issuer(GITHUB_OIDC_ISSUER).unwrap();
    // Enterprise-scoped issuer (GHEC "unique OIDC issuer URL"), see
    // jsr-io/jsr#1485.
    validate_oidc_issuer(
      "https://token.actions.githubusercontent.com/octocat-inc",
    )
    .unwrap();
  }

  #[test]
  fn validate_oidc_issuer_rejects_other_issuers() {
    for iss in [
      "",
      "https://example.com",
      "https://token.actions.githubusercontent.com/",
      "https://token.actions.githubusercontent.com//foo",
      "https://token.actions.githubusercontent.com/foo/bar",
      "https://token.actions.githubusercontent.com/foo?x=1",
      "https://token.actions.githubusercontent.com/../foo",
      "https://token.actions.githubusercontent.com.evil.com",
      "https://token.actions.githubusercontent.com.evil.com/foo",
      "https://token.actions.ghe.com/foo",
    ] {
      assert!(validate_oidc_issuer(iss).is_err(), "accepted: {iss}");
    }
  }

  #[test]
  fn extract_unverified_issuer_reads_iss_claim() {
    let payload = BASE64_URL_SAFE_NO_PAD.encode(
      br#"{"iss":"https://token.actions.githubusercontent.com/octocat-inc"}"#,
    );
    let token = format!("e30.{payload}.signature");
    assert_eq!(
      extract_unverified_issuer(&token).unwrap(),
      "https://token.actions.githubusercontent.com/octocat-inc"
    );

    assert!(extract_unverified_issuer("garbage").is_err());
    assert!(extract_unverified_issuer("e30.!!!.sig").is_err());
    let no_iss = BASE64_URL_SAFE_NO_PAD.encode(br#"{"sub":"x"}"#);
    assert!(extract_unverified_issuer(&format!("e30.{no_iss}.sig")).is_err());
  }
}
