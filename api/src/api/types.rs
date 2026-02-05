// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;

use crate::db::*;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeDescription;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::provenance::ProvenanceBundle;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ApiPublishingTaskStatus {
  Pending,
  Processing,
  Processed,
  Success,
  Failure,
}

impl From<PublishingTaskStatus> for ApiPublishingTaskStatus {
  fn from(value: PublishingTaskStatus) -> Self {
    match value {
      PublishingTaskStatus::Pending => ApiPublishingTaskStatus::Pending,
      PublishingTaskStatus::Processing => ApiPublishingTaskStatus::Processing,
      PublishingTaskStatus::Processed => ApiPublishingTaskStatus::Processed,
      PublishingTaskStatus::Success => ApiPublishingTaskStatus::Success,
      PublishingTaskStatus::Failure => ApiPublishingTaskStatus::Failure,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApiPublishingTaskError {
  pub code: String,
  pub message: String,
}

impl From<PublishingTaskError> for ApiPublishingTaskError {
  fn from(value: PublishingTaskError) -> Self {
    Self {
      code: value.code,
      message: value.message,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApiPublishingTask {
  pub id: Uuid,
  pub status: ApiPublishingTaskStatus,
  pub error: Option<ApiPublishingTaskError>,
  pub user: Option<ApiUser>,
  pub package_scope: ScopeName,
  pub package_name: PackageName,
  pub package_version: Version,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

impl From<(PublishingTask, Option<UserPublic>)> for ApiPublishingTask {
  fn from((value, user): (PublishingTask, Option<UserPublic>)) -> Self {
    Self {
      id: value.id,
      status: value.status.into(),
      error: value.error.map(Into::into),
      user: user.map(Into::into),
      package_scope: value.package_scope,
      package_name: value.package_name,
      package_version: value.package_version,
      created_at: value.created_at,
      updated_at: value.updated_at,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiDependencyGraphItem {
  pub id: usize,
  pub dependency: super::package::DependencyKind,
  pub children: indexmap::IndexSet<usize>,
  pub size: Option<u64>,
  pub media_type: Option<String>,
}

impl
  From<(
    super::package::DependencyKind,
    super::package::DependencyInfo,
  )> for ApiDependencyGraphItem
{
  fn from(
    (kind, info): (
      super::package::DependencyKind,
      super::package::DependencyInfo,
    ),
  ) -> Self {
    Self {
      id: info.id,
      dependency: kind,
      children: info.children,
      size: info.size,
      media_type: info.media_type.map(|media_type| media_type.to_string()),
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApiUser {
  pub id: Uuid,
  pub name: String,
  pub github_id: Option<i64>,
  pub avatar_url: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<User> for ApiUser {
  fn from(user: User) -> Self {
    Self {
      id: user.id,
      name: user.name,
      github_id: user.github_id,
      avatar_url: user.avatar_url,
      updated_at: user.updated_at,
      created_at: user.created_at,
    }
  }
}

impl From<UserPublic> for ApiUser {
  fn from(user: UserPublic) -> Self {
    Self {
      id: user.id,
      name: user.name,
      github_id: user.github_id,
      avatar_url: user.avatar_url,
      updated_at: user.updated_at,
      created_at: user.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiFullUser {
  pub id: Uuid,
  pub name: String,
  pub email: Option<String>,
  pub avatar_url: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub github_id: Option<i64>,
  pub is_blocked: bool,
  pub is_staff: bool,
  pub scope_usage: i32,
  pub scope_limit: i32,
  pub invite_count: u64,
  pub newer_ticket_messages_count: u64,
}

impl From<User> for ApiFullUser {
  fn from(user: User) -> Self {
    Self {
      id: user.id,
      name: user.name,
      email: user.email,
      avatar_url: user.avatar_url,
      updated_at: user.updated_at,
      created_at: user.created_at,
      github_id: user.github_id,
      is_blocked: user.is_blocked,
      is_staff: user.is_staff,
      scope_usage: user.scope_usage as i32,
      scope_limit: user.scope_limit,
      invite_count: user.invite_count as u64,
      newer_ticket_messages_count: user.newer_ticket_messages_count as u64,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiScope {
  pub scope: ScopeName,
  pub description: ScopeDescription,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<Scope> for ApiScope {
  fn from(scope: Scope) -> Self {
    Self {
      scope: scope.scope,
      description: scope.description,
      updated_at: scope.updated_at,
      created_at: scope.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiScopeQuotas {
  pub package_usage: i32,
  pub package_limit: i32,
  pub new_package_per_week_usage: i32,
  pub new_package_per_week_limit: i32,
  pub publish_attempts_per_week_usage: i32,
  pub publish_attempts_per_week_limit: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiFullScope {
  pub scope: ScopeName,
  pub description: ScopeDescription,
  pub creator: ApiUser,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub quotas: ApiScopeQuotas,
  pub gh_actions_verify_actor: bool,
  #[serde(rename = "requirePublishingFromCI")]
  pub require_publishing_from_ci: bool,
}

impl From<(Scope, ScopeUsage, UserPublic)> for ApiFullScope {
  fn from((scope, scope_usage, user): (Scope, ScopeUsage, UserPublic)) -> Self {
    assert_eq!(scope.creator, user.id);
    Self {
      scope: scope.scope,
      description: scope.description,
      creator: user.into(),
      updated_at: scope.updated_at,
      created_at: scope.created_at,
      quotas: ApiScopeQuotas {
        package_usage: scope_usage.package,
        package_limit: scope.package_limit,
        new_package_per_week_usage: scope_usage.new_package_per_week,
        new_package_per_week_limit: scope.new_package_per_week_limit,
        publish_attempts_per_week_usage: scope_usage.publish_attempts_per_week,
        publish_attempts_per_week_limit: scope.publish_attempts_per_week_limit,
      },
      gh_actions_verify_actor: scope.verify_oidc_actor,
      require_publishing_from_ci: scope.require_publishing_from_ci,
    }
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ApiScopeOrFullScope {
  Partial(ApiScope),
  Full(ApiFullScope),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCreateScopeRequest {
  pub scope: ScopeName,
  pub description: ScopeDescription,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiScopeMember {
  pub scope: ScopeName,
  pub user: ApiUser,
  pub is_admin: bool,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<(ScopeMember, UserPublic)> for ApiScopeMember {
  fn from((scope_member, user): (ScopeMember, UserPublic)) -> Self {
    assert_eq!(scope_member.user_id, user.id);
    Self {
      scope: scope_member.scope,
      user: user.into(),
      is_admin: scope_member.is_admin,
      updated_at: scope_member.updated_at,
      created_at: scope_member.created_at,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiAddScopeMemberRequest {
  GithubLogin(String),
  Id(Uuid),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiUpdateScopeMemberRequest {
  pub is_admin: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiScopeInvite {
  pub scope: ScopeName,
  pub target_user: ApiUser,
  pub requesting_user: ApiUser,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<(ScopeInvite, UserPublic, UserPublic)> for ApiScopeInvite {
  fn from(
    (scope_invite, target_user, requesting_user): (
      ScopeInvite,
      UserPublic,
      UserPublic,
    ),
  ) -> Self {
    assert_eq!(scope_invite.target_user_id, target_user.id);
    assert_eq!(scope_invite.requesting_user_id, requesting_user.id);
    Self {
      scope: scope_invite.scope,
      target_user: target_user.into(),
      requesting_user: requesting_user.into(),
      updated_at: scope_invite.updated_at,
      created_at: scope_invite.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPackageScore {
  pub has_readme: bool,
  pub has_readme_examples: bool,
  pub all_entrypoints_docs: bool,
  pub percentage_documented_symbols: f32,
  pub all_fast_check: bool,
  pub has_provenance: bool,

  // package wide
  pub has_description: bool,
  pub at_least_one_runtime_compatible: bool,
  pub multiple_runtimes_compatible: bool,

  pub total: u32,
}

impl ApiPackageScore {
  pub const MAX_SCORE: u32 = 17;

  pub fn score_percentage(&self) -> u32 {
    u32::min((self.total * 100) / Self::MAX_SCORE, 100)
  }
}

impl From<(&PackageVersionMeta, &Package)> for ApiPackageScore {
  fn from((meta, package): (&PackageVersionMeta, &Package)) -> Self {
    let mut score = 0;

    if meta.has_readme {
      score += 2;
    }

    if meta.has_readme_examples {
      score += 1;
    }

    if meta.all_entrypoints_docs {
      score += 1;
    }

    if meta.has_provenance {
      score += 1;
    }

    // You only need to document 80% of your symbols to get all the points.
    score += ((meta.percentage_documented_symbols / 0.8).min(1.0) * 5.0).floor()
      as u32;

    if meta.all_fast_check {
      score += 5;
    }

    // package wide

    if !package.description.is_empty() {
      score += 1;
    }

    let mut compatible_runtimes_count = 0;
    if package.runtime_compat.deno.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }
    if package.runtime_compat.bun.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }
    if package.runtime_compat.node.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }
    if package.runtime_compat.browser.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }
    if package.runtime_compat.workerd.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }

    if compatible_runtimes_count >= 1 {
      score += 1;
    }

    if compatible_runtimes_count >= 2 {
      score += 1;
    }

    Self {
      has_readme: meta.has_readme,
      has_readme_examples: meta.has_readme_examples,
      all_entrypoints_docs: meta.all_entrypoints_docs,
      percentage_documented_symbols: meta.percentage_documented_symbols,
      all_fast_check: meta.all_fast_check,
      has_provenance: meta.has_provenance,
      has_description: !package.description.is_empty(),
      at_least_one_runtime_compatible: compatible_runtimes_count >= 1,
      multiple_runtimes_compatible: compatible_runtimes_count >= 2,
      total: score,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPackage {
  pub scope: ScopeName,
  pub name: PackageName,
  pub description: String,
  pub github_repository: Option<ApiGithubRepository>,
  pub runtime_compat: ApiRuntimeCompat,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub version_count: u64,
  pub dependency_count: u64,
  pub dependent_count: u64,
  pub score: Option<u32>,
  pub latest_version: Option<String>,
  pub when_featured: Option<DateTime<Utc>>,
  pub is_archived: bool,
  pub readme_source: ApiReadmeSource,
}

impl From<PackageWithGitHubRepoAndMeta> for ApiPackage {
  fn from((package, repo, meta): PackageWithGitHubRepoAndMeta) -> Self {
    assert_eq!(package.github_repository_id, repo.as_ref().map(|r| r.id));

    let score = ApiPackageScore::from((&meta, &package));

    Self {
      scope: package.scope,
      name: package.name,
      description: package.description,
      github_repository: repo.map(ApiGithubRepository::from),
      runtime_compat: package.runtime_compat.into(),
      updated_at: package.updated_at,
      created_at: package.created_at,
      version_count: package.version_count as u64,
      dependency_count: 0,
      dependent_count: 0,
      score: package
        .latest_version
        .as_ref()
        .map(|_| score.score_percentage()),
      latest_version: package.latest_version,
      when_featured: package.when_featured,
      is_archived: package.is_archived,
      readme_source: package.readme_source.into(),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCreatePackageRequest {
  pub package: PackageName,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiUpdatePackageRequest {
  Description(String),
  GithubRepository(Option<ApiUpdatePackageGithubRepositoryRequest>),
  RuntimeCompat(ApiRuntimeCompat),
  ReadmeSource(ApiReadmeSource),
  IsFeatured(bool),
  IsArchived(bool),
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApiReadmeSource {
  Readme,
  JSDoc,
}

impl From<ApiReadmeSource> for ReadmeSource {
  fn from(value: ApiReadmeSource) -> Self {
    match value {
      ApiReadmeSource::Readme => ReadmeSource::Readme,
      ApiReadmeSource::JSDoc => ReadmeSource::JSDoc,
    }
  }
}

impl From<ReadmeSource> for ApiReadmeSource {
  fn from(value: ReadmeSource) -> Self {
    match value {
      ReadmeSource::Readme => ApiReadmeSource::Readme,
      ReadmeSource::JSDoc => ApiReadmeSource::JSDoc,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiUpdatePackageGithubRepositoryRequest {
  pub owner: String,
  pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiProvenanceStatementRequest {
  pub bundle: ProvenanceBundle,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiUpdatePackageVersionRequest {
  pub yanked: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGithubRepository {
  pub id: i64,
  pub owner: String,
  pub name: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<GithubRepository> for ApiGithubRepository {
  fn from(repo: GithubRepository) -> Self {
    Self {
      id: repo.id,
      owner: repo.owner,
      name: repo.name,
      updated_at: repo.updated_at,
      created_at: repo.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRuntimeCompat {
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

impl From<RuntimeCompat> for ApiRuntimeCompat {
  fn from(value: RuntimeCompat) -> Self {
    Self {
      browser: value.browser,
      deno: value.deno,
      node: value.node,
      workerd: value.workerd,
      bun: value.bun,
    }
  }
}

impl From<ApiRuntimeCompat> for RuntimeCompat {
  fn from(value: ApiRuntimeCompat) -> Self {
    Self {
      browser: value.browser,
      deno: value.deno,
      node: value.node,
      workerd: value.workerd,
      bun: value.bun,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPackageVersion {
  pub scope: ScopeName,
  pub package: PackageName,
  pub version: Version,
  pub yanked: bool,
  pub uses_npm: bool,
  pub newer_versions_count: u64,
  pub lifetime_download_count: u64,
  pub rekor_log_id: Option<String>,
  pub license: Option<String>,
  pub readme_path: Option<PackagePath>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[allow(clippy::large_enum_variant)]
pub enum ApiPackageVersionDocs {
  #[serde(rename_all = "camelCase")]
  Content {
    version: ApiPackageVersion,
    css: Cow<'static, str>,
    comrak_css: Cow<'static, str>,
    script: Cow<'static, str>,
    breadcrumbs: Option<String>,
    toc: Option<String>,
    main: String,
  },
  Redirect {
    symbol: String,
  },
}

impl From<PackageVersion> for ApiPackageVersion {
  fn from(value: PackageVersion) -> Self {
    ApiPackageVersion {
      scope: value.scope,
      package: value.name,
      version: value.version,
      yanked: value.is_yanked,
      uses_npm: value.uses_npm,
      newer_versions_count: value.newer_versions_count as u64,
      lifetime_download_count: value.lifetime_download_count as u64,
      rekor_log_id: value.rekor_log_id,
      license: value.license,
      readme_path: value.readme_path,
      updated_at: value.updated_at,
      created_at: value.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApiSourceDirEntryKind {
  Dir,
  File,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiSourceDirEntry {
  pub name: String,
  pub size: usize,
  pub kind: ApiSourceDirEntryKind,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ApiSource {
  Dir { entries: Vec<ApiSourceDirEntry> },
  File { size: usize, view: Option<String> },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPackageVersionSource {
  pub version: ApiPackageVersion,
  pub css: Cow<'static, str>,
  pub comrak_css: Cow<'static, str>,
  pub script: Cow<'static, str>,
  pub source: ApiSource,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPackageVersionWithUser {
  pub scope: ScopeName,
  pub package: PackageName,
  pub version: Version,
  pub user: Option<ApiUser>,
  pub yanked: bool,
  pub uses_npm: bool,
  pub newer_versions_count: i64,
  pub lifetime_download_count: i64,
  pub rekor_log_id: Option<String>,
  pub readme_path: Option<PackagePath>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<(PackageVersion, Option<UserPublic>)> for ApiPackageVersionWithUser {
  fn from(
    (package_version, user): (PackageVersion, Option<UserPublic>),
  ) -> Self {
    assert_eq!(
      package_version.user_id.as_ref(),
      user.as_ref().map(|user| &user.id)
    );
    ApiPackageVersionWithUser {
      scope: package_version.scope,
      package: package_version.name,
      version: package_version.version,
      user: user.map(|user| user.into()),
      yanked: package_version.is_yanked,
      uses_npm: package_version.uses_npm,
      newer_versions_count: package_version.newer_versions_count,
      lifetime_download_count: package_version.lifetime_download_count,
      rekor_log_id: package_version.rekor_log_id,
      readme_path: package_version.readme_path,
      updated_at: package_version.updated_at,
      created_at: package_version.created_at,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAdminUpdateUserRequest {
  pub is_staff: Option<bool>,
  pub is_blocked: Option<bool>,
  pub scope_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAdminUpdateScopeRequest {
  pub package_limit: Option<i32>,
  pub new_package_per_week_limit: Option<i32>,
  pub publish_attempts_per_week_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiUpdateScopeRequest {
  #[serde(rename = "ghActionsVerifyActor")]
  GhActionsVerifyActor(bool),
  #[serde(rename = "requirePublishingFromCI")]
  RequirePublishingFromCI(bool),
  #[serde(rename = "description")]
  Description(Option<String>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStats {
  pub newest: Vec<ApiPackage>,
  pub updated: Vec<ApiPackageVersion>,
  pub featured: Vec<ApiPackage>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiMetrics {
  pub packages: usize,
  pub packages_1d: usize,
  pub packages_7d: usize,
  pub packages_30d: usize,

  pub users: usize,
  pub users_1d: usize,
  pub users_7d: usize,
  pub users_30d: usize,

  pub package_versions: usize,
  pub package_versions_1d: usize,
  pub package_versions_7d: usize,
  pub package_versions_30d: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCreateAuthorizationResponse {
  pub verification_url: String,
  pub code: String,
  pub exchange_token: String,
  pub poll_interval: i64,
  pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCreateAuthorizationRequest {
  pub permissions: Option<Permissions>,
  pub challenge: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAuthorizationExchangeResponse {
  pub token: String,
  pub user: ApiUser,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAuthorizationExchangeRequest {
  pub exchange_token: String,
  pub verifier: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiAuthorization {
  pub code: String,
  pub permissions: Option<Permissions>,
  pub expires_at: DateTime<Utc>,
}

impl From<Authorization> for ApiAuthorization {
  fn from(value: Authorization) -> Self {
    Self {
      code: value.code,
      permissions: value.permissions,
      expires_at: value.expires_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ApiDependencyKind {
  Jsr,
  Npm,
}

impl From<DependencyKind> for ApiDependencyKind {
  fn from(value: DependencyKind) -> Self {
    match value {
      DependencyKind::Jsr => ApiDependencyKind::Jsr,
      DependencyKind::Npm => ApiDependencyKind::Npm,
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ApiDependency {
  pub kind: ApiDependencyKind,
  pub name: String,
  pub constraint: String,
  pub path: String,
}

impl From<PackageVersionDependency> for ApiDependency {
  fn from(dep: PackageVersionDependency) -> Self {
    Self {
      kind: dep.dependency_kind.into(),
      name: dep.dependency_name,
      constraint: dep.dependency_constraint,
      path: dep.dependency_path,
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ApiDependent {
  pub scope: ScopeName,
  pub package: PackageName,
  pub versions: Vec<Version>,
  pub total_versions: usize,
}

impl From<Dependent> for ApiDependent {
  fn from(value: Dependent) -> Self {
    Self {
      scope: value.scope,
      package: value.name,
      versions: value.versions,
      total_versions: value.total_versions as usize,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiDownloadDataPoint {
  pub time_bucket: DateTime<Utc>,
  pub kind: ApiDownloadKind,
  pub count: u64,
}

impl From<DownloadDataPoint> for ApiDownloadDataPoint {
  fn from(value: DownloadDataPoint) -> Self {
    Self {
      time_bucket: value.time_bucket,
      kind: value.kind.into(),
      count: value.count as u64,
    }
  }
}

impl From<VersionDownloadDataPoint> for ApiDownloadDataPoint {
  fn from(value: VersionDownloadDataPoint) -> Self {
    Self {
      time_bucket: value.time_bucket,
      kind: value.kind.into(),
      count: value.count as u64,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiDownloadKind {
  JsrMeta,
  NpmTarball,
}

impl From<DownloadKind> for ApiDownloadKind {
  fn from(value: DownloadKind) -> Self {
    match value {
      DownloadKind::JsrMeta => ApiDownloadKind::JsrMeta,
      DownloadKind::NpmTgz => ApiDownloadKind::NpmTarball,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiList<T> {
  pub items: Vec<T>,
  pub total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiTokenType {
  Web,
  Device,
  Personal,
}

impl From<TokenType> for ApiTokenType {
  fn from(value: TokenType) -> Self {
    match value {
      TokenType::Web => ApiTokenType::Web,
      TokenType::Device => ApiTokenType::Device,
      TokenType::Personal => ApiTokenType::Personal,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiToken {
  pub id: Uuid,
  pub description: Option<String>,
  pub user_id: Uuid,
  pub r#type: ApiTokenType,
  pub expires_at: Option<DateTime<Utc>>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
  pub permissions: Option<Permissions>,
}

impl From<Token> for ApiToken {
  fn from(value: Token) -> Self {
    Self {
      id: value.id,
      description: value.description,
      user_id: value.user_id,
      r#type: value.r#type.into(),
      expires_at: value.expires_at,
      updated_at: value.updated_at,
      created_at: value.created_at,
      permissions: value.permissions,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCreateTokenRequest {
  pub description: String,
  pub expires_at: Option<DateTime<Utc>>,
  pub permissions: Option<Permissions>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCreatedToken {
  pub secret: String,
  pub token: ApiToken,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAssignScopeRequest {
  pub scope: ScopeName,
  pub user_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPackageDownloads {
  pub total: Vec<ApiDownloadDataPoint>,
  pub recent_versions: Vec<ApiPackageDownloadsRecentVersion>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPackageDownloadsRecentVersion {
  pub version: Version,
  pub downloads: Vec<ApiDownloadDataPoint>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ApiTicketMessageOrAuditLog {
  Message {
    message: TicketMessage,
    user: UserPublic,
  },
  #[serde(rename_all = "camelCase")]
  AuditLog {
    audit_log: AuditLog,
    user: UserPublic,
  },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTicketOverview {
  pub id: Uuid,
  pub kind: TicketKind,
  pub creator: ApiUser,
  pub meta: serde_json::Value,
  pub closed: bool,
  pub events: Vec<ApiTicketMessageOrAuditLog>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<(Ticket, User, Vec<ApiTicketMessageOrAuditLog>)>
  for ApiTicketOverview
{
  fn from(
    (value, user, events): (Ticket, User, Vec<ApiTicketMessageOrAuditLog>),
  ) -> Self {
    Self {
      id: value.id,
      kind: value.kind,
      creator: user.into(),
      meta: value.meta,
      closed: value.closed,
      events,
      updated_at: value.updated_at,
      created_at: value.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTicket {
  pub id: Uuid,
  pub kind: TicketKind,
  pub creator: ApiUser,
  pub meta: serde_json::Value,
  pub closed: bool,
  pub messages: Vec<ApiTicketMessage>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<(Ticket, User, Vec<(TicketMessage, UserPublic)>)> for ApiTicket {
  fn from(
    (value, user, messages): (Ticket, User, Vec<(TicketMessage, UserPublic)>),
  ) -> Self {
    Self {
      id: value.id,
      kind: value.kind,
      creator: user.into(),
      meta: value.meta,
      closed: value.closed,
      messages: messages.into_iter().map(|message| message.into()).collect(),
      updated_at: value.updated_at,
      created_at: value.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTicketMessage {
  pub author: ApiUser,
  pub message: String,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<(TicketMessage, UserPublic)> for ApiTicketMessage {
  fn from((value, user): (TicketMessage, UserPublic)) -> Self {
    Self {
      author: user.into(),
      message: value.message,
      updated_at: value.updated_at,
      created_at: value.created_at,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAdminUpdateTicketRequest {
  pub closed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAuditLog {
  pub actor: ApiUser,
  pub action: String,
  pub is_sudo: bool,
  pub meta: serde_json::Value,
  pub created_at: DateTime<Utc>,
}

impl From<(AuditLog, UserPublic)> for ApiAuditLog {
  fn from((value, user): (AuditLog, UserPublic)) -> Self {
    assert_eq!(value.actor_id, user.id);
    Self {
      actor: user.into(),
      action: value.action,
      is_sudo: value.is_sudo,
      meta: value.meta,
      created_at: value.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCreateWebhookEndpointRequest {
  pub url: String,
  pub description: String,
  pub secret: Option<String>,
  pub events: Vec<WebhookEventKind>,
  pub payload_format: WebhookPayloadFormat,
  pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiUpdateWebhookEndpointRequest {
  pub url: Option<String>,
  pub description: Option<String>,
  pub secret: Option<String>, // TODO: it already is an option, how to distinguish between clearing and not changing it?
  pub events: Option<Vec<WebhookEventKind>>,
  pub payload_format: Option<WebhookPayloadFormat>,
  pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiWebhookEndpoint {
  pub id: Uuid,
  pub scope: ScopeName,
  pub package: Option<PackageName>,
  pub url: String,
  pub description: String,
  pub has_secret: bool,
  pub events: Vec<WebhookEventKind>,
  pub payload_format: WebhookPayloadFormat,
  pub is_active: bool,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<WebhookEndpoint> for ApiWebhookEndpoint {
  fn from(value: WebhookEndpoint) -> Self {
    Self {
      id: value.id,
      scope: value.scope,
      package: value.package,
      url: value.url,
      description: value.description,
      has_secret: value.secret.is_some(),
      events: value.events,
      payload_format: value.payload_format,
      is_active: value.is_active,
      updated_at: value.updated_at,
      created_at: value.created_at,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiWebhookDelivery {
  pub id: Uuid,
  pub status: WebhookDeliveryStatus,
  pub event: WebhookEventKind,
  pub request_headers: Option<serde_json::Value>,
  pub request_body: Option<serde_json::Value>,
  pub response_http_code: Option<i32>,
  pub response_headers: Option<serde_json::Value>,
  pub response_body: Option<String>,
  pub error: Option<String>,
  pub updated_at: DateTime<Utc>,
  pub created_at: DateTime<Utc>,
}

impl From<(WebhookDelivery, WebhookEvent)> for ApiWebhookDelivery {
  fn from((delivery, event): (WebhookDelivery, WebhookEvent)) -> Self {
    Self {
      id: delivery.id,
      status: delivery.status,
      event: event.event,
      request_headers: delivery.request_headers,
      request_body: delivery.request_body,
      response_http_code: delivery.response_http_code,
      response_headers: delivery.response_headers,
      response_body: delivery.response_body,
      error: delivery.error,
      updated_at: delivery.updated_at,
      created_at: delivery.created_at,
    }
  }
}
