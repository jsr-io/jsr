# Experiment: `registry_api` as a `wasm32-unknown-emscripten` Cloudflare Worker

This is an **experimental, non-portable** branch that runs the *entire* jsr API
server (`api/`, crate `registry_api`) as a Cloudflare Worker compiled to
`wasm32-unknown-emscripten` — using the [workers-rs] emscripten pipeline, which
(unlike `wasm32-unknown-unknown`) supports tokio, hyper, real sockets, DNS, and
rustls+ring TLS.

**Status (2026-10-10):** builds, links, runs and serves real requests on
**stock `wrangler dev`** — no forked workerd, no hand-patched emscripten, no
sibling `workers-rs` checkout. Everything below was measured with six per-crate
cfg fixes applied **locally**; those are not in the repo (jsr carries no local
patches of third-party crates), so the wasm link does not succeed from a clean
checkout until they land upstream. The pipeline is `api/build-worker.sh`
(`worker-build --emscripten --release`, workers-rs 0.8.7) driven by wrangler's
`[build]` command, on the upstream tokio patchset. Against a local Postgres:
`GET /api/packages?limit=1` → `{"items":[],"total":0}` in ~0.13 s, **32/32 and
64/64 concurrent** requests return 200, `/api/scopes/std` → a proper
`404 scopeNotFound`, `/api/users/<bad>` → `400 malformedRequest`. Release wasm is
**24.1 MB** (`opt-level = "s"` + fat LTO; see [Size](#size)).

Getting past "it builds" originally took a chain of **runtime** fixes — see
[Runtime reliability](#runtime-reliability); they are all still in place.

> **Toolchain status (2026-09-30):** superseded by `worker-build --emscripten`,
> which installs the pinned emsdk 6.0.10 into its cache and applies the
> emscripten frontend patches the Rust link needs (`epoll-listeners.patch`,
> `noderawsockets-dns.patch`) itself, fetches the `wasm-bindgen` 0.2.129 CLI and
> esbuild, and rewrites emscripten's `import source` of the wasm module to a
> plain `import` — so the old `guybedford/emscripten@cf` checkout and the
> post-link re-prepend workaround are both gone.

> **Patchset status (2026-09-30):** the workers-rs side is no longer a sibling
> checkout. `[patch.crates-io]` now takes the **published** emscripten tokio
> patchset verbatim from `cloudflare/workers-rs@main`
> [`examples/emscripten-tokio/Cargo.toml`](https://github.com/cloudflare/workers-rs/tree/main/examples/emscripten-tokio)
> — git-tagged `guybedford/tokio@1.53.1-cf.emscripten` +
> `guybedford/mio@1.2.3-cf.emscripten` (libc now comes from crates.io) — and the
> whole wasm-bindgen family comes from crates.io (0.2.129 / futures 0.4.79 /
> js-sys 0.3.106 / web-sys 0.3.106), which carries
> `#[wasm_bindgen(experimental_tokio)]` upstream. The **3 uncommitted tokio
> methods** the old sockfs fork needed are gone: the layering tokio drives
> standard `net` through mio, so hyper 0.14's server and connector types compile
> as-is. Verified on this patchset: `cargo check` passes for both targets, the
> `worker-build --emscripten --release` link succeeds, and the worker serves on
> stock `wrangler dev` (see Status).
>
> **The wasm link does not succeed from a clean checkout, by design.** jsr does
> not carry local patches of third-party crates, so the only `[patch]` entries in
> the repo are git tags — the workers-rs tokio patchset, which is Cloudflare's own
> and tracks two open PRs. Six further cfg fixes (five crates) are *not* here;
> every measurement below was taken with them applied locally, and they are
> upstream-bound. ["Which patches are still needed"](#which-patches-are-still-needed-measured-2026-09-30)
> records what each one is and what has to happen for it to go away. Native
> `cargo build`/`test`/`clippy` are unaffected and green.

[workers-rs]: https://github.com/cloudflare/workers-rs

## What's in this PR (committed)

| File | Change |
| --- | --- |
| `api/src/main.rs` | Split the entry point: native keeps `#[tokio::main]` + `Server::bind().serve()`; wasm adds a `#[wasm_bindgen(experimental_tokio = "isolated")] fetch(Request, env, ctx)` export (no `js_namespace` — worker-build's generated shim provides the `export default`, a `WorkerEntrypoint` subclass that forwards to it) that bridges `web_sys::Request ↔ hyper ↔ routerify`. Shared setup extracted into `build_router(config)`. The `fetch` export **builds the router (and DB connection) per request** — a Worker can't reuse async I/O across requests — and **DNS-prewarms** the DB/S3 hosts (`tokio::net::lookup_host`) before the blocking `getaddrinfo` inside `Database::connect`/rust-s3. |
| `api/src/tracing.rs` | Make `set_global_default` idempotent on wasm: the per-request router rebuild calls `setup_tracing` each time, and the global subscriber may only be installed once. Native still panics on an unexpected double-init. |
| `api/src/db/database.rs` | On wasm, use a **lazy single-connection pool** (`max_connections(1)`, `min_connections(0)`, `connect_lazy_with`, no idle/lifetime reaper, `test_before_acquire(false)`): a pooled connection that is opened, parked idle, then reused hangs across the persistent thread-local emscripten reactor. Opens one connection at query time and consumes it in-request, like the goose example. |
| `api/src/api/mod.rs` | `cfg`-gate the `/debug/mem_*` (jemalloc) routes off wasm. |
| `api/src/tarball.rs` | Replace `async-tar` (pulls async-std → async-io epoll reactor, won't build on wasm) with **`astral-tokio-tar`** (tokio-based). Bridges the S3 futures-io stream via `tokio_util::compat` + `tokio::io::BufReader` + `async_compression::tokio::bufread::GzipDecoder`. |
| `api/src/npm/tarball.rs` | Same `async-tar → tokio_tar` swap in the test. |
| `api/Cargo.toml` | `cfg`-gate jemalloc/rust-s3-tls/opentelemetry off wasm; add the `worker` SDK dep worker-build requires (with `experimental_tokio`); trim tokio from `full` to a wasm-buildable feature set (the patched tokio hard-errors on `process`/`signal`/`rt-multi-thread` for emscripten) and re-add `full` for native; add wasm-target deps (wasm-bindgen/web-sys/js-sys/futures); swap async-tar→astral-tokio-tar; `askalono`'s `gzip` feature (zstd's C lib won't build on wasm); **jsonwebtoken 8→10 / jsonwebkey 0.3→0.4 / x509-parser 0.15→0.16** (see "ring" below). |
| `.cargo/config.toml` | `wasm32-unknown-emscripten` rustflags + emcc link args, incl. `--cfg=tokio_unstable` and `--cfg=wasm_bindgen_unstable_tokio` (no-op for native targets). |
| `api/wrangler.toml` | Worker config: `main = "build/index.js"` plus a `[build] command` that runs `worker-build --emscripten --release` with the extra `RUSTFLAGS` it cannot infer. |

## Reproduction recipe

### Toolchain
`worker-build --emscripten` (workers-rs 0.8.7, `cargo install worker-build` —
needs rustc ≥ 1.91, so not jsr's pinned 1.89.0) is the driver. It provisions the
Emscripten SDK and patches its frontend, and supplies `-Crelocation-model=static`,
the exnref exception-handling flags (`-Cllvm-args=-wasm-use-legacy-eh=false`,
`EMCC_CFLAGS=-fwasm-exceptions -sWASM_LEGACY_EXCEPTIONS=0`) and the `-s` link
args. Three things about it that shape this crate's config:

- **It requires the `worker` SDK crate in the graph** (`^0.8`) and refuses to run
  without it — it version-locks its own toolchain against it. `registry_api`
  therefore carries `worker = { version = "0.8.7", features =
  ["experimental_tokio"] }` as a wasm-only dependency even though the `fetch`
  export is raw wasm-bindgen and none of the SDK's router or macros are used.
- That `experimental_tokio` feature is also how worker-build decides to pass
  `--cfg=tokio_unstable --cfg=wasm_bindgen_unstable_tokio` (see
  `worker_tokio_feature()` in `worker-build/src/build/manifest.rs`).
- It passes flags through the `RUSTFLAGS` **env var**, which *overrides*
  `[target.*].rustflags` in `.cargo/config.toml` (ours go first, worker-build
  appends its own). Anything this project needs on top has to be in that env
  var: `-sSTACK_SIZE=8MB` (the 64KB default overflows building regex-automata's
  meta DFA) and `-Oz` (emcc's link step defaults to `-O0`, which skips wasm-opt
  entirely). `api/wrangler.toml`'s `[build] command` sets them.

`.cargo/config.toml` keeps the full flag set for **plain** `cargo build`/`cargo
check --target wasm32-unknown-emscripten` runs, which do not go through
worker-build (they still need an `emcc` on `PATH` for the C build scripts of
`ring`, `tree-sitter`, `zstd`).

### Root `Cargo.toml` `[patch.crates-io]`
```toml
# The workers-rs emscripten tokio patchset (upstream's version also patches libc
# from git; 0.2.190 made that unnecessary — this only redirects the mio fork's
# hardcoded git libc at the published crate):
[patch."https://github.com/rust-lang/libc"]
libc = { version = "0.2.190" }

[patch.crates-io]
tokio = { git = "https://github.com/guybedford/tokio", tag = "1.53.1-cf.emscripten" }       # tokio-rs/tokio#8484: emscripten event-loop runtime
tokio-macros = { git = "https://github.com/guybedford/tokio", tag = "1.53.1-cf.emscripten" }
mio = { git = "https://github.com/guybedford/mio", tag = "1.2.3-cf.emscripten" }  # tokio-rs/mio#1969: emscripten backend

```

That is the whole committed patch table: three git tags from Cloudflare's own
patchset. **Six more fixes, across five crates, are deliberately absent** — jsr
does not vendor or locally patch third-party crates, so these have to land
upstream (or the crate has to leave the graph) before the emscripten link can
work from a clean checkout. Measured sizes, for reference:

| Crate | What it needs | Changed lines |
| --- | --- | --- |
| `ring` 0.17.9 | implement `SecureRandom for SystemRandom` on emscripten; without it rustls does not compile | 2 |
| `socket2` 0.5.10 | add emscripten to the `IovLen = c_int` arm | 4 |
| `reqwest` 0.11.27 / 0.12.12 | force the native (hyper) backend instead of browser fetch | 110, all the same cfg string over ~55 dependency blocks |
| `slug` 0.1.6 | drop the `cdylib` crate-type | 3 |
| `astral-tokio-tar` 0.6.3 | unix-vs-wasm32 duplicate item definitions | ~10 |

The recurring theme: a crate assumes `target_arch =
"wasm32"` ⟹ browser, but emscripten is wasm **and** unix with a full libc.
Every `cfg(target_arch = "wasm32")` browser branch becomes
`all(target_arch = "wasm32", not(target_os = "emscripten"))`, and its
`not(target_arch = "wasm32")` counterpart becomes
`any(not(target_arch = "wasm32"), target_os = "emscripten")`.

### Which patches are still needed (re-checked 2026-10-08)
Dropped every patch, then added back only what a **full release build** plus the
native test suite actually required (a `cargo check` is not enough — it never links,
so it misses `slug`, and it aborts at the first failure, hiding everything
downstream).

**Retired by upgrading rather than patching:**

| Crate | How |
| --- | --- |
| `socket2` 0.6 | 0.6.5 carries the emscripten `IovLen` cfg upstream; tokio uses it unpatched |
| `deno_doc` | [denoland/deno_doc#857] shipped in **0.208.0**. The bump also needs `deno_graph` `=0.109.0` → `=0.111.0` (deno_doc 0.208 requires it, and two deno_graph versions in one graph are different types), plus three mechanical API updates: `BuildOptions::unstable_text_imports` is gone (0.111 allows `with { type: "text" }` unconditionally — note jsr had it `false`, and there is no flag left to keep it off), `BuildOptions` gained `unstable_config_imports` + `prefer_cached_jsr_versions` (both `false` = previous behaviour), and `GenerateOptions` gained `symbol_listing_limit` (`None` = render everything, as before) |
| `libc` | The declarations shipped in **0.2.190** (2026-10-02). Note the release alone was not enough: the mio fork pins libc by git URL, so `[patch."https://github.com/rust-lang/libc"]` has to redirect that source at the published crate, otherwise the graph carries two libc copies |
| `tar` | **0.4.46** builds for emscripten unpatched. The blocker was jsr-side: `testdata/tarballs/big_file/big.txt` is 21MB of zeros stored *sparsely*, and tar ≥ 0.4.46 detects holes and writes it as a `GNUSparse` entry, which the reader rejects as `invalidEntryType` before reaching the size check `publish::tests::big_file` is about. Fixed by `tar.sparse(false)` in the test helpers, which also makes the fixture independent of the checkout's filesystem |

**Still required, with the exact blocker:**

| Crate | Why it is needed | Blocker |
| --- | --- | --- |
| `ring` 0.17.9 | No emscripten `SystemRandom`, so rustls does not compile | Fix is open upstream ([briansmith/ring#2877]) but sits on ring main, and ring ≥ 0.17.10 needs `cc ^1.2`. `tree-sitter-javascript 0.21.4` is the **only** crate left pinning `cc = "~1.0.90"` (all other tree-sitter crates already allow 1.2); its next release needs tree-sitter 0.23, so this means bumping tree-sitter-highlight + all eleven grammars together (94 call sites). Then the PR branch works as-is |
| `socket2` 0.5.10 | `msg_iovlen` is an `int` on emscripten | In the graph solely because routerify declares `hyper = { features = ["server", "tcp"] }`, and hyper 0.14's `tcp` feature pulls socket2 0.5. Retires with a move off routerify/hyper 0.14 |
| `reqwest` 0.11.27 | Picks the browser fetch backend; its wasm path also pulls wasm-streams 0.4, which does not compile under emscripten's unwinding panics | The emscripten-aware gate exists **only in 0.13**, and this group cannot get there: besides jsr's own client, 0.11 is required by `oauth2` 4.4.2 and `postmark` 0.10. `oauth2` 5.0.0 (latest) is on reqwest `^0.12` — so **0.12 is this group's ceiling**, `postmark` 2.0.1 notwithstanding (it is on `^0.13`). Consolidating 0.11 -> 0.12 deletes *this* patch but not the other one, and costs the oauth2 4->5 typestate rewrite + postmark 0.10->2.0 |
| `reqwest` 0.12.12 | same | Not jsr's at all: `rust-s3` 0.38.0, `opentelemetry-otlp` 0.27 and `opentelemetry-http` — **even 0.31, the latest** — all require `^0.12`, which 0.13 cannot satisfy, and a `[patch]` must match the requirement. The fix was never backported: **0.12.28**, the newest 0.12, still carries bare `cfg(target_arch = "wasm32")` on every dependency block |
| `slug` 0.1.6 | cdylib + `-Crelocation-model=static` → "recompile with -fPIC" at link | jsr never uses slug: it is a **mandatory** (non-optional, un-gated) dependency of comrak 0.29, which deno_doc pins exactly. Newer comrak demotes it to a dev-dependency, so this retires when deno_doc bumps comrak |
| `astral-tokio-tar` 0.6.3 | unix-vs-wasm32 cfg, duplicate item definitions | Nothing upstream; 0.7.0 still pairs bare `cfg(unix)` with `cfg(target_arch = "wasm32")`, so bumping does not help. Only replacing the dependency fixes it |

**Checked again 2026-10-08, no change for the rest:** ring is still 0.17.14 with
[#2877] open; there is no new socket2 0.5.x; reqwest's fix is still only on 0.13
(latest 0.13.5) and **rust-s3 0.38.0 — released 2026-10-04 — is still on reqwest
0.12**; comrak 0.56.0 demotes slug to a dev-dependency but deno_doc 0.208.0 still
pins comrak 0.29.0; astral-tokio-tar 0.7.0 still pairs bare `cfg(unix)` with
`cfg(target_arch = "wasm32")`; tokio#8484 is still open; worker-build is still 0.8.7.

**reqwest, looked at properly 2026-10-10:** neither copy can be upgraded from
jsr's side, and they fail for different reasons. The emscripten gate
(`all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))`)
landed in 0.13 and was never backported — reqwest 0.12.28, the latest 0.12, still
has the bare `cfg(target_arch = "wasm32")`. The 0.12 copy is pulled only by
rust-s3 and opentelemetry (both `^0.12`, including opentelemetry-http 0.31), so it
needs releases from those projects. The 0.11 copy is capped at 0.12 by oauth2
5.0.0 (`^0.12`), so the only reachable move is a 0.11 -> 0.12 consolidation, which
retires one of the two patches at the cost of two major upgrades in the
auth/email path (oauth2 4->5, postmark 0.10->2.0) — declined earlier on
risk/reward.

[#2877]: https://github.com/briansmith/ring/pull/2877
[briansmith/ring#2877]: https://github.com/briansmith/ring/pull/2877
[rust-lang/socket2#660]: https://github.com/rust-lang/socket2/pull/660
[denoland/deno_doc#857]: https://github.com/denoland/deno_doc/pull/857

### Build & run
```sh
# 0. Bring up a local Postgres + apply migrations. sqlx's `query!` macros verify
#    against a LIVE database at COMPILE time; the build below uses the committed
#    `api/.sqlx` data instead (SQLX_OFFLINE=true), but the running worker needs
#    the schema.
docker compose up -d postgres
DATABASE_URL='postgres://user:password@localhost/registry' \
  sqlx migrate run --source api/migrations

# 1. Install the build driver (needs rustc >= 1.91, not jsr's pinned 1.89.0).
RUSTUP_TOOLCHAIN=stable cargo install worker-build --version 0.8.7

# 2. Build + run. wrangler's `[build] command` in api/wrangler.toml runs
#    worker-build itself (provisioning emsdk 6.0.10 + the wasm-bindgen CLI +
#    esbuild on first use, ~1.5 min for the link afterwards), and `main` points
#    at its `api/build/index.js` output.
#    Override localhost→127.0.0.1 (emscripten's resolver returns EAI -2 for
#    `localhost`), disable OTLP (its background export tasks starve the event
#    loop), and skip migrations (already applied above; running them per request
#    opens an eager connection).
cd api && npx wrangler@latest dev --ip 127.0.0.1 --port 8787 \
    --var DATABASE_URL:'postgres://user:password@127.0.0.1:5432/registry' \
    --var S3_ENDPOINT:'http://127.0.0.1:9000' \
    --var OTLP_ENDPOINT:'' \
    --var DATABASE_DISABLE_MIGRATIONS:'1'

curl -s 'http://127.0.0.1:8787/api/packages?limit=1'   # -> {"items":[],"total":0}

# Fast iteration without a link (type-check only) still needs an emcc on PATH
# for the C build scripts, and uses .cargo/config.toml's rustflags:
SQLX_OFFLINE=true cargo check --target wasm32-unknown-emscripten -p registry_api
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

## Size

Measured 2026-09-30, `index_bg.wasm` after worker-build's link (emcc already runs
`wasm-opt -Oz` + meta-DCE during it, via `-Clink-arg=-Oz`):

| Release profile | Size | Sequential latency | 64 parallel |
| --- | --- | --- | --- |
| `opt-level = 3`, no LTO (cargo default) | 36.6 MB | ~0.11 s | 64/64 × 200 |
| `opt-level = "s"` + `lto = true`, 1 CGU | **24.3 MB** | ~0.13 s | 64/64 × 200 |
| `opt-level = "z"` + `lto = true`, 1 CGU | 22.0 MB | ~0.31 s | 53/64 × 200, 11 × 500 |
| `opt-level = 3` + `lto = true`, 1 CGU | — | — | does not link (see below) |

`build-worker.sh` uses the `"s"` row: −34% for +20% latency. `"z"` buys another
2 MB but triples latency, and at 64-way concurrency the slower code starves the
per-request single-connection pool — 11 of 64 requests failed with
`pool timed out while waiting for an open connection` after 18 s. These settings
are exported per build rather than written into `[profile.release]`, which the
native production binary also uses.

**A second `wasm-opt --converge -Oz` pass is not worth it.** With the module's own
feature set it saves 7 KB. Adding `--all-features` appears to save ~600 KB, but it
also enables custom descriptors, and workerd then refuses the module at startup:
`CompileError: WasmModuleObject::Compile(): invalid heap type 'exact', enable with
--wasm-custom-descriptors`.

**Two toolchain bugs found while measuring** (both worth reporting upstream, not
worked around here):
1. `opt-level = 3` + fat LTO produces a module the wasm-bindgen 0.2.129 CLI cannot
   parse during emcc's post-link step: `failed to parse code section: type
   mismatch: catch_all_ref label must a subtype of (ref exn)`. `"s"` and `"z"` are
   fine, so it is specific to what LLVM emits for the exnref EH encoding at `-O3`.
2. wasm-opt 132's `--all-features` is not a safe superset for a Workers module —
   see the custom-descriptors failure above.

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
- **Concurrency works** since `experimental_tokio = "isolated"` (a fresh hosted
  runtime per `fetch`): 64/64 parallel requests return 200 on the release build,
  with no "Cannot perform I/O on behalf of a different request" or "Promise will
  never complete". The remaining ceiling is memory — jsr rebuilds the entire
  router (DB pool, S3 clients, moka caches, tracing) per request, times a
  runtime each — not correctness.
- **S3 paths untested** — the smoke run has no MinIO up, so only DB-backed routes
  (e.g. `/api/packages`) are exercised end to end.
- The release `.wasm` is 24.26 MB — see [Size](#size) for the profile trade-offs.
- **The committed branch does not link for wasm.** The two `[patch]` entries it
  does carry are pre-release: tokio#8484 is still open and mio#1969 is **merged**
  but unreleased (mio 1.2.4 predates it). The six per-crate cfg fixes are not in
  the repo at all, because jsr does not carry local patches of third-party
  crates — they have to land upstream first. Native `cargo test` is green without
  them (173/173). The 3 formerly uncommitted `workers-rs/tokio` methods and the
  emscripten import-source workaround are both gone — see Patchset/Toolchain
  status.
