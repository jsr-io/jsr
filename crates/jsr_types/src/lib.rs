// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Shared, wasm-safe data types for the JSR API services.
//!
//! See `docs/design/api-service-split.md`. This crate is consumed by both the
//! Cloud Run compute service (`registry_api`, with the `sqlx` feature on) and
//! the future workers-rs front (wasm32, with the `sqlx` feature off).

pub mod ids;
