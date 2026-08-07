# Experiment: `registry_api` as a `wasm32-unknown-emscripten` Cloudflare Worker

This is an **experimental, non-portable** branch that runs the *entire* jsr API
server (`api/`, crate `registry_api`) as a Cloudflare Worker compiled to
`wasm32-unknown-emscripten` — using the [workers-rs] emscripten pipeline, which
(unlike `wasm32-unknown-unknown`) supports tokio, hyper, real sockets, DNS, and
rustls+ring TLS.

**Status (2026-07-11):** builds, links, runs, and **reliably serves real
requests** on `guybedford/workerd@hyperdrive-synthetic-ip` via `wrangler dev`.
Against a local Postgres, `GET /api/packages` returns real query results across
**85/85 sequential requests** (~0.11 s each); other routes behave correctly
(e.g. `/api/scopes/std` → a proper `404 scopeNotFound` JSON body). The module
instantiates, emscripten bootstraps, the `fetch` export runs, config is read
from the JS `env`, the router is built per request, tokio async networking is
live, and the sqlx Postgres connection works. Getting there past "it builds"
took a chain of **runtime** fixes — see [Runtime reliability](#runtime-reliability).
Remaining rough edge: **concurrent/parallel** requests are still flaky (single
DB connection + single-threaded emscripten runtime); sequential is solid.

> **Toolchain status (2026-07-11):** the two `tools/acorn-optimizer.mjs` bugs the
> earlier reconstruction hit are now **fixed upstream** in `guybedford/emscripten@cf`
> (`a1c42cd71` source-phase wasm imports, `7e31b8a3b` reserved `default` export),
> so the `--release` link succeeds on a fresh `cf`. One release-only issue
> remains: the optimized JS pipeline **drops emscripten's
> `import source wasmModule from './registry_api.wasm'`** statement (the debug
> build keeps it), so workerd throws `ReferenceError: wasmModule is not defined`
> at startup. Worked around by re-prepending that line to the built `.js`
> post-link; the real fix belongs in emscripten (reported separately, not
> patched here).

> **This branch is not buildable by CI or on another machine as-is.** The
> `[patch.crates-io]` recipe below points at sibling checkouts (`../workers-rs`
> tokio fork + wasm-bindgen) plus a `.emscripten-patches/` directory of vendored
> crates that are **not committed here** (24 MB, mostly noise). The committed
> changes are the jsr-side source + config only; native `cargo build` still
> works. **The workers-rs tokio fork also needs 3 uncommitted methods** (see
> below) that exist only locally — the one genuine non-jsr source change, shipped
> as a patch in the companion gist ([b4c401d](https://gist.github.com/crowlKats/b4c401de9997b49406ad782297e0e869)). This doc is the reproduction recipe.

[workers-rs]: https://github.com/cloudflare/workers-rs

## What's in this PR (committed)

| File | Change |
| --- | --- |
| `api/src/main.rs` | Split the entry point: native keeps `#[tokio::main]` + `Server::bind().serve()`; wasm adds a `#[wasm_bindgen(tokio)] fetch(Request, env, ctx)` export that bridges `web_sys::Request ↔ hyper ↔ routerify`. Shared setup extracted into `build_router(config)`. The `fetch` export **builds the router (and DB connection) per request** — a Worker can't reuse async I/O across requests — and **DNS-prewarms** the DB/S3 hosts (`tokio::net::lookup_host`) before the blocking `getaddrinfo` inside `Database::connect`/rust-s3. |
| `api/src/tracing.rs` | Make `set_global_default` idempotent on wasm: the per-request router rebuild calls `setup_tracing` each time, and the global subscriber may only be installed once. Native still panics on an unexpected double-init. |
| `api/src/db/database.rs` | On wasm, use a **lazy single-connection pool** (`max_connections(1)`, `min_connections(0)`, `connect_lazy_with`, no idle/lifetime reaper, `test_before_acquire(false)`): a pooled connection that is opened, parked idle, then reused hangs across the persistent thread-local emscripten reactor. Opens one connection at query time and consumes it in-request, like the goose example. |
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
  so hyper 0.14's server + connector types compile (each is a one-liner or
  small shim; the two `TcpListener` methods are unreachable — `TcpListener` is
  an uninhabited `enum {}` on emscripten — and exist only to typecheck):
  - `TcpStream::poll_peek(&self, cx, buf) -> Poll<io::Result<usize>>` — real
    impl over `recv(MSG_PEEK)` on the reactor-backed fd
    (hyper `server/tcp.rs` `AddrStream::poll_peek`).
  - `impl FromRawFd for TcpSocket` — hyper's connector
    (`client/connect/http.rs`) does `TcpSocket::from_raw_fd(socket.into_raw_fd())`
    via the trait, not the existing inherent `from_std_stream`.
  - `TcpListener::poll_accept(&self, cx) -> Poll<io::Result<(TcpStream, SocketAddr)>>`
    — `match *self {}` (hyper `server/tcp.rs` `AddrIncoming`).

### Root `Cargo.toml` `[patch.crates-io]`
```toml
# EXPERIMENT (temporary): build registry_api against workers-rs emscripten patches.
[patch.crates-io]
# workers-rs sibling forks — real emscripten support absent upstream:
libc = { path = "…/workers-rs/libc" }                          # +12 lines: pthread_sigmask/sigwait/faccessat decls
tokio = { path = "…/workers-rs/tokio/tokio" }                  # +41 commits: sockfs TcpStream, async DNS, event-loop rt (upstream is mio-only, no emscripten)
tokio-macros = { path = "…/workers-rs/tokio/tokio-macros" }    # moves with tokio
wasm-bindgen = { path = "…/workers-rs/wasm-bindgen" }          # implements #[wasm_bindgen(tokio)] (used by main.rs fetch export)
wasm-bindgen-macro-support = { path = "…/workers-rs/wasm-bindgen/crates/macro-support" }
wasm-bindgen-shared = { path = "…/workers-rs/wasm-bindgen/crates/shared" }
wasm-bindgen-futures = { path = "…/workers-rs/wasm-bindgen/crates/futures" }
js-sys = { path = "…/workers-rs/wasm-bindgen/crates/js-sys" }  # wasm-bindgen family is version-locked as a set
web-sys = { path = "…/workers-rs/wasm-bindgen/crates/web-sys" }

# .emscripten-patches/ — one-to-few-line cfg fixes to registry crates:
ring = { path = ".emscripten-patches/ring-0.17.9" }            # +emscripten to getrandom cfg (SystemRandom). 0.17.9 not the fork's 0.17.14: graph resolves to 0.17.9 (rustls 0.21), so a 0.17.14 patch is "not used in the crate graph"
socket2 = { path = ".emscripten-patches/socket2-0.6.4" }       # +emscripten to IovLen=c_int arm (graph wants 0.6.4, not 0.6.2)
socket2_05 = { package = "socket2", path = ".emscripten-patches/socket2-0.5.7" }  # same fix, 0.5.x via older hyper/reqwest
reqwest = { path = ".emscripten-patches/reqwest-0.11.27" }     # force native backend on emscripten
reqwest_012 = { package = "reqwest", path = ".emscripten-patches/reqwest-0.12.12" }
slug = { path = ".emscripten-patches/slug-0.1.6" }             # drop cdylib crate-type
tar = { path = ".emscripten-patches/tar-0.4.41" }              # unix-vs-wasm32 cfg (emscripten is both) → not(target_os="emscripten")
astral-tokio-tar = { path = ".emscripten-patches/astral-tokio-tar-0.6.3" }  # same fix
deno_doc = { path = ".emscripten-patches/deno_doc-0.201.0" }   # exclude emscripten from browser-wasm branch
```

The recurring `.emscripten-patches/` theme: a crate assumes `target_arch =
"wasm32"` ⟹ browser, but emscripten is wasm **and** unix with a full libc.
Every `cfg(target_arch = "wasm32")` browser branch becomes
`all(target_arch = "wasm32", not(target_os = "emscripten"))`, and its
`not(target_arch = "wasm32")` counterpart becomes
`any(not(target_arch = "wasm32"), target_os = "emscripten")`.

### `.emscripten-patches/` (vendored, one-to-few-line changes each)
`ring-0.17.9`, `socket2` + `socket2-0.5.7`, `reqwest-0.11.27` + `reqwest-0.12.12`,
`slug-0.1.6`, `tar-0.4.41`, `astral-tokio-tar-0.6.3`, `deno_doc-0.201.0`.
Recurring theme: crates assume `target_arch = "wasm32"` ⟹ browser, but
emscripten is wasm **and** unix with a full libc — so their wasm branches are
gated with `not(target_os = "emscripten")` or `not(unix)`.

### Build & run
```sh
# 0. Bring up a local Postgres + apply migrations. sqlx's `query!` macros verify
#    against a LIVE database at COMPILE time, so this is needed to build at all.
docker compose up -d postgres
DATABASE_URL='postgres://user:password@localhost/registry' \
  sqlx migrate run --source api/migrations

# 1. Release build (needs EM_BINARYEN_ROOT — the emsdk .emscripten's is wrong;
#    DATABASE_URL must be set for the sqlx compile-time query checks).
RUSTUP_TOOLCHAIN=1.95.0 EMCC_CFLAGS='-fwasm-exceptions' \
  DATABASE_URL='postgres://user:password@localhost/registry' \
  EM_CONFIG=…/emsdk/.emscripten \
  EM_LLVM_ROOT=…/emsdk/upstream/bin \
  EM_BINARYEN_ROOT=…/emsdk/upstream \
  PATH=…/emscripten:…/workers-rs/wasm-bindgen/target/debug:$PATH \
  cargo build --release --target wasm32-unknown-emscripten -p registry_api

# 2. Work around the release import-source stripping (see Toolchain status):
#    re-prepend the line the optimizer drops, to BOTH the primary and deps copy.
for js in target/wasm32-unknown-emscripten/release/{,deps/}registry_api.js; do
  head -c13 "$js" | grep -q 'import source' \
    || sed -i "1i import source wasmModule from './registry_api.wasm';" "$js"
done

# 3. Run on the forked workerd. Override localhost→127.0.0.1 (emscripten's
#    resolver returns EAI -2 for `localhost`), disable OTLP (its background
#    export tasks starve the single-threaded event loop), and skip migrations
#    (already applied; running them per request opens an eager connection).
MINIFLARE_WORKERD_PATH=…/workerd/bazel-bin/src/workerd/server/workerd \
  npx wrangler@^4 dev --ip 127.0.0.1 --port 8787 \
    --var DATABASE_URL:'postgres://user:password@127.0.0.1:5432/registry' \
    --var S3_ENDPOINT:'http://127.0.0.1:9000' \
    --var OTLP_ENDPOINT:'' \
    --var DATABASE_DISABLE_MIGRATIONS:'1'

curl -s 'http://127.0.0.1:8787/api/packages?limit=1'   # -> {"items":[],"total":0}
```

## Runtime reliability

"It builds and links" was only half the battle; making it *serve requests
reliably* needed four more fixes, each found by observing a distinct failure
mode against a real DB. Proof these are jsr-side (not platform) bugs: the
`emscripten-goose` example does repeated real outbound TCP+TLS calls reliably
(5/5), so socket I/O across requests works — jsr's own patterns were the issue.

1. **DNS — literal IP + prewarm.** emscripten's resolver returns `EAI -2` for
   `localhost` (both the sync `getaddrinfo` and tokio's async `lookup_host`), so
   the DB/S3 hosts must be a literal `127.0.0.1`. For real hostnames, the sync
   `getaddrinfo` only answers from the resolution cache, so `fetch` first
   **prewarms** each host via `tokio::net::lookup_host` (goose's pattern).
2. **Disable OTLP in the worker.** With `OTLP_ENDPOINT` set, the tracing
   `BatchSpanProcessor`'s `tokio::spawn`ed export tasks starve the single-threaded
   emscripten event loop, and DB `acquire()` times out. `OTLP_ENDPOINT=''`.
3. **Per-request router.** A Cloudflare Worker isolates I/O per request
   (*"Cannot perform I/O on behalf of a different request"*). The router — and its
   DB connection — is therefore built inside each request, not cached in a
   `OnceCell`. (`setup_tracing` was made idempotent so the repeat rebuild's global
   subscriber install doesn't panic.)
4. **Lazy single-connection sqlx pool.** The actual reliability killer: a pooled
   connection opened, **parked idle, then reused** hangs across the persistent
   thread-local emscripten reactor. The wasm pool is `max_connections(1)` +
   `connect_lazy_with` + no idle/lifetime reaper, opening one connection at query
   time and consuming it in-request (`DATABASE_DISABLE_MIGRATIONS=1` avoids an
   eager connection at router-build time).

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
- **Concurrent requests are flaky.** Parallel requests contend on the single DB
  connection and the single-threaded emscripten runtime ("Promise will never
  complete"); sequential request handling is reliable (85/85). A per-request
  isolate (production Workers) or a concurrency-safe connection story would be
  needed to lift this.
- **S3 paths untested** — the smoke run has no MinIO up, so only DB-backed routes
  (e.g. `/api/packages`) are exercised end to end.
- The release `.wasm` is ~35 MB (over CF's production size limit; fine for `dev`).
- Depends on 3 uncommitted `workers-rs/tokio` methods (companion gist ([b4c401d](https://gist.github.com/crowlKats/b4c401de9997b49406ad782297e0e869)) patch) and
  the emscripten release import-source workaround above.
