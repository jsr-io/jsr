// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.
#![allow(dead_code)]

use chrono::DateTime;
use chrono::Utc;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use sqlx::FromRow;
use sqlx::Row;
use sqlx::ValueRef;
use sqlx::types::Json;
use uuid::Uuid;

use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeDescription;
use crate::ids::ScopeName;
use crate::ids::Version;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct User {
  pub id: Uuid,
  pub name: String,
  pub email: Option<String>,
  pub avatar_url: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub github_id: Option<i64>,
  pub gitlab_id: Option<i64>,
  pub is_blocked: bool,
  pub is_staff: bool,
  pub scope_usage: i64,
  pub scope_limit: i32,
  pub invite_count: i64,
  pub newer_ticket_messages_count: i64,
}

impl FromRow<'_, sqlx::postgres::PgRow> for User {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      id: try_get_row_or(row, "id", "user_id")?,
      name: try_get_row_or(row, "name", "user_name")?,
      email: try_get_row_or(row, "email", "user_email")?,
      avatar_url: try_get_row_or(row, "avatar_url", "user_avatar_url")?,
      github_id: try_get_row_or(row, "github_id", "user_github_id")?,
      gitlab_id: try_get_row_or(row, "gitlab_id", "user_gitlab_id")?,
      is_blocked: try_get_row_or(row, "is_blocked", "user_is_blocked")?,
      is_staff: try_get_row_or(row, "is_staff", "user_is_staff")?,
      scope_usage: try_get_row_or(row, "scope_usage", "user_scope_usage")?,
      scope_limit: try_get_row_or(row, "scope_limit", "user_scope_limit")?,
      invite_count: try_get_row_or(row, "invite_count", "user_invite_count")?,
      updated_at: try_get_row_or(row, "created_at", "user_created_at")?,
      created_at: try_get_row_or(row, "created_at", "user_created_at")?,
      newer_ticket_messages_count: try_get_row_or(
        row,
        "newer_ticket_messages_count",
        "user_newer_ticket_messages_count",
      )?,
    })
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPublic {
  pub id: Uuid,
  pub name: String,
  pub avatar_url: String,
  pub github_id: Option<i64>,
  pub gitlab_id: Option<i64>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
  fn from(user: User) -> UserPublic {
    UserPublic {
      id: user.id,
      name: user.name,
      avatar_url: user.avatar_url,
      github_id: user.github_id,
      gitlab_id: user.gitlab_id,
      updated_at: user.updated_at,
      created_at: user.created_at,
    }
  }
}

impl FromRow<'_, sqlx::postgres::PgRow> for UserPublic {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      id: try_get_row_or(row, "id", "user_id")?,
      name: try_get_row_or(row, "name", "user_name")?,
      avatar_url: try_get_row_or(row, "avatar_url", "user_avatar_url")?,
      github_id: try_get_row_or(row, "github_id", "user_github_id")?,
      gitlab_id: try_get_row_or(row, "gitlab_id", "user_gitlab_id")?,
      updated_at: try_get_row_or(row, "created_at", "user_created_at")?,
      created_at: try_get_row_or(row, "created_at", "user_created_at")?,
    })
  }
}

#[derive(Debug, Default)]
pub struct NewUser<'s> {
  pub name: &'s str,
  pub email: Option<&'s str>,
  pub avatar_url: &'s str,
  pub github_id: Option<i64>,
  pub gitlab_id: Option<i64>,
  pub is_blocked: bool,
  pub is_staff: bool,
}

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "task_status", rename_all = "lowercase")]
pub enum PublishingTaskStatus {
  /// The task is scheduled to start processing.
  Pending,
  /// The task is currently being processed. Processing entails unpacking the
  /// package tarball, validating the package, and publishing individual files
  /// to the registry. It is finalized by uploading the package version
  /// manifest to GCS and inserting the published version into the database.
  Processing,
  /// The task processing has been completed. The package manifest on GCS is
  /// being updated to reflect the new version.
  Processed,
  /// The task has been completed successfully.
  Success,
  /// The task encountered a fatal error and publishing cannot be completed.
  Failure,
}

