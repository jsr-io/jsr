// Copyright 2024 the JSR authors. All rights reserved. MIT license.
mod database;
#[cfg(test)]
mod ephemeral_database;
mod migrate;
pub(crate) mod models;
mod query;
pub(crate) mod sql_fragments;
#[cfg(test)]
mod tests;
#[cfg(not(target_arch = "wasm32"))]
mod tls;

pub use database::*;
#[cfg(test)]
pub use ephemeral_database::EphemeralDatabase;
pub use models::*;
pub use query::Error as DbError;
