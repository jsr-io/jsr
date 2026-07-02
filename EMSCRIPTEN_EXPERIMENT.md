# Experiment: `registry_api` as a `wasm32-unknown-emscripten` Cloudflare Worker

This is an **experimental, non-portable** branch that runs the *entire* jsr API
server (`api/`, crate `registry_api`) as a Cloudflare Worker compiled to
`wasm32-unknown-emscripten` — using the [workers-rs] emscripten pipeline, which
(unlike `wasm32-unknown-unknown`) supports tokio, hyper, real sockets, DNS, and
rustls+ring TLS.

**Status:** builds, links, and **runs in workerd** (`wrangler dev`): the module
instantiates, emscripten bootstraps, the `fetch` export runs, config is read
from the JS `env`, the router is built, and tokio async networking is live
(verified via a real emscripten async DNS lookup). With real DB/S3 behind it,
request handling would proceed; with the dummy smoke-test config it stops at
`Database::connect` (`EAI -2`, `localhost` unresolvable) — which is the expected
"everything up to the router works" signal.

> **This branch is not buildable by CI or on another machine as-is.** The
> `[patch.crates-io]` recipe below points at machine-absolute checkouts
> (workers-rs tokio fork, vendored wasm-bindgen) and a `.emscripten-patches/`
> directory of vendored crates that are **not committed here** (24 MB, mostly
> noise). The committed changes are the jsr-side source + config only; native
> `cargo build` still works. This doc is the reproduction recipe.

[workers-rs]: https://github.com/cloudflare/workers-rs

## What's in this PR (committed)

