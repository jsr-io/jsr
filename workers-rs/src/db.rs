// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::str::FromStr;

use jsr_types::api::ApiMetrics;
use jsr_types::api::ApiStats;
use jsr_types::api::ApiStatsPackage;
use jsr_types::api::ApiStatsPackageVersion;
use jsr_types::ids::PackageName;
use jsr_types::ids::ScopeName;
use jsr_types::ids::Version;
use tokio_postgres::Client;
use tokio_postgres::NoTls;
use tokio_postgres::Row;
use worker::Env;
use worker::Error;
use worker::Result;

// Values read back from the DB are already valid (they were validated on the way
// in), so a parse failure here means data corruption — surface it as an error.
fn map_err<E: std::fmt::Display>(e: E) -> Error {
  Error::RustError(format!("postgres query failed: {e}"))
}

fn scope_name(row: &Row, idx: &str) -> Result<ScopeName> {
  ScopeName::try_from(row.get::<_, String>(idx)).map_err(map_err)
}

fn package_name(row: &Row, idx: &str) -> Result<PackageName> {
  PackageName::try_from(row.get::<_, String>(idx)).map_err(map_err)
}

// Opens a Postgres connection through the Hyperdrive binding. Hyperdrive
// terminates TLS to the origin, so the Worker→Hyperdrive hop is plaintext
// (NoTls); tokio-postgres runs over the worker::Socket via connect_raw.
pub async fn connect(env: &Env) -> Result<Client> {
  let hyperdrive = env.hyperdrive("HYPERDRIVE")?;
  let config = tokio_postgres::Config::from_str(
    &hyperdrive.connection_string(),
  )
  .map_err(|e| Error::RustError(format!("invalid connection string: {e}")))?;
  let socket = hyperdrive.connect()?;
  let (client, connection) = config
    .connect_raw(socket, NoTls)
    .await
    .map_err(|e| Error::RustError(format!("postgres connect failed: {e}")))?;
  wasm_bindgen_futures::spawn_local(async move {
    if let Err(e) = connection.await {
      worker::console_error!("postgres connection closed: {e}");
    }
  });
  Ok(client)
}

pub async fn ping(client: &Client) -> Result<i32> {
  let row = client
    .query_one("SELECT 1", &[])
    .await
    .map_err(|e| Error::RustError(format!("postgres query failed: {e}")))?;
  Ok(row.get::<_, i32>(0))
}

/// Front-page stats: `GET /api/stats`. Ports `Database::package_stats` — the
/// queries are kept verbatim with the compute side so the JSON is identical.
pub async fn stats(client: &Client) -> Result<ApiStats> {
  let newest_rows = client
    .query(
      r#"SELECT packages.scope as "scope", packages.name as "name"
      FROM packages
      WHERE EXISTS (
        SELECT 1 FROM package_versions
        WHERE scope = packages.scope AND name = packages.name AND is_yanked = false
      ) AND NOT packages.is_archived
      ORDER BY packages.created_at DESC
      LIMIT 10"#,
      &[],
    )
    .await
    .map_err(map_err)?;

  let updated_rows = client
    .query(
      r#"SELECT package_versions.scope as "scope", package_versions.name as "name", package_versions.version as "version"
      FROM package_versions
      JOIN packages ON packages.scope = package_versions.scope AND packages.name = package_versions.name
      WHERE NOT packages.is_archived
      ORDER BY package_versions.created_at DESC
      LIMIT 10"#,
      &[],
    )
    .await
    .map_err(map_err)?;

  let featured_rows = client
    .query(
      r#"SELECT packages.scope as "scope", packages.name as "name"
      FROM packages
      WHERE packages.when_featured IS NOT NULL AND NOT packages.is_archived
      ORDER BY packages.when_featured DESC
      LIMIT 10"#,
      &[],
    )
    .await
    .map_err(map_err)?;

  let mut newest = Vec::with_capacity(newest_rows.len());
  for row in &newest_rows {
    newest.push(ApiStatsPackage {
      scope: scope_name(row, "scope")?,
      name: package_name(row, "name")?,
    });
  }

  let mut updated = Vec::with_capacity(updated_rows.len());
  for row in &updated_rows {
    updated.push(ApiStatsPackageVersion {
      scope: scope_name(row, "scope")?,
      package: package_name(row, "name")?,
      version: Version::try_from(row.get::<_, String>("version").as_str())
        .map_err(map_err)?,
    });
  }

  let mut featured = Vec::with_capacity(featured_rows.len());
  for row in &featured_rows {
    featured.push(ApiStatsPackage {
      scope: scope_name(row, "scope")?,
      name: package_name(row, "name")?,
    });
  }

  Ok(ApiStats {
    newest,
    updated,
    featured,
  })
}

/// Registry-wide counts: `GET /api/metrics`. Ports `Database::metrics`. `COUNT`
/// returns a non-null `bigint` (`i64`); counts are non-negative so the
/// `i64 -> usize` conversion never fails in practice.
pub async fn metrics(client: &Client) -> Result<ApiMetrics> {
  let packages = client
    .query_one(
      r#"
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
      "#,
      &[],
    )
    .await
    .map_err(map_err)?;

  let users = client
    .query_one(
      r#"
      SELECT
        COUNT(*) AS count_total,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '1 DAY' THEN 1 END) AS count_1d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '7 DAY' THEN 1 END) AS count_7d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '30 DAY' THEN 1 END) AS count_30d
      FROM users
      "#,
      &[],
    )
    .await
    .map_err(map_err)?;

  let package_versions = client
    .query_one(
      r#"
      SELECT
        COUNT(*) AS count_total,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '1 DAY' THEN 1 END) AS count_1d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '7 DAY' THEN 1 END) AS count_7d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '30 DAY' THEN 1 END) AS count_30d
      FROM package_versions
      "#,
      &[],
    )
    .await
    .map_err(map_err)?;

  let count = |row: &Row, col: &str| -> Result<usize> {
    usize::try_from(row.get::<_, i64>(col)).map_err(map_err)
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