#[derive(Debug, Clone)]
pub struct PublishingTask {
  pub id: Uuid,
  pub status: PublishingTaskStatus,
  pub error: Option<PublishingTaskError>,
  pub package_scope: ScopeName,
  pub package_name: PackageName,
  pub package_version: Version,
  pub config_file: PackagePath,
  pub user_id: Option<Uuid>,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for PublishingTask {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      id: try_get_row_or(row, "id", "task_id")?,
      status: try_get_row_or(row, "status", "task_status")?,
      error: try_get_row_or(row, "error", "task_error")?,
      package_scope: try_get_row_or(
        row,
        "package_scope",
        "task_package_scope",
      )?,
      package_name: try_get_row_or(row, "package_name", "task_package_name")?,
      package_version: try_get_row_or(
        row,
        "package_version",
        "task_package_version",
      )?,
      config_file: try_get_row_or(row, "config_file", "task_config_file")?,
      updated_at: try_get_row_or(row, "updated_at", "task_updated_at")?,
      created_at: try_get_row_or(row, "created_at", "task_created_at")?,
      user_id: try_get_row_or(row, "user_id", "task_user_id")?,
    })
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishingTaskError {
  pub code: String,
  pub message: String,
}

impl sqlx::Decode<'_, sqlx::Postgres> for PublishingTaskError {
  fn decode(
    value: sqlx::postgres::PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    let s: sqlx::types::Json<PublishingTaskError> =
      sqlx::Decode::<'_, sqlx::Postgres>::decode(value)?;
    Ok(s.0)
  }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for PublishingTaskError {
  fn encode_by_ref(
    &self,
    buf: &mut <sqlx::Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    <sqlx::types::Json<&PublishingTaskError> as sqlx::Encode<
      '_,
      sqlx::Postgres,
    >>::encode_by_ref(&sqlx::types::Json(self), buf)
  }
}

impl sqlx::Type<sqlx::Postgres> for PublishingTaskError {
  fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
    <sqlx::types::Json<PublishingTaskError> as sqlx::Type<sqlx::Postgres>>::type_info()
  }
}

pub struct NewPublishingTask<'s> {
  pub package_scope: &'s ScopeName,
  pub package_name: &'s PackageName,
  pub package_version: &'s Version,
  pub config_file: &'s PackagePath,
  pub user_id: Option<Uuid>,
}

#[derive(Debug)]
pub struct Scope {
  pub scope: ScopeName,
  pub description: ScopeDescription,
  pub creator: Uuid,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub package_limit: i32,
  pub new_package_per_week_limit: i32,
  pub publish_attempts_per_week_limit: i32,
  pub verify_oidc_actor: bool,
  pub require_publishing_from_ci: bool,
}

fn try_get_row_or<
  'r,
  T: sqlx::Decode<'r, <sqlx::postgres::PgRow as sqlx::Row>::Database>
    + sqlx::Type<<sqlx::postgres::PgRow as sqlx::Row>::Database>,
>(
  row: &'r sqlx::postgres::PgRow,
  a: &str,
  b: &str,
) -> Result<T, sqlx::Error> {
  match row.try_get(a) {
    Err(sqlx::Error::ColumnNotFound(_)) => row.try_get(b),
    row => row,
  }
}

