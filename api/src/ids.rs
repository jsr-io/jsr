// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! The id newtypes (`ScopeName`, `PackageName`, `Version`, `PackagePath`, …)
//! now live in the shared, wasm-safe [`jsr_types`] crate so the upcoming
//! workers-rs front can use them without pulling in sqlx or the heavy native
//! crates. This module re-exports them so existing `crate::ids::*` paths keep
//! working.

pub use jsr_types::ids::*;
