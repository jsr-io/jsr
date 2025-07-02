// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use crate::util::USER_AGENT;
use hyper::StatusCode;
use serde::Deserialize;
use tracing::instrument;

pub struct GitLabUserClient {
  access_token: String,
}

impl GitLabUserClient {
  pub fn new(access_token: String) -> Self {
    Self { access_token }
  }

  async fn request(
    &self,
    path: &str,
  ) -> Result<reqwest::Response, anyhow::Error> {
    let response = reqwest::Client::builder()
      .user_agent(USER_AGENT)
      .build()?
      .get(format!("https://gitlab.com/api/v4{}", path))
      .bearer_auth(&self.access_token)
      .send()
      .await?;
    Ok(response)
  }

  #[instrument(name = "GitLabUserClient::current_user", skip(self), err)]
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

  #[instrument(name = "GitLabUserClient::emails", skip(self), err)]
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

  #[instrument(name = "GitLabUserClient::get_user", skip(self), err)]
  pub async fn get_user(
    &self,
    name: &str,
  ) -> Result<Option<User>, anyhow::Error> {
    let name = super::sanitize_url_part(name);
    let res = self.request(&format!("/users?username={name}")).await?;
    let status = res.status();
    if status == StatusCode::NOT_FOUND {
      Ok(None)
    } else if status.is_success() {
      let user_list: Vec<User> = res.json().await?;
      Ok(Some(user_list.into_iter().next().unwrap()))
    } else {
      let response = res.text().await?;
      Err(anyhow::anyhow!(
        "failed to get user '{name}' (status {status}): {response}"
      ))
    }
  }

  #[instrument(name = "GitLabUserClient::get_repo", skip(self), err)]
  pub async fn get_repo(
    &self,
    owner: &str,
    name: &str,
  ) -> Result<Option<Repository>, anyhow::Error> {
    let owner = super::sanitize_url_part(owner);
    let name = super::sanitize_url_part(name);
    let res = self.request(&format!("/projects/{owner}%2F{name}")).await?;
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

#[derive(Debug, Deserialize, Clone, Eq, PartialEq)]
pub struct User {
  pub id: i64,
  pub username: String,
  pub name: String,
  pub avatar_url: String,
  pub created_at: Option<chrono::DateTime<chrono::Utc>>,
  pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct Email {
  pub email: String,
  pub confirmed_at: Option<chrono::DateTime<chrono::Utc>>,
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
  pub username: String,
}

#[derive(Deserialize)]
pub struct RepositoryPermissions {
  pub project_access: Option<RepositoryAccess>,
  pub group_access: Option<RepositoryAccess>,
}

#[derive(Deserialize)]
pub struct RepositoryAccess {
  pub access_level: u8,
}
