// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use chrono::DateTime;
use chrono::Utc;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use sqlx::types::Json;
use sqlx::ValueRef;
use thiserror::Error;
use uuid::Uuid;

use crate::ids::PackageName;
use crate::ids::PackageNameValidateError;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::ScopeNameValidateError;
use crate::ids::Version;

#[derive(Debug, Clone)]
pub struct User {
  pub id: Uuid,
  pub name: String,
  pub email: Option<String>,
  pub avatar_url: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub github_id: Option<i64>,
  pub is_blocked: bool,
  pub is_staff: bool,
  pub scope_usage: i64,
  pub scope_limit: i32,
  pub invite_count: i64,
}

#[derive(Debug, Clone)]
pub struct UserPublic {
  pub id: Uuid,
  pub name: String,
  pub avatar_url: String,
  pub github_id: Option<i64>,
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
      updated_at: user.updated_at,
      created_at: user.created_at,
    }
  }
}

#[derive(Debug, Default)]
pub struct NewUser<'s> {
  pub name: &'s str,
  pub email: Option<&'s str>,
  pub avatar_url: &'s str,
  pub github_id: Option<i64>,
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
  pub creator: Uuid,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub package_limit: i32,
  pub new_package_per_week_limit: i32,
  pub publish_attempts_per_week_limit: i32,
  pub verify_oidc_actor: bool,
  pub require_publishing_from_ci: bool,
}

#[derive(Debug)]
pub struct ScopeUsage {
  pub package: i32,
  pub new_package_per_week: i32,
  pub publish_attempts_per_week: i32,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PackageVersionMeta {
  pub has_readme: bool,
  pub has_readme_examples: bool,
  pub all_entrypoints_docs: bool,
  pub percentage_documented_symbols: f32,
  pub all_fast_check: bool,
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

#[derive(Debug, Clone)]
pub struct Alias {
  pub name: String,
  pub major_version: i32,
  pub target: AliasTarget,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq)]
pub enum AliasTarget {
  Jsr(ScopeName, PackageName),
  Npm(String),
}

impl std::fmt::Display for AliasTarget {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Jsr(scope, name) => write!(f, "jsr:@{}/{}", scope, name),
      Self::Npm(name) => write!(f, "npm:{}", name),
    }
  }
}

impl std::fmt::Debug for AliasTarget {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Display::fmt(self, f)
  }
}

impl std::str::FromStr for AliasTarget {
  type Err = AliasTargetParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let Some((scheme, target)) = s.split_once(':') else {
      return Err(AliasTargetParseError::MissingScheme);
    };
    match scheme {
      "jsr" => {
        let Some((scope_with_at, name)) = target.split_once('/') else {
          return Err(AliasTargetParseError::MissingSlashInJSRTarget);
        };
        let Some(scope) = scope_with_at.strip_prefix('@') else {
          return Err(
            AliasTargetParseError::MissingAtPrefixingScopeNameInJSRTarget,
          );
        };
        let scope_name = ScopeName::try_from(scope)
          .map_err(AliasTargetParseError::InvalidScopeName)?;
        let package_name = PackageName::try_from(name)
          .map_err(AliasTargetParseError::InvalidPackageName)?;
        Ok(Self::Jsr(scope_name, package_name))
      }
      "npm" => Ok(Self::Npm(target.to_owned())),
      _ => Err(AliasTargetParseError::UnknownScheme(scheme.to_owned())),
    }
  }
}

impl serde::Serialize for AliasTarget {
  fn serialize<S: serde::Serializer>(
    &self,
    serializer: S,
  ) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&self.to_string())
  }
}

impl<'de> serde::Deserialize<'de> for AliasTarget {
  fn deserialize<D: serde::Deserializer<'de>>(
    deserializer: D,
  ) -> Result<Self, D::Error> {
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
  }
}

#[derive(Debug, Clone, Error)]
pub enum AliasTargetParseError {
  #[error("missing target scheme")]
  MissingScheme,
  #[error("unknown target scheme: {0}")]
  UnknownScheme(String),
  #[error("missing slash in JSR target")]
  MissingSlashInJSRTarget,
  #[error("missing @ prefix for scope in JSR target")]
  MissingAtPrefixingScopeNameInJSRTarget,
  #[error("invalid scope name: {0}")]
  InvalidScopeName(ScopeNameValidateError),
  #[error("invalid package name: {0}")]
  InvalidPackageName(PackageNameValidateError),
}

#[derive(Debug)]
pub struct GithubRepository {
  pub id: i64,
  pub owner: String,
  pub name: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
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
