// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Postgres connectivity for the API Worker, over Cloudflare Hyperdrive.
//!
//! The Worker runs on `wasm32` and has no VPC route to Cloud SQL, so it cannot
//! use `sqlx`/`tokio-postgres`' native TCP runtime. Instead it opens the
//! connection itself through the Hyperdrive binding and hands the resulting
//! [`worker::Socket`] to `tokio-postgres` via
//! [`tokio_postgres::Config::connect_raw`]:
//!
//! ```text
//!   env.hyperdrive("HYPERDRIVE").connect()  ->  worker::Socket  (TCP, TLS
//!     terminated by Hyperdrive at the edge)  ->  tokio_postgres::Client
//! ```
//!
//! This is the **step-3 connectivity spike** of the API-split migration
//! (`docs/design/api-service-split.md`): it proves the wasm DB story end-to-end
//! with a single trivial read. Real query modules land per endpoint group in
//! later PRs.

use std::str::FromStr;

use tokio_postgres::Client;
use tokio_postgres::NoTls;
use worker::Env;
use worker::Error;
use worker::Result;

/// Name of the Hyperdrive binding declared in `wrangler.toml` (local dev) and in
/// the Worker's Terraform-managed production config.
const HYPERDRIVE_BINDING: &str = "HYPERDRIVE";

/// Opens a Postgres connection through Hyperdrive and returns a ready
/// [`Client`].
///
/// The connection's background task is spawned onto the Worker's event loop with
/// [`wasm_bindgen_futures::spawn_local`]; it lives until the socket closes. The
/// caller owns the returned [`Client`] for the duration of the request.
///
/// Hyperdrive terminates TLS to the database origin, so the Worker→Hyperdrive
/// hop is plaintext over the socket and we use [`NoTls`] — no rustls in wasm.
pub async fn connect(env: &Env) -> Result<Client> {
  let hyperdrive = env.hyperdrive(HYPERDRIVE_BINDING)?;

  // The connection string carries the auth/startup params (user, password,
  // dbname); the actual transport is the Hyperdrive socket below, not a TCP
  // dial of the string's host.
  let config = tokio_postgres::Config::from_str(
    &hyperdrive.connection_string(),
  )
  .map_err(|e| {
    Error::RustError(format!("invalid hyperdrive connection string: {e}"))
  })?;

  let socket = hyperdrive.connect()?;

  let (client, connection) = config
    .connect_raw(socket, NoTls)
    .await
    .map_err(|e| Error::RustError(format!("postgres connect failed: {e}")))?;

  wasm_bindgen_futures::spawn_local(async move {
    if let Err(e) = connection.await {
      // The connection task ending in error is expected on socket close; log it
      // for visibility but there is nothing to recover here.
      worker::console_error!("postgres connection closed: {e}");
    }
  });

  Ok(client)
}

/// Trivial round-trip read used by the `GET /api/db_health` probe: `SELECT 1`.
///
/// Returns the integer the database echoed back (always `1`) so the caller can
/// confirm the query actually executed, not just that a connection opened.
pub async fn ping(client: &Client) -> Result<i32> {
  let row = client
    .query_one("SELECT 1", &[])
    .await
    .map_err(|e| Error::RustError(format!("postgres query failed: {e}")))?;
  Ok(row.get::<_, i32>(0))
}
