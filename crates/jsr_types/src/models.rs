// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.
#![allow(dead_code)]

use chrono::DateTime;
use chrono::Utc;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
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
  pub deletion_hold: bool,
  pub scope_usage: i64,
  pub scope_limit: Option<i32>,
  pub invite_count: i64,
  pub newer_ticket_messages_count: i64,
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

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
  feature = "postgres",
  derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[cfg_attr(
  feature = "postgres",
  postgres(name = "task_status", rename_all = "lowercase")
)]
pub enum PublishingTaskStatus {
  /// The task is scheduled to start processing.
  Pending,
  /// The task is currently being processed. Processing entails unpacking the
  /// package tarball, validating the package, and publishing individual files
  /// to the registry. It is finalized by uploading the package version
  /// manifest to S3 and inserting the published version into the database.
  Processing,
  /// The task processing has been completed. The package manifest on S3 is
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
  pub is_archived: bool,
  pub readme_source: ReadmeSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
  feature = "postgres",
  derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[cfg_attr(
  feature = "postgres",
  postgres(name = "package_readme_source", rename_all = "lowercase")
)]
#[serde(rename_all = "lowercase")]
pub enum ReadmeSource {
  Readme,
  JSDoc,
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
  pub meta: PackageVersionMeta,
  pub rekor_log_id: Option<String>,
  pub license: Option<String>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct PackageVersionWithNewerVersionsCount {
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
  pub license: Option<String>,
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
  pub license: String,
}

#[derive(Debug)]
pub struct PackageVersionForResolution {
  pub version: Version,
  pub exports: ExportsMap,
}

#[derive(Debug)]
pub struct PackageVersionForMetadata {
  pub version: Version,
  pub is_yanked: bool,
  pub created_at: DateTime<Utc>,
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
  pub entrypoints_without_docs: Vec<String>,
  pub percentage_documented_symbols: f32,
  pub all_fast_check: bool, // mean no slow types
  pub has_provenance: bool,
  /// Number of distinct symbols the generated docs list for this version, used
  /// to show the package's API surface area in the nav (jsr-io/jsr#1206).
  /// `None` on versions published before this was recorded, which render
  /// without the chip rather than claiming zero symbols.
  pub symbol_count: Option<u32>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
  feature = "postgres",
  derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[cfg_attr(
  feature = "postgres",
  postgres(name = "token_type", rename_all = "lowercase")
)]
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
  /// The user that initiated this flow, if any. `None` for the login flow (no
  /// authenticated user yet); `Some` for the account-linking ("connect") flow,
  /// where the callback must match the authenticated user.
  pub user_id: Option<Uuid>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewOauthState<'a> {
  pub csrf_token: &'a str,
  pub pkce_code_verifier: &'a str,
  pub redirect_url: &'a str,
  pub user_id: Option<Uuid>,
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

  #[cfg(any(test, feature = "testing"))]
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

  pub fn as_inner(&self) -> &IndexMap<String, String> {
    &self.0
  }

  pub fn into_inner(self) -> IndexMap<String, String> {
    self.0
  }

  pub fn contains_key(&self, key: &str) -> bool {
    self.0.contains_key(key)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
  feature = "postgres",
  derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[cfg_attr(
  feature = "postgres",
  postgres(name = "dependency_kind", rename_all = "lowercase")
)]
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
  pub dependency_fallback_url: Option<String>,
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
  pub dependency_fallback_url: Option<&'s str>,
}

pub type PackageWithGitHubRepoAndMeta =
  (Package, Option<GithubRepository>, PackageVersionMeta);

#[derive(Debug)]
pub struct StatsPackage {
  pub scope: ScopeName,
  pub name: PackageName,
  pub description: String,
}

#[derive(Debug)]
pub struct StatsPackageVersion {
  pub scope: ScopeName,
  pub name: PackageName,
  pub version: Version,
  pub description: String,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
  feature = "postgres",
  derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[cfg_attr(
  feature = "postgres",
  postgres(name = "download_kind", rename_all = "snake_case")
)]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
  /// A download of the version's JSR $version_meta.json file.
  JsrMeta,
  /// A download of the NPM tarball.
  NpmTgz,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
  feature = "postgres",
  derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
  feature = "postgres",
  postgres(name = "ticket_kind", rename_all = "snake_case")
)]
pub enum TicketKind {
  UserScopeQuotaIncrease,
  ScopeQuotaIncrease,
  ScopeClaim,
  PackageReport,
  Other,
  /// Opened by staff, addressed to a user. The ticket's creator is the user
  /// being contacted, and its first message is outbound.
  StaffOutreach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
  feature = "postgres",
  derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
  feature = "postgres",
  postgres(name = "ticket_status", rename_all = "snake_case")
)]
pub enum TicketStatus {
  Open,
  WaitingOnUser,
  WaitingOnSupport,
  Closed,
  Spam,
}

impl std::str::FromStr for TicketStatus {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(match s {
      "open" => Self::Open,
      "waiting_on_user" => Self::WaitingOnUser,
      "waiting_on_support" => Self::WaitingOnSupport,
      "closed" => Self::Closed,
      "spam" => Self::Spam,
      _ => return Err(()),
    })
  }
}

impl TicketStatus {
  /// Whether the ticket is still being worked. `spam` counts as resolved: it is
  /// a parking spot for junk mail, not part of the queue.
  pub fn is_active(self) -> bool {
    !matches!(self, TicketStatus::Closed | TicketStatus::Spam)
  }
}