impl FromRow<'_, sqlx::postgres::PgRow> for Scope {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      scope: try_get_row_or(row, "scope", "scope_scope")?,
      description: try_get_row_or(row, "description", "scope_description")?,
      creator: try_get_row_or(row, "creator", "scope_creator")?,
      updated_at: try_get_row_or(row, "updated_at", "scope_updated_at")?,
      created_at: try_get_row_or(row, "created_at", "scope_created_at")?,
      package_limit: try_get_row_or::<i32>(
        row,
        "package_limit",
        "scope_package_limit",
      )?,
      new_package_per_week_limit: try_get_row_or::<i32>(
        row,
        "new_package_per_week_limit",
        "scope_new_package_per_week_limit",
      )?,
      publish_attempts_per_week_limit: try_get_row_or::<i32>(
        row,
        "publish_attempts_per_week_limit",
        "scope_publish_attempts_per_week_limit",
      )?,
      verify_oidc_actor: try_get_row_or(
        row,
        "verify_oidc_actor",
        "scope_verify_oidc_actor",
      )?,
      require_publishing_from_ci: try_get_row_or(
        row,
        "require_publishing_from_ci",
        "scope_require_publishing_from_ci",
      )?,
    })
  }
}

#[derive(Debug)]
pub struct ScopeUsage {
  pub package: i32,
  pub new_package_per_week: i32,
  pub publish_attempts_per_week: i32,
}

impl FromRow<'_, sqlx::postgres::PgRow> for ScopeUsage {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      package: try_get_row_or::<i64>(row, "package", "usage_package")?
        .try_into()
        .unwrap(),
      new_package_per_week: try_get_row_or::<i64>(
        row,
        "new_package_per_week",
        "usage_new_package_per_week",
      )?
      .try_into()
      .unwrap(),
      publish_attempts_per_week: try_get_row_or::<i64>(
        row,
        "publish_attempts_per_week",
        "usage_publish_attempts_per_week",
      )?
      .try_into()
      .unwrap(),
    })
  }
}

#[derive(Debug)]
pub struct ScopeMember {
  pub scope: ScopeName,
  pub user_id: Uuid,
  pub is_admin: bool,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewScopeMember<'s> {
  pub scope: &'s ScopeName,
  pub user_id: Uuid,
  pub is_admin: bool,
}

#[derive(Debug)]
pub struct ScopeInvite {
  pub target_user_id: Uuid,
  pub requesting_user_id: Uuid,
  pub scope: ScopeName,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewScopeInvite<'s> {
  pub target_user_id: Uuid,
  pub requesting_user_id: Uuid,
  pub scope: &'s ScopeName,
}

#[derive(Debug)]
pub struct Package {
  pub scope: ScopeName,
  pub name: PackageName,
  pub description: String,
  pub github_repository_id: Option<i64>,
  pub runtime_compat: RuntimeCompat,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub version_count: i64,
  pub latest_version: Option<String>,
  pub when_featured: Option<DateTime<Utc>>,
  pub is_archived: bool,
  pub readme_source: ReadmeSource,
}

#[derive(Debug, Clone, PartialEq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "package_readme_source", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ReadmeSource {
  Readme,
  JSDoc,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Package {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      scope: try_get_row_or(row, "scope", "package_scope")?,
      name: try_get_row_or(row, "name", "package_name")?,
      description: try_get_row_or(row, "description", "package_description")?,
      github_repository_id: try_get_row_or(
        row,
        "github_repository_id",
        "package_repository_id",
      )?,
      runtime_compat: try_get_row_or(
        row,
        "runtime_compat",
        "package_runtime_compat",
      )?,
      updated_at: try_get_row_or(row, "updated_at", "package_updated_at")?,
      created_at: try_get_row_or(row, "created_at", "package_created_at")?,
      version_count: try_get_row_or(
        row,
        "version_count",
        "package_version_count",
      )?,
      latest_version: try_get_row_or(
        row,
        "latest_version",
        "package_latest_version",
      )?,
      when_featured: try_get_row_or(
        row,
        "when_featured",
        "package_when_featured",
      )?,
      is_archived: try_get_row_or(row, "is_archived", "package_is_archived")?,
      readme_source: try_get_row_or(
        row,
        "readme_source",
        "package_readme_source",
      )?,
    })
  }
}

