// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use chrono::DateTime;
use chrono::Utc;
use serde_json::json;
use sqlx::migrate;
use sqlx::postgres::PgPoolOptions;
use sqlx::FromRow;
use sqlx::Result;
use sqlx::Row;
use tracing::instrument;
use uuid::Uuid;

use crate::api::ApiMetrics;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::Version;

use super::models::*;

macro_rules! sort_by {
  ($maybe_sort:expr => { $(@timestamps $($timestamp:literal),+;)? $( $key:expr $(=> $val:expr)? ),+, } || $default:expr) => {
    if let Some(sort) = $maybe_sort {
      let mut inverse = sort.starts_with('!');
      let sort = if inverse { &sort[1..] } else { sort };

      $(
        if [$($timestamp),+].contains(&sort) {
          inverse = !inverse;
        }
      )?

      let order = if inverse { "DESC" } else { "ASC" };

      match sort {
        $(
          $key => format!("{} {order}", sort_by!(@expand $key $(, $val)? )),
        )+
        _ => $default.to_string(),
      }
    } else {
      $default.to_string()
    }
  };

  (@expand $key:expr, $val:expr) => { $val };
  (@expand $key:expr) => { $key };
}

#[derive(Debug, Clone)]
pub struct Database {
  pool: sqlx::PgPool,
}

impl Database {
  pub async fn connect(
    database_url: &str,
    pool_size: u32,
    acquire_timeout: std::time::Duration,
  ) -> anyhow::Result<Self> {
    let pool = PgPoolOptions::new()
      .max_connections(pool_size)
      .acquire_timeout(acquire_timeout)
      .connect(database_url)
      .await?;
    if std::env::var("DATABASE_DISABLE_MIGRATIONS").is_err() {
      migrate!("./migrations")
        .run(&pool)
        .await
        .expect("database schema error");
    }
    println!("Database ready");
    Ok(Database { pool })
  }

