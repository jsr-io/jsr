// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// SQL fragments for use with sqlx_query! / sqlx_query_as! macros.
// These constants are resolved at compile time by the proc macro.
pub const USER_SELECT_FULL: &str = r#"id, name, email, avatar_url, updated_at, created_at, github_id, gitlab_id, is_blocked, is_staff, scope_limit,
(SELECT COUNT(created_at) FROM scope_invites WHERE target_user_id = id) as "invite_count!",
(SELECT COUNT(created_at) FROM scopes WHERE creator = id) as "scope_usage!",
(CASE WHEN users.is_staff THEN (
  SELECT count(tickets.created_at) FROM tickets WHERE closed = false AND EXISTS (
    SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id  = tickets.id AND tm.author = tickets.creator AND tm.created_at = (
      SELECT MAX(ticket_messages.created_at) FROM ticket_messages WHERE ticket_messages.ticket_id = tickets.id
    )
  )
) ELSE (
  SELECT COUNT(created_at) FROM tickets WHERE closed = false AND tickets.creator = users.id AND EXISTS (
    SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id = tickets.id AND tm.author != users.id AND tm.created_at > (
      SELECT MAX(tm2.created_at) FROM ticket_messages as tm2 WHERE tm2.ticket_id = tm.ticket_id AND tm2.author = users.id
    )
  )
) END) as "newer_ticket_messages_count!" "#;

// Runtime-safe variant without sqlx type annotations, for use with sqlx::query_as() / format!().
pub const USER_SELECT_FULL_RT: &str = r#"id, name, email, avatar_url, updated_at, created_at, github_id, gitlab_id, is_blocked, is_staff, scope_limit,
(SELECT COUNT(created_at) FROM scope_invites WHERE target_user_id = id) as "invite_count",
(SELECT COUNT(created_at) FROM scopes WHERE creator = id) as "scope_usage",
(CASE WHEN users.is_staff THEN (
  SELECT count(tickets.created_at) FROM tickets WHERE closed = false AND EXISTS (
    SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id  = tickets.id AND tm.author = tickets.creator AND tm.created_at = (
      SELECT MAX(ticket_messages.created_at) FROM ticket_messages WHERE ticket_messages.ticket_id = tickets.id
    )
  )
) ELSE (
  SELECT COUNT(created_at) FROM tickets WHERE closed = false AND tickets.creator = users.id AND EXISTS (
    SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id = tickets.id AND tm.author != users.id AND tm.created_at > (
      SELECT MAX(tm2.created_at) FROM ticket_messages as tm2 WHERE tm2.ticket_id = tm.ticket_id AND tm2.author = users.id
    )
  )
) END) as "newer_ticket_messages_count" "#;

pub const SCOPE_SELECT: &str = r#"scope as "scope: ScopeName", description as "description: ScopeDescription", creator, package_limit, new_package_per_week_limit, publish_attempts_per_week_limit, verify_oidc_actor, require_publishing_from_ci, updated_at, created_at"#;

pub const PACKAGE_SELECT: &str = r#"scope as "scope: ScopeName", name as "name: PackageName", description, github_repository_id, runtime_compat as "runtime_compat: RuntimeCompat", readme_source as "readme_source: ReadmeSource", when_featured, is_archived, updated_at, created_at"#;

