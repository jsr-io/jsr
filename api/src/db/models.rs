// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! The database model structs now live in the shared, wasm-safe [`jsr_types`]
//! crate. The `postgres-types` `FromSql`/`ToSql` impls are gated behind
//! `jsr_types`'s `postgres` feature, which this crate enables; the row
//! mappers live in `sql_fragments.rs`. This module re-exports the models so
//! existing `crate::db::models::*` / `crate::db::*` paths keep working.

pub use jsr_types::models::*;