#[derive(Debug)]
pub struct PackageVersion {
  pub scope: ScopeName,
  pub name: PackageName,
  pub version: Version,
  pub user_id: Option<Uuid>,
  pub exports: ExportsMap,
  pub is_yanked: bool,
  pub readme_path: Option<PackagePath>,
  pub uses_npm: bool,
  pub newer_versions_count: i64,
  pub lifetime_download_count: i64,
  pub meta: PackageVersionMeta,
  pub rekor_log_id: Option<String>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewPackageVersion<'s> {
  pub scope: &'s ScopeName,
  pub name: &'s PackageName,
  pub version: &'s Version,
  pub user_id: Option<&'s Uuid>,
  pub readme_path: Option<&'s PackagePath>,
  pub exports: &'s ExportsMap,
  pub uses_npm: bool,
  pub meta: PackageVersionMeta,
}

#[derive(Debug)]
pub struct PackageVersionForResolution {
  pub version: Version,
  pub exports: ExportsMap,
}

#[derive(Debug)]
pub struct PackageVersionForNpmVersionManifest {
  pub version: Version,
  pub is_yanked: bool,
  pub created_at: DateTime<Utc>,
  pub npm_tarball_revision: i32,
  pub npm_tarball_sha1: String,
  pub npm_tarball_sha512: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PackageVersionMeta {
  pub has_readme: bool,
  pub has_readme_examples: bool,
  pub all_entrypoints_docs: bool,
  pub percentage_documented_symbols: f32,
  pub all_fast_check: bool, // mean no slow types
  pub has_provenance: bool,
}

impl sqlx::Decode<'_, sqlx::Postgres> for PackageVersionMeta {
  fn decode(
    value: sqlx::postgres::PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    if !value.is_null() {
      let s: sqlx::types::Json<PackageVersionMeta> =
        sqlx::Decode::<'_, sqlx::Postgres>::decode(value)?;
      Ok(s.0)
    } else {
      Ok(Default::default())
    }
  }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for PackageVersionMeta {
  fn encode_by_ref(
    &self,
    buf: &mut <sqlx::Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    <sqlx::types::Json<&PackageVersionMeta> as sqlx::Encode<
      '_,
      sqlx::Postgres,
    >>::encode_by_ref(&Json(self), buf)
  }
}

impl sqlx::Type<sqlx::Postgres> for PackageVersionMeta {
  fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
    <sqlx::types::Json<PackageVersionMeta> as sqlx::Type<sqlx::Postgres>>::type_info(
    )
  }
}

#[derive(Debug)]
pub struct PackageFile {
  pub scope: ScopeName,
  pub name: PackageName,
  pub version: Version,
  pub path: PackagePath,
  pub size: i32,
  pub checksum: Option<String>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewPackageFile<'s> {
  pub scope: &'s ScopeName,
  pub name: &'s PackageName,
  pub version: &'s Version,
  pub path: &'s PackagePath,
  pub size: i32,
  pub checksum: Option<&'s str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "token_type", rename_all = "lowercase")]
pub enum TokenType {
  /// Token obtained when signing in with GitHub, used in the web UI.
  Web,
  /// Token obtained through device flow.
  Device,
  /// Personal access token obtained through the web UI.
  Personal,
}

impl TokenType {
  pub fn prefix(self) -> &'static str {
    match self {
      Self::Web => "jsrw",
      Self::Device => "jsrd",
      Self::Personal => "jsrp",
    }
  }
}

#[derive(Debug)]
pub struct OauthState {
  pub csrf_token: String,
  pub pkce_code_verifier: String,
  pub redirect_url: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewOauthState<'a> {
  pub csrf_token: &'a str,
  pub pkce_code_verifier: &'a str,
  pub redirect_url: &'a str,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct OauthDeviceState {
  pub id: Uuid,
  pub auth: String,
}