pub const PACKAGE_SELECT_JOINED: &str = r#"packages.scope "package_scope: ScopeName", packages.name "package_name: PackageName", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat "package_runtime_compat: RuntimeCompat", packages.readme_source "package_readme_source: ReadmeSource", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at", packages.created_at "package_created_at",
(SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "package_version_count!",
(SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_latest_version",
(SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta: PackageVersionMeta""#;

// Base package columns without version aggregates (for use with lateral joins in list queries)
pub const PACKAGE_BASE_SELECT_JOINED: &str = r#"packages.scope "package_scope: ScopeName", packages.name "package_name: PackageName", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat "package_runtime_compat: RuntimeCompat", packages.readme_source "package_readme_source: ReadmeSource", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at", packages.created_at "package_created_at""#;

// Version aggregate columns from lateral join aliases (SELECT clause)
pub const PACKAGE_VERSION_AGG_SELECT: &str = r#"COALESCE(pv_count.cnt, 0) as "package_version_count!", pv_latest.version as "package_latest_version?", pv_latest.meta as "package_version_meta?: PackageVersionMeta""#;

// Lateral joins replacing correlated subqueries — combines latest version + meta into a single lookup
pub const PACKAGE_VERSION_LATERAL_JOINS: &str = r#"LEFT JOIN LATERAL (SELECT COUNT(*) as cnt FROM package_versions WHERE scope = packages.scope AND name = packages.name) pv_count ON true LEFT JOIN LATERAL (SELECT version, meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) pv_latest ON true"#;

pub const GITHUB_REPOSITORY_SELECT_JOINED: &str = r#"github_repositories.id "github_repository_id?", github_repositories.owner "github_repository_owner?", github_repositories.name "github_repository_name?", github_repositories.updated_at "github_repository_updated_at?", github_repositories.created_at "github_repository_created_at?""#;

pub const SCOPE_SELECT_JOINED_RT: &str = r#"scopes.scope as "scope_scope", scopes.description as "scope_description", scopes.creator as "scope_creator", scopes.package_limit as "scope_package_limit", scopes.new_package_per_week_limit as "scope_new_package_per_week_limit", scopes.publish_attempts_per_week_limit as "scope_publish_attempts_per_week_limit", scopes.verify_oidc_actor as "scope_verify_oidc_actor", scopes.require_publishing_from_ci as "scope_require_publishing_from_ci", scopes.updated_at as "scope_updated_at", scopes.created_at as "scope_created_at""#;

pub const USER_PUBLIC_SELECT_JOINED_RT: &str = r#"users.id as "user_id", users.name as "user_name", users.avatar_url as "user_avatar_url", users.github_id as "user_github_id", users.gitlab_id as "user_gitlab_id", users.updated_at as "user_updated_at", users.created_at as "user_created_at""#;

pub const SCOPE_USAGE_SELECT_RT: &str = r#"(SELECT COUNT(created_at) FROM packages WHERE packages.scope = scopes.scope) AS "usage_package",
(SELECT COUNT(created_at) FROM packages WHERE packages.scope = scopes.scope AND created_at > now() - '1 week'::interval) AS "usage_new_package_per_week",
(SELECT COUNT(created_at) FROM publishing_tasks WHERE publishing_tasks.package_scope = scopes.scope AND created_at > now() - '1 week'::interval) AS "usage_publish_attempts_per_week""#;

// Runtime-safe variant without sqlx type annotations, for use with sqlx::query() / format!().
pub const PACKAGE_SELECT_JOINED_RT: &str = r#"packages.scope "package_scope", packages.name "package_name", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat as "package_runtime_compat", packages.readme_source "package_readme_source", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at", packages.created_at "package_created_at",
(SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "package_version_count",
(SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_latest_version",
(SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta""#;

pub const GITHUB_REPOSITORY_SELECT_JOINED_RT: &str = r#"github_repositories.id "github_repository_id", github_repositories.owner "github_repository_owner", github_repositories.name "github_repository_name", github_repositories.updated_at "github_repository_updated_at", github_repositories.created_at "github_repository_created_at""#;

// Runtime lateral join variants
pub const PACKAGE_BASE_SELECT_JOINED_RT: &str = r#"packages.scope "package_scope", packages.name "package_name", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat as "package_runtime_compat", packages.readme_source "package_readme_source", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at", packages.created_at "package_created_at""#;

pub const PACKAGE_VERSION_AGG_SELECT_RT: &str = r#"COALESCE(pv_count.cnt, 0) as "package_version_count", pv_latest.version as "package_latest_version", pv_latest.meta as "package_version_meta""#;

pub const PACKAGE_VERSION_LATERAL_JOINS_RT: &str = r#"LEFT JOIN LATERAL (SELECT COUNT(*) as cnt FROM package_versions WHERE scope = packages.scope AND name = packages.name) pv_count ON true LEFT JOIN LATERAL (SELECT version, meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) pv_latest ON true"#;

pub const PACKAGE_VERSION_SELECT: &str = r#"scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", user_id, readme_path as "readme_path: PackagePath", exports as "exports: ExportsMap", is_yanked, uses_npm, meta as "meta: PackageVersionMeta", updated_at, created_at, rekor_log_id, license"#;

pub const NEWER_VERSIONS_COUNT_SUBQUERY: &str = r#"(SELECT COUNT(*)
        FROM package_versions AS pv
        WHERE pv.scope = package_versions.scope
        AND pv.name = package_versions.name
        AND pv.version > package_versions.version
        AND pv.version NOT LIKE '%-%'
        AND pv.is_yanked = false) as "newer_versions_count!""#;

pub const PACKAGE_VERSION_SELECT_JOINED: &str = r#"package_versions.scope as "package_version_scope: ScopeName", package_versions.name as "package_version_name: PackageName", package_versions.version as "package_version_version: Version", package_versions.user_id as "package_version_user_id", package_versions.readme_path as "package_version_readme_path: PackagePath", package_versions.exports as "package_version_exports: ExportsMap", package_versions.is_yanked as "package_version_is_yanked", package_versions.uses_npm as "package_version_uses_npm", package_versions.meta as "package_version_meta: PackageVersionMeta", package_versions.updated_at as "package_version_updated_at", package_versions.created_at as "package_version_created_at", package_versions.rekor_log_id as "package_version_rekor_log_id", package_versions.license as "package_version_license""#;

pub const USER_PUBLIC_SELECT_JOINED: &str = r#"users.id as "user_id?", users.name as "user_name?", users.avatar_url as "user_avatar_url?", users.github_id as "user_github_id", users.gitlab_id as "user_gitlab_id", users.updated_at as "user_updated_at?", users.created_at as "user_created_at?""#;

pub const SCOPE_MEMBER_SELECT: &str =
  r#"scope as "scope: ScopeName", user_id, is_admin, updated_at, created_at"#;

pub const SCOPE_INVITE_SELECT: &str = r#"scope as "scope: ScopeName", target_user_id, requesting_user_id, updated_at, created_at"#;

pub const TOKEN_SELECT: &str = r#"id, hash, user_id, type "type: _", description, expires_at, permissions "permissions: _", updated_at, created_at"#;

pub const PUBLISHING_TASK_SELECT: &str = r#"id, status as "status: PublishingTaskStatus", error as "error: PublishingTaskError", user_id, package_scope as "package_scope: ScopeName", package_name as "package_name: PackageName", package_version as "package_version: Version", config_file as "config_file: PackagePath", created_at, updated_at"#;

pub const OAUTH_STATE_SELECT: &str =
  "csrf_token, pkce_code_verifier, redirect_url, updated_at, created_at";

pub const AUTHORIZATION_SELECT: &str = r#"exchange_token, code, challenge, permissions "permissions: _", approved, user_id, expires_at, created_at, updated_at"#;

pub const GITHUB_IDENTITY_SELECT: &str = "github_id, access_token, access_token_expires_at, refresh_token, refresh_token_expires_at, updated_at, created_at";

pub const PACKAGE_FILE_SELECT: &str = r#"scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", path as "path: PackagePath", size, checksum, updated_at, created_at"#;

pub const NPM_TARBALL_SELECT: &str = r#"scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", revision, sha1, sha512, size, updated_at, created_at"#;

pub const PACKAGE_VERSION_DEPENDENCY_SELECT: &str = r#"package_scope as "package_scope: ScopeName", package_name as "package_name: PackageName", package_version as "package_version: Version", dependency_kind as "dependency_kind: DependencyKind", dependency_name, dependency_constraint, dependency_path, updated_at, created_at"#;

pub const PUBLISHING_TASK_SELECT_JOINED: &str = r#"publishing_tasks.id as "task_id", publishing_tasks.status as "task_status: PublishingTaskStatus", publishing_tasks.error as "task_error: PublishingTaskError", publishing_tasks.user_id as "task_user_id", publishing_tasks.package_scope as "task_package_scope: ScopeName", publishing_tasks.package_name as "task_package_name: PackageName", publishing_tasks.package_version as "task_package_version: Version", publishing_tasks.config_file as "task_config_file: PackagePath", publishing_tasks.created_at as "task_created_at", publishing_tasks.updated_at as "task_updated_at""#;

pub const PUBLISHING_TASK_SELECT_JOINED_RT: &str = r#"publishing_tasks.id as "task_id", publishing_tasks.status as "task_status", publishing_tasks.error as "task_error", publishing_tasks.user_id as "task_user_id", publishing_tasks.package_scope as "task_package_scope", publishing_tasks.package_name as "task_package_name", publishing_tasks.package_version as "task_package_version", publishing_tasks.config_file as "task_config_file", publishing_tasks.created_at as "task_created_at", publishing_tasks.updated_at as "task_updated_at""#;

pub const USER_PUBLIC_SELECT_JOINED_OPTIONAL: &str = r#"users.id as "user_id?", users.name as "user_name?", users.avatar_url as "user_avatar_url?", users.github_id as "user_github_id?", users.gitlab_id as "user_gitlab_id?", users.updated_at as "user_updated_at?", users.created_at as "user_created_at?""#;

pub const SCOPE_INVITE_SELECT_JOINED: &str = r#"scope_invites.scope as "scope_invite_scope: ScopeName", scope_invites.target_user_id as "scope_invite_target_user_id", scope_invites.requesting_user_id as "scope_invite_requesting_user_id", scope_invites.updated_at as "scope_invite_updated_at", scope_invites.created_at as "scope_invite_created_at",
        target_user.id as "target_user_id", target_user.name as "target_user_name", target_user.avatar_url as "target_user_avatar_url", target_user.github_id as "target_user_github_id", target_user.gitlab_id as "target_user_gitlab_id", target_user.updated_at as "target_user_updated_at", target_user.created_at as "target_user_created_at",
        requesting_user.id as "requesting_user_id", requesting_user.name as "requesting_user_name", requesting_user.avatar_url as "requesting_user_avatar_url", requesting_user.github_id as "requesting_user_github_id", requesting_user.gitlab_id as "requesting_user_gitlab_id", requesting_user.updated_at as "requesting_user_updated_at", requesting_user.created_at as "requesting_user_created_at""#;

pub const SCOPE_MEMBER_SELECT_JOINED: &str = r#"scope_members.scope as "scope_member_scope: ScopeName", scope_members.user_id as "scope_member_user_id", scope_members.is_admin as "scope_member_is_admin", scope_members.updated_at as "scope_member_updated_at", scope_members.created_at as "scope_member_created_at""#;

pub const TICKET_SELECT_JOINED: &str = r#"tickets.id as "ticket_id", tickets.kind as "ticket_kind: TicketKind", tickets.creator as "ticket_creator", tickets.meta as "ticket_meta", tickets.closed as "ticket_closed", tickets.updated_at as "ticket_updated_at", tickets.created_at as "ticket_created_at""#;

pub const TICKET_SELECT_JOINED_RT: &str = r#"tickets.id as "ticket_id", tickets.kind as "ticket_kind", tickets.creator as "ticket_creator", tickets.meta as "ticket_meta", tickets.closed as "ticket_closed", tickets.updated_at as "ticket_updated_at", tickets.created_at as "ticket_created_at""#;

pub const USER_SELECT_FULL_JOINED: &str = r#"users.id as "user_id", users.name as "user_name", users.email as "user_email", users.avatar_url as "user_avatar_url", users.github_id as "user_github_id", users.gitlab_id as "user_gitlab_id", users.is_blocked as "user_is_blocked", users.is_staff as "user_is_staff", users.scope_limit as "user_scope_limit", users.updated_at as "user_updated_at", users.created_at as "user_created_at",
(SELECT COUNT(scope_invites.created_at) FROM scope_invites WHERE scope_invites.target_user_id = users.id) as "user_invite_count!",
(SELECT COUNT(scopes.created_at) FROM scopes WHERE scopes.creator = users.id) as "user_scope_usage!",
(CASE WHEN users.is_staff THEN (
  SELECT count(tickets.created_at) FROM tickets WHERE closed = false AND EXISTS (
    SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id  = tickets.id AND tm.author = tickets.creator AND tm.created_at = (
      SELECT MAX(ticket_messages.created_at) FROM ticket_messages WHERE ticket_messages.ticket_id = tickets.id
    )
  )
) ELSE (
  SELECT COUNT(created_at) FROM tickets WHERE closed = false AND tickets.creator = users.id AND EXISTS (
    SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id = tickets.id AND tm.author != users.id AND tm.created_at > (
      SELECT MAX(tm2.created_at) FROM ticket_messages as tm2 WHERE tm2.ticket_id = tm.ticket_id AND tm2.author = users.id
    )
  )
) END) as "user_newer_ticket_messages_count!""#;

pub const USER_SELECT_FULL_JOINED_RT: &str = r#"users.id as "user_id", users.name as "user_name", users.email as "user_email", users.avatar_url as "user_avatar_url", users.github_id as "user_github_id", users.gitlab_id as "user_gitlab_id", users.is_blocked as "user_is_blocked", users.is_staff as "user_is_staff", users.scope_limit as "user_scope_limit", users.updated_at as "user_updated_at", users.created_at as "user_created_at",
(SELECT COUNT(scope_invites.created_at) FROM scope_invites WHERE scope_invites.target_user_id = users.id) as "user_invite_count",
(SELECT COUNT(scopes.created_at) FROM scopes WHERE scopes.creator = users.id) as "user_scope_usage",
(CASE WHEN users.is_staff THEN (
  SELECT count(tickets.created_at) FROM tickets WHERE closed = false AND EXISTS (
    SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id  = tickets.id AND tm.author = tickets.creator AND tm.created_at = (
      SELECT MAX(ticket_messages.created_at) FROM ticket_messages WHERE ticket_messages.ticket_id = tickets.id
    )
  )
) ELSE (
  SELECT COUNT(created_at) FROM tickets WHERE closed = false AND tickets.creator = users.id AND EXISTS (
    SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id = tickets.id AND tm.author != users.id AND tm.created_at > (
      SELECT MAX(tm2.created_at) FROM ticket_messages as tm2 WHERE tm2.ticket_id = tm.ticket_id AND tm2.author = users.id
    )
  )
) END) as "user_newer_ticket_messages_count""#;

pub const TICKET_MESSAGE_SELECT_JOINED: &str = r#"ticket_messages.ticket_id as "message_ticket_id", ticket_messages.author as "message_author", ticket_messages.message as "message_message", ticket_messages.updated_at as "message_updated_at", ticket_messages.created_at as "message_created_at""#;

pub const AUDIT_LOG_SELECT_JOINED: &str = r#"audit_logs.actor_id as "audit_log_actor_id", audit_logs.is_sudo as "audit_log_is_sudo", audit_logs.action as "audit_log_action", audit_logs.meta as "audit_log_meta", audit_logs.created_at as "audit_log_created_at""#;
