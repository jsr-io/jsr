// Copyright 2024 the JSR authors. All rights reserved. MIT license.
mod database;
#[cfg(any(test, feature = "testing"))]
mod ephemeral_database;
mod migrate;
mod query;
pub(crate) mod sql_fragments;
#[cfg(test)]
mod tests;
#[cfg(not(target_arch = "wasm32"))]
mod tls;

pub use database::*;
#[cfg(any(test, feature = "testing"))]
pub use ephemeral_database::EphemeralDatabase;
pub use query::Error as DbError;
