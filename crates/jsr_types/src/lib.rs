// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Shared, wasm-safe JSR data types.
//!
//! This crate holds the id newtypes and the plain database model structs that
//! both the Cloud Run API server and the (upcoming) workers-rs front need. It
//! has no dependency on any native-only/heavy crate, so it builds for
//! `wasm32-unknown-unknown`.
//!
//! The sqlx `FromRow`/`Type`/`Encode`/`Decode` impls on these types live here
//! too, but are gated behind the default-off `sqlx` feature so wasm builds
//! never compile them.

pub mod ids;
pub mod models;
