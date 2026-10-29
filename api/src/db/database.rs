// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::api::ApiMetrics;
use crate::ids::PackageName;
use crate::ids::ScopeDescription;
use crate::ids::ScopeName;
use crate::ids::Version;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use const_format::concatcp;
use deadpool_postgres::Transaction;
use serde_json::json;
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

use super::models::*;
use super::query::FromRow;
use super::query::Result;
use super::query::query;
use super::query::query_as;
use super::query::query_scalar;
use super::sql_fragments::*;

/// Picks the statement for a user-requested sort order. `$before` and
/// `$after` are the SQL around the `ORDER BY` expression; each allowed key
/// (`!key` for the reverse order) gets its own statement, built at compile
/// time, so the SQL that runs is always one of a fixed set.
macro_rules! sort_by {
  ($maybe_sort:expr, $before:expr, $after:expr => { $(@timestamps $($timestamp:literal),+;)? $( $key:literal $(=> $val:literal)? ),+, } || $default:literal) => {{
    const BEFORE: &str = $before;
    const AFTER: &str = $after;

    let sort = $maybe_sort.map(|sort| {
      let mut desc = sort.starts_with('!');
      let sort = if desc { &sort[1..] } else { sort };

      // Timestamps sort newest first by default.
      $(
        if [$($timestamp),+].contains(&sort) {
          desc = !desc;
        }
      )?

      (sort, desc)
    });

    match sort {
      $(
        Some(($key, false)) => concatcp!(BEFORE, sort_by!(@expand $key $(, $val)?), " ASC", AFTER),
        Some(($key, true)) => concatcp!(BEFORE, sort_by!(@expand $key $(, $val)?), " DESC", AFTER),
      )+
      _ => concatcp!(BEFORE, $default, AFTER),
    }
  }};

  (@expand $key:literal, $val:literal) => { $val };
  (@expand $key:literal) => { $key };
}

/// How long a publishing task may stay in a non-terminal state
/// (`processing`/`processed`) before it is considered stranded. The publish
/// queue normally finishes a task in seconds, and Cloud Run caps a single
/// request well under this, so a task older than this is not actively being
/// processed. Stranded tasks are re-driven by the reaper in `tasks.rs`, and
/// `create_publishing_task` fails a stranded `processing` task inline when a
/// new publish for the same version arrives.
pub const STALE_PUBLISHING_TASK_SECS: i64 = 30 * 60;
pub const ABANDON_PUBLISHING_TASK_SECS: i64 = 24 * 60 * 60;

#[derive(Clone)]
pub struct Database {
  pool: deadpool_postgres::Pool,
}

impl std::fmt::Debug for Database {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Database").finish_non_exhaustive()
  }
}

/// Client-certificate TLS material for connecting to the database. Supplied
/// when the database requires a client certificate (`ssl_mode =
/// TRUSTED_CLIENT_CERTIFICATE_REQUIRED`); the same cert is also presented by
/// the Hyperdrive-backed `api` Worker so both reach Cloud SQL over mTLS.
// The worker reaches the database through Hyperdrive, which does the TLS.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub struct DbTls {
  pub client_cert: String,
  pub client_key: String,
}

impl Database {
  pub async fn connect(
    database_url: &str,
    pool_size: u32,
    acquire_timeout: std::time::Duration,
    tls: Option<DbTls>,
  ) -> anyhow::Result<Self> {
    #[cfg(not(target_arch = "wasm32"))]
    let pool = {
      let mut pg_config = tokio_postgres::Config::from_str(database_url)?;
      // Check a connection is still alive before handing it out, like sqlx's
      // pool did, so one dropped while idle doesn't fail the next query.
      let manager_config = deadpool_postgres::ManagerConfig {
        recycling_method: deadpool_postgres::RecyclingMethod::Verified,
      };
      let manager = if let Some(tls) = tls {
        // See `make_tls_connect` for why the server certificate isn't verified.
        pg_config.ssl_mode(tokio_postgres::config::SslMode::Require);
        deadpool_postgres::Manager::from_config(
          pg_config,
          super::tls::make_tls_connect(&tls)?,
          manager_config,
        )
      } else {
        deadpool_postgres::Manager::from_config(
          pg_config,
          tokio_postgres::NoTls,
          manager_config,
        )
      };
      let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(pool_size as usize)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .wait_timeout(Some(acquire_timeout))
        .create_timeout(Some(acquire_timeout))
        .build()?;
      // Fail at startup if the database is unreachable, like the pool used to.
      drop(pool.get().await?);
      pool
    };
    // A Worker can't reuse I/O across requests, so open one connection when a
    // query needs it. See `WorkerConnect` for how it reaches the database.
    #[cfg(target_arch = "wasm32")]
    let pool = {
      let _ = (pool_size, tls);
      let manager = deadpool_postgres::Manager::from_connect(
        tokio_postgres::Config::from_str(database_url)?,
        crate::worker_js::WorkerConnect,
        deadpool_postgres::ManagerConfig {
          recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        },
      );
      deadpool_postgres::Pool::builder(manager)
        .max_size(1)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .wait_timeout(Some(acquire_timeout))
        .create_timeout(Some(acquire_timeout))
        .build()?
    };
    println!("Database ready");
    Ok(Database { pool })
  }

  /// Applies the migrations in `api/migrations` that haven't run yet.
  pub async fn migrate(&self) -> anyhow::Result<()> {
    super::migrate::run(&self.pool).await?;
    Ok(())
  }