#[derive(Debug, Clone)]
pub struct GithubIdentity {
  pub github_id: i64,
  pub access_token: Option<String>,
  pub access_token_expires_at: Option<DateTime<Utc>>,
  pub refresh_token: Option<String>,
  pub refresh_token_expires_at: Option<DateTime<Utc>>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGithubIdentity {
  pub github_id: i64,
  pub access_token: Option<String>,
  pub access_token_expires_at: Option<DateTime<Utc>>,
  pub refresh_token: Option<String>,
  pub refresh_token_expires_at: Option<DateTime<Utc>>,
}

impl From<GithubIdentity> for NewGithubIdentity {
  fn from(t: GithubIdentity) -> Self {
    Self {
      github_id: t.github_id,
      access_token: t.access_token,
      access_token_expires_at: t.access_token_expires_at,
      refresh_token: t.refresh_token,
      refresh_token_expires_at: t.refresh_token_expires_at,
    }
  }
}

#[derive(Debug, Clone)]
pub struct GitlabIdentity {
  pub gitlab_id: i64,
  pub access_token: Option<String>,
  pub access_token_expires_at: Option<DateTime<Utc>>,
  pub refresh_token: Option<String>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGitlabIdentity {
  pub gitlab_id: i64,
  pub access_token: Option<String>,
  pub access_token_expires_at: Option<DateTime<Utc>>,
  pub refresh_token: Option<String>,
}

impl From<GitlabIdentity> for NewGitlabIdentity {
  fn from(t: GitlabIdentity) -> Self {
    Self {
      gitlab_id: t.gitlab_id,
      access_token: t.access_token,
      access_token_expires_at: t.access_token_expires_at,
      refresh_token: t.refresh_token,
    }
  }
}

#[derive(Debug, Clone)]
pub struct Token {
  pub id: Uuid,
  pub hash: String,
  pub user_id: Uuid,
  pub r#type: TokenType,
  pub description: Option<String>,
  pub expires_at: Option<DateTime<Utc>>,
  /// `None` means the token has no permissions policy, which is equivalent to
  /// the token having all permissions.
  pub permissions: Option<Permissions>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewToken {
  pub hash: String,
  pub user_id: Uuid,
  pub r#type: TokenType,
  pub description: Option<String>,
  pub expires_at: Option<DateTime<Utc>>,
  pub permissions: Option<Permissions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permissions(pub Vec<Permission>);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "permission")]
pub enum Permission {
  #[serde(rename = "package/publish")]
  PackagePublish(PackagePublishPermission),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PackagePublishPermission {
  #[serde(rename_all = "camelCase")]
  Version {
    scope: ScopeName,
    package: PackageName,
    version: Version,
    tarball_hash: String,
  },
  #[serde(rename_all = "camelCase")]
  Package {
    scope: ScopeName,
    package: PackageName,
  },
  #[serde(rename_all = "camelCase")]
  Scope { scope: ScopeName },
}

impl sqlx::Decode<'_, sqlx::Postgres> for Permissions {
  fn decode(
    value: sqlx::postgres::PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    let s: sqlx::types::Json<Permissions> =
      sqlx::Decode::<'_, sqlx::Postgres>::decode(value)?;
    Ok(s.0)
  }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for Permissions {
  fn encode_by_ref(
    &self,
    buf: &mut <sqlx::Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    <sqlx::types::Json<&Permissions> as sqlx::Encode<
      '_,
      sqlx::Postgres,
    >>::encode_by_ref(&sqlx::types::Json(self), buf)
  }
}

impl sqlx::Type<sqlx::Postgres> for Permissions {
  fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
    <sqlx::types::Json<Permissions> as sqlx::Type<sqlx::Postgres>>::type_info()
  }
}

#[derive(Debug)]
pub struct GithubRepository {
  pub id: i64,
  pub owner: String,
  pub name: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for GithubRepository {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      id: try_get_row_or(row, "id", "github_repository_id")?,
      owner: try_get_row_or(row, "owner", "github_repository_owner")?,
      name: try_get_row_or(row, "name", "github_repository_name")?,
      updated_at: try_get_row_or(
        row,
        "updated_at",
        "github_repository_updated_at",
      )?,
      created_at: try_get_row_or(
        row,
        "created_at",
        "github_repository_created_at",
      )?,
    })
  }
}

