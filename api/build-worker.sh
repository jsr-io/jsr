#!/usr/bin/env bash
# EXPERIMENT (temporary): build registry_api as a wasm32-unknown-emscripten
# Worker. Invoked by wrangler's `[build] command` in api/wrangler.toml; also
# usable directly from api/.
#
# worker-build provisions the Emscripten SDK (and patches its frontend) and
# supplies the relocation model, the exnref exception-handling flags and the
# `-s` link args. It passes them through the RUSTFLAGS *env var*, which
# OVERRIDES `[target.*].rustflags` in .cargo/config.toml — ours are passed here
# first, worker-build appends its own after them.
set -euo pipefail

# `--cfg`s: tokio's emscripten runtime and wasm-bindgen's
# `#[wasm_bindgen(experimental_tokio)]` are both gated behind these. worker-build
# only adds them for crates depending on `worker` with its `experimental_tokio`
# feature; registry_api talks to wasm-bindgen directly.
# STACK_SIZE: emscripten's 64KB default overflows building regex-automata's meta
# DFA (it faults as an OOB memory access).
# -Oz: emcc's link step defaults to -O0, which skips wasm-opt entirely.
export RUSTFLAGS="${RUSTFLAGS:-} --cfg=tokio_unstable --cfg=wasm_bindgen_unstable_tokio -Clink-arg=-sSTACK_SIZE=8MB -Clink-arg=-Oz"
# Size: the worker is app code, not std, so opt-level=z + fat LTO is where the
# wins are. Set per-build rather than in [profile.release], which the native
# production binary also uses.
export CARGO_PROFILE_RELEASE_OPT_LEVEL=s
export CARGO_PROFILE_RELEASE_LTO=true
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
# sqlx's `query!` macros verify against a live database at compile time; use the
# committed `api/.sqlx` data instead.
export SQLX_OFFLINE=true
# Needs nightly: stable's LLVM 22 emits exception-handling code that
# wasm-bindgen 0.2.129 cannot parse. CI pins the nightly it was tested with.
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-nightly}"

worker-build --emscripten --release "$@"

# emscripten's node glue calls `createRequire(import.meta.url)`, and workerd only
# defines `import.meta.url` under the experimental `new_module_registry` flag,
# which Cloudflare does not accept on deploy.
sed -i 's#import\.meta\.url#"file:///bundle/index.js"#g' build/index.js

# NOTE: a second `wasm-opt --converge -Oz` pass over the linked module is NOT
# worth it here. With the module's own feature set it saves ~7KB; the ~600KB an
# `--all-features` run appears to save comes from features V8 rejects, and the
# worker then dies at startup with
# `CompileError: invalid heap type 'exact', enable with --wasm-custom-descriptors`.
