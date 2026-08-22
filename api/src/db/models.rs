// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! The database model structs now live in the shared, wasm-safe [`jsr_types`]
//! crate. The sqlx `FromRow`/`Type`/`Encode`/`Decode` impls are gated behind
//! `jsr_types`'s `sqlx` feature, which this crate enables. This module
//! re-exports the models so existing `crate::db::models::*` / `crate::db::*`
//! paths keep working.

pub use jsr_types::models::*;
