// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// The id newtypes (`ScopeName`, `PackageName`, `Version`, …) were extracted into
// the shared, wasm-safe `jsr_types` crate (see docs/design/api-service-split.md).
// They are re-exported here so existing `crate::ids::…` paths keep working. The
// Postgres sqlx impls live in `jsr_types` behind its `sqlx` feature, which this
// crate enables.
pub use jsr_types::ids::*;