pub struct NewGithubRepository<'s> {
  pub id: i64,
  pub owner: &'s str,
  pub name: &'s str,
}

#[derive(Debug)]
pub struct Authorization {
  pub exchange_token: String,
  pub code: String,

  pub challenge: String,
  pub permissions: Option<Permissions>,
  pub approved: Option<bool>,
  pub user_id: Option<Uuid>,

  pub expires_at: DateTime<Utc>,

  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

pub struct NewAuthorization<'s> {
  pub exchange_token: &'s str,
  pub code: &'s str,

  pub challenge: &'s str,
  pub permissions: Option<Permissions>,

  pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ExportsMap(IndexMap<String, String>);

impl ExportsMap {
  pub fn new(exports: IndexMap<String, String>) -> Self {
    Self(exports)
  }

  #[cfg(test)]
  pub fn mock() -> Self {
    let mut exports = IndexMap::new();
    exports.insert(".".to_owned(), "./mod.ts".to_owned());
    Self::new(exports)
  }

  pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
    self.0.iter()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn into_inner(self) -> IndexMap<String, String> {
    self.0
  }

  pub fn contains_key(&self, key: &str) -> bool {
    self.0.contains_key(key)
  }
}

impl sqlx::Decode<'_, sqlx::Postgres> for ExportsMap {
  fn decode(
    value: sqlx::postgres::PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    let s: sqlx::types::Json<IndexMap<String, String>> =
      sqlx::Decode::<'_, sqlx::Postgres>::decode(value)?;
    Ok(ExportsMap(s.0))
  }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ExportsMap {
  fn encode_by_ref(
    &self,
    buf: &mut <sqlx::Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    <sqlx::types::Json<&IndexMap<String, String>> as sqlx::Encode<
      '_,
      sqlx::Postgres,
    >>::encode_by_ref(&sqlx::types::Json(&self.0), buf)
  }
}

impl sqlx::Type<sqlx::Postgres> for ExportsMap {
  fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
    <sqlx::types::Json<IndexMap<String, String>> as sqlx::Type<
      sqlx::Postgres,
    >>::type_info()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "dependency_kind", rename_all = "lowercase")]
pub enum DependencyKind {
  Jsr,
  Npm,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PackageVersionDependency {
  pub package_scope: ScopeName,
  pub package_name: PackageName,
  pub package_version: Version,
  pub dependency_kind: DependencyKind,
  pub dependency_name: String,
  pub dependency_constraint: String,
  pub dependency_path: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PackageVersionReference {
  pub scope: ScopeName,
  pub name: PackageName,
  pub version: Version,
}

#[derive(Debug, Clone)]
pub struct Dependent {
  pub scope: ScopeName,
  pub name: PackageName,
  pub versions: Vec<Version>,
  pub total_versions: i64,
}

#[derive(Debug, Clone)]
pub struct NewPackageVersionDependency<'s> {
  pub package_scope: &'s ScopeName,
  pub package_name: &'s PackageName,
  pub package_version: &'s Version,
  pub dependency_kind: DependencyKind,
  pub dependency_name: &'s str,
  pub dependency_constraint: &'s str,
  pub dependency_path: &'s str,
}

pub type PackageWithGitHubRepoAndMeta =
  (Package, Option<GithubRepository>, PackageVersionMeta);

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NpmTarball {
  pub scope: ScopeName,
  pub name: PackageName,
  pub version: Version,
  pub revision: i32,
  pub sha1: String,
  pub sha512: String,
  pub size: i32,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewNpmTarball<'s> {
  pub scope: &'s ScopeName,
  pub name: &'s PackageName,
  pub version: &'s Version,
  pub revision: i32,
  pub sha1: &'s str,
  pub sha512: &'s str,
  pub size: i32,
}

/// Keys reference https://runtime-keys.proposal.wintercg.org/.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCompat {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub browser: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub deno: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub node: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub workerd: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bun: Option<bool>,
}

