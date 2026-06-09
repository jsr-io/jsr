# API overload / cache-miss investigation

**Date:** 2026-06-08 **Trigger:** The API server was doing ~100 req/s "felt"
while Cloudflare reported only ~3% of requests cached. The service was too
expensive and something looked structurally wrong.

## TL;DR

- The real load was **~600 req/s hitting the compute API origin, with 53%
  returning `504 Gateway Timeout`** — an overload death-spiral, not a healthy
  100 req/s.
- Root cause was **not** TTLs, `Vary`, or response headers. The `lb` Worker's
  `caches.default` cache writes were **fire-and-forget**
  (`caches.default.put(...)` with no `await` and no `ctx.waitUntil(...)`), and
  the `fetch` handler didn't even accept the execution context. Cloudflare tears
  the invocation down before the async `put` completes, so **nothing the lb
  proxied was ever cached** (all frontend pages + all API responses). That is
  the ~3% hit rate.
- **Fix:** thread the execution context through the lb and wrap both cache
  writes in `ctx.waitUntil(...)` (falling back to `await` when no ctx).
  Implemented in `lb/proxy.ts` + `lb/main.ts`, with a regression test in
  `lb/proxy_test.ts`.

## How we got there (evidence)

All figures from the Cloudflare GraphQL Analytics API, `jsr.io` zone, 24h
window.

### 1. Cache status by host

- `jsr.io` host: ~528M req/24h (~6,100 req/s); **0.8% hit**, 55% miss, 44%
  "none".
- **`registry-api-…run.app` (compute origin): ~598 req/s**, 53% miss / 47%
  "dynamic".
- `api.jsr.io` proper: only ~7 req/s. So the API load is _not_ arriving via
  `api.jsr.io` — it's the compute origin behind the proxy.

### 2. The compute load is concentrated on ~8 packages, not a long-tail crawl

Top compute-origin paths (req/s), each hitting `package` + `downloads` +
`versions` in lockstep:

- `functionalscript/functionalscript` ≈ 54 req/s
- `stsoftware/neat-ai` ≈ 48 req/s
- `windmill/windmill` ≈ 25 req/s
- then `mtkruto/mtkruto`, `zod/zod`, `skmtc/core`, `gramio/types`,
  `o-industrial/common`…

The three endpoints map exactly to one **package-page render** fan-out. Requests
show empty User-Agent and `run.app` host → frontend **SSR** subrequests (via the
lb service binding), which Cloudflare zone analytics cannot attribute to an
inbound client (service-binding calls are internal). Pinning the true upstream
originator would need GCP/Cloud Run request logs.

### 3. The origin is timing out

Compute origin (`run.app`) status codes over 24h:

- **504: 53.1% (~317 req/s)**
- 200: 44.7% (~267 req/s)
- 404 1.4%, 500 0.4%, rest negligible.

504s are uncacheable, so every retry/re-render re-hits the overloaded origin →
self-sustaining overload.

### 4. Capacity was _not_ the binding constraint

- `registry-api` Cloud Run: `max_instance_count = 30`,
  `max_instance_request_concurrency = 100`, 1 CPU, 1 Gi
  (`terraform/cloud_run_api.tf`).
- Owner confirmed only ~10 of the allotted instances are ever used → raising
  instance count would not help. The lever is **caching**.

### 5. The decisive test — the lb caches nothing it proxies

Live `curl` against production:

| Request                  | Path                               | Result                    |
| ------------------------ | ---------------------------------- | ------------------------- |
| `jsr.io/logo.svg`        | normal CDN (no Worker cache)       | `cf-cache-status: HIT` ✅ |
| `jsr.io/`                | frontend page via lb `cachedFetch` | no `Age`, never cached ❌ |
| `api.jsr.io/…/downloads` | API via lb `cachedFetch`           | no `Age`, never cached ❌ |

The raw backend (Cloud Run) response is fully cacheable and the TTLs are already
long:

```
cache-control: public, max-age=30, s-maxage=86400, stale-while-revalidate=86400   (downloads)
cache-control: public, max-age=30, s-maxage=2592000, stale-while-revalidate=86400 (package, 30d)
```