  #[instrument(name = "Database::get_user", skip(self), err)]
  pub async fn get_user(&self, id: Uuid) -> Result<Option<User>> {
    sqlx::query_as!(
      User,
      r#"SELECT id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
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
        ) END) as "newer_ticket_messages_count!"
      FROM users
      WHERE id = $1"#,
      id
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_user_public", skip(self), err)]
  pub async fn get_user_public(&self, id: Uuid) -> Result<Option<UserPublic>> {
    sqlx::query_as!(
      UserPublic,
      r#"SELECT id, name, avatar_url, github_id, updated_at, created_at
      FROM users
      WHERE id = $1"#,
      id
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_user_by_github_id", skip(self), err)]
  pub async fn get_user_by_github_id(
    &self,
    github_id: i64,
  ) -> Result<Option<User>> {
    sqlx::query_as!(
      User,
      r#"SELECT id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
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
        ) END) as "newer_ticket_messages_count!"
      FROM users
      WHERE github_id = $1"#,
      github_id
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_users", skip(self), err)]
  pub async fn list_users(
    &self,
    start: i64,
    limit: i64,
    maybe_search_query: Option<&str>,
    maybe_sort: Option<&str>,
  ) -> Result<(usize, Vec<User>)> {
    let mut tx = self.pool.begin().await?;

    let maybe_id = maybe_search_query
      .and_then(|search_query| Uuid::parse_str(search_query).ok());
    let search = format!(
      "%{}%",
      if maybe_id.is_some() {
        ""
      } else {
        maybe_search_query.unwrap_or("")
      }
    );

    let sort = sort_by!(maybe_sort => {
      @timestamps "created_at";
      "email",
      "github_id",
      "scope_limit",
      "is_staff",
      "is_blocked",
      "created_at",
    } || "created_at DESC");

    let users: Vec<User> = sqlx::query_as(
      &format!(r#"SELECT id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
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
        ) END) as "newer_ticket_messages_count"
      FROM users
      WHERE (name ILIKE $1 OR email ILIKE $1) AND (id = $2 OR $2 IS NULL)
      ORDER BY {sort} OFFSET $3 LIMIT $4"#)
    )
      .bind(&search)
      .bind(maybe_id)
      .bind(start)
      .bind(limit)
      .fetch_all(&mut *tx).await?;

    let total_users = sqlx::query!(
      r#"SELECT COUNT(created_at) as "count!" FROM users WHERE (name ILIKE $1 OR email ILIKE $1) AND (id = $2 OR $2 IS NULL);"#,
      search,
      maybe_id,
    )
      .map(|r| r.count)
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((total_users as usize, users))
  }

  #[instrument(
    name = "Database::insert_user",
    skip(self, new_user),
    err,
    fields(user.name = new_user.name, user.email = new_user.email, user.avatar_url = new_user.avatar_url, user.github_id = new_user.github_id, user.is_blocked = new_user.is_blocked, user.is_staff = new_user.is_staff
    )
  )]
  pub async fn insert_user(&self, new_user: NewUser<'_>) -> Result<User> {
    sqlx::query_as!(
      User,
      r#"INSERT INTO users (name, email, avatar_url, github_id, is_blocked, is_staff)
      VALUES ($1, $2, $3, $4, $5, $6)
      RETURNING id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
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
        ) END) as "newer_ticket_messages_count!"
      "#,
      new_user.name,
      new_user.email,
      new_user.avatar_url,
      new_user.github_id,
      new_user.is_blocked,
      new_user.is_staff
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::upsert_user_by_github_id", skip(
    self,
    new_user
  ), err, fields(user.name = new_user.name, user.email = new_user.email, user.avatar_url = new_user.avatar_url, user.github_id = new_user.github_id, user.is_blocked = new_user.is_blocked, user.is_staff = new_user.is_staff
  ))]
  pub async fn upsert_user_by_github_id(
    &self,
    new_user: NewUser<'_>,
  ) -> Result<User> {
    assert!(new_user.github_id.is_some(), "github_id is required");
    sqlx::query_as!(
      User,
      r#"INSERT INTO users (name, email, avatar_url, github_id, is_blocked, is_staff)
      VALUES ($1, $2, $3, $4, $5, $6)
      ON CONFLICT(github_id) DO UPDATE
      SET name = $1, email = $2, avatar_url = $3
      RETURNING id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
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
        ) END) as "newer_ticket_messages_count!"
      "#,
      new_user.name,
      new_user.email,
      new_user.avatar_url,
      new_user.github_id,
      new_user.is_blocked,
      new_user.is_staff
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::user_set_staff", skip(self), err)]
  pub async fn user_set_staff(
    &self,
    staff_id: &Uuid,
    user_id: Uuid,
    is_staff: bool,
  ) -> Result<User> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      staff_id,
      true,
      "user_set_staff",
      json!({
        "user_id": user_id,
        "is_staff": is_staff,
      }),
    )
    .await?;

    let user = sqlx::query_as!(
      User,
      r#"UPDATE users SET is_staff = $1 WHERE id = $2
      RETURNING id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
        (SELECT COUNT(created_at) FROM scope_invites WHERE target_user_id = id) as "invite_count!",
        (SELECT COUNT(created_at) FROM scopes WHERE creator = id) as "scope_usage!",
        (SELECT COUNT(created_at) FROM tickets WHERE closed = false AND tickets.creator = users.id AND EXISTS (
            SELECT 1 FROM ticket_messages as tm WHERE tm.ticket_id = tickets.id AND tm.author != users.id AND tm.created_at > (
                SELECT MAX(tm2.created_at) FROM ticket_messages as tm2 WHERE tm2.ticket_id = tm.ticket_id AND tm2.author = users.id
            )
        )) as "newer_ticket_messages_count!"
      "#,
      is_staff,
      user_id
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(user)
  }

  #[instrument(name = "Database::user_set_blocked", skip(self), err)]
  pub async fn user_set_blocked(
    &self,
    staff_id: &Uuid,
    user_id: Uuid,
    is_blocked: bool,
  ) -> Result<User> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      staff_id,
      true,
      "user_set_blocked",
      json!({
        "user_id": user_id,
        "is_blocked": is_blocked,
      }),
    )
    .await?;

    let user = sqlx::query_as!(
      User,
      r#"UPDATE users SET is_blocked = $1 WHERE id = $2
      RETURNING id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
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
        ) END) as "newer_ticket_messages_count!"
      "#,
      is_blocked,
      user_id
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(user)
  }

  #[instrument(name = "Database::user_set_scope_limit", skip(self), err)]
  pub async fn user_set_scope_limit(
    &self,
    staff_id: &Uuid,
    user_id: Uuid,
    scope_limit: i32,
  ) -> Result<User> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      staff_id,
      true,
      "user_set_scope_limit",
      json!({
        "user_id": user_id,
        "scope_limit": scope_limit,
      }),
    )
    .await?;

    let user = sqlx::query_as!(
      User,
      r#"UPDATE users SET scope_limit = $1 WHERE id = $2
      RETURNING id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
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
        ) END) as "newer_ticket_messages_count!"
      "#,
      scope_limit,
      user_id
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(user)
  }

  #[instrument(name = "Database::delete_user", skip(self), err)]
  pub async fn delete_user(&self, id: Uuid) -> Result<Option<User>> {
    sqlx::query_as!(
      User,
      r#"DELETE FROM users
      WHERE id = $1
      RETURNING id, name, email, avatar_url, updated_at, created_at, github_id, is_blocked, is_staff, scope_limit,
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
        ) END) as "newer_ticket_messages_count!"
      "#,
      id
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_package", skip(self), err)]
  pub async fn get_package(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Option<PackageWithGitHubRepoAndMeta>> {
    sqlx::query!(
      r#"SELECT packages.scope "package_scope: ScopeName", packages.name "package_name: PackageName", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat "package_runtime_compat: RuntimeCompat", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at", packages.created_at "package_created_at",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "package_version_count!",
        (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_latest_version",
        (SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta: PackageVersionMeta",
        github_repositories.id "github_repository_id?", github_repositories.owner "github_repository_owner?", github_repositories.name "github_repository_name?", github_repositories.updated_at "github_repository_updated_at?", github_repositories.created_at "github_repository_created_at?"
      FROM packages
      LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
      WHERE packages.scope = $1 AND packages.name = $2"#,
      scope as _,
      name as _
    )
      .map(|r| {
        let package = Package {
          scope: r.package_scope,
          name: r.package_name,
          description: r.package_description,
          github_repository_id: r.package_github_repository_id,
          runtime_compat: r.package_runtime_compat,
          created_at: r.package_created_at,
          updated_at: r.package_updated_at,
          version_count: r.package_version_count,
          latest_version: r.package_latest_version,
          when_featured: r.package_when_featured,
          is_archived: r.package_is_archived,
        };
        let github_repository = if r.package_github_repository_id.is_some() {
          Some(GithubRepository {
            id: r.github_repository_id.unwrap(),
            owner: r.github_repository_owner.unwrap(),
            name: r.github_repository_name.unwrap(),
            created_at: r.github_repository_created_at.unwrap(),
            updated_at: r.github_repository_updated_at.unwrap(),
          })
        } else {
          None
        };

        let meta = r.package_version_meta.unwrap_or_default();

        (package, github_repository, meta)
      })
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::create_package", skip(self), err)]
  pub async fn create_package(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<CreatePackageResult> {
    let mut tx = self.pool.begin().await?;
    let res = sqlx::query_as!(
      Package,
      r#"
      INSERT INTO packages (scope, name)
      VALUES ($1, $2)
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", description, github_repository_id, runtime_compat as "runtime_compat: RuntimeCompat", when_featured, is_archived, updated_at, created_at,
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "version_count!",
        (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "latest_version"
      "#,
      scope as _,
      name as _
    )
      .fetch_one(&mut *tx)
      .await;
    let package = match res {
      Ok(package) => package,
      Err(err) => {
        if let Some(dberr) = err.as_database_error() {
          if dberr.is_unique_violation() {
            return Ok(CreatePackageResult::AlreadyExists);
          }
        }
        return Err(err);
      }
    };

    if let Some(res) = finalize_package_creation(tx, scope).await? {
      return Ok(res);
    };

    Ok(CreatePackageResult::Ok(package))
  }

  #[instrument(
    name = "Database::insert_provenance_statements",
    skip(self),
    err
  )]
  pub async fn insert_provenance_statement(
    &self,
    package_scope: &ScopeName,
    package_name: &PackageName,
    version: &Version,
    rekor_log_id: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"UPDATE package_versions
      SET rekor_log_id = $1, meta = jsonb_set_lax(meta, '{hasProvenance}', 'true'::jsonb, true)
      WHERE scope = $2 AND name = $3 AND version = $4 AND rekor_log_id IS NULL AND created_at > now() - '2 minute'::interval"#,
      rekor_log_id,
      package_scope as _,
      package_name as _,
      version as _
    )
      .execute(&self.pool)
      .await?;

    Ok(())
  }

  #[instrument(name = "Database::update_package_description", skip(self), err)]
  pub async fn update_package_description(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
    description: &str,
  ) -> Result<PackageWithGitHubRepoAndMeta> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "update_package_description",
      json!({
          "scope": scope,
          "name": name,
      }),
    )
    .await?;

    let package = sqlx::query!(
      r#"UPDATE packages
      SET description = $3
      WHERE scope = $1 AND name = $2
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", description, github_repository_id, runtime_compat as "runtime_compat: RuntimeCompat", when_featured, is_archived, updated_at, created_at,
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count!",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version",
        (SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta: PackageVersionMeta""#,
      scope as _,
      name as _,
      description
    )
      .map(|r| {
        let package = Package {
          scope: r.scope,
          name: r.name,
          description: r.description,
          github_repository_id: r.github_repository_id,
          runtime_compat: r.runtime_compat,
          updated_at: r.updated_at,
          created_at: r.created_at,
          version_count: r.version_count,
          latest_version: r.latest_version,
          when_featured: r.when_featured,
          is_archived: r.is_archived,
        };

        (package, None, r.package_version_meta.unwrap_or_default())
      })
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(package)
  }

  #[instrument(name = "Database::update_package_github_repository", skip(
    self,
    repo
  ), err, fields(repo.id = repo.id, repo.owner = repo.owner, repo.name = repo.name
  ))]
  pub async fn update_package_github_repository(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
    repo: NewGithubRepository<'_>,
  ) -> Result<(Package, GithubRepository, PackageVersionMeta)> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "update_package_github_repository",
      json!({
        "scope": scope,
        "name": name,
        "repo": repo.id,
      }),
    )
    .await?;

    let repo = sqlx::query_as!(
      GithubRepository,
      "INSERT INTO github_repositories (id, owner, name)
      VALUES ($1, $2, $3)
      ON CONFLICT(id) DO UPDATE
      SET owner = $2, name = $3
      RETURNING id, owner, name, updated_at, created_at",
      repo.id,
      repo.owner,
      repo.name
    )
    .fetch_one(&mut *tx)
    .await?;

    let (package, meta) = sqlx::query!(
      r#"UPDATE packages
      SET github_repository_id = $3
      WHERE scope = $1 AND name = $2
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", description, github_repository_id, runtime_compat as "runtime_compat: RuntimeCompat", when_featured, is_archived, updated_at, created_at,
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count!",
        (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "latest_version",
        (SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta: PackageVersionMeta""#,
      scope as _,
      name as _,
      repo.id
    )
      .map(|r| {
        let package = Package {
          scope: r.scope,
          name: r.name,
          description: r.description,
          github_repository_id: r.github_repository_id,
          runtime_compat: r.runtime_compat,
          updated_at: r.updated_at,
          created_at: r.created_at,
          version_count: r.version_count,
          latest_version: r.latest_version,
          when_featured: r.when_featured,
          is_archived: r.is_archived,
        };

        (package, r.package_version_meta.unwrap_or_default())
      })
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((package, repo, meta))
  }

  #[instrument(
    name = "Database::delete_package_github_repository",
    skip(self),
    err
  )]
  pub async fn delete_package_github_repository(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Package> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "delete_package_github_repository",
      json!({
        "scope": scope,
        "name": name,
      }),
    )
    .await?;

    let package = sqlx::query_as!(
      Package,
      r#"UPDATE packages
      SET github_repository_id = NULL
      WHERE scope = $1 AND name = $2
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", description, github_repository_id, runtime_compat as "runtime_compat: RuntimeCompat", when_featured, is_archived, updated_at, created_at,
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count!",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#,
      scope as _,
      name as _,
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(package)
  }

  #[instrument(
    name = "Database::update_package_runtime_compat",
    skip(self),
    err
  )]
  pub async fn update_package_runtime_compat(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
    runtime_compat: &RuntimeCompat,
  ) -> Result<Package> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "update_package_runtime_compat",
      json!({
        "scope": scope,
        "name": name,
        "runtime_compat": runtime_compat,
      }),
    )
    .await?;

    let package = sqlx::query_as!(
      Package,
      r#"UPDATE packages
      SET runtime_compat = $3
      WHERE scope = $1 AND name = $2
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", description, github_repository_id, runtime_compat as "runtime_compat: RuntimeCompat", when_featured, is_archived, updated_at, created_at,
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count!",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#,
      scope as _,
      name as _,
      runtime_compat as _
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(package)
  }

  #[instrument(name = "Database::update_package_is_featured", skip(self), err)]
  pub async fn update_package_is_featured(
    &self,
    staff_id: &Uuid,
    scope: &ScopeName,
    name: &PackageName,
    is_featured: bool,
  ) -> Result<Package> {
    let mut tx = self.pool.begin().await?;

    let when_featured = if is_featured { Some(Utc::now()) } else { None };

    audit_log(
      &mut tx,
      staff_id,
      true,
      "feature_package",
      json!({
      "scope": scope,
      "name": name,
      "is_featured": when_featured,
      }),
    )
    .await?;

    let package = sqlx::query_as!(
      Package,
      r#"UPDATE packages
      SET when_featured = $3
      WHERE scope = $1 AND name = $2
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", description, github_repository_id, runtime_compat as "runtime_compat: RuntimeCompat", when_featured, is_archived, updated_at, created_at,
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count!",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#,
      scope as _,
      name as _,
      when_featured,
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(package)
  }

  #[instrument(name = "Database::update_package_is_archived", skip(self), err)]
  pub async fn update_package_is_archived(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
    is_archived: bool,
  ) -> Result<Package> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "archive_package",
      json!({
          "scope": scope,
          "name": name,
          "is_archived": is_archived,
      }),
    )
    .await?;

    let package = sqlx::query_as!(
      Package,
      r#"UPDATE packages
      SET is_archived = $3
      WHERE scope = $1 AND name = $2
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", description, github_repository_id, runtime_compat as "runtime_compat: RuntimeCompat", when_featured, is_archived, updated_at, created_at,
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count!",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#,
      scope as _,
      name as _,
      is_archived,
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(package)
  }

  #[instrument(name = "Database::create_scope", skip(self), err)]
  pub async fn create_scope(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    user_id: Uuid,
  ) -> Result<Scope> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      if is_sudo {
        "assign_scope"
      } else {
        "create_scope"
      },
      json!({
          "scope": scope,
          "user_id": user_id,
      }),
    )
    .await?;

    let scope = sqlx::query_as!(
      Scope,
      r#"
        WITH ins_scope AS (
            INSERT INTO scopes (scope, creator) VALUES ($1, $2)
            RETURNING
            scope,
            creator,
            package_limit,
            new_package_per_week_limit,
            publish_attempts_per_week_limit,
            verify_oidc_actor,
            require_publishing_from_ci,
            updated_at,
            created_at
        ),
        ins_member AS (
            INSERT INTO scope_members (scope, user_id, is_admin)
            VALUES ($1, $2, true)
        )
        SELECT
        scope as "scope: ScopeName",
        creator,
        package_limit,
        new_package_per_week_limit,
        publish_attempts_per_week_limit,
        verify_oidc_actor,
        require_publishing_from_ci,
        updated_at,
        created_at
        FROM ins_scope
        "#,
      scope as _,
      user_id
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(scope)
  }

  #[instrument(name = "Database::update_scope_limits", skip(self), err)]
  pub async fn update_scope_limits(
    &self,
    staff_id: &Uuid,
    scope: &ScopeName,
    package_limit: Option<i32>,
    new_package_per_week_limit: Option<i32>,
    publish_attempts_per_week_limit: Option<i32>,
  ) -> Result<(Scope, ScopeUsage, UserPublic)> {
    let mut tx = self.pool.begin().await?;

    if let Some(package_limit) = package_limit {
      audit_log(
        &mut tx,
        staff_id,
        true,
        "scope_set_package_limit",
        json!({
          "scope": scope,
          "package_limit": package_limit,
        }),
      )
      .await?;

      sqlx::query!(
        r#"UPDATE scopes SET package_limit = $1 WHERE scope = $2"#,
        package_limit,
        scope as _
      )
      .execute(&mut *tx)
      .await?;
    }

    if let Some(new_package_per_week_limit) = new_package_per_week_limit {
      audit_log(
        &mut tx,
        staff_id,
        true,
        "scope_set_package_per_week_limit",
        json!({
          "scope": scope,
          "new_package_per_week_limit": new_package_per_week_limit,
        }),
      )
      .await?;

      sqlx::query!(
        r#"UPDATE scopes SET new_package_per_week_limit = $1 WHERE scope = $2"#,
        new_package_per_week_limit,
        scope as _
      )
      .execute(&mut *tx)
      .await?;
    }

    if let Some(publish_attempts_per_week_limit) =
      publish_attempts_per_week_limit
    {
      audit_log(
        &mut tx,
        staff_id,
        true,
        "scope_set_publish_attempts_per_week_limit",
        json!({
          "scope": scope,
          "publish_attempts_per_week_limit": publish_attempts_per_week_limit,
        }),
      )
      .await?;

      sqlx::query!(
        r#"UPDATE scopes SET publish_attempts_per_week_limit = $1 WHERE scope = $2"#,
        publish_attempts_per_week_limit,
        scope as _
      )
        .execute(&mut *tx)
        .await?;
    }

    let res = sqlx::query!(
      r#"
      WITH usage AS (
        SELECT
          (SELECT COUNT(created_at) FROM packages WHERE scope = $1) AS package,
          (SELECT COUNT(created_at) FROM packages WHERE scope = $1 AND created_at > now() - '1 week'::interval) AS new_package_per_week,
          (SELECT COUNT(created_at) FROM publishing_tasks WHERE package_scope = $1 AND created_at > now() - '1 week'::interval) AS publish_attempts_per_week
      )
      SELECT
      scopes.scope as "scope_scope: ScopeName",
      scopes.creator as "scope_creator",
      scopes.package_limit as "scope_package_limit",
      scopes.new_package_per_week_limit as "scope_new_package_per_week_limit",
      scopes.publish_attempts_per_week_limit as "scope_publish_attempts_per_week_limit",
      scopes.verify_oidc_actor as "scope_verify_oidc_actor",
      scopes.require_publishing_from_ci as "scope_require_publishing_from_ci",
      scopes.updated_at as "scope_updated_at",
      scopes.created_at as "scope_created_at",
      users.id as "user_id", users.name as "user_name", users.avatar_url as "user_avatar_url", users.github_id as "user_github_id", users.updated_at as "user_updated_at", users.created_at as "user_created_at",
      usage.package as "usage_package", usage.new_package_per_week as "usage_new_package_per_week", usage.publish_attempts_per_week as "usage_publish_attempts_per_week"
      FROM scopes
      LEFT JOIN users ON scopes.creator = users.id
      CROSS JOIN usage
      WHERE scopes.scope = $1
      "#,
      scope as _
    )
      .map(|r| {
        let scope = Scope {
          scope: r.scope_scope,
          creator: r.scope_creator,
          updated_at: r.scope_updated_at,
          created_at: r.scope_created_at,
          package_limit: r.scope_package_limit,
          new_package_per_week_limit: r.scope_new_package_per_week_limit,
          publish_attempts_per_week_limit: r.scope_publish_attempts_per_week_limit,
          verify_oidc_actor: r.scope_verify_oidc_actor,
          require_publishing_from_ci: r.scope_require_publishing_from_ci,
        };
        let usage = ScopeUsage {
          package: r.usage_package.unwrap().try_into().unwrap(),
          new_package_per_week: r.usage_new_package_per_week.unwrap().try_into().unwrap(),
          publish_attempts_per_week: r.usage_publish_attempts_per_week.unwrap().try_into().unwrap(),
        };
        let user = UserPublic {
          id: r.user_id,
          name: r.user_name,
          avatar_url: r.user_avatar_url,
          github_id: r.user_github_id,
          updated_at: r.user_updated_at,
          created_at: r.user_created_at,
        };
        (scope, usage, user)
      })
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(res)
  }

  #[allow(clippy::type_complexity)]
  #[instrument(name = "Database::list_scopes", skip(self), err)]
  pub async fn list_scopes(
    &self,
    start: i64,
    limit: i64,
    maybe_search_query: Option<&str>,
    maybe_sort: Option<&str>,
  ) -> Result<(usize, Vec<(Scope, ScopeUsage, UserPublic)>)> {
    let mut tx = self.pool.begin().await?;

    let search = format!("%{}%", maybe_search_query.unwrap_or(""));
    let sort = sort_by!(maybe_sort => {
      @timestamps "created_at";
      "scope" => "scopes.scope",
      "creator" => "users.name",
      "package_limit" => "scopes.package_limit",
      "new_package_per_week_limit" => "scopes.new_package_per_week_limit",
      "publish_attempts_per_week_limit" => "scopes.publish_attempts_per_week_limit",
      "created_at" => "scopes.created_at",
    } || "scopes.created_at DESC");

    let scopes = sqlx::query(&format!(
      r#"SELECT
      scopes.scope as "scope_scope",
      scopes.creator as "scope_creator",
      scopes.package_limit as "scope_package_limit",
      scopes.new_package_per_week_limit as "scope_new_package_per_week_limit",
      scopes.publish_attempts_per_week_limit as "scope_publish_attempts_per_week_limit",
      scopes.updated_at as "scope_updated_at",
      scopes.verify_oidc_actor as "scope_verify_oidc_actor",
      scopes.require_publishing_from_ci as "scope_require_publishing_from_ci",
      scopes.created_at as "scope_created_at",
      users.id as "user_id", users.name as "user_name", users.avatar_url as "user_avatar_url", users.github_id as "user_github_id", users.updated_at as "user_updated_at", users.created_at as "user_created_at",
      (SELECT COUNT(created_at) FROM packages WHERE packages.scope = scopes.scope) AS "usage_package",
      (SELECT COUNT(created_at) FROM packages WHERE packages.scope = scopes.scope AND created_at > now() - '1 week'::interval) AS "usage_new_package_per_week",
      (SELECT COUNT(created_at) FROM publishing_tasks WHERE publishing_tasks.package_scope = scopes.scope AND created_at > now() - '1 week'::interval) AS "usage_publish_attempts_per_week"
      FROM scopes
      LEFT JOIN users ON scopes.creator = users.id
      WHERE scopes.scope ILIKE $1 OR users.name ILIKE $1
      ORDER BY {sort}
      OFFSET $2 LIMIT $3
      "#,
    ))
      .bind(&search)
      .bind(start)
      .bind(limit)
      .try_map(|r| {
        let scope = Scope::from_row(&r)?;
        let usage = ScopeUsage::from_row(&r)?;
        let user = UserPublic::from_row(&r)?;

        Ok((scope, usage, user))
      })
      .fetch_all(&mut *tx)
      .await?;

    let total_scopes = sqlx::query!(
      r#"SELECT COUNT(scopes.created_at) FROM scopes LEFT JOIN users ON scopes.creator = users.id WHERE scopes.scope ILIKE $1 OR users.name ILIKE $1;"#,
      search,
    )
      .map(|r| r.count.unwrap())
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((total_scopes as usize, scopes))
  }

  #[instrument(name = "Database::list_scopes_created_by_user", skip(self), err)]
  pub async fn list_scopes_created_by_user(
    &self,
    user_id: Uuid,
  ) -> Result<Vec<Scope>> {
    sqlx::query_as!(
      Scope,
      r#"SELECT
      scope as "scope: ScopeName",
      creator,
      package_limit,
      new_package_per_week_limit,
      publish_attempts_per_week_limit,
      verify_oidc_actor,
      require_publishing_from_ci,
      updated_at,
      created_at
      FROM scopes WHERE creator = $1
      ORDER BY scope ASC"#,
      user_id
    )
    .fetch_all(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_scope", skip(self), err)]
  pub async fn get_scope(&self, scope: &ScopeName) -> Result<Option<Scope>> {
    sqlx::query_as!(
      Scope,
      r#"SELECT
      scope as "scope: ScopeName",
      creator,
      package_limit,
      new_package_per_week_limit,
      publish_attempts_per_week_limit,
      verify_oidc_actor,
      require_publishing_from_ci,
      updated_at,
      created_at
      FROM scopes WHERE scope = $1"#,
      scope as _
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_scope_usage", skip(self), err)]
  pub async fn get_scope_usage(&self, scope: &ScopeName) -> Result<ScopeUsage> {
    sqlx::query!(
      r#"SELECT
      (SELECT COUNT(created_at) FROM packages WHERE scope = $1 AND created_at > now() - '1 week'::interval) AS new_package_per_week,
      (SELECT COUNT(created_at) FROM packages WHERE scope = $1) AS package,
      (SELECT COUNT(created_at) FROM publishing_tasks WHERE package_scope = $1 AND created_at > now() - '1 week'::interval) AS publish_attempts_per_week;"#,
    scope as _,
    )
      .map(|r| {
        ScopeUsage {
          package: r.package.unwrap().try_into().unwrap(),
          new_package_per_week: r.new_package_per_week.unwrap().try_into().unwrap(),
          publish_attempts_per_week: r.publish_attempts_per_week.unwrap().try_into().unwrap(),
        }
      })
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::scope_set_verify_oidc_actor", skip(self), err)]
  pub async fn scope_set_verify_oidc_actor(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    verify_oidc_actor: bool,
  ) -> Result<Scope> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "scope_set_verify_oidc_actor",
      json!({
        "scope": scope,
        "verify_oidc_actor": verify_oidc_actor,
      }),
    )
    .await?;

    let scope = sqlx::query_as!(
      Scope,
      r#"
        UPDATE scopes SET verify_oidc_actor = $1 WHERE scope = $2
        RETURNING
          scope as "scope: ScopeName",
          creator,
          package_limit,
          new_package_per_week_limit,
          publish_attempts_per_week_limit,
          verify_oidc_actor,
          require_publishing_from_ci,
          updated_at,
          created_at

      "#,
      verify_oidc_actor,
      scope as _
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(scope)
  }

  #[instrument(
    name = "Database::scope_set_require_publishing_from_ci",
    skip(self),
    err
  )]
  pub async fn scope_set_require_publishing_from_ci(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    require_publishing_from_ci: bool,
  ) -> Result<Scope> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "scope_set_require_publishing_from_ci",
      json!({
        "scope": scope,
        "require_publishing_from_ci": require_publishing_from_ci,
      }),
    )
    .await?;

    let scope = sqlx::query_as!(
      Scope,
      r#"
        UPDATE scopes SET require_publishing_from_ci = $1 WHERE scope = $2
        RETURNING
          scope as "scope: ScopeName",
          creator,
          package_limit,
          new_package_per_week_limit,
          publish_attempts_per_week_limit,
          verify_oidc_actor,
          require_publishing_from_ci,
          updated_at,
          created_at

      "#,
      require_publishing_from_ci,
      scope as _
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(scope)
  }

  #[instrument(name = "Database::list_packages_by_scope", skip(self), err)]
  pub async fn list_packages_by_scope(
    &self,
    scope: &ScopeName,
    show_archived: bool,
    start: i64,
    limit: i64,
  ) -> Result<(usize, Vec<PackageWithGitHubRepoAndMeta>)> {
    let mut tx = self.pool.begin().await?;

    let packages = sqlx::query!(
      r#"SELECT packages.scope "package_scope: ScopeName", packages.name "package_name: PackageName", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat as "package_runtime_compat: RuntimeCompat", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at",  packages.created_at "package_created_at",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "package_version_count!",
        (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_latest_version",
        (SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta: PackageVersionMeta",
        github_repositories.id "github_repository_id?", github_repositories.owner "github_repository_owner?", github_repositories.name "github_repository_name?", github_repositories.updated_at "github_repository_updated_at?", github_repositories.created_at "github_repository_created_at?"
      FROM packages
      LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
      WHERE packages.scope = $1 AND ($2 = true OR packages.is_archived = false)
      ORDER BY packages.name
      OFFSET $3 LIMIT $4"#,
      scope as _,
      show_archived,
      start,
      limit
    )
      .map(|r| {
        let package = Package {
          scope: r.package_scope,
          name: r.package_name,
          description: r.package_description,
          github_repository_id: r.package_github_repository_id,
          runtime_compat: r.package_runtime_compat,
          created_at: r.package_created_at,
          updated_at: r.package_updated_at,
          version_count: r.package_version_count,
          latest_version: r.package_latest_version,
          when_featured: r.package_when_featured,
          is_archived: r.package_is_archived,
        };
        let github_repository = if r.package_github_repository_id.is_some() {
          Some(GithubRepository {
            id: r.github_repository_id.unwrap(),
            owner: r.github_repository_owner.unwrap(),
            name: r.github_repository_name.unwrap(),
            created_at: r.github_repository_created_at.unwrap(),
            updated_at: r.github_repository_updated_at.unwrap(),
          })
        } else {
          None
        };

        let meta = r.package_version_meta.unwrap_or_default();

        (package, github_repository, meta)
      })
      .fetch_all(&mut *tx)
      .await?;

    let total_packages = sqlx::query!(
      r#"SELECT COUNT(created_at) FROM packages WHERE scope = $1 AND ($2 = true OR packages.is_archived = false);"#,
      scope as _,
      show_archived,
    )
      .map(|r| r.count.unwrap())
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((total_packages as usize, packages))
  }

  #[instrument(name = "Database::list_packages", skip(self), err)]
  pub async fn list_packages(
    &self,
    start: i64,
    limit: i64,
    maybe_search_query: Option<&str>,
    maybe_github_repo_id: Option<i64>,
    maybe_sort: Option<&str>,
  ) -> Result<(usize, Vec<PackageWithGitHubRepoAndMeta>)> {
    let mut tx = self.pool.begin().await?;

    let (
      scope_ilike_query,
      scope_exact_query,
      package_ilike_query,
      package_exact_query,
    ) = if let Some(search_query) = maybe_search_query {
      // 1. Strip leading `@`.
      let search_query = search_query.strip_prefix('@').unwrap_or(search_query);

      // 2. If there's a space in the search query, we're gonna split it
      // and use the first term for scope search and the reminder for package
      // search.
      let (scope_query, package_query) = if let Some((
        scope_query,
        package_query,
      )) = search_query.split_once(' ')
      {
        (scope_query, package_query)
      } else {
        // 3. If there's no space in the search query, we're gonna split it
        // at `/` and use the first term for scope search and the reminder for package
        // search.
        search_query
          .split_once('/')
          .unwrap_or((search_query, search_query))
      };

      (
        format!("%{}%", scope_query),
        scope_query.to_string(),
        format!("%{}%", package_query),
        package_query.to_string(),
      )
    } else {
      (
        "%%".to_string(),
        "".to_string(),
        "%%".to_string(),
        "".to_string(),
      )
    };
    let sort = sort_by!(maybe_sort => {
      @timestamps "when_featured", "updated_at", "created_at";
      "scope" => "packages.scope",
      "name" => "packages.name",
      // "repository",
      "is_archived" => "packages.is_archived",
      "when_featured" => "packages.when_featured",
      "updated_at" => "packages.updated_at",
      "created_at" => "packages.created_at",
    } || "packages.name ASC, packages.scope ASC");

    let packages = sqlx::query(
      &format!(r#"SELECT packages.scope "package_scope", packages.name "package_name", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat as "package_runtime_compat", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at",  packages.created_at "package_created_at",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "package_version_count",
        (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_latest_version",
        (SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta",
        github_repositories.id "github_repository_id", github_repositories.owner "github_repository_owner", github_repositories.name "github_repository_name", github_repositories.updated_at "github_repository_updated_at", github_repositories.created_at "github_repository_created_at"
       FROM packages
       LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
       WHERE (packages.scope ILIKE $1 OR packages.name ILIKE $2) AND (packages.github_repository_id = $5 OR $5 IS NULL) AND NOT packages.is_archived
       ORDER BY
         CASE
           WHEN packages.name ILIKE $3 THEN 1 -- Exact match for package name
           WHEN packages.scope ILIKE $4 THEN 2 -- Exact match for scope name
           ELSE 3 -- Fuzzy matches will be ordered by package name and then scope name below
        END,
        {sort}
       OFFSET $6 LIMIT $7"#),
    )
      .bind(&scope_ilike_query)
      .bind(&package_ilike_query)
      .bind(package_exact_query)
      .bind(scope_exact_query)
      .bind(maybe_github_repo_id)
      .bind(start)
      .bind(limit)
      .try_map(|r| {
        let package = Package::from_row(&r)?;

        let github_repository = if r.try_get::<Option<i64>, &str>("github_repository_id")?.is_some() {
          Some(GithubRepository::from_row(&r)?)
        } else {
          None
        };

        let meta: Option<PackageVersionMeta> = r.try_get("package_version_meta")?;
        Ok((package, github_repository, meta.unwrap_or_default()))
      })
      .fetch_all(&mut *tx)
      .await?;

    let total_packages = sqlx::query!(
      r#"SELECT COUNT(created_at) FROM packages WHERE (packages.scope ILIKE $1 OR packages.name ILIKE $2) AND (packages.github_repository_id = $3 OR $3 IS NULL);"#,
      scope_ilike_query,
      package_ilike_query,
      maybe_github_repo_id,
    )
      .map(|r| r.count.unwrap())
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((total_packages as usize, packages))
  }

  #[instrument(name = "Database::package_stats", skip(self), err)]
  pub async fn package_stats(
    &self,
  ) -> Result<(
    Vec<PackageWithGitHubRepoAndMeta>,
    Vec<PackageVersion>,
    Vec<PackageWithGitHubRepoAndMeta>,
  )> {
    let newest = sqlx::query!(
      r#"SELECT packages.scope "package_scope: ScopeName", packages.name "package_name: PackageName", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat as "package_runtime_compat: RuntimeCompat", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at",  packages.created_at "package_created_at",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "package_version_count!",
        (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_latest_version",
        (SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta: PackageVersionMeta",
        github_repositories.id "github_repository_id?", github_repositories.owner "github_repository_owner?", github_repositories.name "github_repository_name?", github_repositories.updated_at "github_repository_updated_at?", github_repositories.created_at "github_repository_created_at?"
      FROM packages
      LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
      WHERE (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND is_yanked = false AND version IS NOT NULL ORDER BY version DESC LIMIT 1) IS NOT NULL AND NOT packages.is_archived
      ORDER BY packages.created_at DESC
      LIMIT 10"#,
    )
      .map(|r| {
        let package = Package {
          scope: r.package_scope,
          name: r.package_name,
          description: r.package_description,
          github_repository_id: r.package_github_repository_id,
          runtime_compat: r.package_runtime_compat,
          created_at: r.package_created_at,
          updated_at: r.package_updated_at,
          version_count: r.package_version_count,
          latest_version: r.package_latest_version,
          when_featured: r.package_when_featured,
          is_archived: r.package_is_archived,
        };
        let github_repository = if r.package_github_repository_id.is_some() {
          Some(GithubRepository {
            id: r.github_repository_id.unwrap(),
            owner: r.github_repository_owner.unwrap(),
            name: r.github_repository_name.unwrap(),
            created_at: r.github_repository_created_at.unwrap(),
            updated_at: r.github_repository_updated_at.unwrap(),
          })
        } else {
          None
        };
        let meta = r.package_version_meta.unwrap_or_default();
        (package, github_repository, meta)
      })
      .fetch_all(&self.pool)
      .await?;

    let updated = sqlx::query_as!(
      PackageVersion,
      r#"SELECT package_versions.scope as "scope: ScopeName", package_versions.name as "name: PackageName", package_versions.version as "version: Version", package_versions.user_id, package_versions.readme_path as "readme_path: PackagePath", package_versions.exports as "exports: ExportsMap", package_versions.is_yanked, package_versions.uses_npm, package_versions.meta as "meta: PackageVersionMeta", package_versions.updated_at, package_versions.created_at, package_versions.rekor_log_id,
      (SELECT COUNT(*)
        FROM package_versions AS pv
        WHERE pv.scope = package_versions.scope
        AND pv.name = package_versions.name
        AND pv.version > package_versions.version
        AND pv.version NOT LIKE '%-%'
        AND pv.is_yanked = false) as "newer_versions_count!",
      (SELECT COALESCE(SUM(dl.count), 0)
        FROM version_download_counts_24h as dl
        WHERE dl.scope = package_versions.scope
        AND dl.package = package_versions.name
        AND dl.version = package_versions.version) as "lifetime_download_count!"
      FROM package_versions
      JOIN packages ON packages.scope = package_versions.scope AND packages.name = package_versions.name
      WHERE NOT packages.is_archived
      ORDER BY package_versions.created_at DESC
      LIMIT 10"#,
    )
      .fetch_all(&self.pool)
      .await?;

    let featured = sqlx::query!(
      r#"SELECT packages.scope "package_scope: ScopeName", packages.name "package_name: PackageName", packages.description "package_description", packages.github_repository_id "package_github_repository_id", packages.runtime_compat as "package_runtime_compat: RuntimeCompat", packages.when_featured "package_when_featured", packages.is_archived "package_is_archived", packages.updated_at "package_updated_at",  packages.created_at "package_created_at",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "package_version_count!",
        (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_latest_version",
        (SELECT meta FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "package_version_meta: PackageVersionMeta",
        github_repositories.id "github_repository_id?", github_repositories.owner "github_repository_owner?", github_repositories.name "github_repository_name?", github_repositories.updated_at "github_repository_updated_at?", github_repositories.created_at "github_repository_created_at?"
      FROM packages
      LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
      WHERE packages.when_featured IS NOT NULL AND NOT packages.is_archived
      ORDER BY packages.when_featured DESC
      LIMIT 10"#,
    )
      .map(|r| {
        let package = Package {
          scope: r.package_scope,
          name: r.package_name,
          description: r.package_description,
          github_repository_id: r.package_github_repository_id,
          runtime_compat: r.package_runtime_compat,
          created_at: r.package_created_at,
          updated_at: r.package_updated_at,
          version_count: r.package_version_count,
          latest_version: r.package_latest_version,
          when_featured: r.package_when_featured,
          is_archived: r.package_is_archived,
        };
        let github_repository = if r.package_github_repository_id.is_some() {
          Some(GithubRepository {
            id: r.github_repository_id.unwrap(),
            owner: r.github_repository_owner.unwrap(),
            name: r.github_repository_name.unwrap(),
            created_at: r.github_repository_created_at.unwrap(),
            updated_at: r.github_repository_updated_at.unwrap(),
          })
        } else {
          None
        };
        let meta = r.package_version_meta.unwrap_or_default();
        (package, github_repository, meta)
      })
      .fetch_all(&self.pool)
      .await?;

    Ok((newest, updated, featured))
  }

  #[instrument(name = "Database::metrics", skip(self), err)]
  pub async fn metrics(&self) -> Result<ApiMetrics> {
    let packages = sqlx::query!(r#"
      SELECT
        COUNT(DISTINCT (packages.name, packages.scope)) AS count_total,
        COUNT(DISTINCT CASE WHEN package_versions.created_at >= NOW() - INTERVAL '1 day' THEN (packages.name, packages.scope) END) AS count_1d,
        COUNT(DISTINCT CASE WHEN package_versions.created_at >= NOW() - INTERVAL '7 day' THEN (packages.name, packages.scope) END) AS count_7d,
        COUNT(DISTINCT CASE WHEN package_versions.created_at >= NOW() - INTERVAL '30 day' THEN (packages.name, packages.scope) END) AS count_30d
      FROM packages
      LEFT JOIN
        package_versions ON packages.name = package_versions.name AND packages.scope = package_versions.scope
      WHERE
        package_versions.name IS NOT NULL
    "#)
      .fetch_one(&self.pool)
      .await?;

    let users = sqlx::query!(r#"
      SELECT
        COUNT(*) AS count_total,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '1 DAY' THEN 1 END) AS count_1d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '7 DAY' THEN 1 END) AS count_7d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '30 DAY' THEN 1 END) AS count_30d
      FROM
        users;
      "#)
      .fetch_one(&self.pool)
      .await?;

    let package_versions =
      sqlx::query!(r#"
      SELECT
        COUNT(*) AS count_total,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '1 DAY' THEN 1 END) AS count_1d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '7 DAY' THEN 1 END) AS count_7d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '30 DAY' THEN 1 END) AS count_30d
      FROM
        package_versions;
      "#)
        .fetch_one(&self.pool)
        .await?;

    Ok(ApiMetrics {
      packages: packages.count_total.unwrap().try_into().unwrap(),
      packages_1d: packages.count_1d.unwrap().try_into().unwrap(),
      packages_7d: packages.count_7d.unwrap().try_into().unwrap(),
      packages_30d: packages.count_30d.unwrap().try_into().unwrap(),

      users: users.count_total.unwrap().try_into().unwrap(),
      users_1d: users.count_1d.unwrap().try_into().unwrap(),
      users_7d: users.count_7d.unwrap().try_into().unwrap(),
      users_30d: users.count_30d.unwrap().try_into().unwrap(),

      package_versions: package_versions
        .count_total
        .unwrap()
        .try_into()
        .unwrap(),
      package_versions_1d: package_versions
        .count_1d
        .unwrap()
        .try_into()
        .unwrap(),
      package_versions_7d: package_versions
        .count_7d
        .unwrap()
        .try_into()
        .unwrap(),
      package_versions_30d: package_versions
        .count_30d
        .unwrap()
        .try_into()
        .unwrap(),
    })
  }

  #[instrument(name = "Database::list_package_versions", skip(self), err)]
  pub async fn list_package_versions(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Vec<(PackageVersion, Option<UserPublic>)>> {
    sqlx::query!(
      r#"SELECT package_versions.scope as "package_version_scope: ScopeName", package_versions.name as "package_version_name: PackageName", package_versions.version as "package_version_version: Version", package_versions.user_id as "package_version_user_id", package_versions.readme_path as "package_version_readme_path: PackagePath", package_versions.exports as "package_version_exports: ExportsMap", package_versions.is_yanked as "package_version_is_yanked", package_versions.uses_npm as "package_version_uses_npm", package_versions.meta as "package_version_meta: PackageVersionMeta", package_versions.updated_at as "package_version_updated_at", package_versions.created_at as "package_version_created_at", package_versions.rekor_log_id as "package_version_rekor_log_id",
      (SELECT COUNT(*)
        FROM package_versions AS pv
        WHERE pv.scope = package_versions.scope
        AND pv.name = package_versions.name
        AND pv.version > package_versions.version
        AND pv.version NOT LIKE '%-%'
        AND pv.is_yanked = false) as "package_version_newer_versions_count!",
      (SELECT COALESCE(SUM(dl.count), 0)
        FROM version_download_counts_24h as dl
        WHERE dl.scope = package_versions.scope
        AND dl.package = package_versions.name
        AND dl.version = package_versions.version) as "package_version_lifetime_download_count!",
      users.id as "user_id?", users.name as "user_name?", users.avatar_url as "user_avatar_url?", users.github_id as "user_github_id", users.updated_at as "user_updated_at?", users.created_at as "user_created_at?"
      FROM package_versions
      LEFT JOIN users ON package_versions.user_id = users.id
      WHERE package_versions.scope = $1 AND package_versions.name = $2
      ORDER BY package_versions.version DESC"#,
      scope as _,
      name as _,
    )
      .map(|r| {
        let package_version = PackageVersion {
          scope: r.package_version_scope,
          name: r.package_version_name,
          version: r.package_version_version,
          user_id: r.package_version_user_id,
          exports: r.package_version_exports,
          is_yanked: r.package_version_is_yanked,
          readme_path: r.package_version_readme_path,
          uses_npm: r.package_version_uses_npm,
          newer_versions_count: r.package_version_newer_versions_count,
          lifetime_download_count: r.package_version_lifetime_download_count,
          meta: r.package_version_meta,
          updated_at: r.package_version_updated_at,
          created_at: r.package_version_created_at,
          rekor_log_id: r.package_version_rekor_log_id,
        };

        let user = if r.package_version_user_id.is_some() {
          let user = UserPublic {
            id: r.user_id.unwrap(),
            name: r.user_name.unwrap(),
            avatar_url: r.user_avatar_url.unwrap(),
            github_id: r.user_github_id,
            updated_at: r.user_updated_at.unwrap(),
            created_at: r.user_created_at.unwrap(),
          };

          Some(user)
        } else {
          None
        };

        (package_version, user)
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::get_latest_unyanked_version_for_package",
    skip(self),
    err
  )]
  pub async fn get_latest_unyanked_version_for_package(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Option<PackageVersion>> {
    sqlx::query_as!(
      PackageVersion,
      r#"SELECT scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", user_id, readme_path as "readme_path: PackagePath", exports as "exports: ExportsMap", is_yanked, uses_npm, meta as "meta: PackageVersionMeta", updated_at, created_at, rekor_log_id,
      (SELECT COUNT(*)
        FROM package_versions AS pv
        WHERE pv.scope = package_versions.scope
        AND pv.name = package_versions.name
        AND pv.version > package_versions.version
        AND pv.version NOT LIKE '%-%'
        AND pv.is_yanked = false) as "newer_versions_count!",
      (SELECT COALESCE(SUM(dl.count), 0)
        FROM version_download_counts_24h as dl
        WHERE dl.scope = package_versions.scope
        AND dl.package = package_versions.name
        AND dl.version = package_versions.version) as "lifetime_download_count!"
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND version NOT LIKE '%-%' AND is_yanked = false
      ORDER BY version DESC
      LIMIT 1"#,
      scope as _,
      name as _,
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::list_latest_package_versions",
    skip(self),
    err
  )]
  pub async fn list_latest_unyanked_versions_for_package(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    limit: u32,
  ) -> Result<Vec<Version>> {
    sqlx::query!(
      r#"
      SELECT version as "version: Version"
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND version NOT LIKE '%-%' AND is_yanked = false
      ORDER BY version DESC
      "#,
      scope as _,
      name as _,
    )
      .map(|r| r.version)
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_package_version", skip(self), err)]
  pub async fn get_package_version(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<Option<PackageVersion>> {
    sqlx::query_as!(
      PackageVersion,
      r#"SELECT scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", user_id, readme_path as "readme_path: PackagePath", exports as "exports: ExportsMap", is_yanked, uses_npm, meta as "meta: PackageVersionMeta", updated_at, created_at, rekor_log_id,
      (SELECT COUNT(*)
        FROM package_versions AS pv
        WHERE pv.scope = package_versions.scope
        AND pv.name = package_versions.name
        AND pv.version > package_versions.version
        AND pv.version NOT LIKE '%-%'
        AND pv.is_yanked = false) as "newer_versions_count!",
      (SELECT COALESCE(SUM(dl.count), 0)
        FROM version_download_counts_24h as dl
        WHERE dl.scope = package_versions.scope
        AND dl.package = package_versions.name
        AND dl.version = package_versions.version) as "lifetime_download_count!"
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND version = $3"#,
      scope as _,
      name as _,
      version as _
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::create_package_version_and_npm_tarball_and_finalize_publishing_task",
    skip(self, new_package_version, new_package_files),
    err,
    fields(package_version.scope = %new_package_version.scope, package_version.name = %new_package_version.name, package_version.version = %new_package_version.version, package_version.exports = ?new_package_version.exports, package_files = new_package_files.len()
    )
  )]
  pub async fn create_package_version_and_npm_tarball_and_finalize_publishing_task(
    &self,
    publishing_task_id: Uuid,
    new_package_version: NewPackageVersion<'_>,
    new_package_files: &[NewPackageFile<'_>],
    new_package_version_dependencies: &[NewPackageVersionDependency<'_>],
    new_npm_tarball: NewNpmTarball<'_>,
  ) -> Result<PublishingTask> {
    let mut tx = self.pool.begin().await?;

    sqlx::query!(
      r#"INSERT INTO package_versions (scope, name, version, user_id, readme_path, exports, uses_npm, meta)
      VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
      new_package_version.scope as _,
      new_package_version.name as _,
      new_package_version.version as _,
      new_package_version.user_id as _,
      new_package_version.readme_path as _,
      new_package_version.exports as _,
      new_package_version.uses_npm as _,
      new_package_version.meta as _,
    )
      .execute(&mut *tx)
      .await?;

    for new_package_file in new_package_files {
      sqlx::query!(
        r#"INSERT INTO package_files (scope, name, version, path, size, checksum)
        VALUES ($1, $2, $3, $4, $5, $6)"#,
        new_package_file.scope as _,
        new_package_file.name as _,
        new_package_file.version as _,
        new_package_file.path as _,
        new_package_file.size,
        new_package_file.checksum,
      )
        .execute(&mut *tx)
        .await?;
    }

    for new_package_version_dependency in new_package_version_dependencies {
      sqlx::query!(
        r#"INSERT INTO package_version_dependencies (package_scope, package_name, package_version, dependency_kind, dependency_name, dependency_constraint, dependency_path)
        VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        new_package_version_dependency.package_scope as _,
        new_package_version_dependency.package_name as _,
        new_package_version_dependency.package_version as _,
        new_package_version_dependency.dependency_kind as _,
        new_package_version_dependency.dependency_name as _,
        new_package_version_dependency.dependency_constraint as _,
        new_package_version_dependency.dependency_path as _,
      )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
      r#"INSERT INTO npm_tarballs (scope, name, version, revision, sha1, sha512, size)
      VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
      new_npm_tarball.scope as _,
      new_npm_tarball.name as _,
      new_npm_tarball.version as _,
      new_npm_tarball.revision,
      new_npm_tarball.sha1,
      new_npm_tarball.sha512,
      new_npm_tarball.size,
    )
      .execute(&mut *tx)
      .await?;

    let task = sqlx::query_as!(
      PublishingTask,
      r#"UPDATE publishing_tasks
      SET status = 'processed'
      WHERE id = $1 AND status = 'processing'
      RETURNING id, status as "status: PublishingTaskStatus", error as "error: PublishingTaskError", user_id, package_scope as "package_scope: ScopeName", package_name as "package_name: PackageName", package_version as "package_version: Version", config_file as "config_file: PackagePath", created_at, updated_at"#,
      publishing_task_id,
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(task)
  }

  #[instrument(name = "Database::create_package_version_for_test", skip(
    self,
    new_package_version
  ), err, fields(package_version.scope = %new_package_version.scope, package_version.name = %new_package_version.name, package_version.version = %new_package_version.version, package_version.exports = ?new_package_version.exports
  ))]
  pub async fn create_package_version_for_test(
    &self,
    new_package_version: NewPackageVersion<'_>,
  ) -> Result<PackageVersion> {
    sqlx::query_as!(
      PackageVersion,
      r#"INSERT INTO package_versions (scope, name, version, user_id, readme_path, exports, uses_npm, meta)
      VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", user_id, readme_path as "readme_path: PackagePath", exports as "exports: ExportsMap", is_yanked, uses_npm, meta as "meta: PackageVersionMeta", updated_at, created_at, rekor_log_id,
      (SELECT COUNT(*)
        FROM package_versions AS pv
        WHERE pv.scope = package_versions.scope
        AND pv.name = package_versions.name
        AND pv.version > package_versions.version
        AND pv.version NOT LIKE '%-%'
        AND pv.is_yanked = false) as "newer_versions_count!",
      (SELECT COALESCE(SUM(dl.count), 0)
        FROM version_download_counts_24h as dl
        WHERE dl.scope = package_versions.scope
        AND dl.package = package_versions.name
        AND dl.version = package_versions.version) as "lifetime_download_count!""#,
      new_package_version.scope as _,
      new_package_version.name as _,
      new_package_version.version as _,
      new_package_version.user_id as _,
      new_package_version.readme_path as _,
      new_package_version.exports as _,
      new_package_version.uses_npm as _,
      new_package_version.meta as _,
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::yank_package_version", skip(self), err)]
  pub async fn yank_package_version(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
    yank: bool,
  ) -> Result<PackageVersion> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "yank_package_version",
      json!({
        "scope": scope,
        "name": name,
        "version": version,
        "yank": yank,
      }),
    )
    .await?;

    let package_version = sqlx::query_as!(
      PackageVersion,
      r#"UPDATE package_versions
      SET is_yanked = $4
      WHERE scope = $1 AND name = $2 AND version = $3
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", user_id, readme_path as "readme_path: PackagePath", exports as "exports: ExportsMap", is_yanked, uses_npm, meta as "meta: PackageVersionMeta", updated_at, created_at, rekor_log_id,
      (SELECT COUNT(*)
        FROM package_versions AS pv
        WHERE pv.scope = package_versions.scope
        AND pv.name = package_versions.name
        AND pv.version > package_versions.version
        AND pv.version NOT LIKE '%-%'
        AND pv.is_yanked = false) as "newer_versions_count!",
      (SELECT COALESCE(SUM(dl.count), 0)
        FROM version_download_counts_24h as dl
        WHERE dl.scope = package_versions.scope
        AND dl.package = package_versions.name
        AND dl.version = package_versions.version) as "lifetime_download_count!""#,
      scope as _,
      name as _,
      version as _,
      yank
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(package_version)
  }

  #[instrument(name = "Database::delete_package_version", skip(self), err)]
  pub async fn delete_package_version(
    &self,
    staff_id: &Uuid,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<()> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      staff_id,
      true,
      "delete_package_version",
      json!({
      "scope": scope,
      "name": name,
      "version": version,
      }),
    )
    .await?;

    sqlx::query_as!(
      PackageVersion,
      r#"DELETE FROM package_versions WHERE scope = $1 AND name = $2 AND version = $3"#,
      scope as _,
      name as _,
      version as _
    )
      .execute(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(())
  }

  #[instrument(name = "Database::get_package_file", skip(self), err)]
  pub async fn get_package_file(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
    path: &PackagePath,
  ) -> Result<Option<PackageFile>> {
    sqlx::query_as!(
      PackageFile,
      r#"SELECT scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", path as "path: PackagePath", size, checksum, updated_at, created_at
      FROM package_files
      WHERE scope = $1 AND name = $2 AND version = $3 AND path = $4"#,
      scope as _,
      name as _,
      version as _,
      path as _
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_package_files", skip(self), err)]
  pub async fn list_package_files(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<Vec<PackageFile>> {
    sqlx::query_as!(
      PackageFile,
      r#"SELECT scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", path as "path: PackagePath", size, checksum, updated_at, created_at
      FROM package_files
      WHERE scope = $1 AND name = $2 AND version = $3"#,
      scope as _,
      name as _,
      version as _
    )
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::create_package_file_for_test", skip(
    self,
    new_package_file
  ), err, fields(package_file.scope = %new_package_file.scope, package_file.name = %new_package_file.name, package_file.version = %new_package_file.version, package_file.path = %new_package_file.path, package_file.size = new_package_file.size, package_file.checksum = new_package_file.checksum
  ))]
  pub async fn create_package_file_for_test(
    &self,
    new_package_file: NewPackageFile<'_>,
  ) -> Result<PackageFile> {
    sqlx::query_as!(
      PackageFile,
      r#"INSERT INTO package_files (scope, name, version, path, size, checksum)
      VALUES ($1, $2, $3, $4, $5, $6)
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", path as "path: PackagePath", size, checksum, updated_at, created_at"#,
      new_package_file.scope as _,
      new_package_file.name as _,
      new_package_file.version as _,
      new_package_file.path as _,
      new_package_file.size,
      new_package_file.checksum
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::create_package_version_dependency",
    skip(self, new_package_version_dependency),
    err
  )]
  pub async fn create_package_version_dependency_for_test(
    &self,
    new_package_version_dependency: NewPackageVersionDependency<'_>,
  ) -> Result<PackageVersionDependency> {
    sqlx::query_as!(
      PackageVersionDependency,
      r#"INSERT INTO package_version_dependencies (package_scope, package_name, package_version, dependency_kind, dependency_name, dependency_constraint, dependency_path)
      VALUES ($1, $2, $3, $4, $5, $6, $7)
      RETURNING package_scope as "package_scope: ScopeName", package_name as "package_name: PackageName", package_version as "package_version: Version", dependency_kind as "dependency_kind: DependencyKind", dependency_name, dependency_constraint, dependency_path, updated_at, created_at"#,
      new_package_version_dependency.package_scope as _,
      new_package_version_dependency.package_name as _,
      new_package_version_dependency.package_version as _,
      new_package_version_dependency.dependency_kind as _,
      new_package_version_dependency.dependency_name as _,
      new_package_version_dependency.dependency_constraint as _,
      new_package_version_dependency.dependency_path as _
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::create_npm_tarball",
    skip(self, new_npm_tarball),
    err
  )]
  pub async fn create_npm_tarball(
    &self,
    new_npm_tarball: NewNpmTarball<'_>,
  ) -> Result<NpmTarball> {
    sqlx::query_as!(
      NpmTarball,
      r#"INSERT INTO npm_tarballs (scope, name, version, revision, sha1, sha512, size)
      VALUES ($1, $2, $3, $4, $5, $6, $7)
      RETURNING scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", revision, sha1, sha512, size, updated_at, created_at"#,
      new_npm_tarball.scope as _,
      new_npm_tarball.name as _,
      new_npm_tarball.version as _,
      new_npm_tarball.revision,
      new_npm_tarball.sha1,
      new_npm_tarball.sha512,
      new_npm_tarball.size
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_scope_member", skip(self), err)]
  pub async fn get_scope_member(
    &self,
    scope: &ScopeName,
    user_id: Uuid,
  ) -> Result<Option<ScopeMember>> {
    sqlx::query_as!(
      ScopeMember,
      r#"SELECT scope as "scope: ScopeName", user_id, is_admin, updated_at, created_at
    FROM scope_members WHERE scope = $1 AND user_id = $2"#,
      scope as _,
      user_id
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_scope_members", skip(self), err)]
  pub async fn list_scope_members(
    &self,
    scope: &ScopeName,
  ) -> Result<Vec<(ScopeMember, UserPublic)>> {
    sqlx::query!(
      r#"SELECT scope_members.scope as "scope_member_scope: ScopeName", scope_members.user_id as "scope_member_user_id", scope_members.is_admin as "scope_member_is_admin", scope_members.updated_at as "scope_member_updated_at", scope_members.created_at as "scope_member_created_at",
        users.id as "user_id", users.name as "user_name", users.avatar_url as "user_avatar_url", users.github_id as "user_github_id", users.updated_at as "user_updated_at", users.created_at as "user_created_at"
      FROM scope_members
      LEFT JOIN users ON scope_members.user_id = users.id
      WHERE scope = $1
      ORDER BY users.name ASC"#,
      scope as _
    )
      .map(|r| {
        let scope_member = ScopeMember {
          scope: r.scope_member_scope,
          user_id: r.scope_member_user_id,
          is_admin: r.scope_member_is_admin,
          created_at: r.scope_member_created_at,
          updated_at: r.scope_member_updated_at,
        };
        let user = UserPublic {
          id: r.user_id,
          name: r.user_name,
          avatar_url: r.user_avatar_url,
          github_id: r.user_github_id,
          updated_at: r.user_updated_at,
          created_at: r.user_created_at,
        };
        (scope_member, user)
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_member_scopes_by_user", skip(self), err)]
  pub async fn get_member_scopes_by_user(
    &self,
    id: &Uuid,
  ) -> Result<Vec<Scope>> {
    sqlx::query_as!(
      Scope,
      r#"SELECT
      scopes.scope as "scope: ScopeName",
      scopes.creator,
      scopes.package_limit,
      scopes.new_package_per_week_limit,
      scopes.publish_attempts_per_week_limit,
      scopes.verify_oidc_actor,
      scopes.require_publishing_from_ci,
      scopes.updated_at,
      scopes.created_at
      FROM scopes
      LEFT JOIN scope_members ON scope_members.scope = scopes.scope
      WHERE user_id = $1"#,
      id
    )
    .fetch_all(&self.pool)
    .await
  }

  #[instrument(name = "Database::add_scope_invite", skip(self), err)]
  pub async fn add_scope_invite(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    new_scope_invite: NewScopeInvite<'_>,
  ) -> Result<ScopeInvite> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "add_scope_invite",
      json!({
          "scope": new_scope_invite.scope,
          "target_user_id": new_scope_invite.target_user_id,
      }),
    )
    .await?;

    let scope_invite = sqlx::query_as!(
      ScopeInvite,
      r#"INSERT INTO scope_invites (scope, target_user_id, requesting_user_id)
      VALUES ($1, $2, $3)
      RETURNING scope as "scope: ScopeName", target_user_id, requesting_user_id, updated_at, created_at"#,
      new_scope_invite.scope as _,
      new_scope_invite.target_user_id,
      new_scope_invite.requesting_user_id,
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(scope_invite)
  }

  #[instrument(name = "Database::add_user_to_scope", skip(
    self,
    new_scope_member
  ), err, fields(scope_member.scope = %new_scope_member.scope, scope_member.user_id = %new_scope_member.user_id, scope_member.is_admin = new_scope_member.is_admin
  ))]
  pub async fn add_user_to_scope(
    &self,
    new_scope_member: NewScopeMember<'_>,
  ) -> Result<ScopeMember> {
    sqlx::query_as!(
      ScopeMember,
      r#"INSERT INTO scope_members (scope, user_id, is_admin)
      VALUES ($1, $2, $3)
      RETURNING scope as "scope: ScopeName", user_id, is_admin, updated_at, created_at"#,
      new_scope_member.scope as _,
      new_scope_member.user_id,
      new_scope_member.is_admin,
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_scope_invites_by_user", skip(self), err)]
  pub async fn get_scope_invites_by_user(
    &self,
    target_user: &Uuid,
  ) -> Result<Vec<(ScopeInvite, UserPublic, UserPublic)>> {
    sqlx::query!(
      r#"SELECT scope_invites.scope as "scope_invite_scope: ScopeName", scope_invites.target_user_id as "scope_invite_target_user_id", scope_invites.requesting_user_id as "scope_invite_requesting_user_id", scope_invites.updated_at as "scope_invite_updated_at", scope_invites.created_at as "scope_invite_created_at",
        target_user.id as "target_user_id", target_user.name as "target_user_name", target_user.github_id as "target_user_github_id", target_user.avatar_url as "target_user_avatar_url", target_user.updated_at as "target_user_updated_at", target_user.created_at as "target_user_created_at",
        requesting_user.id as "requesting_user_id", requesting_user.name as "requesting_user_name", requesting_user.github_id as "requesting_user_github_id", requesting_user.avatar_url as "requesting_user_avatar_url", requesting_user.updated_at as "requesting_user_updated_at", requesting_user.created_at as "requesting_user_created_at"
      FROM scope_invites
      LEFT JOIN users AS target_user ON scope_invites.target_user_id = target_user.id
      LEFT JOIN users AS requesting_user ON scope_invites.requesting_user_id = requesting_user.id
      WHERE target_user_id = $1"#,
      target_user
    )
      .map(|r| {
        let scope_invite = ScopeInvite {
          target_user_id: r.scope_invite_target_user_id,
          requesting_user_id: r.scope_invite_requesting_user_id,
          scope: r.scope_invite_scope,
          created_at: r.scope_invite_created_at,
          updated_at: r.scope_invite_updated_at,
        };
        let target_user = UserPublic {
          id: r.target_user_id,
          name: r.target_user_name,
          avatar_url: r.target_user_avatar_url,
          github_id: r.target_user_github_id,
          updated_at: r.target_user_updated_at,
          created_at: r.target_user_created_at,
        };
        let requesting_user = UserPublic {
          id: r.requesting_user_id,
          name: r.requesting_user_name,
          avatar_url: r.requesting_user_avatar_url,
          github_id: r.requesting_user_github_id,
          updated_at: r.requesting_user_updated_at,
          created_at: r.requesting_user_created_at,
        };
        (scope_invite, target_user, requesting_user)
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_scope_invites_by_scope", skip(self), err)]
  pub async fn get_scope_invites_by_scope(
    &self,
    scope: &ScopeName,
  ) -> Result<Vec<(ScopeInvite, UserPublic, UserPublic)>> {
    sqlx::query!(
      r#"SELECT scope_invites.scope as "scope_invite_scope: ScopeName", scope_invites.target_user_id as "scope_invite_target_user_id", scope_invites.requesting_user_id as "scope_invite_requesting_user_id", scope_invites.updated_at as "scope_invite_updated_at", scope_invites.created_at as "scope_invite_created_at",
        target_user.id as "target_user_id", target_user.name as "target_user_name", target_user.avatar_url as "target_user_avatar_url", target_user.github_id as "target_user_github_id", target_user.updated_at as "target_user_updated_at", target_user.created_at as "target_user_created_at",
        requesting_user.id as "requesting_user_id", requesting_user.name as "requesting_user_name", requesting_user.avatar_url as "requesting_user_avatar_url", requesting_user.github_id as "requesting_user_github_id", requesting_user.updated_at as "requesting_user_updated_at", requesting_user.created_at as "requesting_user_created_at"
      FROM scope_invites
      LEFT JOIN users AS target_user ON scope_invites.target_user_id = target_user.id
      LEFT JOIN users AS requesting_user ON scope_invites.requesting_user_id = requesting_user.id
      WHERE scope = $1"#,
      scope as _
    )
      .map(|r| {
        let scope_invite = ScopeInvite {
          target_user_id: r.scope_invite_target_user_id,
          requesting_user_id: r.scope_invite_requesting_user_id,
          scope: r.scope_invite_scope,
          created_at: r.scope_invite_created_at,
          updated_at: r.scope_invite_updated_at,
        };
        let target_user = UserPublic {
          id: r.target_user_id,
          name: r.target_user_name,
          avatar_url: r.target_user_avatar_url,
          github_id: r.target_user_github_id,
          updated_at: r.target_user_updated_at,
          created_at: r.target_user_created_at,
        };
        let requesting_user = UserPublic {
          id: r.requesting_user_id,
          name: r.requesting_user_name,
          avatar_url: r.requesting_user_avatar_url,
          github_id: r.requesting_user_github_id,
          updated_at: r.requesting_user_updated_at,
          created_at: r.requesting_user_created_at,
        };
        (scope_invite, target_user, requesting_user)
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::accept_scope_invite", skip(self), err)]
  pub async fn accept_scope_invite(
    &self,
    target_user_id: &Uuid,
    scope: &ScopeName,
  ) -> Result<Option<ScopeMember>> {
    let mut tx = self.pool.begin().await?;

    let res = sqlx::query!(
      r#"DELETE FROM scope_invites WHERE target_user_id = $1 AND scope = $2"#,
      target_user_id,
      scope as _,
    )
    .execute(&mut *tx)
    .await?;
    if res.rows_affected() != 1 {
      return Ok(None);
    }

    let member = sqlx::query_as!(
      ScopeMember,
      r#"INSERT INTO scope_members (scope, user_id) VALUES ($1, $2)
      RETURNING scope as "scope: ScopeName", user_id, is_admin, updated_at, created_at"#,
      scope as _,
      target_user_id,
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(Some(member))
  }

  #[instrument(name = "Database::delete_scope_invite", skip(self), err)]
  pub async fn delete_scope_invite(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    target_user_id: &Uuid,
    scope: &ScopeName,
  ) -> Result<()> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "delete_scope_invite",
      json!({
          "scope": scope,
          "target_user_id": target_user_id,
      }),
    )
    .await?;

    sqlx::query!(
      r#"DELETE FROM scope_invites WHERE target_user_id = $1 AND scope = $2"#,
      target_user_id,
      scope as _,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
  }

  #[instrument(name = "Database::delete_package", skip(self), err)]
  pub async fn delete_package(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<bool> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "delete_package",
      json!({
          "scope": scope,
      }),
    )
    .await?;

    let status = sqlx::query!(
      r#"SELECT count(*) FROM publishing_tasks WHERE package_scope = $1 AND package_name = $2 AND status != 'failure'"#,
      scope as _,
      name as _,
    )
      .fetch_one(&mut *tx)
      .await?;
    if status.count.unwrap() > 0 {
      return Ok(false);
    }

    let res = sqlx::query!(
      r#"DELETE FROM packages WHERE scope = $1 AND name = $2"#,
      scope as _,
      name as _,
    )
    .execute(&mut *tx)
    .await;

    match res {
      Ok(res) => {
        let success = res.rows_affected() > 0;
        if success {
          tx.commit().await?;
        }
        Ok(success)
      }
      Err(err) => {
        if let Some(dberr) = err.as_database_error() {
          if dberr.is_foreign_key_violation() {
            return Ok(false);
          }
        }
        Err(err)
      }
    }
  }

  #[instrument(name = "Database::delete_scope", skip(self), err)]
  pub async fn delete_scope(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
  ) -> Result<bool> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "delete_scope",
      json!({
        "scope": scope,
      }),
    )
    .await?;

    sqlx::query!(r#"DELETE FROM scope_members WHERE scope = $1"#, scope as _,)
      .execute(&mut *tx)
      .await?;

    sqlx::query!(r#"DELETE FROM scope_invites WHERE scope = $1"#, scope as _,)
      .execute(&mut *tx)
      .await?;

    let res =
      sqlx::query!(r#"DELETE FROM scopes WHERE scope = $1"#, scope as _,)
        .execute(&mut *tx)
        .await;
    match res {
      Ok(res) => {
        let success = res.rows_affected() > 0;
        if success {
          tx.commit().await?;
        }
        Ok(success)
      }
      Err(err) => {
        if let Some(dberr) = err.as_database_error() {
          if dberr.is_foreign_key_violation() {
            return Ok(false);
          }
        }
        Err(err)
      }
    }
  }

  pub async fn transfer_scope<'a>(
    &self,
    scope: &ScopeName,
    is_creator: bool,
    tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
  ) -> Result<Option<ScopeMemberUpdateResult>> {
    let admins_n = sqlx::query!(
      r#"SELECT COUNT(user_id) FROM scope_members WHERE scope = $1 AND is_admin = true"#,
      scope as _,
    )
      .map(|r| r.count.unwrap())
      .fetch_one(&mut **tx)
      .await?;
    if admins_n == 0 {
      return Ok(Some(ScopeMemberUpdateResult::TargetIsLastAdmin));
    }

    let maybe_available_admin_id = sqlx::query!(
      r#"
      SELECT user_id
      FROM (
        SELECT scope_members.user_id as "user_id", users.scope_limit as "scope_limit", scope_members.created_at as "created_at", (SELECT COUNT(created_at) FROM scopes WHERE creator = users.id) as "scope_usage"
        FROM scope_members
        LEFT JOIN users ON scope_members.user_id = users.id
        WHERE scope_members.scope = $1 AND scope_members.is_admin = true
      ) AS subquery
      WHERE "scope_usage" < scope_limit
      ORDER BY created_at LIMIT 1;
      "#,
      scope as _,
    )
      .map(|r| r.user_id)
      .fetch_optional(&mut **tx)
      .await?;
    let Some(new_creator_id) = maybe_available_admin_id else {
      return Ok(Some(ScopeMemberUpdateResult::TargetIsLastTransferableAdmin));
    };

    if is_creator {
      let _ = sqlx::query!(
        r#"UPDATE scopes SET creator = $1 WHERE scope = $2"#,
        new_creator_id,
        scope as _,
      )
      .execute(&mut **tx)
      .await?;
    }

    Ok(None)
  }

  #[instrument(name = "Database::delete_scope_member", skip(self), err)]
  pub async fn update_scope_member_role(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    user_id: Uuid,
    is_admin: bool,
  ) -> Result<ScopeMemberUpdateResult> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      actor_id,
      is_sudo,
      "scope_update_scope_member_role",
      json!({
        "scope": scope,
        "user_id": user_id,
        "is_admin": is_admin,
      }),
    )
    .await?;

    let maybe_scope_member = sqlx::query!(
      r#"UPDATE scope_members
      SET is_admin = $1
      WHERE scope = $2 AND user_id = $3
      RETURNING scope as "scope: ScopeName", user_id, is_admin, updated_at, created_at,
      (SELECT creator FROM scopes WHERE scope = $2) AS "scope_creator!""#,
      is_admin,
      scope as _,
      user_id,
    )
      .map(|r| {
        (
          ScopeMember {
            scope: r.scope,
            user_id: r.user_id,
            is_admin: r.is_admin,
            updated_at: r.updated_at,
            created_at: r.created_at,
          },
          r.user_id == r.scope_creator
        )
      })
      .fetch_optional(&mut *tx)
      .await?;

    let Some((scope_member, is_creator)) = maybe_scope_member else {
      return Ok(ScopeMemberUpdateResult::TargetNotMember);
    };

    if !scope_member.is_admin {
      if let Some(result) =
        self.transfer_scope(scope, is_creator, &mut tx).await?
      {
        return Ok(result);
      }
    }

    tx.commit().await?;

    Ok(ScopeMemberUpdateResult::Ok(scope_member))
  }

  #[instrument(name = "Database::delete_scope_member", skip(self), err)]
  pub async fn delete_scope_member(
    &self,
    scope: &ScopeName,
    user_id: Uuid,
  ) -> Result<ScopeMemberUpdateResult> {
    let mut tx = self.pool.begin().await?;

    let maybe_scope_member = sqlx::query!(
      r#"DELETE FROM scope_members WHERE scope = $1 AND user_id = $2
      RETURNING scope as "scope: ScopeName", user_id, is_admin, updated_at, created_at,
      (SELECT creator FROM scopes WHERE scope = $1) AS "scope_creator!""#,
      scope as _,
      user_id,
    )
      .map(|r| {
        (
          ScopeMember {
            scope: r.scope,
            user_id: r.user_id,
            is_admin: r.is_admin,
            updated_at: r.updated_at,
            created_at: r.created_at,
          },
          r.user_id == r.scope_creator
        )
      })
      .fetch_optional(&mut *tx)
      .await?;
    let Some((scope_member, is_creator)) = maybe_scope_member else {
      return Ok(ScopeMemberUpdateResult::TargetNotMember);
    };

    if let Some(result) =
      self.transfer_scope(scope, is_creator, &mut tx).await?
    {
      return Ok(result);
    }

    tx.commit().await?;

    Ok(ScopeMemberUpdateResult::Ok(scope_member))
  }

  #[instrument(
    name = "Database::create_publishing_task",
    skip(self, task),
    err,
    fields(publishing_task.package_scope = %task.package_scope, publishing_task.package_name = %task.package_name, publishing_task.package_version = %task.package_version
    )
  )]
  pub async fn create_publishing_task(
    &self,
    task: NewPublishingTask<'_>,
  ) -> Result<CreatePublishingTaskResult> {
    let mut tx = self.pool.begin().await?;

    // only allow insert if no non status==failure tasks exist
    let already_processing = sqlx::query!(
      r#"SELECT
        publishing_tasks.id as "task_id",
        publishing_tasks.status as "task_status: PublishingTaskStatus",
        publishing_tasks.error as "task_error: PublishingTaskError",
        publishing_tasks.user_id as "task_user_id",
        publishing_tasks.package_scope as "task_package_scope: ScopeName",
        publishing_tasks.package_name as "task_package_name: PackageName",
        publishing_tasks.package_version as "task_package_version: Version",
        publishing_tasks.config_file as "task_config_file: PackagePath",
        publishing_tasks.created_at as "task_created_at",
        publishing_tasks.updated_at as "task_updated_at",
        users.id as "user_id?",
        users.name as "user_name?",
        users.avatar_url as "user_avatar_url?",
        users.github_id as "user_github_id?",
        users.updated_at as "user_updated_at?",
        users.created_at as "user_created_at?"
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      WHERE package_scope = $1 AND package_name = $2 AND package_version = $3 AND status != 'failure'
      LIMIT 1"#,
      task.package_scope as _,
      task.package_name as _,
      task.package_version as _
    ).map(|r| {
      let task = PublishingTask {
        id: r.task_id,
        status: r.task_status,
        error: r.task_error,
        package_scope: r.task_package_scope,
        package_name: r.task_package_name,
        package_version: r.task_package_version,
        config_file: r.task_config_file,
        user_id: r.task_user_id,
        created_at: r.task_created_at,
        updated_at: r.task_updated_at,
      };

      let user = task.user_id.map(|_| {
        UserPublic {
          id: r.user_id.unwrap(),
          name: r.user_name.unwrap(),
          avatar_url: r.user_avatar_url.unwrap(),
          github_id: r.user_github_id,
          updated_at: r.user_updated_at.unwrap(),
          created_at: r.user_created_at.unwrap(),
        }
      });

      (task, user)
    })

      .fetch_optional(&mut *tx)
      .await?;
    if let Some(already_processing) = already_processing {
      return Ok(CreatePublishingTaskResult::Exists(already_processing));
    }

    let task = sqlx::query!(
      r#"WITH task AS (
          INSERT INTO publishing_tasks (user_id, package_scope, package_name, package_version, config_file)
          VALUES ($1, $2, $3, $4, $5)
          RETURNING
            id,
            status,
            error,
            user_id,
            package_scope,
            package_name,
            package_version,
            config_file,
            created_at,
            updated_at
        )
        SELECT
          task.id as "task_id",
          task.status as "task_status: PublishingTaskStatus",
          task.error as "task_error: PublishingTaskError",
          task.user_id as "task_user_id",
          task.package_scope as "task_package_scope: ScopeName",
          task.package_name as "task_package_name: PackageName",
          task.package_version as "task_package_version: Version",
          task.config_file as "task_config_file: PackagePath",
          task.created_at as "task_created_at",
          task.updated_at as "task_updated_at",
        users.id as "user_id?",
        users.name as "user_name?",
        users.avatar_url as "user_avatar_url?",
        users.github_id as "user_github_id?",
        users.updated_at as "user_updated_at?",
        users.created_at as "user_created_at?"
        FROM task
        LEFT JOIN users ON task.user_id = users.id"#,
      task.user_id,
      task.package_scope as _,
      task.package_name as _,
      task.package_version as _,
      task.config_file as _,
    )
      .map(|r| {
        let task = PublishingTask {
          id: r.task_id,
          status: r.task_status,
          error: r.task_error,
          package_scope: r.task_package_scope,
          package_name: r.task_package_name,
          package_version: r.task_package_version,
          config_file: r.task_config_file,
          user_id: r.task_user_id,
          created_at: r.task_created_at,
          updated_at: r.task_updated_at,
        };

        let user = task.user_id.map(|_| {
          UserPublic {
            id: r.user_id.unwrap(),
            name: r.user_name.unwrap(),
            avatar_url: r.user_avatar_url.unwrap(),
            github_id: r.user_github_id,
            updated_at: r.user_updated_at.unwrap(),
            created_at: r.user_created_at.unwrap(),
          }
        });

        (task, user)
      })

      .fetch_one(&mut *tx)
      .await?;

    let publish_attempts_per_week_limit = sqlx::query!(
      r#"
      SELECT publish_attempts_per_week_limit FROM scopes WHERE scope = $1;
      "#,
      task.0.package_scope as _,
    )
    .map(|r| r.publish_attempts_per_week_limit)
    .fetch_one(&mut *tx)
    .await?;

    let publish_attempts_from_last_week = sqlx::query!(
      r#"
      SELECT COUNT(created_at) FROM publishing_tasks WHERE package_scope = $1 AND created_at > now() - '1 week'::interval;
      "#,
      task.0.package_scope as _,
    )
      .map(|r| {
        r.count.unwrap()
      })
      .fetch_one(&mut *tx)
      .await?;

    if publish_attempts_from_last_week > publish_attempts_per_week_limit as i64
    {
      tx.rollback().await?;
      return Ok(
        CreatePublishingTaskResult::WeeklyPublishAttemptsLimitExceeded(
          publish_attempts_per_week_limit,
        ),
      );
    }

    tx.commit().await?;

    Ok(CreatePublishingTaskResult::Created(task))
  }

  #[instrument(name = "Database::get_publishing_task", skip(self), err)]
  pub async fn get_publishing_task(
    &self,
    id: Uuid,
  ) -> Result<Option<(PublishingTask, Option<UserPublic>)>> {
    sqlx::query!(
      r#"SELECT
        publishing_tasks.id as "task_id",
        publishing_tasks.status as "task_status: PublishingTaskStatus",
        publishing_tasks.error as "task_error: PublishingTaskError",
        publishing_tasks.user_id as "task_user_id",
        publishing_tasks.package_scope as "task_package_scope: ScopeName",
        publishing_tasks.package_name as "task_package_name: PackageName",
        publishing_tasks.package_version as "task_package_version: Version",
        publishing_tasks.config_file as "task_config_file: PackagePath",
        publishing_tasks.created_at as "task_created_at",
        publishing_tasks.updated_at as "task_updated_at",
        users.id as "user_id?",
        users.name as "user_name?",
        users.avatar_url as "user_avatar_url?",
        users.github_id as "user_github_id?",
        users.updated_at as "user_updated_at?",
        users.created_at as "user_created_at?"
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      WHERE publishing_tasks.id = $1"#,
      id
    )
    .map(|r| {
      let task = PublishingTask {
        id: r.task_id,
        status: r.task_status,
        error: r.task_error,
        package_scope: r.task_package_scope,
        package_name: r.task_package_name,
        package_version: r.task_package_version,
        config_file: r.task_config_file,
        user_id: r.task_user_id,
        created_at: r.task_created_at,
        updated_at: r.task_updated_at,
      };

      let user = task.user_id.map(|_| UserPublic {
        id: r.user_id.unwrap(),
        name: r.user_name.unwrap(),
        avatar_url: r.user_avatar_url.unwrap(),
        github_id: r.user_github_id,
        updated_at: r.user_updated_at.unwrap(),
        created_at: r.user_created_at.unwrap(),
      });

      (task, user)
    })
    .fetch_optional(&self.pool)
    .await
  }

  #[allow(clippy::type_complexity)]
  #[instrument(name = "Database::list_publishing_tasks", skip(self), err)]
  pub async fn list_publishing_tasks(
    &self,
    start: i64,
    limit: i64,
    maybe_search_query: Option<&str>,
    maybe_sort: Option<&str>,
  ) -> Result<(usize, Vec<(PublishingTask, Option<UserPublic>)>)> {
    let mut tx = self.pool.begin().await?;

    let search = format!("%{}%", maybe_search_query.unwrap_or(""));
    let sort = sort_by!(maybe_sort => {
      @timestamps "updated_at", "created_at";
      "status" => "publishing_tasks.status",
      "user" => "users.name",
      "scope" => "publishing_tasks.package_scope",
      "name" => "publishing_tasks.package_name",
      "version" => "publishing_tasks.package_version",
      "updated_at" => "publishing_tasks.updated_at",
      "created_at" => "publishing_tasks.created_at",
    } || "publishing_tasks.created_at DESC");

    let publishing_tasks = sqlx::query(&format!(
      r#"SELECT
        publishing_tasks.id as "task_id",
        publishing_tasks.status as "task_status",
        publishing_tasks.error as "task_error",
        publishing_tasks.user_id as "task_user_id",
        publishing_tasks.package_scope as "task_package_scope",
        publishing_tasks.package_name as "task_package_name",
        publishing_tasks.package_version as "task_package_version",
        publishing_tasks.config_file as "task_config_file",
        publishing_tasks.created_at as "task_created_at",
        publishing_tasks.updated_at as "task_updated_at",
        users.id as "user_id",
        users.name as "user_name",
        users.avatar_url as "user_avatar_url",
        users.github_id as "user_github_id",
        users.updated_at as "user_updated_at",
        users.created_at as "user_created_at"
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      WHERE publishing_tasks.package_scope ILIKE $1
         OR publishing_tasks.package_name ILIKE $1
         OR publishing_tasks.package_version ILIKE $1
      ORDER BY {sort} OFFSET $2 LIMIT $3"#
    ))
    .bind(&search)
    .bind(start)
    .bind(limit)
    .try_map(|r| {
      let task = PublishingTask::from_row(&r)?;

      let user = if r.try_get::<Option<Uuid>, &str>("user_id")?.is_some() {
        Some(UserPublic::from_row(&r)?)
      } else {
        None
      };

      Ok((task, user))
    })
    .fetch_all(&mut *tx)
    .await?;

    let total_publishing_tasks = sqlx::query!(
      r#"SELECT COUNT(created_at) FROM publishing_tasks WHERE package_scope ILIKE $1 OR package_name ILIKE $1 OR package_version ILIKE $1;"#,
      search,
    )
      .map(|r| r.count.unwrap())
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((total_publishing_tasks as usize, publishing_tasks))
  }

  #[instrument(
    name = "Database::list_publishing_tasks_for_package",
    skip(self),
    err
  )]
  pub async fn list_publishing_tasks_for_package(
    &self,
    scope_name: &ScopeName,
    package_name: &PackageName,
  ) -> Result<Vec<(PublishingTask, Option<UserPublic>)>> {
    sqlx::query!(
      r#"SELECT
        publishing_tasks.id as "task_id",
        publishing_tasks.status as "task_status: PublishingTaskStatus",
        publishing_tasks.error as "task_error: PublishingTaskError",
        publishing_tasks.user_id as "task_user_id",
        publishing_tasks.package_scope as "task_package_scope: ScopeName",
        publishing_tasks.package_name as "task_package_name: PackageName",
        publishing_tasks.package_version as "task_package_version: Version",
        publishing_tasks.config_file as "task_config_file: PackagePath",
        publishing_tasks.created_at as "task_created_at",
        publishing_tasks.updated_at as "task_updated_at",
        users.id as "user_id?",
        users.name as "user_name?",
        users.avatar_url as "user_avatar_url?",
        users.github_id as "user_github_id?",
        users.updated_at as "user_updated_at?",
        users.created_at as "user_created_at?"
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      JOIN packages ON publishing_tasks.package_scope = packages.scope AND publishing_tasks.package_name = packages.name
      WHERE publishing_tasks.package_scope = $1 AND publishing_tasks.package_name = $2 AND publishing_tasks.created_at >= packages.created_at
      ORDER BY publishing_tasks.package_version DESC"#,
      scope_name as _,
      package_name as _,
    )
      .map(|r| {
        let task = PublishingTask {
          id: r.task_id,
          status: r.task_status,
          error: r.task_error,
          package_scope: r.task_package_scope,
          package_name: r.task_package_name,
          package_version: r.task_package_version,
          config_file: r.task_config_file,
          user_id: r.task_user_id,
          created_at: r.task_created_at,
          updated_at: r.task_updated_at,
        };

        let user = task.user_id.map(|_| {
          UserPublic {
            id: r.user_id.unwrap(),
            name: r.user_name.unwrap(),
            avatar_url: r.user_avatar_url.unwrap(),
            github_id: r.user_github_id,
            updated_at: r.user_updated_at.unwrap(),
            created_at: r.user_created_at.unwrap(),
          }
        });

        (task, user)
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::update_publishing_task_status",
    skip(self),
    err
  )]
  pub async fn update_publishing_task_status(
    &self,
    staff_id: Option<&Uuid>,
    id: Uuid,
    prev_status: PublishingTaskStatus,
    new_status: PublishingTaskStatus,
    new_error: Option<PublishingTaskError>,
  ) -> Result<PublishingTask> {
    assert_eq!(
      new_error.is_some(),
      new_status == PublishingTaskStatus::Failure,
      "error must be set if and only if status is failure"
    );

    let mut tx = self.pool.begin().await?;

    if let Some(staff_id) = staff_id {
      audit_log(
        &mut tx,
        staff_id,
        true,
        "requeue_publishing_task",
        json!({
          "id": id,
        }),
      )
      .await?;
    }

    let task = sqlx::query_as!(
      PublishingTask,
      r#"UPDATE publishing_tasks
      SET status = $1, error = $2
      WHERE id = $3 AND status = $4
      RETURNING id, status as "status: PublishingTaskStatus", error as "error: PublishingTaskError", user_id, package_scope as "package_scope: ScopeName", package_name as "package_name: PackageName", package_version as "package_version: Version", config_file as "config_file: PackagePath", created_at, updated_at"#,
      new_status as _,
      new_error as _,
      id,
      prev_status as _,
    )
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(task)
  }

  #[instrument(name = "Database::get_oauth_state", skip(self), err)]
  pub async fn get_oauth_state(
    &self,
    csrf_token: &str,
  ) -> Result<Option<OauthState>> {
    sqlx::query_as!(
      OauthState,
      "SELECT csrf_token, pkce_code_verifier, redirect_url, updated_at, created_at FROM oauth_states WHERE csrf_token = $1",
      csrf_token
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::insert_oauth_state", skip(
    self,
    new_oauth_state
  ), err, fields(oauth_state.csrf_token = %new_oauth_state.csrf_token, oauth_state.redirect_url = %new_oauth_state.redirect_url
  ))]
  pub async fn insert_oauth_state<'a>(
    &self,
    new_oauth_state: NewOauthState<'a>,
  ) -> Result<OauthState> {
    sqlx::query_as!(
      OauthState,
      "INSERT INTO oauth_states (csrf_token, pkce_code_verifier, redirect_url)
      VALUES ($1, $2, $3)
      RETURNING csrf_token, pkce_code_verifier, redirect_url, updated_at, created_at",
      new_oauth_state.csrf_token,
      new_oauth_state.pkce_code_verifier,
      new_oauth_state.redirect_url,
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::delete_oauth_state", skip(self), err)]
  pub async fn delete_oauth_state(
    &self,
    csrf_token: &str,
  ) -> Result<Option<OauthState>> {
    sqlx::query_as!(
      OauthState,
      "DELETE FROM oauth_states
      WHERE csrf_token = $1
      RETURNING csrf_token, pkce_code_verifier, redirect_url, updated_at, created_at",
      csrf_token
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::insert_github_identity", skip(
    self,
    new_github_identity
  ), err, fields(github_identity.github_id = new_github_identity.github_id))]
  pub async fn upsert_github_identity(
    &self,
    new_github_identity: NewGithubIdentity,
  ) -> Result<GithubIdentity> {
    sqlx::query_as!(
      GithubIdentity,
      "INSERT INTO github_identities (github_id, access_token, access_token_expires_at, refresh_token, refresh_token_expires_at) VALUES ($1, $2, $3, $4, $5)
      ON CONFLICT (github_id) DO
      UPDATE SET access_token = $2, access_token_expires_at = $3, refresh_token = $4, refresh_token_expires_at = $5
      RETURNING github_id, access_token, access_token_expires_at, refresh_token, refresh_token_expires_at, updated_at, created_at",
      new_github_identity.github_id,
      new_github_identity.access_token,
      new_github_identity.access_token_expires_at,
      new_github_identity.refresh_token,
      new_github_identity.refresh_token_expires_at,
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_github_identity", skip(self), err)]
  pub async fn get_github_identity(
    &self,
    github_id: i64,
  ) -> Result<GithubIdentity> {
    sqlx::query_as!(
      GithubIdentity,
      "SELECT github_id, access_token, access_token_expires_at, refresh_token, refresh_token_expires_at, updated_at, created_at
      FROM github_identities
      WHERE github_id = $1",
      github_id
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::insert_token",
    skip(self, new_token),
    err,
    fields(token.r#type = ?new_token.r#type)
  )]
  pub async fn insert_token(&self, new_token: NewToken) -> Result<Token> {
    sqlx::query_as!(
      Token,
      r#"INSERT INTO tokens (hash, user_id, type, description, expires_at, permissions)
      VALUES ($1, $2, $3, $4, $5, $6)
      RETURNING id, hash, user_id, type "type: _", description, expires_at, permissions "permissions: _", updated_at, created_at"#,
      new_token.hash,
      new_token.user_id,
      new_token.r#type as _,
      new_token.description,
      new_token.expires_at,
      new_token.permissions as _,
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_token_by_hash", skip(self), err)]
  pub async fn get_token_by_hash(&self, hash: &str) -> Result<Option<Token>> {
    sqlx::query_as!(Token, r#"SELECT id, hash, user_id, type "type: _", description, expires_at, permissions "permissions: _", updated_at, created_at FROM tokens WHERE hash = $1"#, hash)
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_token", skip(self), err)]
  pub async fn list_tokens(&self, user_id: Uuid) -> Result<Vec<Token>> {
    // list a user's tokens where the expiration date is at most 1 day in the past
    sqlx::query_as!(
      Token,
      r#"SELECT id, hash, user_id, type "type: _", description, expires_at, permissions "permissions: _", updated_at, created_at
      FROM tokens
      WHERE user_id = $1 AND (expires_at > now() - interval '1 day' OR expires_at IS NULL)
      ORDER BY expires_at DESC NULLS FIRST, created_at DESC
      "#,
      user_id
    )
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::delete_token", skip(self), err)]
  pub async fn delete_token(&self, user_id: Uuid, id: Uuid) -> Result<bool> {
    let res = sqlx::query!(
      r#"DELETE FROM tokens WHERE user_id = $1 ANd id = $2"#,
      user_id,
      id
    )
    .execute(&self.pool)
    .await?;
    Ok(res.rows_affected() > 0)
  }

  #[instrument(
    name = "Database::create_authorization",
    skip(self, new_authorization),
    err
  )]
  pub async fn create_authorization(
    &self,
    new_authorization: NewAuthorization<'_>,
  ) -> Result<Authorization> {
    sqlx::query_as!(
      Authorization,
      r#"INSERT INTO authorizations (exchange_token, code, challenge, permissions, expires_at)
      VALUES ($1, $2, $3, $4, $5)
      RETURNING exchange_token, code, challenge, permissions "permissions: _", approved, user_id, expires_at, created_at, updated_at"#,
      new_authorization.exchange_token,
      new_authorization.code,
      new_authorization.challenge,
      new_authorization.permissions as _,
      new_authorization.expires_at,
    )
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_authorization_by_code", skip(self), err)]
  pub async fn get_authorization_by_code(
    &self,
    code: &str,
  ) -> Result<Option<Authorization>> {
    sqlx::query_as!(
      Authorization,
      r#"SELECT exchange_token, code, challenge, permissions "permissions: _", approved, user_id, expires_at, created_at, updated_at
      FROM authorizations
      WHERE code = $1"#,
      code
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::get_authorization_by_exchange_token",
    skip(self, exchange_token),
    err
  )]
  pub async fn get_authorization_by_exchange_token_and_remove_if_complete(
    &self,
    exchange_token: &str,
  ) -> Result<Option<Authorization>> {
    let mut tx = self.pool.begin().await?;

    let maybe_authorization = sqlx::query_as!(
      Authorization,
      r#"DELETE FROM authorizations
      WHERE exchange_token = $1
      RETURNING exchange_token, code, challenge, permissions "permissions: _", approved, user_id, expires_at, created_at, updated_at"#,
      exchange_token
    )
      .fetch_optional(&mut *tx)
      .await?;

    if let Some(authorization) = &maybe_authorization {
      if authorization.user_id.is_some() {
        tx.commit().await?;
      }
    }

    Ok(maybe_authorization)
  }

  #[instrument(name = "Database::update_authorization", skip(self), err)]
  pub async fn update_authorization(
    &self,
    code: &str,
    approved: bool,
    user_id: Uuid,
  ) -> Result<bool> {
    let res = sqlx::query!(
      r#"UPDATE authorizations
      SET approved = $1, user_id = $2
      WHERE code = $3 AND approved IS NULL"#,
      approved,
      user_id,
      code
    )
    .execute(&self.pool)
    .await?;
    Ok(res.rows_affected() > 0)
  }

  #[instrument(
    name = "Database::list_package_version_dependencies",
    skip(self),
    err
  )]
  pub async fn list_package_version_dependencies(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<Vec<PackageVersionDependency>> {
    sqlx::query_as!(
      PackageVersionDependency,
      r#"SELECT package_scope as "package_scope: ScopeName", package_name as "package_name: PackageName", package_version as "package_version: Version", dependency_kind as "dependency_kind: DependencyKind", dependency_name, dependency_constraint, dependency_path, updated_at, created_at
      FROM package_version_dependencies
      WHERE package_scope = $1 AND package_name = $2 AND package_version = $3
      ORDER BY dependency_kind ASC, dependency_name ASC, dependency_constraint ASC, dependency_path ASC"#,
      scope as _,
      name as _,
      version as _
    )
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_package_dependents", skip(self), err)]
  pub async fn list_package_dependents(
    &self,
    kind: DependencyKind,
    name: &str,
    start: i64,
    limit: i64,
    versions_per_package_limit: i64,
  ) -> Result<(usize, Vec<Dependent>)> {
    let mut tx = self.pool.begin().await?;
    let dependents = sqlx::query_as!(
      Dependent,
      r#"
      SELECT
        package_scope as "scope: ScopeName",
        package_name as "name: PackageName",
        (ARRAY_AGG(DISTINCT package_version))[:$5] as "versions!: Vec<Version>",
        COUNT(DISTINCT package_version) as "total_versions!"
      FROM
        package_version_dependencies
      WHERE
        dependency_kind = $1 AND dependency_name = $2
      GROUP BY package_scope, package_name
      ORDER BY package_scope ASC, package_name ASC OFFSET $3 LIMIT $4;
      "#,
      kind as _,
      name,
      start,
      limit,
      versions_per_package_limit as i32,
    )
    .fetch_all(&mut *tx)
    .await?;

    let total_unique_package_dependents = sqlx::query!(
      r#"SELECT COUNT(DISTINCT (package_scope, package_name)) FROM package_version_dependencies
      WHERE dependency_kind = $1 AND dependency_name = $2;"#,
      kind as _,
      name,
    )
      .map(|r| r.count.unwrap())
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((total_unique_package_dependents as usize, dependents))
  }

  #[instrument(name = "Database::count_package_dependents", skip(self), err)]
  pub async fn count_package_dependents(
    &self,
    kind: DependencyKind,
    name: &str,
  ) -> Result<usize> {
    let total_unique_package_dependents = sqlx::query!(
      r#"SELECT COUNT(DISTINCT (package_scope, package_name)) FROM package_version_dependencies
      WHERE dependency_kind = $1 AND dependency_name = $2;"#,
      kind as _,
      name,
    )
      .map(|r| r.count.unwrap())
      .fetch_one(&self.pool)
      .await?;

    Ok(total_unique_package_dependents as usize)
  }

  #[instrument(name = "Database::count_package_dependencies", skip(self), err)]
  pub async fn count_package_dependencies(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<usize> {
    let total_package_dependencies = sqlx::query!(
      r#"SELECT COUNT(DISTINCT dependency_name)
      FROM package_version_dependencies
      WHERE package_scope = $1 AND package_name = $2 AND package_version = $3"#,
      scope as _,
      name as _,
      version as _
    )
    .fetch_one(&self.pool)
    .await?
    .count
    .unwrap();

    Ok(total_package_dependencies as usize)
  }

  #[instrument(name = "Database::check_bad_word", skip(self), err)]
  pub async fn check_is_bad_word(&self, word: &str) -> Result<bool> {
    let res = sqlx::query!("SELECT * FROM bad_words WHERE word = $1", word)
      .fetch_optional(&self.pool)
      .await?;
    Ok(res.is_some())
  }

  #[instrument(name = "Database::add_bad_word_for_test", skip(self), err)]
  pub async fn add_bad_word_for_test(&self, word: &str) -> Result<()> {
    sqlx::query!("INSERT INTO bad_words (word) VALUES ($1)", word)
      .execute(&self.pool)
      .await?;

    Ok(())
  }

  #[instrument(
    name = "Database::get_latest_npm_tarball_for_version",
    skip(self),
    err
  )]
  pub async fn get_latest_npm_tarball_for_version(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<Option<NpmTarball>> {
    sqlx::query_as!(
      NpmTarball,
      r#"SELECT scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", revision, sha1, sha512, size, updated_at, created_at
      FROM npm_tarballs
      WHERE scope = $1 AND name = $2 AND version = $3
      ORDER BY revision DESC
      LIMIT 1"#,
      scope as _,
      name as _,
      version as _
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::get_latest_npm_tarball_for_version",
    skip(self),
    err
  )]
  pub async fn get_npm_tarball(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
    revision: i32,
  ) -> Result<Option<NpmTarball>> {
    sqlx::query_as!(
      NpmTarball,
      r#"SELECT scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version", revision, sha1, sha512, size, updated_at, created_at
      FROM npm_tarballs
      WHERE scope = $1 AND name = $2 AND version = $3 AND revision = $4
      LIMIT 1"#,
      scope as _,
      name as _,
      version as _,
      revision,
    )
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_missing_npm_tarballs", skip(self), err)]
  pub async fn list_missing_npm_tarballs(
    &self,
    current_revision: i32,
  ) -> Result<Vec<(ScopeName, PackageName, Version)>> {
    // List all package versions (scope, name, version) that do not have a npm_tarball with the current_revision revision
    sqlx::query!(
      r#"SELECT scope as "scope: ScopeName", name as "name: PackageName", version as "version: Version"
      FROM package_versions
      WHERE NOT EXISTS (
        SELECT 1
        FROM npm_tarballs
        WHERE npm_tarballs.scope = package_versions.scope AND npm_tarballs.name = package_versions.name AND npm_tarballs.version = package_versions.version AND npm_tarballs.revision = $1
      )
      ORDER BY created_at ASC
      LIMIT 1000
      "#,
      current_revision,
    )
      .map(|r| {
        (r.scope, r.name, r.version)
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_all_scopes_for_sitemap", skip(self), err)]
  #[allow(clippy::type_complexity)]
  pub async fn list_all_scopes_for_sitemap(
    &self,
  ) -> Result<Vec<(ScopeName, DateTime<Utc>, Option<DateTime<Utc>>)>> {
    sqlx::query!(
      r#"
        SELECT
          scope as "scope: ScopeName",
          updated_at,
          (SELECT updated_at FROM packages WHERE scope = scope ORDER BY updated_At DESC LIMIT 1) as "latest_package_created_at"
        FROM scopes
        ORDER BY scope ASC
        LIMIT 50000
      "#
    )
      .map(|r| (r.scope, r.updated_at, r.latest_package_created_at))
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::list_all_packages_for_sitemap",
    skip(self),
    err
  )]
  #[allow(clippy::type_complexity)]
  pub async fn list_all_packages_for_sitemap(
    &self,
  ) -> Result<Vec<(ScopeName, PackageName, DateTime<Utc>, DateTime<Utc>)>> {
    sqlx::query!(
      r#"SELECT
        scope as "scope: ScopeName", name as "name: PackageName", updated_at,
        (SELECT created_at FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version_updated_at!"
      FROM packages
      WHERE (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) IS NOT NULL
      ORDER BY scope ASC, name ASC
      LIMIT 50000"#
    )
      .map(|r| {
        (
          r.scope,
          r.name,
          r.updated_at,
          r.latest_version_updated_at,
        )
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::insert_download_entries",
    skip(self, entries),
    err
  )]
  pub async fn insert_download_entries(
    &self,
    entries: Vec<VersionDownloadCount>,
  ) -> Result<()> {
    if entries.is_empty() {
      return Ok(());
    }

    let mut tx = self.pool.begin().await?;

    let mut scopes = Vec::with_capacity(entries.len());
    let mut packages = Vec::with_capacity(entries.len());
    let mut versions = Vec::with_capacity(entries.len());
    let mut time_buckets = Vec::with_capacity(entries.len());
    let mut kinds = Vec::with_capacity(entries.len());
    let mut counts = Vec::with_capacity(entries.len());

    let mut smallest_time_bucket = Utc::now();
    let mut largest_time_bucket = DateTime::from_timestamp_nanos(0);

    for entry in entries {
      scopes.push(entry.scope);
      packages.push(entry.package);
      versions.push(entry.version);
      time_buckets.push(entry.time_bucket);
      kinds.push(entry.kind);
      counts.push(entry.count);

      if entry.time_bucket < smallest_time_bucket {
        smallest_time_bucket = entry.time_bucket;
      }
      if entry.time_bucket > largest_time_bucket {
        largest_time_bucket = entry.time_bucket;
      }
    }

    // Upsert data into version_download_counts_4h
    sqlx::query!(
      r#"
      INSERT INTO version_download_counts_4h (scope, package, version, time_bucket, kind, count)
      SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TIMESTAMPTZ[], $5::download_kind[], $6::INT[]) as temp(scope, package, version, time_bucket, kind, count)
      WHERE (SELECT COUNT(*) FROM package_versions WHERE package_versions.scope = temp.scope AND package_versions.name = temp.package AND version = temp.version) > 0
      ON CONFLICT (scope, package, version, time_bucket, kind) DO UPDATE SET count = EXCLUDED.count
      "#,
      &scopes as _,
      &packages as _,
      &versions as _,
      &time_buckets,
      &kinds as _,
      &counts as _,
    )
      .execute(&mut *tx)
      .await?;

    // Compute data in version_download_counts_24h from version_download_counts_4h between smallest_timestamp and largest_timestamp.
    // smallest_timestamp must be truncated down to the nearest day and largest_timestamp must be truncated up to the nearest day.
    sqlx::query!(
      r#"
      INSERT INTO version_download_counts_24h (scope, package, version, time_bucket, kind, count)
      SELECT scope, package, version, date_trunc('day', time_bucket), kind, SUM(count)
      FROM version_download_counts_4h
      WHERE time_bucket >= date_trunc('day', $1::timestamptz) AND time_bucket < date_trunc('day', $2::timestamptz) + interval '1 day'
      GROUP BY scope, package, version, date_trunc('day', time_bucket), kind
      ON CONFLICT (scope, package, version, time_bucket, kind) DO UPDATE SET count = EXCLUDED.count
      "#,
      smallest_time_bucket,
      largest_time_bucket,
    )
      .execute(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(())
  }

  #[instrument(
    name = "Database::get_package_version_downloads_4h",
    skip(self),
    err
  )]
  pub async fn get_package_version_downloads_4h(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
  ) -> Result<Vec<DownloadDataPoint>> {
    sqlx::query_as!(
      DownloadDataPoint,
      r#"
      SELECT time_bucket, kind as "kind: DownloadKind", count
      FROM version_download_counts_4h
      WHERE scope = $1 AND package = $2 AND version = $3 AND time_bucket >= $4 AND time_bucket < $5
      ORDER BY time_bucket ASC
      "#,
      scope as _,
      name as _,
      version as _,
      start,
      end,
    )
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::get_package_version_downloads_24h",
    skip(self),
    err
  )]
  pub async fn get_package_version_downloads_24h(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
  ) -> Result<Vec<DownloadDataPoint>> {
    sqlx::query_as!(
      DownloadDataPoint,
      r#"
      SELECT time_bucket, kind as "kind: DownloadKind", count
      FROM version_download_counts_24h
      WHERE scope = $1 AND package = $2 AND version = $3 AND time_bucket >= $4 AND time_bucket < $5
      ORDER BY time_bucket ASC
      "#,
      scope as _,
      name as _,
      version as _,
      start,
      end,
    )
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_package_downloads_24h", skip(self), err)]
  pub async fn get_package_downloads_24h(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
  ) -> Result<Vec<DownloadDataPoint>> {
    sqlx::query_as!(
      DownloadDataPoint,
      r#"
    SELECT time_bucket, kind as "kind: DownloadKind", SUM(count) as "count!"
    FROM version_download_counts_24h
    WHERE scope = $1 AND package = $2 AND time_bucket >= $3 AND time_bucket < $4
    GROUP BY time_bucket, kind
    ORDER BY time_bucket ASC
    "#,
      scope as _,
      name as _,
      start,
      end,
    )
    .fetch_all(&self.pool)
    .await
  }

  #[instrument(name = "Database::create_ticket", skip(self), err)]
  pub async fn create_ticket(
    &self,
    user_id: Uuid,
    new_ticket: NewTicket,
  ) -> Result<(Ticket, User, TicketMessage)> {
    let mut tx = self.pool.begin().await?;

    let (ticket, user) = sqlx::query!(
      r#"WITH ticket AS (
          INSERT INTO tickets (kind, creator, meta)
          VALUES ($1, $2, $3)
          RETURNING id, kind, creator, meta, closed, updated_at, created_at
        )
        SELECT
            ticket.id as "ticket_id",
            ticket.kind as "ticket_kind: TicketKind",
            ticket.creator as "ticket_creator",
            ticket.meta as "ticket_meta",
            ticket.closed as "ticket_closed",
            ticket.updated_at as "ticket_updated_at",
            ticket.created_at as "ticket_created_at",
            users.id as "user_id",
            users.name as "user_name",
            users.email as "user_email",
            users.avatar_url as "user_avatar_url",
            users.github_id as "user_github_id",
            users.is_blocked as "user_is_blocked",
            users.is_staff as "user_is_staff",
            users.scope_limit as "user_scope_limit",
            users.updated_at as "user_updated_at",
            users.created_at as "user_created_at",
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
            ) END) as "user_newer_ticket_messages_count!"
        FROM ticket
        INNER JOIN users ON users.id = ticket.creator
    "#,
      new_ticket.kind as _,
      user_id as _,
      new_ticket.meta as _,
    )
      .map(|r| {
        let ticket = Ticket {
          id: r.ticket_id,
          kind: r.ticket_kind,
          creator: r.ticket_creator,
          meta: r.ticket_meta,
          closed: r.ticket_closed,
          updated_at: r.ticket_updated_at,
          created_at: r.ticket_created_at,
        };

        let user = User {
          id: r.user_id,
          name: r.user_name,
          email: r.user_email,
          avatar_url: r.user_avatar_url,
          github_id: r.user_github_id,
          is_blocked: r.user_is_blocked,
          is_staff: r.user_is_staff,
          scope_usage: r.user_scope_usage,
          scope_limit: r.user_scope_limit,
          invite_count: r.user_invite_count,
          newer_ticket_messages_count: r.user_newer_ticket_messages_count,
          updated_at: r.user_updated_at,
          created_at: r.user_created_at,
        };

        (ticket, user)
      })
      .fetch_one(&mut *tx)
      .await?;

    let message = sqlx::query!(
      r#"INSERT INTO ticket_messages (ticket_id, author, message)
          VALUES ($1, $2, $3)
          RETURNING ticket_id, author, message, updated_at, created_at
    "#,
      ticket.id as _,
      user_id as _,
      new_ticket.message as _,
    )
    .map(|r| TicketMessage {
      ticket_id: r.ticket_id,
      author: r.author,
      message: r.message,
      updated_at: r.updated_at,
      created_at: r.created_at,
    })
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((ticket, user, message))
  }

  #[instrument(name = "Database::list_tickets", skip(self), err)]
  pub async fn list_tickets(
    &self,
    start: i64,
    limit: i64,
    maybe_search_query: Option<&str>,
    maybe_sort: Option<&str>,
  ) -> Result<(usize, Vec<FullTicket>)> {
    let mut tx = self.pool.begin().await?;

    let search = format!("%{}%", maybe_search_query.unwrap_or(""));
    let sort = sort_by!(maybe_sort => {
      @timestamps "updated_at", "created_at";
      "kind" => "tickets.kind",
      "creator" => "users.name",
      "closed" => "tickets.closed",
      "updated_at" => "tickets.updated_at",
      "created_at" => "tickets.created_at",
    } || "tickets.closed ASC, tickets.created_at DESC");

    let tickets = sqlx::query(
      &format!(r#"SELECT
        tickets.id as "ticket_id",
        tickets.kind as "ticket_kind",
        tickets.creator as "ticket_creator",
        tickets.meta as "ticket_meta",
        tickets.closed as "ticket_closed",
        tickets.updated_at as "ticket_updated_at",
        tickets.created_at as "ticket_created_at",
        users.id as "user_id",
        users.name as "user_name",
        users.email as "user_email",
        users.avatar_url as "user_avatar_url",
        users.github_id as "user_github_id",
        users.is_blocked as "user_is_blocked",
        users.is_staff as "user_is_staff",
        users.scope_limit as "user_scope_limit",
        users.updated_at as "user_updated_at",
        users.created_at as "user_created_at",
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
        ) END) as "user_newer_ticket_messages_count"
    FROM tickets
    INNER JOIN users ON users.id = tickets.creator
    WHERE users.name ILIKE $1
       OR EXISTS (
         SELECT 1
         FROM ticket_messages
         WHERE ticket_messages.ticket_id = tickets.id
           AND ticket_messages.message ILIKE $1
       )
    ORDER BY {sort} OFFSET $2 LIMIT $3
"#))
      .bind(&search)
      .bind(start)
      .bind(limit)
      .try_map(|r| {
        let ticket = Ticket::from_row(&r)?;
        let user = User::from_row(&r)?;

        Ok((ticket, user))
      })
      .fetch_all(&mut *tx)
      .await?;

    let mut out = Vec::with_capacity(tickets.len());
    for (ticket, user) in tickets {
      let messages = sqlx::query!(
      r#"SELECT
            ticket_messages.ticket_id as "message_ticket_id",
            ticket_messages.author as "message_author",
            ticket_messages.message as "message_message",
            ticket_messages.updated_at as "message_updated_at",
            ticket_messages.created_at as "message_created_at",
            users.id as "user_id",
            users.name as "user_name",
            users.avatar_url as "user_avatar_url",
            users.github_id as "user_github_id",
            users.updated_at as "user_updated_at",
            users.created_at as "user_created_at"
        FROM ticket_messages
        LEFT JOIN users ON users.id = ticket_messages.author
        WHERE ticket_messages.ticket_id = $1 ORDER BY ticket_messages.created_at"#,
      ticket.id as _,
    )
        .map(|r| {
          let message = TicketMessage {
            ticket_id: r.message_ticket_id,
            author: r.message_author,
            message: r.message_message,
            updated_at: r.message_updated_at,
            created_at: r.message_created_at,
          };

          let user = UserPublic {
            id: r.user_id,
            name: r.user_name,
            avatar_url: r.user_avatar_url,
            github_id: r.user_github_id,
            updated_at: r.user_updated_at,
            created_at: r.user_created_at,
          };

          (message, user)
        })
        .fetch_all(&mut *tx)
        .await?;

      out.push((ticket, user, messages));
    }

    let total_tickets = sqlx::query!(
      r#"SELECT COUNT(tickets.created_at) FROM tickets
    LEFT JOIN users ON users.id = tickets.creator
    WHERE users.name ILIKE $1
       OR EXISTS (
         SELECT 1
         FROM ticket_messages
         WHERE ticket_messages.ticket_id = tickets.id
           AND ticket_messages.message ILIKE $1
       )"#,
      search,
    )
    .map(|r| r.count.unwrap())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((total_tickets as usize, out))
  }

  #[instrument(name = "Database::list_tickets_for_user", skip(self), err)]
  pub async fn list_tickets_for_user(
    &self,
    user_id: Uuid,
  ) -> Result<Vec<FullTicket>> {
    let mut tx = self.pool.begin().await?;

    let tickets = sqlx::query!(
      r#"SELECT
        tickets.id as "ticket_id",
        tickets.kind as "ticket_kind: TicketKind",
        tickets.creator as "ticket_creator",
        tickets.meta as "ticket_meta",
        tickets.closed as "ticket_closed",
        tickets.updated_at as "ticket_updated_at",
        tickets.created_at as "ticket_created_at",
        users.id as "user_id",
        users.name as "user_name",
        users.email as "user_email",
        users.avatar_url as "user_avatar_url",
        users.github_id as "user_github_id",
        users.is_blocked as "user_is_blocked",
        users.is_staff as "user_is_staff",
        users.scope_limit as "user_scope_limit",
        users.updated_at as "user_updated_at",
        users.created_at as "user_created_at",
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
        ) END) as "user_newer_ticket_messages_count!"
    FROM tickets
    INNER JOIN users ON users.id = tickets.creator
    WHERE tickets.creator = $1
    ORDER BY tickets.closed ASC, tickets.created_at DESC
"#,
      user_id as _,
    )
      .map(|r| {
        let ticket = Ticket {
          id: r.ticket_id,
          kind: r.ticket_kind,
          creator: r.ticket_creator,
          meta: r.ticket_meta,
          closed: r.ticket_closed,
          updated_at: r.ticket_updated_at,
          created_at: r.ticket_created_at,
        };

        let user = User {
          id: r.user_id,
          name: r.user_name,
          email: r.user_email,
          avatar_url: r.user_avatar_url,
          github_id: r.user_github_id,
          is_blocked: r.user_is_blocked,
          is_staff: r.user_is_staff,
          scope_usage: r.user_scope_usage,
          scope_limit: r.user_scope_limit,
          invite_count: r.user_invite_count,
          newer_ticket_messages_count: r.user_newer_ticket_messages_count,
          updated_at: r.user_updated_at,
          created_at: r.user_created_at,
        };

        (ticket, user)
      })
      .fetch_all(&mut *tx)
      .await?;

    let mut out = Vec::with_capacity(tickets.len());
    for (ticket, user) in tickets {
      let messages = sqlx::query!(
      r#"SELECT
            ticket_messages.ticket_id as "message_ticket_id",
            ticket_messages.author as "message_author",
            ticket_messages.message as "message_message",
            ticket_messages.updated_at as "message_updated_at",
            ticket_messages.created_at as "message_created_at",
            users.id as "user_id",
            users.name as "user_name",
            users.avatar_url as "user_avatar_url",
            users.github_id as "user_github_id",
            users.updated_at as "user_updated_at",
            users.created_at as "user_created_at"
        FROM ticket_messages
        INNER JOIN users ON users.id = ticket_messages.author
        WHERE ticket_messages.ticket_id = $1 ORDER BY ticket_messages.created_at"#,
      ticket.id as _,
    )
        .map(|r| {
          let message = TicketMessage {
            ticket_id: r.message_ticket_id,
            author: r.message_author,
            message: r.message_message,
            updated_at: r.message_updated_at,
            created_at: r.message_created_at,
          };

          let user = UserPublic {
            id: r.user_id,
            name: r.user_name,
            avatar_url: r.user_avatar_url,
            github_id: r.user_github_id,
            updated_at: r.user_updated_at,
            created_at: r.user_created_at,
          };

          (message, user)
        })
        .fetch_all(&mut *tx)
        .await?;

      out.push((ticket, user, messages));
    }

    tx.commit().await?;

    Ok(out)
  }

  #[instrument(name = "Database::get_ticket", skip(self), err)]
  pub async fn get_ticket(
    &self,
    ticket_id: Uuid,
  ) -> Result<Option<FullTicket>> {
    let mut tx = self.pool.begin().await?;

    let Some((ticket, user)) = sqlx::query!(
      r#"SELECT
            tickets.id as "ticket_id",
            tickets.kind as "ticket_kind: TicketKind",
            tickets.creator as "ticket_creator",
            tickets.meta as "ticket_meta",
            tickets.closed as "ticket_closed",
            tickets.updated_at as "ticket_updated_at",
            tickets.created_at as "ticket_created_at",
            users.id as "user_id",
            users.name as "user_name",
            users.email as "user_email",
            users.avatar_url as "user_avatar_url",
            users.github_id as "user_github_id",
            users.is_blocked as "user_is_blocked",
            users.is_staff as "user_is_staff",
            users.scope_limit as "user_scope_limit",
            users.updated_at as "user_updated_at",
            users.created_at as "user_created_at",
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
            ) END) as "user_newer_ticket_messages_count!"
        FROM tickets
        INNER JOIN users ON users.id = tickets.creator
        WHERE tickets.id = $1"#,
      ticket_id as _,
    )
      .map(|r| {
        let ticket = Ticket {
          id: r.ticket_id,
          kind: r.ticket_kind,
          creator: r.ticket_creator,
          meta: r.ticket_meta,
          closed: r.ticket_closed,
          updated_at: r.ticket_updated_at,
          created_at: r.ticket_created_at,
        };

        let user = User {
          id: r.user_id,
          name: r.user_name,
          email: r.user_email,
          avatar_url: r.user_avatar_url,
          github_id: r.user_github_id,
          is_blocked: r.user_is_blocked,
          is_staff: r.user_is_staff,
          scope_usage: r.user_scope_usage,
          scope_limit: r.user_scope_limit,
          invite_count: r.user_invite_count,
          newer_ticket_messages_count: r.user_newer_ticket_messages_count,
          updated_at: r.user_updated_at,
          created_at: r.user_created_at,
        };

        (ticket, user)
      })
      .fetch_optional(&mut *tx)
      .await?
    else {
      return Ok(None);
    };

    let messages = sqlx::query!(
      r#"SELECT
            ticket_messages.ticket_id as "message_ticket_id",
            ticket_messages.author as "message_author",
            ticket_messages.message as "message_message",
            ticket_messages.updated_at as "message_updated_at",
            ticket_messages.created_at as "message_created_at",
            users.id as "user_id",
            users.name as "user_name",
            users.avatar_url as "user_avatar_url",
            users.github_id as "user_github_id",
            users.updated_at as "user_updated_at",
            users.created_at as "user_created_at"
        FROM ticket_messages
        INNER JOIN users ON users.id = ticket_messages.author
        WHERE ticket_messages.ticket_id = $1 ORDER BY ticket_messages.created_at"#,
      ticket_id as _,
    )
      .map(|r| {
        let message = TicketMessage {
          ticket_id: r.message_ticket_id,
          author: r.message_author,
          message: r.message_message,
          updated_at: r.message_updated_at,
          created_at: r.message_created_at,
        };

        let user = UserPublic {
          id: r.user_id,
          name: r.user_name,
          avatar_url: r.user_avatar_url,
          github_id: r.user_github_id,
          updated_at: r.user_updated_at,
          created_at: r.user_created_at,
        };

        (message, user)
      })
      .fetch_all(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok(Some((ticket, user, messages)))
  }

  #[instrument(name = "Database::ticket_add_message", skip(self), err)]
  pub async fn ticket_add_message(
    &self,
    id: Uuid,
    author: Uuid,
    message: NewTicketMessage,
  ) -> Result<(TicketMessage, UserPublic)> {
    let mut tx = self.pool.begin().await?;

    let message = sqlx::query!(
      r#"WITH message AS (
          INSERT INTO ticket_messages (ticket_id, author, message)
          VALUES ($1, $2, $3)
          RETURNING ticket_id, author, message, updated_at, created_at
        )
        SELECT
            message.ticket_id as "message_ticket_id",
            message.author as "message_author",
            message.message as "message_message",
            message.updated_at as "message_updated_at",
            message.created_at as "message_created_at",
            users.id as "user_id",
            users.name as "user_name",
            users.avatar_url as "user_avatar_url",
            users.github_id as "user_github_id",
            users.updated_at as "user_updated_at",
            users.created_at as "user_created_at"
        FROM message
        INNER JOIN users ON users.id = message.author
    "#,
      id as _,
      author as _,
      message.message as _,
    )
    .map(|r| {
      let message = TicketMessage {
        ticket_id: r.message_ticket_id,
        author: r.message_author,
        message: r.message_message,
        updated_at: r.message_updated_at,
        created_at: r.message_created_at,
      };

      let user = UserPublic {
        id: r.user_id,
        name: r.user_name,
        avatar_url: r.user_avatar_url,
        github_id: r.user_github_id,
        updated_at: r.user_updated_at,
        created_at: r.user_created_at,
      };

      (message, user)
    })
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
      r#"UPDATE tickets SET updated_at = now() WHERE id = $1"#,
      id as _
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(message)
  }

  #[instrument(name = "Database::update_ticket", skip(self), err)]
  pub async fn update_ticket_closed(
    &self,
    staff_id: &Uuid,
    ticket_id: Uuid,
    closed: bool,
  ) -> Result<FullTicket> {
    let mut tx = self.pool.begin().await?;

    audit_log(
      &mut tx,
      staff_id,
      true,
      "update_ticket_status",
      json!({
      "ticket_id": ticket_id,
      "closed": closed,
      }),
    )
    .await?;

    let (ticket, user) = sqlx::query!(
      r#"WITH ticket AS (
          UPDATE tickets SET closed = $1 WHERE id = $2
          RETURNING id, kind, creator, meta, closed, updated_at, created_at
        )
        SELECT
            ticket.id as "ticket_id",
            ticket.kind as "ticket_kind: TicketKind",
            ticket.creator as "ticket_creator",
            ticket.meta as "ticket_meta",
            ticket.closed as "ticket_closed",
            ticket.updated_at as "ticket_updated_at",
            ticket.created_at as "ticket_created_at",
            users.id as "user_id",
            users.name as "user_name",
            users.email as "user_email",
            users.avatar_url as "user_avatar_url",
            users.github_id as "user_github_id",
            users.is_blocked as "user_is_blocked",
            users.is_staff as "user_is_staff",
            users.scope_limit as "user_scope_limit",
            users.updated_at as "user_updated_at",
            users.created_at as "user_created_at",
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
            ) END) as "user_newer_ticket_messages_count!"
        FROM ticket
        INNER JOIN users ON users.id = ticket.creator
    "#,
      closed as _,
      ticket_id as _,
    )
      .map(|r| {
        let ticket = Ticket {
          id: r.ticket_id,
          kind: r.ticket_kind,
          creator: r.ticket_creator,
          meta: r.ticket_meta,
          closed: r.ticket_closed,
          updated_at: r.ticket_updated_at,
          created_at: r.ticket_created_at,
        };

        let user = User {
          id: r.user_id,
          name: r.user_name,
          email: r.user_email,
          avatar_url: r.user_avatar_url,
          github_id: r.user_github_id,
          is_blocked: r.user_is_blocked,
          is_staff: r.user_is_staff,
          scope_usage: r.user_scope_usage,
          scope_limit: r.user_scope_limit,
          invite_count: r.user_invite_count,
          newer_ticket_messages_count: r.user_newer_ticket_messages_count,
          updated_at: r.user_updated_at,
          created_at: r.user_created_at,
        };

        (ticket, user)
      })
      .fetch_one(&mut *tx)
      .await?;

    let messages = sqlx::query!(
      r#"SELECT
            ticket_messages.ticket_id as "message_ticket_id",
            ticket_messages.author as "message_author",
            ticket_messages.message as "message_message",
            ticket_messages.updated_at as "message_updated_at",
            ticket_messages.created_at as "message_created_at",
            users.id as "user_id",
            users.name as "user_name",
            users.avatar_url as "user_avatar_url",
            users.github_id as "user_github_id",
            users.updated_at as "user_updated_at",
            users.created_at as "user_created_at"
        FROM ticket_messages
        INNER JOIN users ON users.id = ticket_messages.author
        WHERE ticket_messages.ticket_id = $1 ORDER BY ticket_messages.created_at"#,
      ticket_id as _,
    )
      .map(|r| {
        let message = TicketMessage {
          ticket_id: r.message_ticket_id,
          author: r.message_author,
          message: r.message_message,
          updated_at: r.message_updated_at,
          created_at: r.message_created_at,
        };

        let user = UserPublic {
          id: r.user_id,
          name: r.user_name,
          avatar_url: r.user_avatar_url,
          github_id: r.user_github_id,
          updated_at: r.user_updated_at,
          created_at: r.user_created_at,
        };

        (message, user)
      })
      .fetch_all(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((ticket, user, messages))
  }

  #[allow(clippy::type_complexity)]
  #[instrument(name = "Database::list_audit_logs", skip(self), err)]
  pub async fn list_audit_logs(
    &self,
    start: i64,
    limit: i64,
    maybe_search_query: Option<&str>,
    maybe_sort: Option<&str>,
    sudo_only: bool,
  ) -> Result<(usize, Vec<(AuditLog, UserPublic)>)> {
    let mut tx = self.pool.begin().await?;

    let search = format!("%{}%", maybe_search_query.unwrap_or(""));
    let sort = sort_by!(maybe_sort => {
      @timestamps "created_at";
      "action" => "audit_logs.action",
      "user" => "users.name",
      "created_at" => "audit_logs.created_at",
    } || "audit_logs.created_at DESC");

    let scopes = sqlx::query(
      &format!(r#"SELECT
      audit_logs.actor_id as "audit_log_actor_id",
      audit_logs.is_sudo as "audit_log_is_sudo",
      audit_logs.action as "audit_log_action",
      audit_logs.meta as "audit_log_meta",
      audit_logs.created_at as "audit_log_created_at",
      users.id as "user_id", users.name as "user_name", users.avatar_url as "user_avatar_url", users.github_id as "user_github_id", users.updated_at as "user_updated_at", users.created_at as "user_created_at"
      FROM audit_logs
      LEFT JOIN users ON audit_logs.actor_id = users.id
      WHERE (audit_logs.action ILIKE $1
         OR users.name ILIKE $1
         OR audit_logs.meta::text ILIKE $1)
         AND ($2 IS NOT TRUE OR audit_logs.is_sudo = TRUE)
      ORDER BY {sort} OFFSET $3 LIMIT $4
      "#))
      .bind(&search)
      .bind(sudo_only)
      .bind(start)
      .bind(limit)
      .try_map(|r| {
        let audit_log = AuditLog::from_row(&r)?;
        let user = UserPublic::from_row(&r)?;

        Ok((audit_log, user))
      })
      .fetch_all(&mut *tx)
      .await?;

    let total_scopes = sqlx::query!(
      r#"SELECT COUNT(audit_logs.created_at) FROM audit_logs LEFT JOIN users ON audit_logs.actor_id = users.id WHERE audit_logs.action ILIKE $1 OR users.name ILIKE $2 AND ($3 IS NOT TRUE OR audit_logs.is_sudo = TRUE);"#,
      search,
      search,
      sudo_only,
    )
      .map(|r| r.count.unwrap())
      .fetch_one(&mut *tx)
      .await?;

    tx.commit().await?;

    Ok((total_scopes as usize, scopes))
  }
}

async fn finalize_package_creation(
  mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
  scope: &ScopeName,
) -> Result<Option<CreatePackageResult>, sqlx::Error> {
  let (package_limit, new_package_per_week_limit) = sqlx::query!(
    r#"
    SELECT package_limit, new_package_per_week_limit FROM scopes WHERE scope = $1;
    "#,
    scope as _,
  )
    .map(|r| {
      (r.package_limit, r.new_package_per_week_limit)
    })
    .fetch_one(&mut *tx)
    .await?;

  let packages_from_last_week = sqlx::query!(
    r#"
    SELECT COUNT(created_at) FROM packages WHERE scope = $1 AND created_at > now() - '1 week'::interval;
    "#,
    scope as _,
  )
    .map(|r| {
      r.count.unwrap()
    })
    .fetch_one(&mut *tx)
    .await?;

  if packages_from_last_week > new_package_per_week_limit as i64 {
    tx.rollback().await?;
    return Ok(Some(CreatePackageResult::WeeklyPackageLimitExceeded(
      new_package_per_week_limit,
    )));
  }

  let total_packages = sqlx::query!(
    r#"
    SELECT COUNT(created_at) FROM packages WHERE scope = $1;
    "#,
    scope as _,
  )
  .map(|r| r.count.unwrap())
  .fetch_one(&mut *tx)
  .await?;

  if total_packages > package_limit as i64 {
    tx.rollback().await?;
    return Ok(Some(CreatePackageResult::PackageLimitExceeded(
      package_limit,
    )));
  }

  tx.commit().await?;
  Ok(None)
}

async fn audit_log(
  tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  actor_id: &Uuid,
  is_sudo: bool,
  action: &'static str,
  data: serde_json::Value,
) -> Result<()> {
  sqlx::query!(
    r#"INSERT INTO audit_logs (actor_id, is_sudo, action, meta) VALUES ($1, $2, $3, $4)"#,
    actor_id as _,
    is_sudo,
    action,
    data as _,
  )
    .execute(&mut **tx)
    .await?;

  Ok(())
}

#[derive(Debug)]
pub enum ScopeMemberUpdateResult {
  Ok(ScopeMember),
  TargetIsLastAdmin,
  TargetIsLastTransferableAdmin,
  TargetNotMember,
}

#[derive(Debug)]
pub enum CreatePackageResult {
  Ok(Package),
  AlreadyExists,
  WeeklyPackageLimitExceeded(i32),
  PackageLimitExceeded(i32),
}

#[derive(Debug)]
pub enum CreatePublishingTaskResult {
  Created((PublishingTask, Option<UserPublic>)),
  Exists((PublishingTask, Option<UserPublic>)),
  WeeklyPublishAttemptsLimitExceeded(i32),
}
