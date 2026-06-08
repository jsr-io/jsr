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
use tokio_postgres::SimpleQueryMessage;
use tokio_postgres::SimpleQueryRow;
use worker::Env;
use worker::Error;
use worker::Result;

fn map_err<E: std::fmt::Display>(e: E) -> Error {
  Error::RustError(format!("postgres query failed: {e}"))
}

// IMPORTANT: every query here goes through `simple_query` (the simple query
// protocol), NOT `client.query`/`query_one`. The Worker reaches Postgres through
// Cloudflare Hyperdrive, a connection pooler. `tokio-postgres`' normal `query`
// path uses the extended protocol with *named* prepared statements (`s0`, `s1`,
// …); a fresh `Client` per request restarts that counter at `s0`, but Hyperdrive
// reuses backend connections across requests, so we intermittently land on a
// backend that already has `s0` prepared and the wire stream desyncs
// ("unexpected message from server"). `simple_query` prepares nothing, so it is
// pooler-safe. The trade-off: results come back as text, hence the `text`/parse
// helpers below. This is only viable because none of our queries take bind
// parameters (`simple_query` does not support them).

// Collects just the data rows from a `simple_query` response (dropping
// `CommandComplete`/`RowDescription` messages).
fn rows(msgs: Vec<SimpleQueryMessage>) -> Vec<SimpleQueryRow> {
  msgs
    .into_iter()
    .filter_map(|m| match m {
      SimpleQueryMessage::Row(r) => Some(r),
      _ => None,
    })
    .collect()
}

// Reads a non-null text column by name, erroring (not panicking) if it is
// missing or SQL NULL.
fn text<'a>(row: &'a SimpleQueryRow, col: &str) -> Result<&'a str> {
  row
    .try_get(col)
    .map_err(map_err)?
    .ok_or_else(|| Error::RustError(format!("missing or null column: {col}")))
}

fn scope_name(row: &SimpleQueryRow, col: &str) -> Result<ScopeName> {
  ScopeName::try_from(text(row, col)?.to_owned()).map_err(map_err)
}

fn package_name(row: &SimpleQueryRow, col: &str) -> Result<PackageName> {
  PackageName::try_from(text(row, col)?.to_owned()).map_err(map_err)
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
  let msgs = client.simple_query("SELECT 1").await.map_err(map_err)?;
  let row = rows(msgs)
    .into_iter()
    .next()
    .ok_or_else(|| Error::RustError("SELECT 1 returned no rows".into()))?;
  row
    .try_get(0)
    .map_err(map_err)?
    .ok_or_else(|| Error::RustError("SELECT 1 returned null".into()))?
    .parse::<i32>()
    .map_err(map_err)
}

/// `GET /api/stats`. Queries kept verbatim with `Database::package_stats`.
pub async fn stats(client: &Client) -> Result<ApiStats> {
  let newest_rows = rows(
    client
      .simple_query(
        r#"SELECT packages.scope as "scope", packages.name as "name"
      FROM packages
      WHERE EXISTS (
        SELECT 1 FROM package_versions
        WHERE scope = packages.scope AND name = packages.name AND is_yanked = false
      ) AND NOT packages.is_archived
      ORDER BY packages.created_at DESC
      LIMIT 10"#,
      )
      .await
      .map_err(map_err)?,
  );

  let updated_rows = rows(
    client
      .simple_query(
        r#"SELECT package_versions.scope as "scope", package_versions.name as "name", package_versions.version as "version"
      FROM package_versions
      JOIN packages ON packages.scope = package_versions.scope AND packages.name = package_versions.name
      WHERE NOT packages.is_archived
      ORDER BY package_versions.created_at DESC
      LIMIT 10"#,
      )
      .await
      .map_err(map_err)?,
  );

  let featured_rows = rows(
    client
      .simple_query(
        r#"SELECT packages.scope as "scope", packages.name as "name"
      FROM packages
      WHERE packages.when_featured IS NOT NULL AND NOT packages.is_archived
      ORDER BY packages.when_featured DESC
      LIMIT 10"#,
      )
      .await
      .map_err(map_err)?,
  );

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
      version: Version::try_from(text(row, "version")?).map_err(map_err)?,
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

/// `GET /api/metrics`. Queries kept verbatim with `Database::metrics`.
pub async fn metrics(client: &Client) -> Result<ApiMetrics> {
  let packages_rows = rows(
    client
      .simple_query(
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
      )
      .await
      .map_err(map_err)?,
  );

  let users_rows = rows(
    client
      .simple_query(
        r#"
      SELECT
        COUNT(*) AS count_total,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '1 DAY' THEN 1 END) AS count_1d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '7 DAY' THEN 1 END) AS count_7d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '30 DAY' THEN 1 END) AS count_30d
      FROM users
      "#,
      )
      .await
      .map_err(map_err)?,
  );

  let package_versions_rows = rows(
    client
      .simple_query(
        r#"
      SELECT
        COUNT(*) AS count_total,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '1 DAY' THEN 1 END) AS count_1d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '7 DAY' THEN 1 END) AS count_7d,
        COUNT(CASE WHEN created_at >= NOW() - INTERVAL '30 DAY' THEN 1 END) AS count_30d
      FROM package_versions
      "#,
      )
      .await
      .map_err(map_err)?,
  );

  let single = |rows: &[SimpleQueryRow], what: &str| -> Result<()> {
    if rows.is_empty() {
      Err(Error::RustError(format!("{what} returned no rows")))
    } else {
      Ok(())
    }
  };
  single(&packages_rows, "packages metrics")?;
  single(&users_rows, "users metrics")?;
  single(&package_versions_rows, "package_versions metrics")?;
  let packages = &packages_rows[0];
  let users = &users_rows[0];
  let package_versions = &package_versions_rows[0];

  let count = |row: &SimpleQueryRow, col: &str| -> Result<usize> {
    text(row, col)?.parse::<usize>().map_err(map_err)
  };

  Ok(ApiMetrics {
    packages: count(packages, "count_total")?,
    packages_1d: count(packages, "count_1d")?,
    packages_7d: count(packages, "count_7d")?,
    packages_30d: count(packages, "count_30d")?,

    users: count(users, "count_total")?,
    users_1d: count(users, "count_1d")?,
    users_7d: count(users, "count_7d")?,
    users_30d: count(users, "count_30d")?,

    package_versions: count(package_versions, "count_total")?,
    package_versions_1d: count(package_versions, "count_1d")?,
    package_versions_7d: count(package_versions, "count_7d")?,
    package_versions_30d: count(package_versions, "count_30d")?,
  })
}