  #[instrument(name = "Database::get_user", skip(self), err)]
  pub async fn get_user(&self, id: Uuid) -> Result<Option<User>> {
    query_as::<User>(
      concatcp!("SELECT ", USER_SELECT_FULL, " FROM users WHERE id = $1"),
      &[&id],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_user_public", skip(self), err)]
  pub async fn get_user_public(&self, id: Uuid) -> Result<Option<UserPublic>> {
    query_as::<UserPublic>(r#"SELECT id, name, avatar_url, github_id, gitlab_id, updated_at, created_at
      FROM users
      WHERE id = $1"#, &[&id])
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_user_by_github_id", skip(self), err)]
  pub async fn get_user_by_github_id(
    &self,
    github_id: i64,
  ) -> Result<Option<User>> {
    query_as::<User>(
      concatcp!(
        "SELECT ",
        USER_SELECT_FULL,
        " FROM users WHERE github_id = $1"
      ),
      &[&github_id],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_user_by_gitlab_id", skip(self), err)]
  pub async fn get_user_by_gitlab_id(
    &self,
    gitlab_id: i64,
  ) -> Result<Option<User>> {
    query_as::<User>(
      concatcp!(
        "SELECT ",
        USER_SELECT_FULL,
        " FROM users WHERE gitlab_id = $1"
      ),
      &[&gitlab_id],
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

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

    let sql = sort_by!(
      maybe_sort,
      concatcp!("SELECT ", USER_SELECT_FULL, "
      FROM users
      WHERE (name ILIKE $1 OR email ILIKE $1) AND (id = $2 OR $2 IS NULL)
      ORDER BY "),
      " OFFSET $3 LIMIT $4"
      => {
      @timestamps "created_at";
      "email",
      "github_id",
      "scope_limit",
      "is_staff",
      "is_blocked",
      "created_at",
    } || "created_at DESC");

    let users: Vec<User> =
      query_as::<User>(sql, &[&search, &maybe_id, &start, &limit])
        .fetch_all(&tx)
        .await?;

    let total_users = query_scalar::<i64>(r#"SELECT COUNT(created_at) as "count" FROM users WHERE (name ILIKE $1 OR email ILIKE $1) AND (id = $2 OR $2 IS NULL);"#, &[&search, &maybe_id])
      .fetch_one(&tx)
      .await?;

    tx.commit().await?;

    Ok((total_users as usize, users))
  }

  #[cfg(test)]
  #[instrument(
    name = "Database::insert_user",
    skip(self, new_user),
    err,
    fields(user.name = new_user.name, user.email = new_user.email, user.avatar_url = new_user.avatar_url, user.github_id = new_user.github_id, user.is_blocked = new_user.is_blocked, user.is_staff = new_user.is_staff
    )
  )]
  pub async fn insert_user(&self, new_user: NewUser<'_>) -> Result<User> {
    query_as::<User>(concatcp!("INSERT INTO users (name, email, avatar_url, github_id, gitlab_id, is_blocked, is_staff)
      VALUES ($1, $2, $3, $4, $5, $6, $7)
      RETURNING ", USER_SELECT_FULL), &[&new_user.name, &new_user.email, &new_user.avatar_url, &new_user.github_id, &new_user.gitlab_id, &new_user.is_blocked, &new_user.is_staff])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::upsert_user_by_github_id", skip(
    self,
    new_user
  ), err, fields(user.github_id = new_user.github_id, user.is_blocked = new_user.is_blocked, user.is_staff = new_user.is_staff
  ))]
  pub async fn upsert_user_by_github_id(
    &self,
    new_user: NewUser<'_>,
  ) -> Result<User> {
    assert!(new_user.github_id.is_some(), "github_id is required");
    query_as::<User>(concatcp!("INSERT INTO users (name, email, avatar_url, github_id, is_blocked, is_staff)
      VALUES ($1, $2, $3, $4, $5, $6)
      ON CONFLICT(github_id) DO UPDATE
      SET name = $1, email = $2, avatar_url = $3
      RETURNING ", USER_SELECT_FULL), &[&new_user.name, &new_user.email, &new_user.avatar_url, &new_user.github_id, &new_user.is_blocked, &new_user.is_staff])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::upsert_user_by_gitlab_id", skip(
    self,
    new_user
  ), err, fields(user.github_id = new_user.github_id, user.gitlab_id = new_user.gitlab_id, user.is_blocked = new_user.is_blocked, user.is_staff = new_user.is_staff
  ))]
  pub async fn upsert_user_by_gitlab_id(
    &self,
    new_user: NewUser<'_>,
  ) -> Result<User> {
    assert!(new_user.gitlab_id.is_some(), "gitlab_id is required");
    query_as::<User>(concatcp!("INSERT INTO users (name, email, avatar_url, gitlab_id, is_blocked, is_staff)
      VALUES ($1, $2, $3, $4, $5, $6)
      ON CONFLICT(gitlab_id) DO UPDATE
      SET name = $1, email = $2, avatar_url = $3
      RETURNING ", USER_SELECT_FULL), &[&new_user.name, &new_user.email, &new_user.avatar_url, &new_user.gitlab_id, &new_user.is_blocked, &new_user.is_staff])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::user_set_github_id", skip(self), err)]
  pub async fn user_set_github_id(
    &self,
    user: Uuid,
    id: Option<i64>,
  ) -> Result<User> {
    query_as::<User>(
      concatcp!(
        "UPDATE users SET github_id = $1 WHERE id = $2
      RETURNING ",
        USER_SELECT_FULL
      ),
      &[&id, &user],
    )
    .fetch_one(&self.pool)
    .await
  }

  #[instrument(name = "Database::user_set_gitlab_id", skip(self), err)]
  pub async fn user_set_gitlab_id(
    &self,
    user: Uuid,
    id: Option<i64>,
  ) -> Result<User> {
    query_as::<User>(
      concatcp!(
        "UPDATE users SET gitlab_id = $1 WHERE id = $2
      RETURNING ",
        USER_SELECT_FULL
      ),
      &[&id, &user],
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      staff_id,
      true,
      "user_set_staff",
      json!({
        "user_id": user_id,
        "is_staff": is_staff,
      }),
    )
    .await?;

    let user = query_as::<User>(
      concatcp!(
        "UPDATE users SET is_staff = $1 WHERE id = $2
      RETURNING ",
        USER_SELECT_FULL
      ),
      &[&is_staff, &user_id],
    )
    .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      staff_id,
      true,
      "user_set_blocked",
      json!({
        "user_id": user_id,
        "is_blocked": is_blocked,
      }),
    )
    .await?;

    let user = query_as::<User>(
      concatcp!(
        "UPDATE users SET is_blocked = $1 WHERE id = $2
      RETURNING ",
        USER_SELECT_FULL
      ),
      &[&is_blocked, &user_id],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok(user)
  }

  #[instrument(name = "Database::user_set_deletion_hold", skip(self), err)]
  pub async fn user_set_deletion_hold(
    &self,
    staff_id: &Uuid,
    user_id: Uuid,
    deletion_hold: bool,
  ) -> Result<User> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      staff_id,
      true,
      "user_set_deletion_hold",
      json!({
        "user_id": user_id,
        "deletion_hold": deletion_hold,
      }),
    )
    .await?;

    let user = query_as::<User>(
      concatcp!(
        "UPDATE users SET deletion_hold = $1 WHERE id = $2
      RETURNING ",
        USER_SELECT_FULL
      ),
      &[&deletion_hold, &user_id],
    )
    .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      staff_id,
      true,
      "user_set_scope_limit",
      json!({
        "user_id": user_id,
        "scope_limit": scope_limit,
      }),
    )
    .await?;

    let user = query_as::<User>(
      concatcp!(
        "UPDATE users SET scope_limit = $1 WHERE id = $2
      RETURNING ",
        USER_SELECT_FULL
      ),
      &[&scope_limit, &user_id],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok(user)
  }

  #[instrument(name = "Database::delete_user", skip(self), err)]
  pub async fn delete_user(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    id: Uuid,
  ) -> Result<UserDeleteResult> {
    let service_account_id = Uuid::nil();
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    // Check user exists, and capture the identity data for the deletion
    // tombstone below. The deletion hold is re-checked here, with the row
    // locked for the duration of the transaction, so a hold placed after
    // the handler's own check can't race with the deletion.
    let Some(user_row) = query(
      r#"SELECT name, email, github_id, gitlab_id, deletion_hold FROM users WHERE id = $1 FOR UPDATE"#,
      &[&id],
    )
    .fetch_optional(&tx)
    .await?
    else {
      return Ok(UserDeleteResult::NotFound);
    };

    if user_row.try_get::<_, bool>("deletion_hold")? {
      return Ok(UserDeleteResult::Held);
    }

    // For scopes where the user is the last member, transfer to service account.
    // For scopes where the user is creator but others exist, transfer creator.
    // Everything else (scope_members, invites, tokens, etc.) cascades via FK constraints.
    // Note: this runs at READ COMMITTED, so a member added to one of these
    // scopes concurrently can end up as a peer of the service account instead
    // of preventing the transfer. That's acceptable for this rare flow.
    struct Membership {
      scope: ScopeName,
      other_member_count: i64,
      is_creator: bool,
    }
    let memberships = query(
      r#"SELECT scope,
        (SELECT COUNT(*) FROM scope_members sm2 WHERE sm2.scope = scope_members.scope AND sm2.user_id != $1) as "other_member_count",
        (SELECT creator FROM scopes WHERE scopes.scope = scope_members.scope) = $1 as "is_creator"
      FROM scope_members WHERE user_id = $1"#,
      &[&id],
    )
    .map(|r| {
      Ok(Membership {
        scope: r.try_get("scope")?,
        other_member_count: r.try_get("other_member_count")?,
        is_creator: r.try_get("is_creator")?,
      })
    })
    .fetch_all(&tx)
    .await?;

    for membership in &memberships {
      if membership.other_member_count == 0 {
        // Last member: transfer scope to service account
        query(
          r#"UPDATE scopes SET creator = $1 WHERE scope = $2"#,
          &[&service_account_id, &membership.scope],
        )
        .execute(&tx)
        .await?;

        query(
          r#"INSERT INTO scope_members (scope, user_id, is_admin)
          VALUES ($1, $2, true)
          ON CONFLICT (scope, user_id) DO NOTHING"#,
          &[&membership.scope, &service_account_id],
        )
        .execute(&tx)
        .await?;
      } else if membership.is_creator {
        // Creator with other members: transfer to another admin (or any member)
        let new_creator_id = query_scalar::<Uuid>(
          r#"SELECT user_id FROM scope_members
          WHERE scope = $1 AND user_id != $2
          ORDER BY is_admin DESC, created_at ASC
          LIMIT 1"#,
          &[&membership.scope, &id],
        )
        .fetch_one(&tx)
        .await?;

        query(
          r#"UPDATE scopes SET creator = $1 WHERE scope = $2"#,
          &[&new_creator_id, &membership.scope],
        )
        .execute(&tx)
        .await?;

        // Ensure the new creator is an admin
        query(
          r#"UPDATE scope_members SET is_admin = true
          WHERE scope = $1 AND user_id = $2"#,
          &[&membership.scope, &new_creator_id],
        )
        .execute(&tx)
        .await?;
      }
    }

    // Transfer tickets, ticket_messages, and audit_logs to service account,
    // rather than nulling them: a ticket without a creator must have a
    // reporter email (tickets_reporter_xor), which a deleted user's ticket lacks.
    query(
      r#"UPDATE ticket_messages SET author = $1 WHERE author = $2"#,
      &[&service_account_id, &id],
    )
    .execute(&tx)
    .await?;

    query(
      r#"UPDATE tickets SET creator = $1 WHERE creator = $2"#,
      &[&service_account_id, &id],
    )
    .execute(&tx)
    .await?;

    // Write the deletion audit entry, storing the actual actor in meta
    // so it's preserved even when the actor is the user being deleted.
    // The user's identity data is retained as a tombstone so abuse and
    // supply-chain attacks can still be investigated after the account is
    // gone (GDPR art. 17(3)(b)/(e), CCPA 1798.105(d)). Name and email are
    // stripped from the entry after one year by the
    // /tasks/anonymize_deleted_users cron; the internal user ID and the
    // identity provider IDs are kept indefinitely so the account can be
    // re-identified through the provider if legally required.
    let audit_actor = if *actor_id == id {
      &service_account_id
    } else {
      actor_id
    };
    audit_log(
      &tx,
      audit_actor,
      is_sudo,
      "user_delete",
      json!({
        "user_id": id,
        "performed_by": actor_id,
        "name": user_row.try_get::<_, String>("name")?,
        "email": user_row.try_get::<_, Option<String>>("email")?,
        "github_id": user_row.try_get::<_, Option<i64>>("github_id")?,
        "gitlab_id": user_row.try_get::<_, Option<i64>>("gitlab_id")?,
      }),
    )
    .await?;

    // Reassign all of the deleted user's audit log entries to the service
    // account, preserving the original actor in each entry's meta so the
    // audit trail stays attributable after deletion.
    query(
      r#"UPDATE audit_logs
      SET actor_id = $1,
        meta = jsonb_set(meta, '{original_actor_id}', to_jsonb($2::uuid))
      WHERE actor_id = $2"#,
      &[&service_account_id, &id],
    )
    .execute(&tx)
    .await?;

    // Delete the user — FK cascades handle scope_members, scope_invites,
    // tokens, authorizations; SET NULL handles publishing_tasks,
    // package_versions, scopes.creator.
    query(r#"DELETE FROM users WHERE id = $1"#, &[&id])
      .execute(&tx)
      .await?;

    tx.commit().await?;
    Ok(UserDeleteResult::Deleted)
  }

  /// Strips name and email from `user_delete` audit tombstones older than
  /// the cutoff, completing the anonymization of deleted accounts. The
  /// internal user ID and identity provider IDs remain (see `delete_user`).
  #[instrument(name = "Database::anonymize_deleted_users", skip(self), err)]
  pub async fn anonymize_deleted_users(
    &self,
    older_than: DateTime<Utc>,
  ) -> Result<u64> {
    let result = query(
      r#"UPDATE audit_logs
      SET meta = meta - 'name' - 'email'
      WHERE action = 'user_delete'
        AND created_at < $1
        AND (meta ? 'name' OR meta ? 'email')"#,
      &[&older_than],
    )
    .execute(&self.pool)
    .await?;
    Ok(result)
  }

  #[instrument(name = "Database::get_package", skip(self), err)]
  pub async fn get_package(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Option<PackageWithGitHubRepoAndMeta>> {
    query(concatcp!("SELECT ", PACKAGE_SELECT_JOINED, ", ", GITHUB_REPOSITORY_SELECT_JOINED, "
      FROM packages
      LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
      WHERE packages.scope = $1 AND packages.name = $2"), &[&scope, &name])
      .map(package_with_github_repository_and_meta)
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(name = "Database::create_package", skip(self), err)]
  pub async fn create_package(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<CreatePackageResult> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;
    let res = query_as::<Package>(concatcp!("INSERT INTO packages (scope, name)
      VALUES ($1, $2)
      RETURNING ", PACKAGE_SELECT, r#",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = packages.scope AND name = packages.name) as "version_count",
        (SELECT version FROM package_versions WHERE scope = packages.scope AND name = packages.name AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1) as "latest_version"
      "#), &[&scope, &name])
      .fetch_one(&tx)
      .await;
    let package = match res {
      Ok(package) => package,
      Err(err) => {
        if err.is_unique_violation() {
          return Ok(CreatePackageResult::AlreadyExists);
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
    query(r#"UPDATE package_versions
      SET rekor_log_id = $1, meta = jsonb_set_lax(meta, '{hasProvenance}', 'true'::jsonb, true)
      WHERE scope = $2 AND name = $3 AND version = $4 AND rekor_log_id IS NULL AND created_at > now() - '2 minute'::interval"#, &[&rekor_log_id, &package_scope, &package_name, &version])
      .execute(&self.pool)
      .await?;

    Ok(())
  }

  /// Replaces the stored score metadata of a published version.
  #[instrument(
    name = "Database::update_package_version_meta",
    skip(self, meta),
    err
  )]
  pub async fn update_package_version_meta(
    &self,
    staff_id: &Uuid,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
    meta: &PackageVersionMeta,
  ) -> Result<()> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      staff_id,
      true,
      "update_package_version_meta",
      json!({
        "scope": scope,
        "name": name,
        "version": version,
      }),
    )
    .await?;

    query(
      r#"UPDATE package_versions SET meta = $1
      WHERE scope = $2 AND name = $3 AND version = $4"#,
      &[&meta, &scope, &name, &version],
    )
    .execute(&tx)
    .await?;

    tx.commit().await?;

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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "update_package_description",
      json!({
          "scope": scope,
          "name": name,
      }),
    )
    .await?;

    let package = query(
      concatcp!(
        "UPDATE packages
      SET description = $3
      WHERE scope = $1 AND name = $2
      RETURNING ",
        PACKAGE_SELECT_JOINED
      ),
      &[&scope, &name, &description],
    )
    .map(|r| {
      let package = Package::from_row_prefixed(r, "package_")?;

      Ok((package, None, r.try_get("package_version_meta")?))
    })
    .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
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

    let repo = query_as::<GithubRepository>(
      "INSERT INTO github_repositories (id, owner, name)
      VALUES ($1, $2, $3)
      ON CONFLICT(id) DO UPDATE
      SET owner = $2, name = $3
      RETURNING id, owner, name, updated_at, created_at",
      &[&repo.id, &repo.owner, &repo.name],
    )
    .fetch_one(&tx)
    .await?;

    let (package, meta) = query(
      concatcp!(
        "UPDATE packages
      SET github_repository_id = $3
      WHERE scope = $1 AND name = $2
      RETURNING ",
        PACKAGE_SELECT_JOINED
      ),
      &[&scope, &name, &repo.id],
    )
    .map(|r| {
      let package = Package::from_row_prefixed(r, "package_")?;

      Ok((package, r.try_get("package_version_meta")?))
    })
    .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "delete_package_github_repository",
      json!({
        "scope": scope,
        "name": name,
      }),
    )
    .await?;

    let package = query_as::<Package>(concatcp!("UPDATE packages
      SET github_repository_id = NULL
      WHERE scope = $1 AND name = $2
      RETURNING ", PACKAGE_SELECT, r#",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#), &[&scope, &name])
      .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
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

    let package = query_as::<Package>(concatcp!("UPDATE packages
      SET runtime_compat = $3
      WHERE scope = $1 AND name = $2
      RETURNING ", PACKAGE_SELECT, r#",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#), &[&scope, &name, &runtime_compat])
      .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let when_featured = if is_featured { Some(Utc::now()) } else { None };

    audit_log(
      &tx,
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

    let package = query_as::<Package>(concatcp!("UPDATE packages
      SET when_featured = $3
      WHERE scope = $1 AND name = $2
      RETURNING ", PACKAGE_SELECT, r#",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#), &[&scope, &name, &when_featured])
      .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
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

    let package = query_as::<Package>(concatcp!("UPDATE packages
      SET is_archived = $3
      WHERE scope = $1 AND name = $2
      RETURNING ", PACKAGE_SELECT, r#",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#), &[&scope, &name, &is_archived])
      .fetch_one(&tx)
      .await?;

    tx.commit().await?;

    Ok(package)
  }

  #[instrument(name = "Database::update_package_source", skip(self), err)]
  pub async fn update_package_source(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
    source: ReadmeSource,
  ) -> Result<Package> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "package_update_source",
      json!({
          "scope": scope,
          "name": name,
          "source": source,
      }),
    )
    .await?;

    let package = query_as::<Package>(concatcp!("UPDATE packages
      SET readme_source = $3
      WHERE scope = $1 AND name = $2
      RETURNING ", PACKAGE_SELECT, r#",
        (SELECT COUNT(created_at) FROM package_versions WHERE scope = scope AND name = name) as "version_count",
        (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version""#), &[&scope, &name, &source])
      .fetch_one(&tx)
      .await?;

    tx.commit().await?;

    Ok(package)
  }

  #[instrument(name = "Database::create_scope", skip(self), err)]
  pub async fn create_scope(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope_name: &ScopeName,
    user_id: Uuid,
    scope_description: &ScopeDescription,
  ) -> Result<Scope> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      if is_sudo {
        "assign_scope"
      } else {
        "create_scope"
      },
      json!({
          "scope": scope_name,
          "user_id": user_id,
      }),
    )
    .await?;

    let scope = query_as::<Scope>(concatcp!("WITH ins_scope AS (
            INSERT INTO scopes (scope, creator) VALUES ($1, $2)
            RETURNING scope, description, creator, package_limit, new_package_per_week_limit, publish_attempts_per_week_limit, verify_oidc_actor, require_publishing_from_ci, updated_at, created_at
        ),
        ins_member AS (
            INSERT INTO scope_members (scope, user_id, is_admin)
            VALUES ($1, $2, true)
        )
        SELECT ", SCOPE_SELECT, " FROM ins_scope"), &[&scope_name, &user_id])
    .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    if let Some(package_limit) = package_limit {
      audit_log(
        &tx,
        staff_id,
        true,
        "scope_set_package_limit",
        json!({
          "scope": scope,
          "package_limit": package_limit,
        }),
      )
      .await?;

      query(
        r#"UPDATE scopes SET package_limit = $1 WHERE scope = $2"#,
        &[&package_limit, &scope],
      )
      .execute(&tx)
      .await?;
    }

    if let Some(new_package_per_week_limit) = new_package_per_week_limit {
      audit_log(
        &tx,
        staff_id,
        true,
        "scope_set_package_per_week_limit",
        json!({
          "scope": scope,
          "new_package_per_week_limit": new_package_per_week_limit,
        }),
      )
      .await?;

      query(
        r#"UPDATE scopes SET new_package_per_week_limit = $1 WHERE scope = $2"#,
        &[&new_package_per_week_limit, &scope],
      )
      .execute(&tx)
      .await?;
    }

    if let Some(publish_attempts_per_week_limit) =
      publish_attempts_per_week_limit
    {
      audit_log(
        &tx,
        staff_id,
        true,
        "scope_set_publish_attempts_per_week_limit",
        json!({
          "scope": scope,
          "publish_attempts_per_week_limit": publish_attempts_per_week_limit,
        }),
      )
      .await?;

      query(r#"UPDATE scopes SET publish_attempts_per_week_limit = $1 WHERE scope = $2"#, &[&publish_attempts_per_week_limit, &scope])
        .execute(&tx)
        .await?;
    }

    let res = query(r#"
      WITH usage AS (
        SELECT
          (SELECT COUNT(created_at) FROM packages WHERE scope = $1) AS package,
          (SELECT COUNT(created_at) FROM packages WHERE scope = $1 AND created_at > now() - '1 week'::interval) AS new_package_per_week,
          (SELECT COUNT(created_at) FROM publishing_tasks WHERE package_scope = $1 AND created_at > now() - '1 week'::interval) AS publish_attempts_per_week
      )
      SELECT
      scopes.scope as "scope_scope",
      scopes.description as "scope_description",
      scopes.creator as "scope_creator",
      scopes.package_limit as "scope_package_limit",
      scopes.new_package_per_week_limit as "scope_new_package_per_week_limit",
      scopes.publish_attempts_per_week_limit as "scope_publish_attempts_per_week_limit",
      scopes.verify_oidc_actor as "scope_verify_oidc_actor",
      scopes.require_publishing_from_ci as "scope_require_publishing_from_ci",
      scopes.updated_at as "scope_updated_at",
      scopes.created_at as "scope_created_at",
      users.id as "user_id", users.name as "user_name", users.avatar_url as "user_avatar_url", users.github_id as "user_github_id",
users.gitlab_id as "user_gitlab_id", users.updated_at as "user_updated_at", users.created_at as "user_created_at",
      usage.package as "usage_package", usage.new_package_per_week as "usage_new_package_per_week", usage.publish_attempts_per_week as "usage_publish_attempts_per_week"
      FROM scopes
      LEFT JOIN users ON scopes.creator = users.id
      CROSS JOIN usage
      WHERE scopes.scope = $1
      "#, &[&scope])
      .map(|r| {
        let scope = Scope::from_row_prefixed(r, "scope_")?;
        let usage = ScopeUsage::from_row_prefixed(r, "usage_")?;
        let user = UserPublic::from_row_prefixed(r, "user_")?;
        Ok((scope, usage, user))
      })
      .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let search = format!("%{}%", maybe_search_query.unwrap_or(""));
    let sql = sort_by!(
      maybe_sort,
      concatcp!("SELECT ", SCOPE_SELECT_JOINED, ", ", USER_PUBLIC_SELECT_JOINED, ", ", SCOPE_USAGE_SELECT, "
      FROM scopes
      LEFT JOIN users ON scopes.creator = users.id
      WHERE scopes.scope ILIKE $1 OR users.name ILIKE $1
      ORDER BY "),
      "
      OFFSET $2 LIMIT $3
      "
      => {
      @timestamps "created_at";
      "scope" => "scopes.scope",
      "creator" => "users.name",
      "package_limit" => "scopes.package_limit",
      "new_package_per_week_limit" => "scopes.new_package_per_week_limit",
      "publish_attempts_per_week_limit" => "scopes.publish_attempts_per_week_limit",
      "created_at" => "scopes.created_at",
    } || "scopes.created_at DESC");

    let scopes = query(sql, &[&search, &start, &limit])
      .map(|r| {
        let scope = Scope::from_row_prefixed(r, "scope_")?;
        let usage = ScopeUsage::from_row_prefixed(r, "usage_")?;
        let user = UserPublic::from_row_prefixed(r, "user_")?;

        Ok((scope, usage, user))
      })
      .fetch_all(&tx)
      .await?;

    let total_scopes = query_scalar::<i64>(r#"SELECT COUNT(scopes.created_at) FROM scopes LEFT JOIN users ON scopes.creator = users.id WHERE scopes.scope ILIKE $1 OR users.name ILIKE $1;"#, &[&search])
      .fetch_one(&tx)
      .await?;

    tx.commit().await?;

    Ok((total_scopes as usize, scopes))
  }

  #[cfg(test)]
  #[instrument(name = "Database::list_scopes_created_by_user", skip(self), err)]
  pub async fn list_scopes_created_by_user(
    &self,
    user_id: Uuid,
  ) -> Result<Vec<Scope>> {
    query_as::<Scope>(
      concatcp!(
        "SELECT ",
        SCOPE_SELECT,
        " FROM scopes WHERE creator = $1 ORDER BY scope ASC"
      ),
      &[&user_id],
    )
    .fetch_all(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_scope", skip(self), err)]
  pub async fn get_scope(&self, scope: &ScopeName) -> Result<Option<Scope>> {
    query_as::<Scope>(
      concatcp!("SELECT ", SCOPE_SELECT, " FROM scopes WHERE scope = $1"),
      &[&scope],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_scope_usage", skip(self), err)]
  pub async fn get_scope_usage(&self, scope: &ScopeName) -> Result<ScopeUsage> {
    query(r#"SELECT
      (SELECT COUNT(created_at) FROM packages WHERE scope = $1 AND created_at > now() - '1 week'::interval) AS new_package_per_week,
      (SELECT COUNT(created_at) FROM packages WHERE scope = $1) AS package,
      (SELECT COUNT(created_at) FROM publishing_tasks WHERE package_scope = $1 AND created_at > now() - '1 week'::interval) AS publish_attempts_per_week;"#, &[&scope])
      .map(ScopeUsage::from_row)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "scope_set_verify_oidc_actor",
      json!({
        "scope": scope,
        "verify_oidc_actor": verify_oidc_actor,
      }),
    )
    .await?;

    let scope = query_as::<Scope>(
      concatcp!(
        "UPDATE scopes SET verify_oidc_actor = $1 WHERE scope = $2
        RETURNING ",
        SCOPE_SELECT
      ),
      &[&verify_oidc_actor, &scope],
    )
    .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "scope_set_require_publishing_from_ci",
      json!({
        "scope": scope,
        "require_publishing_from_ci": require_publishing_from_ci,
      }),
    )
    .await?;

    let scope = query_as::<Scope>(
      concatcp!(
        "UPDATE scopes SET require_publishing_from_ci = $1 WHERE scope = $2
        RETURNING ",
        SCOPE_SELECT
      ),
      &[&require_publishing_from_ci, &scope],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok(scope)
  }

  #[instrument(name = "Database::scope_set_description", skip(self), err)]
  pub async fn scope_set_description(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    description: Option<String>,
  ) -> Result<Scope> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "scope_set_description",
      json!({
        "scope": scope,
        "description": description,
      }),
    )
    .await?;

    let scope = query_as::<Scope>(
      concatcp!(
        "UPDATE scopes SET description = $1 WHERE scope = $2
        RETURNING ",
        SCOPE_SELECT
      ),
      &[&description, &scope],
    )
    .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let packages = query(concatcp!("SELECT ", PACKAGE_BASE_SELECT_JOINED, ",
      ", PACKAGE_VERSION_AGG_SELECT, ",
      ", GITHUB_REPOSITORY_SELECT_JOINED, "
      FROM packages
      LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
      ", PACKAGE_VERSION_LATERAL_JOINS, "
      WHERE packages.scope = $1 AND ($2 = true OR packages.is_archived = false)
      ORDER BY packages.is_archived ASC, packages.name
      OFFSET $3 LIMIT $4"), &[&scope, &show_archived, &start, &limit])
      .map(package_with_github_repository_and_meta)
      .fetch_all(&tx)
      .await?;

    let total_packages = query_scalar::<i64>(r#"SELECT COUNT(created_at) FROM packages WHERE scope = $1 AND ($2 = true OR packages.is_archived = false);"#, &[&scope, &show_archived])
      .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

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
    let sql = sort_by!(
      maybe_sort,
      concatcp!("SELECT ", PACKAGE_BASE_SELECT_JOINED, ", ", PACKAGE_VERSION_AGG_SELECT, ", ", GITHUB_REPOSITORY_SELECT_JOINED, "
       FROM packages
       LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
       ", PACKAGE_VERSION_LATERAL_JOINS, "
       WHERE (packages.scope ILIKE $1 OR packages.name ILIKE $2) AND (packages.github_repository_id = $5 OR $5 IS NULL) AND NOT packages.is_archived
       ORDER BY
         CASE
           WHEN packages.name ILIKE $3 THEN 1 -- Exact match for package name
           WHEN packages.scope ILIKE $4 THEN 2 -- Exact match for scope name
           ELSE 3 -- Fuzzy matches will be ordered by package name and then scope name below
        END,
        "),
      "
       OFFSET $6 LIMIT $7"
      => {
      @timestamps "when_featured", "updated_at", "created_at";
      "scope" => "packages.scope",
      "name" => "packages.name",
      // "repository",
      "is_archived" => "packages.is_archived",
      "when_featured" => "packages.when_featured",
      "updated_at" => "packages.updated_at",
      "created_at" => "packages.created_at",
    } || "packages.name ASC, packages.scope ASC");

    let packages = query(
      sql,
      &[
        &scope_ilike_query,
        &package_ilike_query,
        &package_exact_query,
        &scope_exact_query,
        &maybe_github_repo_id,
        &start,
        &limit,
      ],
    )
    .map(package_with_github_repository_and_meta)
    .fetch_all(&tx)
    .await?;

    let total_packages = query_scalar::<i64>(r#"SELECT COUNT(created_at) FROM packages WHERE (packages.scope ILIKE $1 OR packages.name ILIKE $2) AND (packages.github_repository_id = $3 OR $3 IS NULL);"#, &[&scope_ilike_query, &package_ilike_query, &maybe_github_repo_id])
      .fetch_one(&tx)
      .await?;

    tx.commit().await?;

    Ok((total_packages as usize, packages))
  }

  #[instrument(name = "Database::package_stats", skip(self), err)]
  pub async fn package_stats(
    &self,
  ) -> Result<(
    Vec<StatsPackage>,
    Vec<StatsPackageVersion>,
    Vec<StatsPackage>,
  )> {
    let newest_fut = query_as::<StatsPackage>(r#"SELECT packages.scope, packages.name, packages.description
      FROM packages
      WHERE EXISTS (
        SELECT 1 FROM package_versions
        WHERE scope = packages.scope AND name = packages.name AND is_yanked = false
      ) AND NOT packages.is_archived
      ORDER BY packages.created_at DESC
      LIMIT 10"#, &[])
      .fetch_all(&self.pool);

    let updated_fut = query_as::<StatsPackageVersion>(r#"SELECT package_versions.scope, package_versions.name, package_versions.version, packages.description
      FROM package_versions
      JOIN packages ON packages.scope = package_versions.scope AND packages.name = package_versions.name
      WHERE NOT packages.is_archived
      ORDER BY package_versions.created_at DESC
      LIMIT 10"#, &[])
      .fetch_all(&self.pool);

    let featured_fut = query_as::<StatsPackage>(
      r#"SELECT packages.scope, packages.name, packages.description
      FROM packages
      WHERE packages.when_featured IS NOT NULL AND NOT packages.is_archived
      ORDER BY packages.when_featured DESC
      LIMIT 10"#,
      &[],
    )
    .fetch_all(&self.pool);

    Ok(tokio::try_join!(newest_fut, updated_fut, featured_fut)?)
  }

  #[instrument(name = "Database::metrics", skip(self), err)]
  pub async fn metrics(&self) -> Result<ApiMetrics> {
    let packages = query(r#"
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
    "#, &[])
      .fetch_one(&self.pool)
      .await?;

    let users = query(r#"
      SELECT
        COUNT(*) AS count_total,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '1 DAY' THEN 1 END) AS count_1d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '7 DAY' THEN 1 END) AS count_7d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '30 DAY' THEN 1 END) AS count_30d
      FROM
        users;
      "#, &[])
      .fetch_one(&self.pool)
      .await?;

    let package_versions =
      query(r#"
      SELECT
        COUNT(*) AS count_total,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '1 DAY' THEN 1 END) AS count_1d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '7 DAY' THEN 1 END) AS count_7d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '30 DAY' THEN 1 END) AS count_30d
      FROM
        package_versions;
      "#, &[])
        .fetch_one(&self.pool)
        .await?;

    let count = |row: &tokio_postgres::Row, column| -> Result<usize> {
      Ok(row.try_get::<_, i64>(column)? as usize)
    };

    Ok(ApiMetrics {
      packages: count(&packages, "count_total")?,
      packages_1d: count(&packages, "count_1d")?,
      packages_7d: count(&packages, "count_7d")?,
      packages_30d: count(&packages, "count_30d")?,

      users: count(&users, "count_total")?,
      users_1d: count(&users, "count_1d")?,
      users_7d: count(&users, "count_7d")?,
      users_30d: count(&users, "count_30d")?,

      package_versions: count(&package_versions, "count_total")?,
      package_versions_1d: count(&package_versions, "count_1d")?,
      package_versions_7d: count(&package_versions, "count_7d")?,
      package_versions_30d: count(&package_versions, "count_30d")?,
    })
  }

  #[instrument(
    name = "Database::list_package_versions_for_metadata",
    skip(self),
    err
  )]
  pub async fn list_package_versions_for_metadata(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Vec<PackageVersionForMetadata>> {
    query_as::<PackageVersionForMetadata>(
      r#"SELECT version, is_yanked, created_at
      FROM package_versions
      WHERE scope = $1 AND name = $2
      ORDER BY version DESC"#,
      &[&scope, &name],
    )
    .fetch_all(&self.pool)
    .await
  }

  #[allow(clippy::type_complexity)]
  #[instrument(
    name = "Database::list_package_versions_paginated",
    skip(self),
    err
  )]
  pub async fn list_package_versions_paginated(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    start: i64,
    limit: i64,
  ) -> Result<(usize, Vec<(PackageVersion, Option<UserPublic>)>)> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let versions = query(
      concatcp!(
        "SELECT ",
        PACKAGE_VERSION_SELECT_JOINED,
        ",
      ",
        USER_PUBLIC_SELECT_JOINED,
        "
      FROM package_versions
      LEFT JOIN users ON package_versions.user_id = users.id
      WHERE package_versions.scope = $1 AND package_versions.name = $2
      ORDER BY package_versions.version DESC
      OFFSET $3 LIMIT $4"
      ),
      &[&scope, &name, &start, &limit],
    )
    .map(|r| {
      let package_version =
        PackageVersion::from_row_prefixed(r, "package_version_")?;

      let user = UserPublic::from_row_optional(r, "user_", "id")?;

      Ok((package_version, user))
    })
    .fetch_all(&tx)
    .await?;

    let total = query_scalar::<i64>(
      r#"SELECT COUNT(*) FROM package_versions WHERE scope = $1 AND name = $2"#,
      &[&scope, &name],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok((total as usize, versions))
  }

  #[instrument(
    name = "Database::list_package_versions_for_resolution",
    skip(self),
    err
  )]
  pub async fn list_package_versions_for_resolution(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Vec<PackageVersionForResolution>> {
    query_as::<PackageVersionForResolution>(
      r#"SELECT package_versions.version, package_versions.exports
      FROM package_versions
      WHERE package_versions.scope = $1 AND package_versions.name = $2
      ORDER BY package_versions.version DESC"#,
      &[&scope, &name],
    )
    .fetch_all(&self.pool)
    .await
  }

  #[instrument(
    name = "Database::list_package_versions_for_npm_version_manifest",
    skip(self),
    err
  )]
  pub async fn list_package_versions_for_npm_version_manifest(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Vec<PackageVersionForNpmVersionManifest>> {
    query_as::<PackageVersionForNpmVersionManifest>(r#"SELECT package_versions.version, package_versions.is_yanked, package_versions.created_at,
      npm_tarballs.revision as "npm_tarball_revision", npm_tarballs.sha1 as "npm_tarball_sha1", npm_tarballs.sha512 as "npm_tarball_sha512"
      FROM package_versions
      INNER JOIN LATERAL (
        SELECT revision, sha1, sha512
        FROM npm_tarballs
        WHERE npm_tarballs.scope = package_versions.scope
        AND npm_tarballs.name = package_versions.name
        AND npm_tarballs.version = package_versions.version
        ORDER BY revision DESC
        LIMIT 1
      ) npm_tarballs ON true
      WHERE package_versions.scope = $1 AND package_versions.name = $2
      ORDER BY package_versions.version DESC"#, &[&scope, &name])
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
    query_as::<PackageVersion>(concatcp!("SELECT ", PACKAGE_VERSION_SELECT, "
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND version NOT LIKE '%-%' AND is_yanked = false
      ORDER BY version DESC
      LIMIT 1"), &[&scope, &name])
      .fetch_optional(&self.pool)
      .await
  }

  /// Resolves the version whose docs are served for a package. This is the
  /// latest unyanked stable version, or - for packages that only have
  /// prerelease versions - the latest unyanked prerelease version. Ordering
  /// stable releases ahead of prereleases keeps the result identical to
  /// `get_latest_unyanked_version_for_package` whenever a stable release
  /// exists.
  #[instrument(
    name = "Database::get_latest_unyanked_version_for_package_for_docs",
    skip(self),
    err
  )]
  pub async fn get_latest_unyanked_version_for_package_for_docs(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Option<PackageVersion>> {
    query_as::<PackageVersion>(
      concatcp!(
        "SELECT ",
        PACKAGE_VERSION_SELECT,
        "
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND is_yanked = false
      ORDER BY (version NOT LIKE '%-%') DESC, version DESC
      LIMIT 1"
      ),
      &[&scope, &name],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(
    name = "Database::get_latest_unyanked_version_for_package_with_newer_versions_count",
    skip(self),
    err
  )]
  pub async fn get_latest_unyanked_version_for_package_with_newer_versions_count(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Option<PackageVersionWithNewerVersionsCount>> {
    query_as::<PackageVersionWithNewerVersionsCount>(concatcp!("SELECT ", PACKAGE_VERSION_SELECT, ",
      ", NEWER_VERSIONS_COUNT_SUBQUERY, "
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND version NOT LIKE '%-%' AND is_yanked = false
      ORDER BY version DESC
      LIMIT 1"), &[&scope, &name])
      .fetch_optional(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::list_latest_unyanked_versions_for_package",
    skip(self),
    err
  )]
  pub async fn list_latest_unyanked_versions_for_package(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    limit: u32,
  ) -> Result<Vec<Version>> {
    query_scalar(r#"
      SELECT version
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND version NOT LIKE '%-%' AND is_yanked = false
      ORDER BY version DESC
      LIMIT $3
      "#, &[&scope, &name, &(limit as i64)])
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
    query_as::<PackageVersion>(
      concatcp!(
        "SELECT ",
        PACKAGE_VERSION_SELECT,
        "
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND version = $3"
      ),
      &[&scope, &name, &version],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(
    name = "Database::get_package_version_with_newer_versions_count",
    skip(self),
    err
  )]
  pub async fn get_package_version_with_newer_versions_count(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<Option<PackageVersionWithNewerVersionsCount>> {
    query_as::<PackageVersionWithNewerVersionsCount>(
      concatcp!(
        "SELECT ",
        PACKAGE_VERSION_SELECT,
        ",
      ",
        NEWER_VERSIONS_COUNT_SUBQUERY,
        "
      FROM package_versions
      WHERE scope = $1 AND name = $2 AND version = $3"
      ),
      &[&scope, &name, &version],
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    query(r#"INSERT INTO package_versions (scope, name, version, user_id, readme_path, exports, uses_npm, meta, license)
      VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#, &[&new_package_version.scope, &new_package_version.name, &new_package_version.version, &new_package_version.user_id, &new_package_version.readme_path, &new_package_version.exports, &new_package_version.uses_npm, &new_package_version.meta, &new_package_version.license])
      .execute(&tx)
      .await?;

    for new_package_file in new_package_files {
      query(r#"INSERT INTO package_files (scope, name, version, path, size, checksum)
        VALUES ($1, $2, $3, $4, $5, $6)"#, &[&new_package_file.scope, &new_package_file.name, &new_package_file.version, &new_package_file.path, &new_package_file.size, &new_package_file.checksum])
        .execute(&tx)
        .await?;
    }

    for new_package_version_dependency in new_package_version_dependencies {
      query(r#"INSERT INTO package_version_dependencies (package_scope, package_name, package_version, dependency_kind, dependency_name, dependency_constraint, dependency_path, dependency_fallback_url)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#, &[&new_package_version_dependency.package_scope, &new_package_version_dependency.package_name, &new_package_version_dependency.package_version, &new_package_version_dependency.dependency_kind, &new_package_version_dependency.dependency_name, &new_package_version_dependency.dependency_constraint, &new_package_version_dependency.dependency_path, &new_package_version_dependency.dependency_fallback_url])
        .execute(&tx)
        .await?;
    }

    query(r#"INSERT INTO npm_tarballs (scope, name, version, revision, sha1, sha512, size)
      VALUES ($1, $2, $3, $4, $5, $6, $7)"#, &[&new_npm_tarball.scope, &new_npm_tarball.name, &new_npm_tarball.version, &new_npm_tarball.revision, &new_npm_tarball.sha1, &new_npm_tarball.sha512, &new_npm_tarball.size])
      .execute(&tx)
      .await?;

    let task = query_as::<PublishingTask>(
      concatcp!(
        "UPDATE publishing_tasks
      SET status = 'processed'
      WHERE id = $1 AND status = 'processing'
      RETURNING ",
        PUBLISHING_TASK_SELECT
      ),
      &[&publishing_task_id],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok(task)
  }

  #[cfg(test)]
  #[instrument(name = "Database::create_package_version_for_test", skip(
    self,
    new_package_version
  ), err, fields(package_version.scope = %new_package_version.scope, package_version.name = %new_package_version.name, package_version.version = %new_package_version.version, package_version.exports = ?new_package_version.exports
  ))]
  pub async fn create_package_version_for_test(
    &self,
    new_package_version: NewPackageVersion<'_>,
  ) -> Result<PackageVersionWithNewerVersionsCount> {
    query_as::<PackageVersionWithNewerVersionsCount>(concatcp!("INSERT INTO package_versions (scope, name, version, user_id, readme_path, exports, uses_npm, meta)
      VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
      RETURNING ", PACKAGE_VERSION_SELECT, ",
      ", NEWER_VERSIONS_COUNT_SUBQUERY), &[&new_package_version.scope, &new_package_version.name, &new_package_version.version, &new_package_version.user_id, &new_package_version.readme_path, &new_package_version.exports, &new_package_version.uses_npm, &new_package_version.meta])
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
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

    let package_version = query_as::<PackageVersion>(
      concatcp!(
        "UPDATE package_versions
      SET is_yanked = $4
      WHERE scope = $1 AND name = $2 AND version = $3
      RETURNING ",
        PACKAGE_VERSION_SELECT
      ),
      &[&scope, &name, &version, &yank],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok(package_version)
  }

  #[instrument(name = "Database::delete_package_version", skip(self), err)]
  pub async fn delete_package_version(
    &self,
    actor_id: &Uuid,
    is_sudo: bool,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<bool> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "delete_package_version",
      json!({
      "scope": scope,
      "name": name,
      "version": version,
      }),
    )
    .await?;

    let res = query(r#"DELETE FROM package_versions WHERE scope = $1 AND name = $2 AND version = $3"#, &[&scope, &name, &version])
    .execute(&tx)
    .await?;

    tx.commit().await?;

    Ok(res > 0)
  }

  #[instrument(
    name = "Database::list_package_dependent_constraints",
    skip(self),
    err
  )]
  pub async fn list_package_dependent_constraints(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Vec<String>> {
    let jsr_name = format!("@{scope}/{name}");
    let npm_name = crate::npm::NpmMappedJsrPackageName {
      scope,
      package: name,
    }
    .to_string();
    let constraints = query_scalar(
      r#"SELECT DISTINCT dependency_constraint
      FROM package_version_dependencies
      WHERE (dependency_kind = 'jsr' AND dependency_name = $1)
        OR (dependency_kind = 'npm' AND dependency_name = $2)"#,
      &[&jsr_name, &npm_name],
    )
    .fetch_all(&self.pool)
    .await?;

    Ok(constraints)
  }

  #[instrument(name = "Database::list_package_files", skip(self), err)]
  pub async fn list_package_files(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<Vec<PackageFile>> {
    query_as::<PackageFile>(
      concatcp!(
        "SELECT ",
        PACKAGE_FILE_SELECT,
        "
      FROM package_files
      WHERE scope = $1 AND name = $2 AND version = $3"
      ),
      &[&scope, &name, &version],
    )
    .fetch_all(&self.pool)
    .await
  }

  #[cfg(test)]
  #[instrument(name = "Database::create_package_file_for_test", skip(
    self,
    new_package_file
  ), err, fields(package_file.scope = %new_package_file.scope, package_file.name = %new_package_file.name, package_file.version = %new_package_file.version, package_file.path = %new_package_file.path, package_file.size = new_package_file.size, package_file.checksum = new_package_file.checksum
  ))]
  pub async fn create_package_file_for_test(
    &self,
    new_package_file: NewPackageFile<'_>,
  ) -> Result<PackageFile> {
    query_as::<PackageFile>(
      concatcp!(
        "INSERT INTO package_files (scope, name, version, path, size, checksum)
      VALUES ($1, $2, $3, $4, $5, $6)
      RETURNING ",
        PACKAGE_FILE_SELECT
      ),
      &[
        &new_package_file.scope,
        &new_package_file.name,
        &new_package_file.version,
        &new_package_file.path,
        &new_package_file.size,
        &new_package_file.checksum,
      ],
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
    query_as::<NpmTarball>(concatcp!("INSERT INTO npm_tarballs (scope, name, version, revision, sha1, sha512, size)
      VALUES ($1, $2, $3, $4, $5, $6, $7)
      RETURNING ", NPM_TARBALL_SELECT), &[&new_npm_tarball.scope, &new_npm_tarball.name, &new_npm_tarball.version, &new_npm_tarball.revision, &new_npm_tarball.sha1, &new_npm_tarball.sha512, &new_npm_tarball.size])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_scope_member", skip(self), err)]
  pub async fn get_scope_member(
    &self,
    scope: &ScopeName,
    user_id: Uuid,
  ) -> Result<Option<ScopeMember>> {
    query_as::<ScopeMember>(
      concatcp!(
        "SELECT ",
        SCOPE_MEMBER_SELECT,
        " FROM scope_members WHERE scope = $1 AND user_id = $2"
      ),
      &[&scope, &user_id],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::list_scope_members", skip(self), err)]
  pub async fn list_scope_members(
    &self,
    scope: &ScopeName,
  ) -> Result<Vec<(ScopeMember, UserPublic)>> {
    query(
      concatcp!(
        "SELECT ",
        SCOPE_MEMBER_SELECT_JOINED,
        ",
        ",
        USER_PUBLIC_SELECT_JOINED,
        "
      FROM scope_members
      LEFT JOIN users ON scope_members.user_id = users.id
      WHERE scope = $1
      ORDER BY users.name ASC"
      ),
      &[&scope],
    )
    .map(|r| {
      let scope_member = ScopeMember::from_row_prefixed(r, "scope_member_")?;
      let user = UserPublic::from_row_prefixed(r, "user_")?;
      Ok((scope_member, user))
    })
    .fetch_all(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_member_scopes_by_user", skip(self), err)]
  pub async fn get_member_scopes_by_user(
    &self,
    id: &Uuid,
  ) -> Result<Vec<Scope>> {
    query_as::<Scope>(
      r#"SELECT
      scopes.scope,
      scopes.description,
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
      &[&id],
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "add_scope_invite",
      json!({
          "scope": new_scope_invite.scope,
          "target_user_id": new_scope_invite.target_user_id,
      }),
    )
    .await?;

    let scope_invite = query_as::<ScopeInvite>(
      concatcp!(
        "INSERT INTO scope_invites (scope, target_user_id, requesting_user_id)
      VALUES ($1, $2, $3)
      RETURNING ",
        SCOPE_INVITE_SELECT
      ),
      &[
        &new_scope_invite.scope,
        &new_scope_invite.target_user_id,
        &new_scope_invite.requesting_user_id,
      ],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok(scope_invite)
  }

  #[cfg(test)]
  #[instrument(name = "Database::add_user_to_scope", skip(
    self,
    new_scope_member
  ), err, fields(scope_member.scope = %new_scope_member.scope, scope_member.user_id = %new_scope_member.user_id, scope_member.is_admin = new_scope_member.is_admin
  ))]
  pub async fn add_user_to_scope(
    &self,
    new_scope_member: NewScopeMember<'_>,
  ) -> Result<ScopeMember> {
    query_as::<ScopeMember>(
      concatcp!(
        "INSERT INTO scope_members (scope, user_id, is_admin)
      VALUES ($1, $2, $3)
      RETURNING ",
        SCOPE_MEMBER_SELECT
      ),
      &[
        &new_scope_member.scope,
        &new_scope_member.user_id,
        &new_scope_member.is_admin,
      ],
    )
    .fetch_one(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_scope_invites_by_user", skip(self), err)]
  pub async fn get_scope_invites_by_user(
    &self,
    target_user: &Uuid,
  ) -> Result<Vec<(ScopeInvite, UserPublic, UserPublic)>> {
    query(concatcp!("SELECT ", SCOPE_INVITE_SELECT_JOINED, "
      FROM scope_invites
      LEFT JOIN users AS target_user ON scope_invites.target_user_id = target_user.id
      LEFT JOIN users AS requesting_user ON scope_invites.requesting_user_id = requesting_user.id
      WHERE target_user_id = $1"), &[&target_user])
      .map(|r| {
        let scope_invite = ScopeInvite::from_row_prefixed(r, "scope_invite_")?;
        let target_user = UserPublic::from_row_prefixed(r, "target_user_")?;
        let requesting_user = UserPublic::from_row_prefixed(r, "requesting_user_")?;
        Ok((scope_invite, target_user, requesting_user))
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_scope_invites_by_scope", skip(self), err)]
  pub async fn get_scope_invites_by_scope(
    &self,
    scope: &ScopeName,
  ) -> Result<Vec<(ScopeInvite, UserPublic, UserPublic)>> {
    query(concatcp!("SELECT ", SCOPE_INVITE_SELECT_JOINED, "
      FROM scope_invites
      LEFT JOIN users AS target_user ON scope_invites.target_user_id = target_user.id
      LEFT JOIN users AS requesting_user ON scope_invites.requesting_user_id = requesting_user.id
      WHERE scope = $1"), &[&scope])
      .map(|r| {
        let scope_invite = ScopeInvite::from_row_prefixed(r, "scope_invite_")?;
        let target_user = UserPublic::from_row_prefixed(r, "target_user_")?;
        let requesting_user = UserPublic::from_row_prefixed(r, "requesting_user_")?;
        Ok((scope_invite, target_user, requesting_user))
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let res = query(
      r#"DELETE FROM scope_invites WHERE target_user_id = $1 AND scope = $2"#,
      &[&target_user_id, &scope],
    )
    .execute(&tx)
    .await?;
    if res != 1 {
      return Ok(None);
    }

    let member = query_as::<ScopeMember>(
      concatcp!(
        "INSERT INTO scope_members (scope, user_id) VALUES ($1, $2)
      RETURNING ",
        SCOPE_MEMBER_SELECT
      ),
      &[&scope, &target_user_id],
    )
    .fetch_one(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "delete_scope_invite",
      json!({
          "scope": scope,
          "target_user_id": target_user_id,
      }),
    )
    .await?;

    query(
      r#"DELETE FROM scope_invites WHERE target_user_id = $1 AND scope = $2"#,
      &[&target_user_id, &scope],
    )
    .execute(&tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "delete_package",
      json!({
          "scope": scope,
      }),
    )
    .await?;

    let status = query_scalar::<i64>(r#"SELECT count(*) FROM publishing_tasks WHERE package_scope = $1 AND package_name = $2 AND status != 'failure'"#, &[&scope, &name])
      .fetch_one(&tx)
      .await?;
    if status > 0 {
      return Ok(false);
    }

    let res = query(
      r#"DELETE FROM packages WHERE scope = $1 AND name = $2"#,
      &[&scope, &name],
    )
    .execute(&tx)
    .await;

    match res {
      Ok(res) => {
        let success = res > 0;
        if success {
          tx.commit().await?;
        }
        Ok(success)
      }
      Err(err) => {
        if err.is_foreign_key_violation() {
          return Ok(false);
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      actor_id,
      is_sudo,
      "delete_scope",
      json!({
        "scope": scope,
      }),
    )
    .await?;

    query(r#"DELETE FROM scope_members WHERE scope = $1"#, &[&scope])
      .execute(&tx)
      .await?;

    query(r#"DELETE FROM scope_invites WHERE scope = $1"#, &[&scope])
      .execute(&tx)
      .await?;

    let res = query(r#"DELETE FROM scopes WHERE scope = $1"#, &[&scope])
      .execute(&tx)
      .await;
    match res {
      Ok(res) => {
        let success = res > 0;
        if success {
          tx.commit().await?;
        }
        Ok(success)
      }
      Err(err) => {
        if err.is_foreign_key_violation() {
          return Ok(false);
        }
        Err(err)
      }
    }
  }

  pub async fn transfer_scope<'a>(
    &self,
    scope: &ScopeName,
    is_creator: bool,
    tx: &Transaction<'a>,
  ) -> Result<Option<ScopeMemberUpdateResult>> {
    let admins_n = query_scalar::<i64>(r#"SELECT COUNT(user_id) FROM scope_members WHERE scope = $1 AND is_admin = true"#, &[&scope])
      .fetch_one(tx)
      .await?;
    if admins_n == 0 {
      return Ok(Some(ScopeMemberUpdateResult::TargetIsLastAdmin));
    }

    let maybe_available_admin_id = query_scalar::<Uuid>(r#"
      SELECT user_id
      FROM (
        SELECT scope_members.user_id, users.scope_limit, scope_members.created_at, (SELECT COUNT(created_at) FROM scopes WHERE creator = users.id) as "scope_usage"
        FROM scope_members
        LEFT JOIN users ON scope_members.user_id = users.id
        WHERE scope_members.scope = $1 AND scope_members.is_admin = true
      ) AS subquery
      WHERE "scope_usage" < scope_limit
      ORDER BY created_at LIMIT 1;
      "#, &[&scope])
      .fetch_optional(tx)
      .await?;
    let Some(new_creator_id) = maybe_available_admin_id else {
      return Ok(Some(ScopeMemberUpdateResult::TargetIsLastTransferableAdmin));
    };

    if is_creator {
      let _ = query(
        r#"UPDATE scopes SET creator = $1 WHERE scope = $2"#,
        &[&new_creator_id, &scope],
      )
      .execute(tx)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
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

    let maybe_scope_member = query(
      r#"UPDATE scope_members
      SET is_admin = $1
      WHERE scope = $2 AND user_id = $3
      RETURNING scope, user_id, is_admin, updated_at, created_at,
      (SELECT creator FROM scopes WHERE scope = $2) AS "scope_creator""#,
      &[&is_admin, &scope, &user_id],
    )
    .map(|r| {
      Ok((
        ScopeMember::from_row(r)?,
        r.try_get::<_, Uuid>("user_id")?
          == r.try_get::<_, Uuid>("scope_creator")?,
      ))
    })
    .fetch_optional(&tx)
    .await?;

    let Some((scope_member, is_creator)) = maybe_scope_member else {
      return Ok(ScopeMemberUpdateResult::TargetNotMember);
    };

    if !scope_member.is_admin
      && let Some(result) = self.transfer_scope(scope, is_creator, &tx).await?
    {
      return Ok(result);
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let maybe_scope_member = query(
      r#"DELETE FROM scope_members WHERE scope = $1 AND user_id = $2
      RETURNING scope, user_id, is_admin, updated_at, created_at,
      (SELECT creator FROM scopes WHERE scope = $1) AS "scope_creator""#,
      &[&scope, &user_id],
    )
    .map(|r| {
      Ok((
        ScopeMember::from_row(r)?,
        r.try_get::<_, Uuid>("user_id")?
          == r.try_get::<_, Uuid>("scope_creator")?,
      ))
    })
    .fetch_optional(&tx)
    .await?;
    let Some((scope_member, is_creator)) = maybe_scope_member else {
      return Ok(ScopeMemberUpdateResult::TargetNotMember);
    };

    if let Some(result) = self.transfer_scope(scope, is_creator, &tx).await? {
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    // only allow insert if no non status==failure tasks exist
    let already_processing = query(concatcp!("SELECT
        ", PUBLISHING_TASK_SELECT_JOINED, ",
        ", USER_PUBLIC_SELECT_JOINED, "
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      WHERE package_scope = $1 AND package_name = $2 AND package_version = $3 AND status != 'failure'
      LIMIT 1"), &[&task.package_scope, &task.package_name, &task.package_version]).map(|r| {
      let task = PublishingTask::from_row_prefixed(r, "task_")?;

      let user = UserPublic::from_row_optional(r, "user_", "id")?;

      Ok((task, user))
    })

      .fetch_optional(&tx)
      .await?;
    if let Some((existing_task, existing_user)) = already_processing {
      let stale_threshold =
        Utc::now() - Duration::seconds(STALE_PUBLISHING_TASK_SECS);
      let is_stale_processing = existing_task.status
        == PublishingTaskStatus::Processing
        && existing_task.updated_at < stale_threshold;

      if !is_stale_processing {
        return Ok(CreatePublishingTaskResult::Exists((
          existing_task,
          existing_user,
        )));
      }

      // Failing a stranded `processing` task is safe: its version row never
      // committed (the finalize transaction is atomic). `processed` tasks
      // have committed data and are left for the requeue reaper.
      let stale_error = PublishingTaskError {
        code: "stale".to_string(),
        message: "task stranded in processing".to_string(),
      };
      let res = query(
        "UPDATE publishing_tasks
         SET status = $1, error = $2
         WHERE id = $3 AND status = $4",
        &[
          &PublishingTaskStatus::Failure,
          &stale_error,
          &existing_task.id,
          &PublishingTaskStatus::Processing,
        ],
      )
      .execute(&tx)
      .await?;
      // Lost a race: the reaper requeued the task (or a worker advanced it)
      // between our read and this update. The task is active again, so hand
      // it back instead of creating a duplicate.
      if res == 0 {
        return Ok(CreatePublishingTaskResult::Exists((
          existing_task,
          existing_user,
        )));
      }
    }

    let task = query(concatcp!("WITH task AS (
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
          task.id as \"task_id\",
          task.status::text as \"task_status\",
          task.error as \"task_error\",
          task.user_id as \"task_user_id\",
          task.package_scope as \"task_package_scope\",
          task.package_name as \"task_package_name\",
          task.package_version as \"task_package_version\",
          task.config_file as \"task_config_file\",
          task.created_at as \"task_created_at\",
          task.updated_at as \"task_updated_at\",
        ", USER_PUBLIC_SELECT_JOINED, "
        FROM task
        LEFT JOIN users ON task.user_id = users.id"), &[&task.user_id, &task.package_scope, &task.package_name, &task.package_version, &task.config_file])
      .map(|r| {
        let task = PublishingTask::from_row_prefixed(r, "task_")?;

        let user = UserPublic::from_row_optional(r, "user_", "id")?;

        Ok((task, user))
      })

      .fetch_one(&tx)
      .await?;

    let publish_attempts_per_week_limit = query_scalar::<i32>(
      r#"
      SELECT publish_attempts_per_week_limit FROM scopes WHERE scope = $1;
      "#,
      &[&task.0.package_scope],
    )
    .fetch_one(&tx)
    .await?;

    let publish_attempts_from_last_week = query_scalar::<i64>(r#"
      SELECT COUNT(created_at) FROM publishing_tasks WHERE package_scope = $1 AND created_at > now() - '1 week'::interval;
      "#, &[&task.0.package_scope])
      .fetch_one(&tx)
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

  #[cfg(test)]
  pub async fn backdate_publishing_task_updated_at(
    &self,
    id: Uuid,
    secs_ago: i64,
  ) -> Result<()> {
    query(
      "UPDATE publishing_tasks
       SET updated_at = now() - ($1::bigint * interval '1 second')
       WHERE id = $2",
      &[&secs_ago, &id],
    )
    .execute(&self.pool)
    .await?;
    Ok(())
  }

  #[instrument(name = "Database::get_publishing_task", skip(self), err)]
  pub async fn get_publishing_task(
    &self,
    id: Uuid,
  ) -> Result<Option<(PublishingTask, Option<UserPublic>)>> {
    query(
      concatcp!(
        "SELECT
        ",
        PUBLISHING_TASK_SELECT_JOINED,
        ",
        ",
        USER_PUBLIC_SELECT_JOINED,
        "
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      WHERE publishing_tasks.id = $1"
      ),
      &[&id],
    )
    .map(|r| {
      let task = PublishingTask::from_row_prefixed(r, "task_")?;

      let user = UserPublic::from_row_optional(r, "user_", "id")?;

      Ok((task, user))
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let search = format!("%{}%", maybe_search_query.unwrap_or(""));
    let sql = sort_by!(
      maybe_sort,
      concatcp!("SELECT
        ", PUBLISHING_TASK_SELECT_JOINED, ", ", USER_PUBLIC_SELECT_JOINED, "
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      WHERE publishing_tasks.package_scope ILIKE $1
         OR publishing_tasks.package_name ILIKE $1
         OR publishing_tasks.package_version ILIKE $1
      ORDER BY "),
      " OFFSET $2 LIMIT $3"
      => {
      @timestamps "updated_at", "created_at";
      "status" => "publishing_tasks.status",
      "user" => "users.name",
      "scope" => "publishing_tasks.package_scope",
      "name" => "publishing_tasks.package_name",
      "version" => "publishing_tasks.package_version",
      "updated_at" => "publishing_tasks.updated_at",
      "created_at" => "publishing_tasks.created_at",
    } || "publishing_tasks.created_at DESC");

    let publishing_tasks = query(sql, &[&search, &start, &limit])
      .map(|r| {
        let task = PublishingTask::from_row_prefixed(r, "task_")?;
        let user = UserPublic::from_row_optional(r, "user_", "id")?;

        Ok((task, user))
      })
      .fetch_all(&tx)
      .await?;

    let total_publishing_tasks = query_scalar::<i64>(r#"SELECT COUNT(created_at) FROM publishing_tasks WHERE package_scope ILIKE $1 OR package_name ILIKE $1 OR package_version ILIKE $1;"#, &[&search])
      .fetch_one(&tx)
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
    query(concatcp!("SELECT
        ", PUBLISHING_TASK_SELECT_JOINED, ",
        ", USER_PUBLIC_SELECT_JOINED, "
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      JOIN packages ON publishing_tasks.package_scope = packages.scope AND publishing_tasks.package_name = packages.name
      WHERE publishing_tasks.package_scope = $1 AND publishing_tasks.package_name = $2 AND publishing_tasks.created_at >= packages.created_at
      ORDER BY publishing_tasks.package_version DESC"), &[&scope_name, &package_name])
      .map(|r| {
        let task = PublishingTask::from_row_prefixed(r, "task_")?;

        let user = UserPublic::from_row_optional(r, "user_", "id")?;

        Ok((task, user))
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::get_publishing_task_for_version",
    skip(self),
    err
  )]
  pub async fn get_publishing_task_for_version(
    &self,
    scope_name: &ScopeName,
    package_name: &PackageName,
    version: &Version,
  ) -> Result<(PublishingTask, Option<UserPublic>)> {
    query(concatcp!("SELECT
        ", PUBLISHING_TASK_SELECT_JOINED, ",
        ", USER_PUBLIC_SELECT_JOINED, "
      FROM publishing_tasks
      LEFT JOIN users on publishing_tasks.user_id = users.id
      JOIN packages ON publishing_tasks.package_scope = packages.scope AND publishing_tasks.package_name = packages.name
      WHERE publishing_tasks.package_scope = $1 AND publishing_tasks.package_name = $2 AND publishing_tasks.package_version = $3 AND publishing_tasks.created_at >= packages.created_at
      ORDER BY publishing_tasks.created_at DESC
      LIMIT 1"), &[&scope_name, &package_name, &version])
      .map(|r| {
        let task = PublishingTask::from_row_prefixed(r, "task_")?;

        let user = UserPublic::from_row_optional(r, "user_", "id")?;

        Ok((task, user))
      })
      .fetch_one(&self.pool)
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

    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    if let Some(staff_id) = staff_id {
      audit_log(
        &tx,
        staff_id,
        true,
        "requeue_publishing_task",
        json!({
          "id": id,
        }),
      )
      .await?;
    }

    let task = query_as::<PublishingTask>(
      concatcp!(
        "UPDATE publishing_tasks
      SET status = $1, error = $2
      WHERE id = $3 AND status = $4
      RETURNING ",
        PUBLISHING_TASK_SELECT
      ),
      &[&new_status, &new_error, &id, &prev_status],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok(task)
  }

  /// Claim publishing tasks that have been stuck in a non-terminal state
  /// (`pending`, `processing` or `processed`) for longer than
  /// `stale_after_seconds`.
  ///
  /// A task is normally driven from `pending` to `success` within seconds by
  /// the publish queue. If the queue worker is killed mid-flight (e.g. the
  /// Cloud Run request times out, or a publish is cancelled) a task can be
  /// stranded: `processing` means the version row was never committed, while
  /// `processed` means the version exists but its package-level `meta.json`
  /// was never regenerated, leaving the version invisible to the resolver.
  /// A `pending` task is stranded when the publish request died between
  /// creating the task and enqueueing it, so no worker ever saw it. Any of
  /// these states blocks re-publishing that exact version (see the
  /// `status != 'failure'` guard in `create_publishing_task`). The reaper at
  /// `POST /tasks/requeue_stuck_publishing_tasks` re-drives these.
  #[instrument(
    name = "Database::claim_stale_publishing_tasks",
    skip(self),
    err
  )]
  pub async fn claim_stale_publishing_tasks(
    &self,
    stale_after_seconds: i64,
    abandon_after_seconds: i64,
  ) -> Result<StalePublishingTasks> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let abandon_error = PublishingTaskError {
      code: "internalError".to_string(),
      message: "the registry could not finish processing this publish; please retry, and report the problem at https://github.com/jsr-io/jsr/issues if it keeps happening".to_string(),
    };
    let abandoned = query_scalar(
      r#"UPDATE publishing_tasks
      SET status = 'failure', error = $3
      WHERE status IN ('pending', 'processing')
        AND updated_at < now() - ($1::bigint * interval '1 second')
        AND created_at < now() - ($2::bigint * interval '1 second')
      RETURNING id"#,
      &[&stale_after_seconds, &abandon_after_seconds, &abandon_error],
    )
    .fetch_all(&tx)
    .await?;

    let claimed = query_scalar(r#"UPDATE publishing_tasks
      SET status = CASE WHEN status = 'processing' THEN 'pending' ELSE status END,
        updated_at = now()
      WHERE id IN (
        SELECT id FROM publishing_tasks
        WHERE status IN ('pending', 'processing', 'processed')
          AND updated_at < now() - ($1::bigint * interval '1 second')
        ORDER BY updated_at ASC
        LIMIT 1000
        FOR UPDATE SKIP LOCKED
      )
      RETURNING id"#, &[&stale_after_seconds])
    .fetch_all(&tx)
    .await?;

    tx.commit().await?;

    Ok(StalePublishingTasks { claimed, abandoned })
  }

  #[cfg(test)]
  pub async fn backdate_publishing_task(
    &self,
    id: Uuid,
    created_secs_ago: i64,
    updated_secs_ago: i64,
  ) -> Result<()> {
    query(
      "UPDATE publishing_tasks
      SET created_at = now() - ($2::bigint * interval '1 second'),
        updated_at = now() - ($3::bigint * interval '1 second')
      WHERE id = $1",
      &[&id, &created_secs_ago, &updated_secs_ago],
    )
    .execute(&self.pool)
    .await?;
    Ok(())
  }

  #[instrument(name = "Database::get_oauth_state", skip(self), err)]
  pub async fn get_oauth_state(
    &self,
    csrf_token: &str,
  ) -> Result<Option<OauthState>> {
    query_as::<OauthState>(
      concatcp!(
        "SELECT ",
        OAUTH_STATE_SELECT,
        " FROM oauth_states WHERE csrf_token = $1"
      ),
      &[&csrf_token],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::delete_expired_oauth_states", skip(self), err)]
  pub async fn delete_expired_oauth_states(
    &self,
    older_than: DateTime<Utc>,
  ) -> Result<u64> {
    let result = query(
      "DELETE FROM oauth_states WHERE created_at < $1",
      &[&older_than],
    )
    .execute(&self.pool)
    .await?;
    Ok(result)
  }

  #[instrument(name = "Database::cleanup_download_counts_4h", skip(self), err)]
  pub async fn cleanup_download_counts_4h(
    &self,
    older_than: DateTime<Utc>,
  ) -> Result<u64> {
    let result = query(
      "DELETE FROM version_download_counts_4h WHERE time_bucket < $1",
      &[&older_than],
    )
    .execute(&self.pool)
    .await?;
    Ok(result)
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
    query_as::<OauthState>(concatcp!("INSERT INTO oauth_states (csrf_token, pkce_code_verifier, redirect_url, user_id)
      VALUES ($1, $2, $3, $4)
      RETURNING ", OAUTH_STATE_SELECT), &[&new_oauth_state.csrf_token, &new_oauth_state.pkce_code_verifier, &new_oauth_state.redirect_url, &new_oauth_state.user_id])
    .fetch_one(&self.pool)
    .await
  }

  #[instrument(name = "Database::delete_oauth_state", skip(self), err)]
  pub async fn delete_oauth_state(
    &self,
    csrf_token: &str,
  ) -> Result<Option<OauthState>> {
    query_as::<OauthState>(
      concatcp!(
        "DELETE FROM oauth_states WHERE csrf_token = $1
      RETURNING ",
        OAUTH_STATE_SELECT
      ),
      &[&csrf_token],
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
    query_as::<GithubIdentity>(concatcp!("INSERT INTO github_identities (github_id, access_token, access_token_expires_at, refresh_token, refresh_token_expires_at) VALUES ($1, $2, $3, $4, $5)
      ON CONFLICT (github_id) DO
      UPDATE SET access_token = $2, access_token_expires_at = $3, refresh_token = $4, refresh_token_expires_at = $5
      RETURNING ", GITHUB_IDENTITY_SELECT), &[&new_github_identity.github_id, &new_github_identity.access_token, &new_github_identity.access_token_expires_at, &new_github_identity.refresh_token, &new_github_identity.refresh_token_expires_at])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_github_identity", skip(self), err)]
  pub async fn get_github_identity(
    &self,
    github_id: i64,
  ) -> Result<GithubIdentity> {
    query_as::<GithubIdentity>(
      concatcp!(
        "SELECT ",
        GITHUB_IDENTITY_SELECT,
        " FROM github_identities WHERE github_id = $1"
      ),
      &[&github_id],
    )
    .fetch_one(&self.pool)
    .await
  }

  #[instrument(name = "Database::delete_github_identity", skip(self), err)]
  pub async fn delete_github_identity(
    &self,
    github_id: i64,
  ) -> Result<GithubIdentity> {
    query_as::<GithubIdentity>("DELETE FROM github_identities WHERE github_id = $1
      RETURNING github_id, access_token, access_token_expires_at, refresh_token, refresh_token_expires_at, updated_at, created_at", &[&github_id])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::insert_gitlab_identity", skip(
    self,
    new_gitlab_identity
  ), err, fields(gitlab_identity.gitlab_id = new_gitlab_identity.gitlab_id))]
  pub async fn upsert_gitlab_identity(
    &self,
    new_gitlab_identity: NewGitlabIdentity,
  ) -> Result<GitlabIdentity> {
    query_as::<GitlabIdentity>("INSERT INTO gitlab_identities (gitlab_id, access_token, access_token_expires_at, refresh_token) VALUES ($1, $2, $3, $4)
      ON CONFLICT (gitlab_id) DO
      UPDATE SET access_token = $2, access_token_expires_at = $3, refresh_token = $4
      RETURNING gitlab_id, access_token, access_token_expires_at, refresh_token, updated_at, created_at", &[&new_gitlab_identity.gitlab_id, &new_gitlab_identity.access_token, &new_gitlab_identity.access_token_expires_at, &new_gitlab_identity.refresh_token])
      .fetch_one(&self.pool)
      .await
  }

  #[cfg(not(test))]
  #[instrument(name = "Database::get_gitlab_identity", skip(self), err)]
  pub async fn get_gitlab_identity(
    &self,
    gitlab_id: i64,
  ) -> Result<GitlabIdentity> {
    query_as::<GitlabIdentity>("SELECT gitlab_id, access_token, access_token_expires_at, refresh_token, updated_at, created_at
      FROM gitlab_identities
      WHERE gitlab_id = $1", &[&gitlab_id])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::delete_gitlab_identity", skip(self), err)]
  pub async fn delete_gitlab_identity(
    &self,
    gitlab_id: i64,
  ) -> Result<GitlabIdentity> {
    query_as::<GitlabIdentity>("DELETE FROM gitlab_identities WHERE gitlab_id = $1
      RETURNING gitlab_id, access_token, access_token_expires_at, refresh_token, updated_at, created_at", &[&gitlab_id])
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
    query_as::<Token>(concatcp!("INSERT INTO tokens (hash, user_id, type, description, expires_at, permissions)
      VALUES ($1, $2, $3, $4, $5, $6)
      RETURNING ", TOKEN_SELECT), &[&new_token.hash, &new_token.user_id, &new_token.r#type, &new_token.description, &new_token.expires_at, &new_token.permissions])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::upsert_service_account_token",
    skip(self, hash),
    err
  )]
  pub async fn upsert_service_account_token(
    &self,
    hash: Option<String>,
  ) -> Result<()> {
    let user_id = Uuid::nil();
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;
    // Only delete tokens that don't match the current hash, and insert with
    // ON CONFLICT DO NOTHING, so concurrently starting replicas never leave
    // a window where no service account token exists.
    query(
      "DELETE FROM tokens WHERE user_id = $1 AND hash IS DISTINCT FROM $2",
      &[&user_id, &hash.as_deref()],
    )
    .execute(&tx)
    .await?;

    if let Some(hash) = hash {
      query(r#"INSERT INTO tokens (hash, user_id, type, description, expires_at, permissions)
        VALUES ($1, $2, 'personal', 'service account token', NULL, NULL)
        ON CONFLICT (hash) DO NOTHING"#, &[&hash, &user_id])
        .execute(&tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
  }

  #[instrument(name = "Database::get_token_by_hash", skip(self), err)]
  pub async fn get_token_by_hash(&self, hash: &str) -> Result<Option<Token>> {
    query_as::<Token>(
      concatcp!("SELECT ", TOKEN_SELECT, " FROM tokens WHERE hash = $1"),
      &[&hash],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::list_token", skip(self), err)]
  pub async fn list_tokens(&self, user_id: Uuid) -> Result<Vec<Token>> {
    // list a user's tokens where the expiration date is at most 1 day in the past
    query_as::<Token>(concatcp!("SELECT ", TOKEN_SELECT, "
      FROM tokens
      WHERE user_id = $1 AND (expires_at > now() - interval '1 day' OR expires_at IS NULL)
      ORDER BY expires_at DESC NULLS FIRST, created_at DESC"), &[&user_id])
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::delete_token", skip(self), err)]
  pub async fn delete_token(&self, user_id: Uuid, id: Uuid) -> Result<bool> {
    let res = query(
      r#"DELETE FROM tokens WHERE user_id = $1 ANd id = $2"#,
      &[&user_id, &id],
    )
    .execute(&self.pool)
    .await?;
    Ok(res > 0)
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
    query_as::<Authorization>(concatcp!("INSERT INTO authorizations (exchange_token, code, challenge, permissions, expires_at)
      VALUES ($1, $2, $3, $4, $5)
      RETURNING ", AUTHORIZATION_SELECT), &[&new_authorization.exchange_token, &new_authorization.code, &new_authorization.challenge, &new_authorization.permissions, &new_authorization.expires_at])
      .fetch_one(&self.pool)
      .await
  }

  #[instrument(name = "Database::get_authorization_by_code", skip(self), err)]
  pub async fn get_authorization_by_code(
    &self,
    code: &str,
  ) -> Result<Option<Authorization>> {
    query_as::<Authorization>(
      concatcp!(
        "SELECT ",
        AUTHORIZATION_SELECT,
        " FROM authorizations WHERE code = $1"
      ),
      &[&code],
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let maybe_authorization = query_as::<Authorization>(
      concatcp!(
        "DELETE FROM authorizations WHERE exchange_token = $1
      RETURNING ",
        AUTHORIZATION_SELECT
      ),
      &[&exchange_token],
    )
    .fetch_optional(&tx)
    .await?;

    if let Some(authorization) = &maybe_authorization
      && authorization.user_id.is_some()
    {
      tx.commit().await?;
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
    let res = query(
      r#"UPDATE authorizations
      SET approved = $1, user_id = $2
      WHERE code = $3 AND approved IS NULL"#,
      &[&approved, &user_id, &code],
    )
    .execute(&self.pool)
    .await?;
    Ok(res > 0)
  }

  #[instrument(name = "Database::list_package_dependencies", skip(self), err)]
  pub async fn list_package_dependencies(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Vec<PackageVersionDependency>> {
    query_as::<PackageVersionDependency>(concatcp!("SELECT ", PACKAGE_VERSION_DEPENDENCY_SELECT, "
      FROM package_version_dependencies
      WHERE package_scope = $1 AND package_name = $2
      ORDER BY dependency_kind ASC, dependency_name ASC, dependency_constraint ASC, dependency_path ASC"), &[&scope, &name])
      .fetch_all(&self.pool)
      .await
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
    query_as::<PackageVersionDependency>(concatcp!("SELECT ", PACKAGE_VERSION_DEPENDENCY_SELECT, "
      FROM package_version_dependencies
      WHERE package_scope = $1 AND package_name = $2 AND package_version = $3
      ORDER BY dependency_kind ASC, dependency_name ASC, dependency_constraint ASC, dependency_path ASC"), &[&scope, &name, &version])
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_package_dependents", skip(self), err)]
  pub async fn list_package_dependents(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    start: i64,
    limit: i64,
    versions_per_package_limit: i64,
  ) -> Result<(usize, Vec<Dependent>)> {
    // Dependents may depend on this package either directly via `jsr:`, or
    // through the npm compatibility layer via `npm:@jsr/scope__name`.
    let jsr_name = format!("@{scope}/{name}");
    let npm_name = crate::npm::NpmMappedJsrPackageName {
      scope,
      package: name,
    }
    .to_string();
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;
    let dependents = query_as::<Dependent>(
      r#"
      SELECT
        package_scope as "scope",
        package_name as "name",
        (ARRAY_AGG(DISTINCT package_version))[:$5] as "versions",
        COUNT(DISTINCT package_version) as "total_versions"
      FROM
        package_version_dependencies
      WHERE
        (dependency_kind = 'jsr' AND dependency_name = $1)
        OR (dependency_kind = 'npm' AND dependency_name = $2)
      GROUP BY package_scope, package_name
      ORDER BY package_scope ASC, package_name ASC OFFSET $3 LIMIT $4;
      "#,
      &[
        &jsr_name,
        &npm_name,
        &start,
        &limit,
        &(versions_per_package_limit as i32),
      ],
    )
    .fetch_all(&tx)
    .await?;

    let total_unique_package_dependents = query_scalar::<i64>(
      r#"SELECT COUNT(*) FROM (
        SELECT DISTINCT package_scope, package_name
        FROM package_version_dependencies
        WHERE (dependency_kind = 'jsr' AND dependency_name = $1)
          OR (dependency_kind = 'npm' AND dependency_name = $2)
      ) t;"#,
      &[&jsr_name, &npm_name],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok((total_unique_package_dependents as usize, dependents))
  }

  #[instrument(name = "Database::count_package_dependents", skip(self), err)]
  pub async fn count_package_dependents(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<usize> {
    let jsr_name = format!("@{scope}/{name}");
    let npm_name = crate::npm::NpmMappedJsrPackageName {
      scope,
      package: name,
    }
    .to_string();
    let total_unique_package_dependents = query_scalar::<i64>(
      r#"SELECT COUNT(*) FROM (
        SELECT DISTINCT package_scope, package_name
        FROM package_version_dependencies
        WHERE (dependency_kind = 'jsr' AND dependency_name = $1)
          OR (dependency_kind = 'npm' AND dependency_name = $2)
      ) t;"#,
      &[&jsr_name, &npm_name],
    )
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
    let total_package_dependencies = query_scalar::<i64>(
      r#"SELECT COUNT(DISTINCT dependency_name)
      FROM package_version_dependencies
      WHERE package_scope = $1 AND package_name = $2 AND package_version = $3"#,
      &[&scope, &name, &version],
    )
    .fetch_one(&self.pool)
    .await?;

    Ok(total_package_dependencies as usize)
  }

  #[instrument(name = "Database::check_bad_word", skip(self), err)]
  pub async fn check_is_bad_word(&self, word: &str) -> Result<bool> {
    let res = query("SELECT * FROM bad_words WHERE word = $1", &[&word])
      .fetch_optional(&self.pool)
      .await?;
    Ok(res.is_some())
  }

  #[cfg(test)]
  #[instrument(name = "Database::add_bad_word_for_test", skip(self), err)]
  pub async fn add_bad_word_for_test(&self, word: &str) -> Result<()> {
    query("INSERT INTO bad_words (word) VALUES ($1)", &[&word])
      .execute(&self.pool)
      .await?;

    Ok(())
  }

  #[cfg(test)]
  #[instrument(
    name = "Database::set_package_version_created_at_for_test",
    skip(self),
    err
  )]
  pub async fn set_package_version_created_at_for_test(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
    created_at: DateTime<Utc>,
  ) -> Result<()> {
    query(r#"UPDATE package_versions SET created_at = $4 WHERE scope = $1 AND name = $2 AND version = $3"#, &[&scope, &name, &version, &created_at])
    .execute(&self.pool)
    .await?;

    Ok(())
  }

  #[instrument(
    name = "Database::list_npm_tarballs_for_version",
    skip(self),
    err
  )]
  pub async fn list_npm_tarballs_for_version(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<Vec<NpmTarball>> {
    query_as::<NpmTarball>(
      concatcp!(
        "SELECT ",
        NPM_TARBALL_SELECT,
        " FROM npm_tarballs
      WHERE scope = $1 AND name = $2 AND version = $3"
      ),
      &[&scope, &name, &version],
    )
    .fetch_all(&self.pool)
    .await
  }

  #[instrument(name = "Database::get_npm_tarball", skip(self), err)]
  pub async fn get_npm_tarball(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
    revision: i32,
  ) -> Result<Option<NpmTarball>> {
    query_as::<NpmTarball>(
      concatcp!(
        "SELECT ",
        NPM_TARBALL_SELECT,
        " FROM npm_tarballs
      WHERE scope = $1 AND name = $2 AND version = $3 AND revision = $4
      LIMIT 1"
      ),
      &[&scope, &name, &version, &revision],
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
    query(r#"SELECT scope, name, version
      FROM package_versions
      WHERE NOT EXISTS (
        SELECT 1
        FROM npm_tarballs
        WHERE npm_tarballs.scope = package_versions.scope AND npm_tarballs.name = package_versions.name AND npm_tarballs.version = package_versions.version AND npm_tarballs.revision = $1
      )
      ORDER BY created_at ASC
      LIMIT 1000
      "#, &[&current_revision])
      .map(|r| {
        Ok((r.try_get("scope")?, r.try_get("name")?, r.try_get("version")?))
      })
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(name = "Database::list_all_scopes_for_sitemap", skip(self), err)]
  #[allow(clippy::type_complexity)]
  pub async fn list_all_scopes_for_sitemap(
    &self,
  ) -> Result<Vec<(ScopeName, DateTime<Utc>, Option<DateTime<Utc>>)>> {
    query(r#"
        SELECT
          scope,
          updated_at,
          (SELECT updated_at FROM packages WHERE scope = scope ORDER BY updated_At DESC LIMIT 1) as "latest_package_created_at"
        FROM scopes
        ORDER BY scope ASC
        LIMIT 50000
      "#, &[])
      .map(|r| Ok((r.try_get("scope")?, r.try_get("updated_at")?, r.try_get("latest_package_created_at")?)))
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
    query(r#"SELECT
        scope, name, updated_at,
        (SELECT created_at FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) as "latest_version_updated_at"
      FROM packages
      WHERE (SELECT version FROM package_versions WHERE scope = scope AND name = name ORDER BY version DESC LIMIT 1) IS NOT NULL
      ORDER BY scope ASC, name ASC
      LIMIT 50000"#, &[])
      .map(|r| {
        Ok((
          r.try_get("scope")?,
          r.try_get("name")?,
          r.try_get("updated_at")?,
          r.try_get("latest_version_updated_at")?,
        ))
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

    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

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
    query(r#"
      INSERT INTO version_download_counts_4h (scope, package, version, time_bucket, kind, count)
      SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TIMESTAMPTZ[], $5::download_kind[], $6::BIGINT[]) as temp(scope, package, version, time_bucket, kind, count)
      WHERE (SELECT COUNT(*) FROM package_versions WHERE package_versions.scope = temp.scope AND package_versions.name = temp.package AND version = temp.version) > 0
      ON CONFLICT (scope, package, version, time_bucket, kind) DO UPDATE SET count = EXCLUDED.count
      "#, &[&scopes, &packages, &versions, &time_buckets, &kinds, &counts])
      .execute(&tx)
      .await?;

    // Compute data in version_download_counts_24h from version_download_counts_4h between smallest_timestamp and largest_timestamp.
    // smallest_timestamp must be truncated down to the nearest day and largest_timestamp must be truncated up to the nearest day.
    query(r#"
      INSERT INTO version_download_counts_24h (scope, package, version, time_bucket, kind, count)
      SELECT scope, package, version, date_trunc('day', time_bucket), kind, SUM(count)
      FROM version_download_counts_4h
      WHERE time_bucket >= date_trunc('day', $1::timestamptz) AND time_bucket < date_trunc('day', $2::timestamptz) + interval '1 day'
      GROUP BY scope, package, version, date_trunc('day', time_bucket), kind
      ON CONFLICT (scope, package, version, time_bucket, kind) DO UPDATE SET count = EXCLUDED.count
      "#, &[&smallest_time_bucket, &largest_time_bucket])
      .execute(&tx)
      .await?;

    // Aggregate version-level 24h counts into package-level 24h counts.
    // Derived from version_download_counts_24h (not 4h) so that the
    // package-level rollup stays correct even after 4h rows are pruned.
    query(r#"
      INSERT INTO package_download_counts_24h (scope, package, time_bucket, kind, count)
      SELECT scope, package, time_bucket, kind, SUM(count)
      FROM version_download_counts_24h
      WHERE time_bucket >= date_trunc('day', $1::timestamptz) AND time_bucket < date_trunc('day', $2::timestamptz) + interval '1 day'
      GROUP BY scope, package, time_bucket, kind
      ON CONFLICT (scope, package, time_bucket, kind) DO UPDATE SET count = EXCLUDED.count
      "#, &[&smallest_time_bucket, &largest_time_bucket])
      .execute(&tx)
      .await?;

    tx.commit().await?;

    Ok(())
  }

  #[instrument(
    name = "Database::get_package_version_downloads_24h",
    skip(self),
    err
  )]
  pub async fn get_package_versions_downloads_24h(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    versions: &[Version],
    start: DateTime<Utc>,
    end: DateTime<Utc>,
  ) -> Result<Vec<VersionDownloadDataPoint>> {
    query_as::<VersionDownloadDataPoint>(r#"
      SELECT version, time_bucket, kind::text AS kind, count::int8 AS count
      FROM version_download_counts_24h
      WHERE scope = $1 AND package = $2 AND version = ANY($3) AND time_bucket >= $4 AND time_bucket < $5
      ORDER BY time_bucket ASC, version DESC
      "#, &[&scope, &name, &versions, &start, &end])
      .fetch_all(&self.pool)
      .await
  }

  #[instrument(
    name = "Database::get_package_version_total_downloads",
    skip(self),
    err
  )]
  pub async fn get_package_version_total_downloads(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<i64> {
    query_scalar(
      r#"
      SELECT COALESCE(SUM(count), 0) as "count"
      FROM version_download_counts_24h
      WHERE scope = $1 AND package = $2 AND version = $3
      "#,
      &[&scope, &name, &version],
    )
    .fetch_one(&self.pool)
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
    query_as::<DownloadDataPoint>(
      r#"
    SELECT time_bucket, kind::text AS kind, count::int8 AS count
    FROM package_download_counts_24h
    WHERE scope = $1 AND package = $2 AND time_bucket >= $3 AND time_bucket < $4
    ORDER BY time_bucket ASC
    "#,
      &[&scope, &name, &start, &end],
    )
    .fetch_all(&self.pool)
    .await
  }

  /// Loads a ticket and, if it has been claimed, the account that owns it.
  async fn load_ticket(
    tx: &Transaction<'_>,
    ticket_id: Uuid,
  ) -> Result<Option<(Ticket, Option<User>)>> {
    query(
      concatcp!(
        "SELECT
            ",
        TICKET_SELECT_JOINED,
        ",
            ",
        USER_SELECT_FULL_JOINED,
        "
        FROM tickets
        LEFT JOIN users ON users.id = tickets.creator
        WHERE tickets.id = $1"
      ),
      &[&ticket_id],
    )
    .map(|r| {
      let ticket = Ticket::from_row_prefixed(r, "ticket_")?;

      let user = User::from_row_optional(r, "user_", "id")?;

      Ok((ticket, user))
    })
    .fetch_optional(tx)
    .await
  }

  /// Loads a ticket's whole conversation: every message, the account that wrote
  /// it where there was one, and any files that came with it.
  async fn load_ticket_messages(
    tx: &Transaction<'_>,
    ticket_id: Uuid,
  ) -> Result<Vec<FullTicketMessage>> {
    let messages = query(
      concatcp!(
        "SELECT
            ",
        TICKET_MESSAGE_SELECT_JOINED,
        ",
            ",
        USER_PUBLIC_SELECT_JOINED,
        "
        FROM ticket_messages
        LEFT JOIN users ON users.id = ticket_messages.author
        WHERE ticket_messages.ticket_id = $1
        ORDER BY ticket_messages.created_at"
      ),
      &[&ticket_id],
    )
    .map(|r| {
      let message = TicketMessage::from_row_prefixed(r, "message_")?;

      let user = UserPublic::from_row_optional(r, "user_", "id")?;

      Ok((message, user))
    })
    .fetch_all(tx)
    .await?;

    let mut attachments = query_as::<TicketAttachment>(r#"SELECT ticket_attachments.id, ticket_attachments.message_id, ticket_attachments.filename, ticket_attachments.content_type, ticket_attachments.size_bytes, ticket_attachments.storage_key, ticket_attachments.created_at
        FROM ticket_attachments
        INNER JOIN ticket_messages ON ticket_messages.id = ticket_attachments.message_id
        WHERE ticket_messages.ticket_id = $1
        ORDER BY ticket_attachments.created_at"#, &[&ticket_id])
    .fetch_all(tx)
    .await?
    .into_iter()
    .fold(
      std::collections::HashMap::<Uuid, Vec<TicketAttachment>>::new(),
      |mut acc, attachment| {
        acc
          .entry(attachment.message_id)
          .or_default()
          .push(attachment);
        acc
      },
    );

    Ok(
      messages
        .into_iter()
        .map(|(message, user)| {
          let files = attachments.remove(&message.id).unwrap_or_default();
          (message, user, files)
        })
        .collect(),
    )
  }

  /// Loads a ticket in full.
  async fn load_full_ticket(
    tx: &Transaction<'_>,
    ticket_id: Uuid,
  ) -> Result<Option<FullTicket>> {
    let Some((ticket, user)) = Self::load_ticket(tx, ticket_id).await? else {
      return Ok(None);
    };
    let messages = Self::load_ticket_messages(tx, ticket_id).await?;
    Ok(Some((ticket, user, messages)))
  }

  /// Queues an email for delivery. The row is the durable record; actually
  /// handing it to Postmark happens in `/tasks/send_email`.
  #[instrument(name = "Database::enqueue_email", skip(self, delivery), err)]
  pub async fn enqueue_email(
    &self,
    delivery: NewEmailDelivery,
  ) -> Result<Uuid> {
    query_scalar(r#"INSERT INTO email_deliveries (to_address, subject, body_text, body_html, message_id, in_reply_to, reference_ids)
          VALUES ($1, $2, $3, $4, $5, $6, $7)
          RETURNING id"#, &[&delivery.to_address, &delivery.subject, &delivery.body_text, &delivery.body_html, &delivery.message_id, &delivery.in_reply_to, &delivery.reference_ids])
    .fetch_one(&self.pool)
    .await
  }

  /// Loads a delivery that still needs sending. Returns `None` if it has
  /// already been sent or abandoned, which is what makes a redelivered task a
  /// no-op rather than a second email.
  #[instrument(name = "Database::get_pending_email_delivery", skip(self), err)]
  pub async fn get_pending_email_delivery(
    &self,
    id: Uuid,
  ) -> Result<Option<EmailDelivery>> {
    query_as::<EmailDelivery>(r#"SELECT id, to_address, subject, body_text, body_html, message_id, in_reply_to, reference_ids, attempts, sent_at, abandoned_at, last_error, updated_at, created_at
        FROM email_deliveries
        WHERE id = $1 AND sent_at IS NULL AND abandoned_at IS NULL"#, &[&id])
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::mark_email_delivery_sent", skip(self), err)]
  pub async fn mark_email_delivery_sent(&self, id: Uuid) -> Result<()> {
    query(r#"UPDATE email_deliveries SET sent_at = now(), attempts = attempts + 1 WHERE id = $1"#, &[&id])
    .execute(&self.pool)
    .await?;
    Ok(())
  }

  /// Records a failed attempt. Past `max_attempts` the delivery is abandoned so
  /// the queue stops retrying it, and the last error is kept so somebody can see
  /// what was lost.
  #[instrument(
    name = "Database::record_email_delivery_failure",
    skip(self),
    err
  )]
  pub async fn record_email_delivery_failure(
    &self,
    id: Uuid,
    error: &str,
    max_attempts: i32,
  ) -> Result<bool> {
    let abandoned = query_scalar(r#"UPDATE email_deliveries
          SET attempts = attempts + 1,
              last_error = $2,
              abandoned_at = CASE WHEN attempts + 1 >= $3 THEN now() ELSE NULL END
          WHERE id = $1
          RETURNING abandoned_at IS NOT NULL as "abandoned""#, &[&id, &error, &max_attempts])
    .fetch_optional(&self.pool)
    .await?;

    Ok(abandoned.unwrap_or(false))
  }

  /// Deliveries that were queued but never reached a terminal state — the Cloud
  /// Tasks hand-off can fail after the row is committed, and nothing else would
  /// ever notice. Re-driven periodically by the sweeper in `tasks.rs`.
  #[instrument(name = "Database::list_stale_email_deliveries", skip(self), err)]
  pub async fn list_stale_email_deliveries(
    &self,
    older_than_secs: i64,
    limit: i64,
  ) -> Result<Vec<Uuid>> {
    query_scalar(
      r#"SELECT id FROM email_deliveries
        WHERE sent_at IS NULL
          AND abandoned_at IS NULL
          AND created_at < now() - make_interval(secs => $1::double precision)
        ORDER BY created_at
        LIMIT $2"#,
      &[&(older_than_secs as f64), &limit],
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let ticket_id = query_scalar(r#"INSERT INTO tickets (kind, creator, meta) VALUES ($1, $2, $3) RETURNING id"#, &[&new_ticket.kind, &user_id, &new_ticket.meta])
    .fetch_one(&tx)
    .await?;

    query(
      r#"INSERT INTO ticket_messages (ticket_id, author, direction, message)
          VALUES ($1, $2, 'inbound', $3)"#,
      &[&ticket_id, &user_id, &new_ticket.message],
    )
    .execute(&tx)
    .await?;

    let (ticket, user, mut messages) = Self::load_full_ticket(&tx, ticket_id)
      .await?
      .expect("ticket was just inserted in this transaction");

    tx.commit().await?;

    let (message, ..) = messages.remove(0);
    let user = user.expect("ticket was just created with a creator");
    Ok((ticket, user, message))
  }

  /// Opens a ticket addressed to `user_id` on behalf of staff.
  ///
  /// The ticket belongs to the user being contacted, so it lists with their own
  /// tickets and their replies are inbound like on any other. The opening
  /// message is outbound from `staff_id`, and the ticket starts out waiting on
  /// the user rather than in the open queue, since nobody is owed a reply by
  /// support yet.
  ///
  /// `email_message_id` is the `Message-ID` the notification email will be
  /// sent under, or `None` when no email goes out.
  #[instrument(
    name = "Database::create_staff_outreach_ticket",
    skip(self, message),
    err
  )]
  pub async fn create_staff_outreach_ticket(
    &self,
    staff_id: &Uuid,
    user_id: Uuid,
    subject: &str,
    meta: serde_json::Value,
    message: &str,
    email_message_id: Option<&str>,
  ) -> Result<(Ticket, User, FullTicketMessage)> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let ticket_id = query_scalar(
      r#"INSERT INTO tickets (kind, creator, subject, meta, status)
          VALUES ('staff_outreach', $1, $2, $3, 'waiting_on_user')
          RETURNING id"#,
      &[&user_id, &subject, &meta],
    )
    .fetch_one(&tx)
    .await?;

    query(r#"INSERT INTO ticket_messages (ticket_id, author, direction, message, email_message_id)
          VALUES ($1, $2, 'outbound', $3, $4)"#, &[&ticket_id, &staff_id, &message, &email_message_id])
    .execute(&tx)
    .await?;

    audit_log(
      &tx,
      staff_id,
      true,
      "create_staff_outreach_ticket",
      json!({
        "ticket_id": ticket_id,
        "user_id": user_id,
      }),
    )
    .await?;

    let (ticket, user, mut messages) = Self::load_full_ticket(&tx, ticket_id)
      .await?
      .expect("ticket was just inserted in this transaction");

    tx.commit().await?;

    // The whole message, author included: unlike a ticket somebody opens
    // themselves, the author here is not the ticket's owner.
    let message = messages.remove(0);
    let user = user.expect("ticket was just created with a creator");
    Ok((ticket, user, message))
  }

  #[instrument(name = "Database::list_tickets", skip(self), err)]
  pub async fn list_tickets(
    &self,
    start: i64,
    limit: i64,
    maybe_search_query: Option<&str>,
    maybe_sort: Option<&str>,
    maybe_status: Option<TicketStatus>,
  ) -> Result<(usize, Vec<FullTicket>)> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let search = format!("%{}%", maybe_search_query.unwrap_or(""));

    // An email-opened ticket has no joined user to search by name, so the
    // reporter's own details, the subject line and the ticket number are
    // searchable too.
    //
    // The status filter is a separate AND so that it narrows the search rather
    // than widening it. `$2` is always bound, and is NULL when no status was
    // asked for.
    const WHERE_CLAUSE: &str = r#"WHERE ($2::ticket_status IS NULL OR tickets.status = $2)
       AND (users.name ILIKE $1
       OR tickets.ticket_number ILIKE $1
       OR tickets.subject ILIKE $1
       OR tickets.reporter_name ILIKE $1
       OR tickets.reporter_email ILIKE $1
       OR EXISTS (
         SELECT 1
         FROM ticket_messages
         WHERE ticket_messages.ticket_id = tickets.id
           AND ticket_messages.message ILIKE $1
       ))"#;

    let sql = sort_by!(
      maybe_sort,
      concatcp!("SELECT ", TICKET_SELECT_JOINED, ", ", USER_SELECT_FULL_JOINED, " FROM tickets
    LEFT JOIN users ON users.id = tickets.creator
    ", WHERE_CLAUSE, "
    ORDER BY "),
      " OFFSET $3 LIMIT $4
"
      => {
      @timestamps "updated_at", "created_at";
      "kind" => "tickets.kind",
      "creator" => "users.name",
      "status" => "tickets.status",
      "updated_at" => "tickets.updated_at",
      "created_at" => "tickets.created_at",
    } || "tickets.status ASC, tickets.created_at DESC");

    let tickets = query(sql, &[&search, &maybe_status, &start, &limit])
      .map(|r| {
        let ticket = Ticket::from_row_prefixed(r, "ticket_")?;
        let user = User::from_row_optional(r, "user_", "id")?;

        Ok((ticket, user))
      })
      .fetch_all(&tx)
      .await?;

    let mut out = Vec::with_capacity(tickets.len());
    for (ticket, user) in tickets {
      let messages = Self::load_ticket_messages(&tx, ticket.id).await?;
      out.push((ticket, user, messages));
    }

    let total_tickets = query_scalar::<i64>(
      concatcp!(
        "SELECT COUNT(tickets.created_at) FROM tickets
    LEFT JOIN users ON users.id = tickets.creator
    ",
        WHERE_CLAUSE
      ),
      &[&search, &maybe_status],
    )
    .fetch_one(&tx)
    .await?;

    tx.commit().await?;

    Ok((total_tickets as usize, out))
  }

  #[instrument(name = "Database::list_tickets_for_user", skip(self), err)]
  pub async fn list_tickets_for_user(
    &self,
    user_id: Uuid,
  ) -> Result<Vec<FullTicket>> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let ticket_ids = query_scalar::<Uuid>(
      r#"SELECT id FROM tickets
        WHERE creator = $1
        ORDER BY tickets.status ASC, tickets.created_at DESC"#,
      &[&user_id],
    )
    .fetch_all(&tx)
    .await?;

    let mut out = Vec::with_capacity(ticket_ids.len());
    for ticket_id in ticket_ids {
      let Some(full) = Self::load_full_ticket(&tx, ticket_id).await? else {
        continue;
      };
      out.push(full);
    }

    tx.commit().await?;

    Ok(out)
  }

  #[instrument(name = "Database::get_ticket", skip(self), err)]
  pub async fn get_ticket(
    &self,
    ticket_id: Uuid,
  ) -> Result<Option<FullTicket>> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;
    let out = Self::load_full_ticket(&tx, ticket_id).await?;
    tx.commit().await?;
    Ok(out)
  }

  /// Finds the ticket an inbound email belongs to by the `Message-ID`s it
  /// references. Ours are unique across all tickets, so any hit is conclusive.
  #[instrument(
    name = "Database::find_ticket_by_email_message_ids",
    skip(self),
    err
  )]
  pub async fn find_ticket_by_email_message_ids(
    &self,
    email_message_ids: &[String],
  ) -> Result<Option<Uuid>> {
    if email_message_ids.is_empty() {
      return Ok(None);
    }

    query_scalar(
      r#"SELECT ticket_id FROM ticket_messages
        WHERE email_message_id = ANY($1)
        ORDER BY created_at DESC
        LIMIT 1"#,
      &[&email_message_ids],
    )
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::find_ticket_by_number", skip(self), err)]
  pub async fn find_ticket_by_number(
    &self,
    ticket_number: &str,
  ) -> Result<Option<Uuid>> {
    query_scalar(
      r#"SELECT id FROM tickets WHERE ticket_number = $1"#,
      &[&ticket_number],
    )
    .fetch_optional(&self.pool)
    .await
  }

  /// Opens a ticket from an inbound email, together with the message it arrived
  /// as and the auto-acknowledgement that will be sent back.
  ///
  /// Returns `None` if the email had already been ingested — the unique index on
  /// `email_message_id` makes a redelivered webhook a no-op rather than a
  /// duplicate ticket.
  #[instrument(name = "Database::create_ticket_from_email", skip(self), err)]
  pub async fn create_ticket_from_email(
    &self,
    new_ticket: NewEmailTicket,
  ) -> Result<Option<FullTicket>> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let ticket_id = query_scalar(r#"INSERT INTO tickets (kind, creator, reporter_email, reporter_name, subject, claim_token, status)
          VALUES ('other', NULL, $1, $2, $3, $4, 'open')
          RETURNING id"#, &[&new_ticket.reporter_email, &new_ticket.reporter_name, &new_ticket.subject, &new_ticket.claim_token])
    .fetch_one(&tx)
    .await?;

    if !insert_inbound_email_message(&tx, ticket_id, &new_ticket.message)
      .await?
    {
      return Ok(None);
    }

    query(r#"INSERT INTO ticket_messages (id, ticket_id, author, direction, message, email_message_id)
          VALUES ($1, $2, NULL, 'outbound', $3, $4)"#, &[&new_ticket.auto_reply.id, &ticket_id, &new_ticket.auto_reply.message, &new_ticket.auto_reply.email_message_id])
    .execute(&tx)
    .await?;

    let full = Self::load_full_ticket(&tx, ticket_id)
      .await?
      .expect("ticket was just inserted in this transaction");

    tx.commit().await?;

    Ok(Some(full))
  }

  /// Appends an inbound email to an existing ticket, reopening it if it had been
  /// closed. Returns `None` if the email had already been ingested.
  #[instrument(name = "Database::ticket_add_email_message", skip(self), err)]
  pub async fn ticket_add_email_message(
    &self,
    ticket_id: Uuid,
    message: NewTicketEmailMessage,
  ) -> Result<Option<FullTicket>> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    if !insert_inbound_email_message(&tx, ticket_id, &message).await? {
      return Ok(None);
    }

    // The ball is back in support's court, and a reply to a closed ticket is a
    // request to reopen it rather than the start of a new conversation. Mail to
    // a ticket already marked spam changes nothing.
    query(r#"UPDATE tickets
          SET status = 'waiting_on_support', closed_at = NULL, updated_at = now()
          WHERE id = $1 AND status <> 'spam'"#, &[&ticket_id])
    .execute(&tx)
    .await?;

    let full = Self::load_full_ticket(&tx, ticket_id).await?;

    tx.commit().await?;

    Ok(full)
  }

  #[instrument(name = "Database::get_ticket_audit_logs", skip(self), err)]
  pub async fn get_ticket_audit_logs(
    &self,
    ticket_id: Uuid,
  ) -> Result<Vec<(AuditLog, UserPublic)>> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let audit_logs = query(
      concatcp!(
        "SELECT
          ",
        AUDIT_LOG_SELECT_JOINED,
        ",
          ",
        USER_PUBLIC_SELECT_JOINED,
        "
        FROM
          audit_logs
        LEFT JOIN
          users ON audit_logs.actor_id = users.id
        WHERE
          audit_logs.meta::text LIKE $1
        ORDER BY audit_logs.created_at DESC"
      ),
      &[&format!("%\"ticket_id\": \"{}\"%", ticket_id)],
    )
    .map(|r| {
      let audit_log = AuditLog::from_row_prefixed(r, "audit_log_")?;

      let user = UserPublic::from_row_prefixed(r, "user_")?;

      Ok((audit_log, user))
    })
    .fetch_all(&tx)
    .await?;

    tx.commit().await?;

    Ok(audit_logs)
  }

  /// Appends a message written through the web UI by a signed-in account.
  ///
  /// `email_message_id` is the `Message-ID` the notification email for this
  /// message will be sent with; storing it up front is what lets the
  /// recipient's reply be threaded back onto this ticket.
  #[instrument(name = "Database::ticket_add_message", skip(self), err)]
  pub async fn ticket_add_message(
    &self,
    id: Uuid,
    author: Uuid,
    email_message_id: Option<&str>,
    message: NewTicketMessage,
  ) -> Result<(TicketMessage, UserPublic)> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    // COALESCE, because a ticket opened by email has no creator until somebody
    // claims it, and `NULL = $2` is NULL rather than false. Without it, every
    // staff reply to an unclaimed ticket fails to decode and 500s.
    let author_is_creator = query_scalar(r#"SELECT COALESCE(creator = $2, false) as "is_creator" FROM tickets WHERE id = $1"#, &[&id, &author])
    .fetch_one(&tx)
    .await?;

    // A note is always from the JSR side, even when the staff member writing it
    // is the person who opened the ticket — which happens whenever staff file
    // one themselves. Treating it as inbound there would both misattribute it
    // and violate the check constraint that ties internal to outbound.
    let direction = if author_is_creator && !message.internal {
      TicketMessageDirection::Inbound
    } else {
      TicketMessageDirection::Outbound
    };

    let message_id = query_scalar::<Uuid>(r#"INSERT INTO ticket_messages (ticket_id, author, direction, message, email_message_id, internal)
          VALUES ($1, $2, $3, $4, $5, $6)
          RETURNING id"#, &[&id, &author, &direction, &message.message, &email_message_id, &message.internal])
    .fetch_one(&tx)
    .await?;

    if message.internal {
      // A note is not a reply: nobody outside the team has seen it, so it
      // cannot change who is owed a response. It is still activity on the
      // ticket, so the timestamp moves.
      query(
        r#"UPDATE tickets SET updated_at = now() WHERE id = $1"#,
        &[&id],
      )
      .execute(&tx)
      .await?;
    } else {
      // Whoever did not just speak is the one now owed a response.
      let status = if author_is_creator {
        TicketStatus::WaitingOnSupport
      } else {
        TicketStatus::WaitingOnUser
      };
      query(
        r#"UPDATE tickets SET status = $2, updated_at = now()
            WHERE id = $1 AND status NOT IN ('closed', 'spam')"#,
        &[&id, &status],
      )
      .execute(&tx)
      .await?;
    }

    let messages = Self::load_ticket_messages(&tx, id).await?;

    tx.commit().await?;

    let (message, user, _) = messages
      .into_iter()
      .find(|(m, ..)| m.id == message_id)
      .expect("message was just inserted in this transaction");

    Ok((
      message,
      user.expect("message was just written by an account"),
    ))
  }

  /// Appends a message written through the web UI by the reporter of an
  /// unclaimed ticket, who proved who they are by holding the claim token
  /// rather than by signing in.
  #[instrument(name = "Database::ticket_add_reporter_message", skip(self), err)]
  pub async fn ticket_add_reporter_message(
    &self,
    ticket_id: Uuid,
    reporter_email: &str,
    reporter_name: Option<&str>,
    email_message_id: Option<&str>,
    message: NewTicketMessage,
  ) -> Result<TicketMessage> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let message_id = query_scalar::<Uuid>(r#"INSERT INTO ticket_messages (ticket_id, author, author_email, author_name, author_email_verified, direction, message, email_message_id)
          VALUES ($1, NULL, $2, $3, true, 'inbound', $4, $5)
          RETURNING id"#, &[&ticket_id, &reporter_email, &reporter_name, &message.message, &email_message_id])
    .fetch_one(&tx)
    .await?;

    query(r#"UPDATE tickets
          SET status = 'waiting_on_support', closed_at = NULL, updated_at = now()
          WHERE id = $1 AND status <> 'spam'"#, &[&ticket_id])
    .execute(&tx)
    .await?;

    let messages = Self::load_ticket_messages(&tx, ticket_id).await?;

    tx.commit().await?;

    let (message, ..) = messages
      .into_iter()
      .find(|(m, ..)| m.id == message_id)
      .expect("message was just inserted in this transaction");

    Ok(message)
  }

  /// Binds an unclaimed, email-opened ticket to an account. The claim token is
  /// spent in the process, so a leaked auto-reply cannot be replayed.
  ///
  /// Returns `None` if the token does not match an unclaimed ticket.
  #[instrument(name = "Database::claim_ticket", skip(self), err)]
  pub async fn claim_ticket(
    &self,
    ticket_id: Uuid,
    claim_token: Uuid,
    user_id: Uuid,
  ) -> Result<Option<FullTicket>> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let claimed = query_scalar::<Uuid>(r#"UPDATE tickets
          SET creator = $3, claim_token = NULL, reporter_email = NULL, reporter_name = NULL
          WHERE id = $1 AND claim_token = $2
          RETURNING id"#, &[&ticket_id, &claim_token, &user_id])
    .fetch_optional(&tx)
    .await?;

    if claimed.is_none() {
      return Ok(None);
    }

    let full = Self::load_full_ticket(&tx, ticket_id).await?;

    tx.commit().await?;

    Ok(full)
  }

  #[instrument(name = "Database::get_ticket_attachment", skip(self), err)]
  pub async fn get_ticket_attachment(
    &self,
    ticket_id: Uuid,
    attachment_id: Uuid,
  ) -> Result<Option<TicketAttachment>> {
    query_as::<TicketAttachment>(r#"SELECT ticket_attachments.id, ticket_attachments.message_id, ticket_attachments.filename, ticket_attachments.content_type, ticket_attachments.size_bytes, ticket_attachments.storage_key, ticket_attachments.created_at
        FROM ticket_attachments
        INNER JOIN ticket_messages ON ticket_messages.id = ticket_attachments.message_id
        WHERE ticket_attachments.id = $1 AND ticket_messages.ticket_id = $2"#, &[&attachment_id, &ticket_id])
    .fetch_optional(&self.pool)
    .await
  }

  #[instrument(name = "Database::update_ticket_status", skip(self), err)]
  pub async fn update_ticket_status(
    &self,
    staff_id: &Uuid,
    ticket_id: Uuid,
    status: TicketStatus,
  ) -> Result<FullTicket> {
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    audit_log(
      &tx,
      staff_id,
      true,
      "update_ticket_status",
      json!({
      "ticket_id": ticket_id,
      "status": status,
      }),
    )
    .await?;

    let closed_at = (status == TicketStatus::Closed).then(Utc::now);
    query(
      r#"UPDATE tickets SET status = $1, closed_at = $2 WHERE id = $3"#,
      &[&status, &closed_at, &ticket_id],
    )
    .execute(&tx)
    .await?;

    let full = Self::load_full_ticket(&tx, ticket_id)
      .await?
      .expect("ticket was just updated in this transaction");

    tx.commit().await?;

    Ok(full)
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
    let mut conn = self.pool.get().await?;
    let tx = conn.transaction().await?;

    let search = format!("%{}%", maybe_search_query.unwrap_or(""));
    let sql = sort_by!(
      maybe_sort,
      concatcp!("SELECT
      ", AUDIT_LOG_SELECT_JOINED, ", ", USER_PUBLIC_SELECT_JOINED, "
      FROM audit_logs
      LEFT JOIN users ON audit_logs.actor_id = users.id
      WHERE (audit_logs.action ILIKE $1
         OR users.name ILIKE $1
         OR audit_logs.meta::text ILIKE $1)
         AND ($2 IS NOT TRUE OR audit_logs.is_sudo = TRUE)
      ORDER BY "),
      " OFFSET $3 LIMIT $4
      "
      => {
      @timestamps "created_at";
      "action" => "audit_logs.action",
      "user" => "users.name",
      "created_at" => "audit_logs.created_at",
    } || "audit_logs.created_at DESC");

    let scopes = query(sql, &[&search, &sudo_only, &start, &limit])
      .map(|r| {
        let audit_log = AuditLog::from_row_prefixed(r, "audit_log_")?;
        let user = UserPublic::from_row_prefixed(r, "user_")?;

        Ok((audit_log, user))
      })
      .fetch_all(&tx)
      .await?;

    let total_scopes = query_scalar::<i64>(r#"SELECT COUNT(audit_logs.created_at) FROM audit_logs LEFT JOIN users ON audit_logs.actor_id = users.id WHERE audit_logs.action ILIKE $1 OR users.name ILIKE $2 AND ($3 IS NOT TRUE OR audit_logs.is_sudo = TRUE);"#, &[&search, &search, &sudo_only])
      .fetch_one(&tx)
      .await?;

    tx.commit().await?;

    Ok((total_scopes as usize, scopes))
  }

  #[instrument(
    name = "Database::get_recently_published_packages_by_user",
    skip(self),
    err
  )]
  pub async fn get_recently_published_packages_by_user(
    &self,
    user_id: &Uuid,
  ) -> Result<Vec<PackageWithGitHubRepoAndMeta>> {
    query(concatcp!("SELECT ", PACKAGE_BASE_SELECT_JOINED, ",
      ", PACKAGE_VERSION_AGG_SELECT, ",
      ", GITHUB_REPOSITORY_SELECT_JOINED, "
      FROM (
        SELECT package_scope, package_name, MAX(created_at) as last_published_at
        FROM publishing_tasks
        WHERE user_id = $1 AND status = 'success'
        GROUP BY package_scope, package_name
        ORDER BY last_published_at DESC
        LIMIT 10
      ) recently_published
      JOIN packages
        ON packages.scope = recently_published.package_scope
        AND packages.name = recently_published.package_name
      LEFT JOIN github_repositories ON packages.github_repository_id = github_repositories.id
      ", PACKAGE_VERSION_LATERAL_JOINS, "
      ORDER BY recently_published.last_published_at DESC"), &[&user_id])
      .map(package_with_github_repository_and_meta)
      .fetch_all(&self.pool)
      .await
  }
}

/// Maps the columns of PACKAGE_SELECT_JOINED and, from a LEFT JOIN,
/// GITHUB_REPOSITORY_SELECT_JOINED.
fn package_with_github_repository_and_meta(
  r: &tokio_postgres::Row,
) -> Result<PackageWithGitHubRepoAndMeta> {
  Ok((
    Package::from_row_prefixed(r, "package_")?,
    GithubRepository::from_row_optional(r, "github_repository_", "id")?,
    r.try_get("package_version_meta")?,
  ))
}

async fn finalize_package_creation(
  tx: Transaction<'_>,
  scope: &ScopeName,
) -> Result<Option<CreatePackageResult>> {
  let (package_limit, new_package_per_week_limit) = query(r#"
    SELECT package_limit, new_package_per_week_limit FROM scopes WHERE scope = $1;
    "#, &[&scope])
    .map(|r| {
      Ok((
        r.try_get::<_, i32>("package_limit")?,
        r.try_get::<_, i32>("new_package_per_week_limit")?,
      ))
    })
    .fetch_one(&tx)
    .await?;

  let packages_from_last_week = query_scalar::<i64>(r#"
    SELECT COUNT(created_at) FROM packages WHERE scope = $1 AND created_at > now() - '1 week'::interval;
    "#, &[&scope])
    .fetch_one(&tx)
    .await?;

  if packages_from_last_week > new_package_per_week_limit as i64 {
    tx.rollback().await?;
    return Ok(Some(CreatePackageResult::WeeklyPackageLimitExceeded(
      new_package_per_week_limit,
    )));
  }

  let total_packages = query_scalar::<i64>(
    r#"
    SELECT COUNT(created_at) FROM packages WHERE scope = $1;
    "#,
    &[&scope],
  )
  .fetch_one(&tx)
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

/// Writes an inbound email and its attachments onto a ticket.
///
/// Returns `false` if this exact email is already on file. Postmark retries a
/// webhook until it gets a 2xx, so the same message can arrive more than once;
/// the unique index on `email_message_id` is what makes that harmless, and the
/// duplicate-key violation is the signal rather than an error worth reporting.
async fn insert_inbound_email_message(
  tx: &Transaction<'_>,
  ticket_id: Uuid,
  message: &NewTicketEmailMessage,
) -> Result<bool> {
  let res = query(r#"INSERT INTO ticket_messages (id, ticket_id, author, author_email, author_name, author_email_verified, direction, message, email_message_id)
        VALUES ($1, $2, NULL, $3, $4, $5, 'inbound', $6, $7)"#, &[&message.id, &ticket_id, &message.author_email, &message.author_name, &message.author_email_verified, &message.message, &message.email_message_id])
  .execute(tx)
  .await;

  match res {
    Err(err) if err.is_unique_violation() => {
      return Ok(false);
    }
    Err(err) => return Err(err),
    Ok(_) => {}
  }

  for attachment in &message.attachments {
    query(r#"INSERT INTO ticket_attachments (message_id, filename, content_type, size_bytes, storage_key)
          VALUES ($1, $2, $3, $4, $5)"#, &[&message.id, &attachment.filename, &attachment.content_type, &attachment.size_bytes, &attachment.storage_key])
    .execute(tx)
    .await?;
  }

  Ok(true)
}
async fn audit_log(
  tx: &Transaction<'_>,
  actor_id: &Uuid,
  is_sudo: bool,
  action: &'static str,
  data: serde_json::Value,
) -> Result<()> {
  query(r#"INSERT INTO audit_logs (actor_id, is_sudo, action, meta) VALUES ($1, $2, $3, $4)"#, &[&actor_id, &is_sudo, &action, &data])
    .execute(tx)
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

#[derive(Debug, PartialEq, Eq)]
pub enum UserDeleteResult {
  Deleted,
  NotFound,
  Held,
}

#[derive(Debug)]
pub enum CreatePackageResult {
  Ok(Package),
  AlreadyExists,
  WeeklyPackageLimitExceeded(i32),
  PackageLimitExceeded(i32),
}

#[derive(Debug)]
pub struct StalePublishingTasks {
  pub claimed: Vec<Uuid>,
  pub abandoned: Vec<Uuid>,
}

#[derive(Debug)]
pub enum CreatePublishingTaskResult {
  Created((PublishingTask, Option<UserPublic>)),
  Exists((PublishingTask, Option<UserPublic>)),
  WeeklyPublishAttemptsLimitExceeded(i32),
}

/// In-memory cache for `count_package_dependents` results. The dependent count
/// for a package only changes when someone publishes a new version of a
/// depending package, so a 15-minute TTL is safe and drastically reduces
/// database load on package page views.
#[derive(Clone)]
pub struct DependentCountCache {
  cache: moka::future::Cache<String, usize>,
}

impl DependentCountCache {
  pub fn new() -> Self {
    Self {
      cache: moka::future::Cache::builder()
        .max_capacity(65536)
        .time_to_live(std::time::Duration::from_secs(900))
        .build(),
    }
  }

  pub async fn count_package_dependents(
    &self,
    db: &Database,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<usize> {
    let key = format!("@{scope}/{name}");
    if let Some(count) = self.cache.get(&key).await {
      return Ok(count);
    }
    let count = db.count_package_dependents(scope, name).await?;
    self.cache.insert(key, count).await;
    Ok(count)
  }
}