/// Which way a message travelled. `inbound` is from the person who opened the
/// ticket, `outbound` is from JSR — a staff reply or the automatic
/// acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
  feature = "postgres",
  derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
  feature = "postgres",
  postgres(name = "ticket_message_direction", rename_all = "snake_case")
)]
pub enum TicketMessageDirection {
  Inbound,
  Outbound,
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
  /// Human-quotable identifier (`TICKET-YYYYMMDD-XXXXX`). Carried in email
  /// subject lines so a reply that has lost its threading headers can still be
  /// matched back to its ticket.
  pub ticket_number: String,
  pub kind: TicketKind,
  /// `None` for a ticket opened by email that nobody has claimed yet; the
  /// `reporter_*` fields identify the reporter until then.
  pub creator: Option<Uuid>,
  pub reporter_email: Option<String>,
  pub reporter_name: Option<String>,
  /// The email subject an email-opened ticket arrived with. `None` for tickets
  /// opened through the web UI, whose title is derived from `kind` and `meta`.
  pub subject: Option<String>,
  /// Secret that lets the reporter of an unclaimed ticket read and reply to it
  /// without an account, and bind it to their account if they make one. Cleared
  /// once claimed.
  pub claim_token: Option<Uuid>,
  pub meta: serde_json::Value,
  pub status: TicketStatus,
  pub closed_at: Option<DateTime<Utc>>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct NewTicketMessage {
  pub message: String,
  /// Write this as a staff-only note rather than a reply. Rejected for anyone
  /// who is not staff.
  #[serde(default)]
  pub internal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketMessage {
  pub id: Uuid,
  pub ticket_id: Uuid,
  /// `None` when the message came in by email from someone without an account
  /// (see `author_email`), or when JSR itself generated it (the auto-reply).
  pub author: Option<Uuid>,
  pub author_email: Option<String>,
  pub author_name: Option<String>,
  /// Whether the sending domain passed SPF and DKIM. Only meaningful alongside
  /// `author_email` — a message from a signed-in user needs no such check.
  pub author_email_verified: Option<bool>,
  pub direction: TicketMessageDirection,
  /// RFC 5322 `Message-ID` of the email this message was sent as or received
  /// from, used to thread replies back onto the ticket. `None` for messages
  /// that were never emailed.
  pub email_message_id: Option<String>,
  /// A note staff wrote to each other. Never emailed, and never shown to the
  /// person who opened the ticket.
  pub internal: bool,
  pub message: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketAttachment {
  pub id: Uuid,
  pub message_id: Uuid,
  pub filename: String,
  pub content_type: String,
  pub size_bytes: i32,
  pub storage_key: String,
  pub created_at: DateTime<Utc>,
}

/// A message together with the account that wrote it, if it had one, and the
/// files that arrived with it.
pub type FullTicketMessage =
  (TicketMessage, Option<UserPublic>, Vec<TicketAttachment>);

pub type FullTicket = (Ticket, Option<User>, Vec<FullTicketMessage>);

/// A queued outgoing email, already rendered. Written in the request that causes
/// it and delivered out of band, so a Postmark failure never reaches the user
/// whose action triggered the mail.
#[derive(Debug, Clone)]
pub struct EmailDelivery {
  pub id: Uuid,
  pub to_address: String,
  pub subject: String,
  pub body_text: String,
  pub body_html: String,
  pub message_id: Option<String>,
  pub in_reply_to: Option<String>,
  pub reference_ids: Vec<String>,
  pub attempts: i32,
  pub sent_at: Option<DateTime<Utc>>,
  pub abandoned_at: Option<DateTime<Utc>>,
  pub last_error: Option<String>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewEmailDelivery {
  pub to_address: String,
  pub subject: String,
  pub body_text: String,
  pub body_html: String,
  pub message_id: Option<String>,
  pub in_reply_to: Option<String>,
  pub reference_ids: Vec<String>,
}

/// A file that arrived attached to an inbound email, already uploaded to the
/// attachment bucket. Only the bookkeeping row is left to write.
#[derive(Debug, Clone)]
pub struct NewTicketAttachment {
  pub filename: String,
  pub content_type: String,
  pub size_bytes: i32,
  pub storage_key: String,
}

/// A message that arrived by email rather than through the web UI.
#[derive(Debug, Clone)]
pub struct NewTicketEmailMessage {
  /// Chosen by the caller rather than the database, so that attachment storage
  /// keys can be built before the row exists.
  pub id: Uuid,
  pub author_email: String,
  pub author_name: Option<String>,
  /// Whether the sending domain passed SPF and DKIM.
  pub author_email_verified: bool,
  /// RFC 5322 `Message-ID` of the email this came from. Doubles as the
  /// idempotency key: the column is unique, so a redelivered webhook is
  /// rejected by the database rather than duplicating the message.
  pub email_message_id: String,
  pub message: String,
  pub attachments: Vec<NewTicketAttachment>,
}

/// An outbound message JSR generated itself, with no user behind it. Recorded so
/// that the reply it invites can be threaded back onto the ticket by its
/// `Message-ID`.
#[derive(Debug, Clone)]
pub struct NewTicketSystemMessage {
  pub id: Uuid,
  pub email_message_id: String,
  pub message: String,
}

/// Everything needed to open a ticket from an inbound email, in one transaction.
#[derive(Debug, Clone)]
pub struct NewEmailTicket {
  pub reporter_email: String,
  pub reporter_name: Option<String>,
  pub subject: String,
  /// Secret handed to the reporter in the auto-reply so they can reach the
  /// ticket on the web without an account.
  pub claim_token: Uuid,
  pub message: NewTicketEmailMessage,
  pub auto_reply: NewTicketSystemMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLog {
  pub actor_id: Uuid,
  pub is_sudo: bool,
  pub action: String,
  pub meta: serde_json::Value,
  pub created_at: DateTime<Utc>,
}
