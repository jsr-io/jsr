#!/usr/bin/env bash
# Builds the API as a wasm32-unknown-emscripten Worker into build/. Run by
# wrangler's `[build] command`.
set -euo pipefail

# emcc skips wasm-opt without an -O on the link. The stack size is set in
# build.rs.
export RUSTFLAGS="${RUSTFLAGS:-} -Clink-arg=-Oz"
export CARGO_PROFILE_RELEASE_OPT_LEVEL=s
export CARGO_PROFILE_RELEASE_LTO=true
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
# Newer than the repository's toolchain.
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"

worker-build --emscripten --release "$@"

# workerd only defines `import.meta.url` under the experimental
# `new_module_registry` flag, which can't be deployed.
perl -pi -e 's#import\.meta\.url#"file:///bundle/index.js"#g' build/index.js