| File | Change |
| --- | --- |
| `api/src/main.rs` | Split the entry point: native keeps `#[tokio::main]` + `Server::bind().serve()`; wasm adds a `#[wasm_bindgen(tokio)] fetch(Request, env, ctx)` export that builds the router once from the JS `env` and bridges `web_sys::Request ↔ hyper ↔ routerify` per request. Shared setup extracted into `build_router(config)`. |
| `api/src/api/mod.rs` | `cfg`-gate the `/debug/mem_*` (jemalloc) routes off wasm. |
| `api/src/tarball.rs` | Replace `async-tar` (pulls async-std → async-io epoll reactor, won't build on wasm) with **`astral-tokio-tar`** (tokio-based). Bridges the S3 futures-io stream via `tokio_util::compat` + `tokio::io::BufReader` + `async_compression::tokio::bufread::GzipDecoder`. |
| `api/src/npm/tarball.rs` | Same `async-tar → tokio_tar` swap in the test. |
| `api/Cargo.toml` | `cfg`-gate jemalloc/rust-s3-tls off wasm; add wasm-target deps (wasm-bindgen/web-sys/js-sys/futures); swap async-tar→astral-tokio-tar; `askalono`'s `gzip` feature (zstd's C lib won't build on wasm); **jsonwebtoken 8→10 / jsonwebkey 0.3→0.4 / x509-parser 0.15→0.16** (see "ring" below). |
| `.cargo/config.toml` | `wasm32-unknown-emscripten` rustflags + emcc link args (no-op for native targets). |
| `wrangler.toml` | Smoke-test worker config (dummy `[vars]`). |

## Reproduction recipe (not committed)

### External checkouts required (machine-absolute paths)
- **emsdk** + an **emscripten** checkout with epoll + async DNS + `-sWASM_BINDGEN` support, on `PATH`.
- **workers-rs** with its in-progress **tokio fork** (`tokio/`), vendored
  **wasm-bindgen** (carrying `#[wasm_bindgen(tokio)]` + emscripten fixes), and
  the built `wasm-bindgen` CLI on `PATH`.
- The tokio fork needs 3 added methods in `tokio/tokio/src/net/emscripten.rs`
  (`TcpStream::poll_peek`, `TcpSocket: FromRawFd`, `TcpListener::poll_accept`)
  so hyper's server types compile.

### Root `Cargo.toml` `[patch.crates-io]`
```toml
# EXPERIMENT (temporary): build registry_api against workers-rs emscripten patches.
[patch.crates-io]
libc = { path = "…/workers-rs/libc" }
ring = { path = ".emscripten-patches/ring-0.17.9" }            # +emscripten SystemRandom cfg
socket2 = { path = ".emscripten-patches/socket2" }             # +emscripten to IovLen=c_int
socket2_05 = { package = "socket2", path = ".emscripten-patches/socket2-0.5.7" }
tokio = { path = "…/workers-rs/tokio/tokio" }
tokio-macros = { path = "…/workers-rs/tokio/tokio-macros" }
reqwest = { path = ".emscripten-patches/reqwest-0.11.27" }     # force native backend on emscripten
reqwest_012 = { package = "reqwest", path = ".emscripten-patches/reqwest-0.12.12" }
wasm-bindgen = { path = "…/workers-rs/wasm-bindgen" }          # +macro-support/shared/futures/js-sys/web-sys
# … (js-sys, web-sys, wasm-bindgen-{macro-support,shared,futures} all local)
slug = { path = ".emscripten-patches/slug-0.1.6" }             # drop cdylib crate-type
tar = { path = ".emscripten-patches/tar-0.4.41" }              # unix-vs-wasm32 cfg → not(unix)
astral-tokio-tar = { path = ".emscripten-patches/astral-tokio-tar-0.6.3" }  # same fix
deno_doc = { path = ".emscripten-patches/deno_doc-0.201.0" }   # exclude emscripten from browser-wasm branch
```

### `.emscripten-patches/` (vendored, one-to-few-line changes each)
`ring-0.17.9`, `socket2` + `socket2-0.5.7`, `reqwest-0.11.27` + `reqwest-0.12.12`,
`slug-0.1.6`, `tar-0.4.41`, `astral-tokio-tar-0.6.3`, `deno_doc-0.201.0`.
Recurring theme: crates assume `target_arch = "wasm32"` ⟹ browser, but
emscripten is wasm **and** unix with a full libc — so their wasm branches are
gated with `not(target_os = "emscripten")` or `not(unix)`.

### Build & run
```sh
# release build (needs EM_BINARYEN_ROOT — the emsdk .emscripten's is wrong)
RUSTUP_TOOLCHAIN=1.95.0 EMCC_CFLAGS='-fwasm-exceptions' \
  EM_CONFIG=…/emsdk/upstream/emscripten/.emscripten \
  EM_LLVM_ROOT=…/emsdk/upstream/bin \
  EM_BINARYEN_ROOT=…/emsdk/upstream \
  PATH=…/emscripten:…/workers-rs/wasm-bindgen/target/debug:$PATH \
  cargo build --release --target wasm32-unknown-emscripten -p registry_api

npx wrangler@latest dev   # global wrangler 4.70 rejects emscripten's `import source`
```

## Blockers cleared (dependency/toolchain)
jemalloc (cfg-gated) · tokio 1.40→mio 1.0 (no emscripten backend → patched tokio
1.52.3/mio 1.2.1) · aws-lc-sys (dropped rust-s3 tls on wasm) · reqwest browser
backend (forced native) · socket2 `IovLen` · ring RNG · hyper server tokio stubs
· wasm-bindgen ecosystem unification · slug cdylib · tar/tokio-tar/deno_doc
wasm-vs-unix cfg · askalono zstd→gzip · async-tar→tokio-tar.

**The one real wall — ring 0.16.20 (jsonwebtoken 8 / x509-parser 0.15) has no
asm-free Montgomery multiply** (asm-only, needed by RSA+ECDSA) — was resolved by
migrating **jsonwebtoken→10 / jsonwebkey→0.4 / x509-parser→0.16**, which use
pure-Rust crypto (rsa/p256/p384/ed25519-dalek/fiat-crypto) and drop ring 0.16
entirely. No jsr source changes were needed for that migration.

## Known gaps
- Config comes from the JS `env` (copied into the process env for clap). Secrets
  via `wrangler secret`.
- Not yet exercised end-to-end at runtime (needs a reachable DB + S3).
- The release `.wasm` is ~35 MB (over CF's production size limit; fine for `dev`).
