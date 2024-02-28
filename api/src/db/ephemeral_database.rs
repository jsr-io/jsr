// Copyright 2024 the JSR authors. All rights reserved. MIT license.
#![allow(dead_code)]
use super::Database;
use once_cell::sync::Lazy;
use sqlx::Connection;
use sqlx::Executor;
use std::fmt::Write;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
use tokio::sync::RwLockWriteGuard;
use url::Url;

static DEFAULT_DATABASE_URL: Lazy<String> = Lazy::new(|| {
  dotenvy::dotenv().ok();
  std::env::var("DATABASE_URL").unwrap_or_else(|_| {
    "postgres://user:password@localhost/registry".to_owned()
  })
});

static EXCLUSIVITY_LOCKER: Lazy<RwLock<()>> = Lazy::new(|| RwLock::new(()));

/// Used for testing. Derefs to Database.
pub struct EphemeralDatabase {
  pub database: Option<Database>,
  pub database_url: String,
  database_name: String,
  _guard: EphemeralDatabaseGuard,
}

enum EphemeralDatabaseGuard {
  Read(RwLockReadGuard<'static, ()>),
  Write(RwLockWriteGuard<'static, ()>),
}

impl EphemeralDatabase {
  #[allow(clippy::await_holding_lock)]
  pub async fn create() -> Self {
    let guard = EXCLUSIVITY_LOCKER.read().await;
    Self::create_inner(EphemeralDatabaseGuard::Read(guard)).await
  }

  #[allow(clippy::await_holding_lock)]
  pub async fn create_exclusive() -> Self {
    let guard = EXCLUSIVITY_LOCKER.write().await;
    Self::create_inner(EphemeralDatabaseGuard::Write(guard)).await
  }

  async fn create_inner(guard: EphemeralDatabaseGuard) -> Self {
    let database_name = format!("registry_{}", random_string(5));
    let mut database_url = Url::parse(&DEFAULT_DATABASE_URL).unwrap();
    database_url.set_path(&format!("/{database_name}"));
    let database_url = database_url.to_string();

    pg_execute(format!("CREATE DATABASE \"{database_name}\""));

    let database = Database::connect(&database_url, 1, Duration::from_secs(5))
      .await
      .unwrap();

    Self {
      database: Some(database),
      database_name,
      database_url,
      _guard: guard,
    }
  }
}

impl std::ops::Deref for EphemeralDatabase {
  type Target = Database;
  fn deref(&self) -> &Self::Target {
    self.database.as_ref().unwrap()
  }
}

impl std::ops::DerefMut for EphemeralDatabase {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.database.as_mut().unwrap()
  }
}

impl Drop for EphemeralDatabase {
  fn drop(&mut self) {
    if self.database.take().is_none() {
      eprintln!(
        "self.database should exist in EphemeralDatabase, but is not present"
      );
    }

    // Drop the ephemeral database.
    pg_execute(format!(
      r#"DROP DATABASE "{}" WITH (FORCE)"#,
      &self.database_name
    ));
  }
}

/// Synchronously issues a postgres call on a standalone connection.
fn pg_execute(query: String) {
  std::thread::spawn(move || {
    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async move {
        let database_url = &*DEFAULT_DATABASE_URL;
        let mut conn = sqlx::postgres::PgConnection::connect(database_url)
          .await
          .unwrap();
        conn.execute(sqlx::query(&query)).await.unwrap();
      })
  })
  .join()
  .unwrap();
}

pub fn random_string(bytes: usize) -> String {
  let mut str = String::with_capacity(bytes * 2);
  for _ in 0..bytes {
    write!(&mut str, "{:02x}", rand::random::<u8>()).unwrap();
  }
  str
}
