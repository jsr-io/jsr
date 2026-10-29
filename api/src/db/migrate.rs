// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Applies the migrations in `api/migrations`, embedded at build time.
//!
//! This keeps the bookkeeping of sqlx's migrator, which ran the migrations
//! before: applied migrations are recorded in `_sqlx_migrations` with the
//! SHA-384 of their SQL, and runs are serialized with the same advisory lock,
//! so databases migrated by sqlx are picked up as-is.

use std::time::Instant;

use sha2::Digest;
use sha2::Sha384;

use super::query::Error;
use super::query::query;

pub struct Migration {
  pub version: i64,
  pub description: &'static str,
  pub sql: &'static str,
}

impl Migration {
  fn checksum(&self) -> Vec<u8> {
    Sha384::digest(self.sql.as_bytes()).to_vec()
  }
}

include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
  #[error(transparent)]
  Database(#[from] Error),
  #[error("migration {0} was previously applied but is missing")]
  Missing(i64),
  #[error("migration {0} was previously applied but has been modified")]
  Modified(i64),
  #[error(
    "migration {0} is partially applied; fix it and remove it from _sqlx_migrations"
  )]
  Dirty(i64),
}

impl From<tokio_postgres::Error> for MigrateError {
  fn from(err: tokio_postgres::Error) -> Self {
    Self::Database(err.into())
  }
}

pub async fn run(pool: &deadpool_postgres::Pool) -> Result<(), MigrateError> {
  let mut conn = pool.get().await.map_err(Error::from)?;

  // Same lock id as sqlx, so a sqlx-based instance starting up during a
  // deploy can't migrate at the same time.
  let database_name: String = conn
    .query_one("SELECT current_database()", &[])
    .await?
    .get(0);
  let lock_id = 0x3d32ad9e * (crc32fast::hash(database_name.as_bytes()) as i64);
  conn
    .execute("SELECT pg_advisory_lock($1)", &[&lock_id])
    .await?;

  let result = run_locked(&mut conn).await;

  conn
    .execute("SELECT pg_advisory_unlock($1)", &[&lock_id])
    .await?;
  result
}

async fn run_locked(
  conn: &mut deadpool_postgres::Client,
) -> Result<(), MigrateError> {
  conn
    .batch_execute(
      "CREATE TABLE IF NOT EXISTS _sqlx_migrations (
        version BIGINT PRIMARY KEY,
        description TEXT NOT NULL,
        installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
        success BOOLEAN NOT NULL,
        checksum BYTEA NOT NULL,
        execution_time BIGINT NOT NULL
      )",
    )
    .await?;

  let applied = conn
    .query(
      "SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version",
      &[],
    )
    .await?;

  let mut applied_versions = Vec::with_capacity(applied.len());
  for row in &applied {
    let version: i64 = row.get("version");
    if !row.get::<_, bool>("success") {
      return Err(MigrateError::Dirty(version));
    }
    let Some(migration) = MIGRATIONS.iter().find(|m| m.version == version)
    else {
      return Err(MigrateError::Missing(version));
    };
    if row.get::<_, &[u8]>("checksum") != migration.checksum() {
      return Err(MigrateError::Modified(version));
    }
    applied_versions.push(version);
  }

  for migration in MIGRATIONS {
    if applied_versions.contains(&migration.version) {
      continue;
    }

    let start = Instant::now();
    let tx = conn.transaction().await?;
    tx.batch_execute(migration.sql).await?;
    query(
      "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
      VALUES ($1, $2, TRUE, $3, $4)",
      &[
        &migration.version,
        &migration.description,
        &migration.checksum(),
        &(start.elapsed().as_nanos() as i64),
      ],
    )
    .execute(&tx)
    .await?;
    tx.commit().await?;
  }

  Ok(())
}
