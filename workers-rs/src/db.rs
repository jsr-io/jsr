// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::str::FromStr;

use tokio_postgres::Client;
use tokio_postgres::NoTls;
use worker::Env;
use worker::Error;
use worker::Result;

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
