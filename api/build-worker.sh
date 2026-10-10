#!/usr/bin/env bash
# Builds the API as a wasm32-unknown-emscripten Worker into build/. Run by
# wrangler's `[build] command`.
set -euo pipefail

# worker-build only adds the `--cfg`s for crates using `worker`'s
# `experimental_tokio`. emscripten's 64KB default stack overflows in
# regex-automata, and emcc skips wasm-opt without an -O on the link.
export RUSTFLAGS="${RUSTFLAGS:-} --cfg=tokio_unstable --cfg=wasm_bindgen_unstable_tokio -Clink-arg=-sSTACK_SIZE=8MB -Clink-arg=-Oz"
export CARGO_PROFILE_RELEASE_OPT_LEVEL=s
export CARGO_PROFILE_RELEASE_LTO=true
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export SQLX_OFFLINE=true
# Stable's LLVM 22 emits exception-handling code wasm-bindgen 0.2.129 can't parse.
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-nightly}"

worker-build --emscripten --release "$@"

# workerd only defines `import.meta.url` under the experimental
# `new_module_registry` flag, which can't be deployed.
sed -i 's#import\.meta\.url#"file:///bundle/index.js"#g' build/index.js