No `Set-Cookie`, no `Vary` on the stored copy. So headers/TTLs were fine.
`cf-cache-status: DYNAMIC` is expected for Worker-served responses and is _not_
a reliable signal; the reliable signal is the **`Age` header**, which Cloudflare
stamps when a response is served from `caches.default`. It never appeared on any
repeat request — meaning the cache `put` never persisted.

## Root cause

`lb/proxy.ts` — both `cachedFetch` and `proxyToR2` did:

```js
caches.default?.put(cacheKey, res.clone()); // not awaited, no waitUntil
```

and `lb/main.ts`'s handler was `async fetch(request, env)` — **no execution
context at all**.

In Cloudflare Workers, `Cache.put()` finishes _after_ the `Response` is returned
(it must read the cloned body and write to storage). Work not registered via
`ctx.waitUntil()` is cancelled when the invocation ends, so the write is
silently dropped. Result: 100% of Worker-proxied traffic (frontend pages **and**
API) misses cache and reaches origin → the 600 req/s + 504 spiral. The ~3%
"cached" was only true edge-cached assets (static files / R2) that never pass
through `cachedFetch`.

Ref: <https://developers.cloudflare.com/workers/runtime-apis/cache/#put>

## The fix (implemented)

- `lb/proxy.ts`: added an `ExecutionCtx` interface +
  `persistCacheWrite(ctx, write)` helper (`ctx.waitUntil(write)` when a context
  exists, else `await write` for tests). Wrapped both cache writes
  (`cachedFetch` for API/frontend, `proxyToR2` for modules/npm).
- `lb/main.ts`: added `ctx: ExecutionContext` to `fetch` and threaded it through
  `route` → `handleAPIRequest` / `handleNPMRequest` / `handleRootRequest` /
  `handleFrontendRoute` / `handleModuleFileRoute` → the proxy functions.
- `lb/proxy_test.ts`: added
  `proxyToBackend registers the cache write with ctx.waitUntil`.
- `deno check`, `deno lint`, and all 16 lb tests pass.

The existing auth/cookie/login bypass and URL-only cache-key logic are unchanged
— only the _persistence_ of the write was fixed.

## Discussion: Cache API vs `fetch` caching

Cloudflare docs recommend that **middleware Workers use `fetch`-based caching**
(`cf: { cacheEverything, cacheTtl/cacheTtlByStatus }`) rather than the manual
Cache API, because the runtime manages the cache lifecycle (and the `waitUntil`
footgun never arises).

We chose to **keep the Cache API + the `waitUntil` fix** because the lb relies
on explicit bypass semantics the Cache API expresses cleanly:

- skip cache for `Authorization` header, `token=` cookie, and `/login*` /
  `/logout` paths;
- URL-only cache key.

`fetch`-caching is riskier here: it keys on URL and does **not** vary on
`Cookie` by default, so a `token=`-authenticated request could be served a
previously-cached **anonymous** response — a data-correctness leak (JSR
responses differ by viewer, e.g. admins see archived packages). Switching would
require re-implementing the same bypass gate (conditional `cf.cacheEverything`),
and would also move the cache key onto the `…run.app` hostname (relevant to any
purge-on-publish design).

**Decision:** ship the `waitUntil` fix as the minimal correct change. A
`fetch`-based refactor remains a valid option but must preserve the auth bypass
and was deemed not worth the correctness risk for the immediate fix.

## Verification & open items

1. **After deploy, confirm caching is live:** `curl` a hot endpoint twice and
   check for an `Age:` header on the second hit. (Repeat the §5 test.)
2. **Rule out a config-side bypass:** if `Age` still doesn't appear, check
   Cloudflare dashboard → Caching → Cache Rules and Rules → Page Rules for any
   "Bypass cache" matching `jsr.io/*` or `api.jsr.io/*` — that would no-op
   `put()` regardless of code.
3. **Same footgun, lower priority:** `trackJSRDownload` / `trackNPMDownload`
   (`lb/main.ts`) are also fire-and-forget; their analytics writes can be
   dropped the same way. Wrap in `ctx.waitUntil` as a follow-up.
4. **`/downloads` cost:** the hammered handler runs multiple 90-day aggregation
   queries (`api/src/api/package.rs`). Once caching absorbs the steady state
   this matters less, but it's the most expensive hot query if cache is ever
   cold/missed.
