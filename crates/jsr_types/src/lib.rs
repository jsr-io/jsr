// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Shared, wasm-safe JSR data types.
//!
//! This crate holds the id newtypes and the plain database model structs that
//! both the Cloud Run API server and the (upcoming) workers-rs front need. It
//! has no dependency on any native-only/heavy crate, so it builds for
//! `wasm32-unknown-unknown`.
//!
//! The `postgres-types` `FromSql`/`ToSql` impls on these types live here too,
//! gated behind the default-off `postgres` feature.

pub mod api;
pub mod ids;
pub mod models;
#[cfg(feature = "postgres")]
pub mod postgres;