impl sqlx::Decode<'_, sqlx::Postgres> for RuntimeCompat {
  fn decode(
    value: sqlx::postgres::PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    let s: sqlx::types::Json<RuntimeCompat> =
      sqlx::Decode::<'_, sqlx::Postgres>::decode(value)?;
    Ok(s.0)
  }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for RuntimeCompat {
  fn encode_by_ref(
    &self,
    buf: &mut <sqlx::Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    <sqlx::types::Json<&RuntimeCompat> as sqlx::Encode<
      '_,
      sqlx::Postgres,
    >>::encode_by_ref(&Json(self), buf)
  }
}

impl sqlx::Type<sqlx::Postgres> for RuntimeCompat {
  fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
    <sqlx::types::Json<RuntimeCompat> as sqlx::Type<sqlx::Postgres>>::type_info(
    )
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDownloadCount {
  pub scope: ScopeName,
  pub package: PackageName,
  pub version: Version,
  pub time_bucket: DateTime<Utc>,
  pub kind: DownloadKind,
  pub count: i64,
}

#[derive(Debug, Clone)]
pub struct DownloadDataPoint {
  pub time_bucket: DateTime<Utc>,
  pub kind: DownloadKind,
  pub count: i64,
}

#[derive(Debug, Clone)]
pub struct VersionDownloadDataPoint {
  pub time_bucket: DateTime<Utc>,
  pub version: Version,
  pub kind: DownloadKind,
  pub count: i64,
}

#[derive(
  Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize,
)]
#[sqlx(type_name = "download_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
  /// A download of the version's JSR $version_meta.json file.
  JsrMeta,
  /// A download of the NPM tarball.
  NpmTgz,
}

impl sqlx::postgres::PgHasArrayType for DownloadKind {
  fn array_type_info() -> sqlx::postgres::PgTypeInfo {
    sqlx::postgres::PgTypeInfo::with_name("_download_kind")
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "ticket_kind", rename_all = "snake_case")]
pub enum TicketKind {
  UserScopeQuotaIncrease,
  ScopeQuotaIncrease,
  ScopeClaim,
  PackageReport,
  Other,
}

#[derive(Debug, Deserialize)]
pub struct NewTicket {
  pub kind: TicketKind,
  pub meta: serde_json::Value,
  pub message: String,
}

#[derive(Debug, Clone)]
pub struct Ticket {
  pub id: Uuid,
  pub kind: TicketKind,
  pub creator: Uuid,
  pub meta: serde_json::Value,
  pub closed: bool,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Ticket {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      id: try_get_row_or(row, "id", "ticket_id")?,
      kind: try_get_row_or(row, "kind", "ticket_kind")?,
      creator: try_get_row_or(row, "creator", "ticket_creator")?,
      meta: try_get_row_or(row, "meta", "ticket_meta")?,
      closed: try_get_row_or(row, "closed", "ticket_closed")?,
      updated_at: try_get_row_or(row, "updated_at", "ticket_updated_at")?,
      created_at: try_get_row_or(row, "created_at", "ticket_created_at")?,
    })
  }
}

#[derive(Debug, Deserialize)]
pub struct NewTicketMessage {
  pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketMessage {
  pub ticket_id: Uuid,
  pub author: Uuid,
  pub message: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

pub type FullTicket = (Ticket, User, Vec<(TicketMessage, UserPublic)>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLog {
  pub actor_id: Uuid,
  pub is_sudo: bool,
  pub action: String,
  pub meta: serde_json::Value,
  pub created_at: DateTime<Utc>,
}

impl FromRow<'_, sqlx::postgres::PgRow> for AuditLog {
  fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(Self {
      actor_id: try_get_row_or(row, "actor_id", "audit_log_actor_id")?,
      is_sudo: try_get_row_or(row, "is_sudo", "audit_log_is_sudo")?,
      action: try_get_row_or(row, "action", "audit_log_action")?,
      meta: try_get_row_or(row, "meta", "audit_log_meta")?,
      created_at: try_get_row_or(row, "created_at", "audit_log_created_at")?,
    })
  }
}