5. **True originator:** identifying _what_ drives the ~8-package SSR storm needs
   GCP/Cloud Run request logs (X-Forwarded-For / User-Agent / Referer), which
   Cloudflare zone analytics can't show for service-binding traffic.

## Follow-up (2026-06-09): docs/diff still hammering origin

After the `waitUntil` fix shipped, overall cache hit rate rose from ~3% to only
~12.5%, and **two endpoints went the wrong way** — request rate to
`/api/scopes/:scope/packages/:package/versions/:version/docs` and
`/.../diff/:old_version/:new_version` **doubled or tripled** while everything
else dropped.

### Why they're the uncacheable tail

These two are the long tail the first fix couldn't help, for three reasons in
the code:

1. **`latest` docs only cached 60s.** The default package page (`/@scope/pkg`)
   renders by calling `/versions/latest/docs` (`frontend/utils/data.ts` →
   `version || "latest"`). The route was `cache_versioned(ONE_MINUTE, …)`, so
   the single most-visited docs call expired every 60s. The 30-day immutable arm
   only applies to pinned `@version` URLs (a minority of traffic).
2. **Per-symbol query fan-out.** docs/diff keys include
   `?entrypoint=…&symbol=…&all_symbols`; every symbol page of every package is a
   distinct cache entry, rarely re-hit before expiry. diff is keyed per
   `(old,new)` pair.
3. **404s were uncacheable at every layer.** A handler `Err` short-circuits
   before `cache()`/`cache_versioned()` set `Cache-Control`, so the error
   response carried none; the lb only stored `res.ok`. Every repeat 404 hit the
   origin. (These same 404s are why the two endpoints dominate error reporting.)

### Why the rate *rose* after the first fix

Before, the origin was in the 504 death-spiral and shed this load (timeouts,
failed renders). Once `package`/`versions`/`downloads` were absorbed by cache,
the origin recovered and the crawl/SSR ran at full speed — and docs/diff are the
part of that crawl that can't be cached. So their per-endpoint origin rate rose
while everything else fell; it's recovered throughput, not new demand.

### Fix (implemented)

- **Negative-cache 404s** on every cached route. `cache()` / `cache_versioned()`
  (`api/src/util.rs`) now catch a `404` `Err` and return the JSON error body
  with a `Cache-Control`:
  - **`EntrypointOrSymbolNotFound` on a non-`latest` (immutable) version** can
    never appear later, so it's cached for the **full duration, exactly like a
    normal `200`** for that version (e.g. 30 days for docs, 30 days for diff).
    This is essentially the whole fix for diff, whose versions are always pinned.
  - **All other 404s** (package/version not found, or anything on `latest`) get a
    short window: anonymous → `public, max-age=30, s-maxage=60,
    stale-while-revalidate=60`; authenticated → `private, no-store` (the lb
    bypasses its shared cache for authed requests anyway, so a `public` 404 can't
    cross viewers).
- **lb stores negative-cached 404s** (`lb/proxy.ts`, `cachedFetch`): cache when
  `res.ok`, or `status === 404` **and** the response opted in with an explicit
  `max-age`/`s-maxage` and isn't `private`/`no-store`. 200s keep prior behaviour
  (cached unless explicitly uncacheable). Regression tests added in
  `lb/proxy_test.ts`.
- **Raised the `latest` docs arm 60s → 5min** (`api/src/api/package.rs`,
  `cache_versioned(FIVE_MINUTES, THIRTY_DAYS, …)`) — ~5× fewer origin hits on the
  hottest docs call, still fresh enough that a new publish appears promptly.

`cargo check` + `test_package_docs`, `deno check`/`lint`/`fmt`, and all 20 lb
tests pass.

### Not done / open

- Only the docs route's `latest` arm was bumped to 5min; `docs/search`,
  `docs/search_structured`, `source`, `dependencies` still use `ONE_MINUTE`. Bump
  them too if their `latest` load stays high (each trades the same publish-→-stale
  window).
- Per-symbol fan-out is inherent; the levers above target the index/latest and
  404 slices, not the unique-symbol tail.

## Note

A Cloudflare API token was shared during this investigation (analytics-read
scope) and should be rotated/deleted.
