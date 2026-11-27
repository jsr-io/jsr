// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use axum::body::Body;
use axum::middleware::Next;
use axum::response::IntoResponse;
use futures::FutureExt;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::header;
use hyper::header::COOKIE;
use hyper::http::HeaderName;
use std::future::Future;
use std::sync::Arc;
use tracing::Span;
use tracing::error;
use tracing::field;
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use crate::api::ApiError;
use crate::external::github;
use crate::external::oidc::OidcProviderKind;
use crate::iam::IamInfo;
use crate::iam::ReqIamExt as _;
use shared::db::Database;
use shared::errors::SharedError;
use shared::ids::PackageName;
use shared::ids::ScopeName;
use shared::ids::Version;
use shared::router::RequestExt;

pub use shared::util::*;

/// The HTTP client the OAuth2 flows use to reach the providers' token and
/// revocation endpoints, in the shape `oauth2` accepts: pass
/// `&oauth2_http_request` to `request_async`. Separate from
/// [`shared_http_client`] because it must not follow redirects: an OAuth2
/// client that does is open to SSRF.
pub async fn oauth2_http_request(
  request: oauth2::HttpRequest,
) -> Result<oauth2::HttpResponse, reqwest::Error> {
  static CLIENT: std::sync::OnceLock<reqwest::Client> =
    std::sync::OnceLock::new();
  let client = CLIENT.get_or_init(|| {
    reqwest::Client::builder()
      .user_agent(USER_AGENT)
      .connect_timeout(std::time::Duration::from_secs(10))
      .timeout(std::time::Duration::from_secs(30))
      .redirect(reqwest::redirect::Policy::none())
      .build()
      .expect("failed to build oauth2 reqwest client")
  });
  #[cfg(target_arch = "wasm32")]
  let mut response = {
    let _ = client;
    shared::worker_js::fetch(request.try_into()?, false, true).await
  };
  #[cfg(not(target_arch = "wasm32"))]
  let mut response = client.execute(request.try_into()?).await?;
  let status = response.status();
  let headers = std::mem::take(response.headers_mut());
  let mut converted = oauth2::HttpResponse::new(response.bytes().await?.into());
  *converted.status_mut() = status;
  *converted.headers_mut() = headers;
  Ok(converted)
}

pub type ApiResult<D> = Result<D, ApiError>;
pub type ApiHandlerFuture<D> = HandlerFuture<D, ApiError>;

pub trait RouteHandler:
  Fn(Request<Body>) -> ApiHandlerFuture<Response<Body>>
  + Clone
  + Send
  + Sync
  + 'static
{
}

impl<T> RouteHandler for T where
  T: Fn(Request<Body>) -> ApiHandlerFuture<Response<Body>>
    + Clone
    + Send
    + Sync
    + 'static
{
}

/// Wrap a handler so its response is explicitly uncacheable
/// (`Cache-Control: no-store`).
///
/// `json` sets no `Cache-Control`, and the lb caches such GET responses for
/// anonymous callers. That must not happen for dynamic, not-behind-[`auth`]
/// endpoints — most importantly the publish-status poll: a cached
/// `pending`/`processing` status makes `deno publish` hang until the entry
/// expires even though the task already finished. Stamping `no-store` keeps the
/// lb (and any downstream cache) from ever storing the response.
pub fn no_store<H, HF>(handler: H) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  let handler = Arc::new(handler);
  move |req: Request<Body>| {
    let handler = handler.clone();
    async move {
      let mut res = handler(req).await?;
      res.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
      );
      Ok(res)
    }
    .boxed()
  }
}

pub fn auth<H, HF>(handler: H) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  let handler = Arc::new(handler);
  move |req: Request<Body>| {
    let handler = handler.clone();
    async move {
      if req.context::<IamInfo>().is_some() {
        let hf = handler(req);
        hf.await
      } else {
        Err(ApiError::MissingAuthentication)
      }
    }
    .map(|res| {
      if let Err(err) = &res {
        error!({ code = err.code() }, "{}", err.message());
      }
      res
    })
    .boxed()
  }
}

pub struct CacheDuration(pub usize);
impl CacheDuration {
  pub const ONE_MINUTE: CacheDuration = CacheDuration(60);
  pub const FIVE_MINUTES: CacheDuration = CacheDuration(60 * 5);
  #[allow(dead_code)]
  pub const TEN_MINUTES: CacheDuration = CacheDuration(60 * 10);
  pub const ONE_HOUR: CacheDuration = CacheDuration(60 * 60);
  pub const ONE_DAY: CacheDuration = CacheDuration(60 * 60 * 24);
  pub const THIRTY_DAYS: CacheDuration = CacheDuration(60 * 60 * 24 * 30);
  pub const FOREVER: CacheDuration = CacheDuration(60 * 60 * 24 * 365);
}

/// Brief negative-cache window (seconds) for `404 Not Found` responses on
/// cached routes. Misses (e.g. docs/diff for a symbol, entrypoint or version
/// that doesn't exist) carry no `Cache-Control` from the error path, so every
/// repeat hits the origin — these are the dominant uncacheable load on the
/// docs/diff handlers. Caching them for a short window collapses the repeats
/// while keeping a newly-published version visible quickly.
const NOT_FOUND_CACHE_SECS: usize = 60;

/// Response header marking a response as identity-independent: its body does not
/// depend on the requesting user, so the lb may serve it from its shared
/// (URL-keyed) cache even to authenticated callers, instead of bypassing the
/// cache whenever auth is present. Only set by the `*_shared` cache wrappers, on
/// routes whose handler provably ignores identity (e.g. docs/diff). The lb
/// strips it before returning to the client. A `Cache-Control` of `public` is
/// also required by the lb, so an accidentally-`private` response can never be
/// shared even if this header leaks onto it.
static X_JSR_CACHE_SHARED: HeaderName =
  HeaderName::from_static("x-jsr-cache-shared");

fn mark_shared(res: &mut Response<Body>, shared: bool) {
  if shared {
    res
      .headers_mut()
      .insert(&X_JSR_CACHE_SHARED, header::HeaderValue::from_static("1"));
  }
}

/// Render an `ApiError` to its JSON response with an explicit `Cache-Control`.
fn error_response(
  err: &ApiError,
  cache_control: header::HeaderValue,
  shared: bool,
) -> Response<Body> {
  let mut res = err.json_response();
  res
    .headers_mut()
    .insert(header::CACHE_CONTROL, cache_control);
  mark_shared(&mut res, shared);
  res
}

/// Short negative-cache `Cache-Control` for a cacheable error (see
/// [`is_cacheable_error_status`]) on a cached route. Anonymous
/// (and identity-independent `shared`) requests get a brief `public` window;
/// other authenticated requests are never cached (the lb skips its shared cache
/// when an `Authorization` header or `token=` cookie is present, so a `public`
/// 404 can't leak across viewers, but we still mark them `no-store` for defence
/// in depth).
fn short_negative_cache_control(public: bool) -> header::HeaderValue {
  header::HeaderValue::from_str(&if public {
    format!(
      "public, max-age=30, s-maxage={NOT_FOUND_CACHE_SECS}, stale-while-revalidate={NOT_FOUND_CACHE_SECS}"
    )
  } else {
    "private, no-store".to_string()
  })
  .unwrap()
}

/// Errors that are a deterministic property of an immutable (non-"latest")
/// version can never resolve differently later, so they are as cacheable as a
/// normal `200` for that version — unlike other 404s (package/version not
/// found, or anything on "latest"), which only get the brief negative-cache
/// window. Returns the long-lived value to use for such an error, or `None` to
/// fall back to short negative caching.
fn immutable_miss_cache_control(
  err: &ApiError,
  is_latest: bool,
  long_lived: impl FnOnce() -> header::HeaderValue,
) -> Option<header::HeaderValue> {
  let deterministic_for_version = matches!(
    err,
    // The symbol is absent from this version's docs.
    ApiError::EntrypointOrSymbolNotFound
      // This version has more symbols than the listing can carry; that is a
      // property of its published contents, so it will not change.
      | ApiError::DocsSymbolListingTooLarge
  );
  (deterministic_for_version && !is_latest).then(long_lived)
}

/// Statuses whose error responses carry `Cache-Control` on a cached route.
///
/// `404` has always been here. `413` joined it because the docs symbol-listing
/// refusal is deterministic per version: without a cache header every retry
/// reached the origin, which is how one crawler produced 871 identical failing
/// requests in an hour.
fn is_cacheable_error_status(status: StatusCode) -> bool {
  status == StatusCode::NOT_FOUND || status == StatusCode::PAYLOAD_TOO_LARGE
}

/// Cache an immutable-ish response for `duration`. See [`cache_shared`] for the
/// identity-independent variant.
pub fn cache<H, HF>(duration: CacheDuration, handler: H) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  cache_impl(duration, false, handler)
}

/// Like [`cache`], but for routes whose response does **not** depend on the
/// requesting identity (no permission/member/sudo branch — e.g. `diff`). Such
/// responses stay `public` even when authenticated and are marked
/// [`X_JSR_CACHE_SHARED`], so the lb serves them from its shared cache to every
/// caller instead of bypassing on auth. Only use this when the handler is
/// genuinely viewer-independent; auditing that is the safety contract.
pub fn cache_shared<H, HF>(
  duration: CacheDuration,
  handler: H,
) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  cache_impl(duration, true, handler)
}

fn cache_impl<H, HF>(
  duration: CacheDuration,
  shared: bool,
  handler: H,
) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  let value =
    header::HeaderValue::from_str(&if duration.0 >= CacheDuration::FOREVER.0 {
      format!("public, max-age=60, s-maxage={}, immutable", duration.0)
    } else {
      let swr = std::cmp::min(duration.0 * 3, CacheDuration::ONE_DAY.0);
      format!(
        "public, max-age=30, s-maxage={}, stale-while-revalidate={}",
        duration.0, swr
      )
    })
    .unwrap();
  let private_value =
    header::HeaderValue::from_str(&if duration.0 >= CacheDuration::FOREVER.0 {
      format!("private, max-age={}, immutable", duration.0)
    } else {
      format!("private, max-age={}", duration.0)
    })
    .unwrap();
  let handler = Arc::new(handler);
  move |req: Request<Body>| {
    let handler = handler.clone();
    let value = value.clone();
    let private_value = private_value.clone();
    async move {
      // A shared route is identity-independent, so it stays `public` for every
      // caller; otherwise authenticated callers get the `private` value.
      let public = shared || req.iam().is_anonymous();
      // No `:version` param (e.g. the diff route's old/new versions are both
      // pinned) ⇒ never "latest" ⇒ a missing entrypoint/symbol is immutable.
      let is_latest =
        req.param("version").map(|v| v == "latest").unwrap_or(false);
      let mut res = match handler(req).await {
        Ok(res) => res,
        Err(err) if is_cacheable_error_status(err.status_code()) => {
          let long_lived = || if public { value } else { private_value };
          let cc = immutable_miss_cache_control(&err, is_latest, long_lived)
            .unwrap_or_else(|| short_negative_cache_control(public));
          return Ok(error_response(&err, cc, shared));
        }
        Err(err) => return Err(err),
      };
      let value = if public { value } else { private_value };
      res
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert_with(|| value);
      mark_shared(&mut res, shared);
      Ok(res)
    }
    .boxed()
  }
}

/// Cache middleware that applies different durations based on whether the
/// `:version` path parameter is a specific version or "latest".
/// This prevents aggressive caching of "latest"-resolved content while
/// allowing long caching for immutable versioned content.
pub fn cache_versioned<H, HF>(
  latest_duration: CacheDuration,
  versioned_duration: CacheDuration,
  handler: H,
) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  cache_versioned_impl(latest_duration, versioned_duration, false, handler)
}

/// Identity-independent variant of [`cache_versioned`] (see [`cache_shared`]):
/// the response stays `public` for every caller and is marked
/// [`X_JSR_CACHE_SHARED`] so the lb shares it across authenticated callers. Used
/// by `docs`, whose handler has no permission/member/sudo branch.
pub fn cache_versioned_shared<H, HF>(
  latest_duration: CacheDuration,
  versioned_duration: CacheDuration,
  handler: H,
) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  cache_versioned_impl(latest_duration, versioned_duration, true, handler)
}

fn cache_versioned_impl<H, HF>(
  latest_duration: CacheDuration,
  versioned_duration: CacheDuration,
  shared: bool,
  handler: H,
) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  let latest_swr =
    std::cmp::min(latest_duration.0 * 3, CacheDuration::ONE_DAY.0);
  let latest_value = header::HeaderValue::from_str(&format!(
    "public, max-age=30, s-maxage={}, stale-while-revalidate={}",
    latest_duration.0, latest_swr
  ))
  .unwrap();
  let versioned_swr =
    std::cmp::min(versioned_duration.0 * 3, CacheDuration::ONE_DAY.0);
  let versioned_value = header::HeaderValue::from_str(&if versioned_duration.0
    >= CacheDuration::FOREVER.0
  {
    format!(
      "public, max-age=60, s-maxage={}, immutable",
      versioned_duration.0
    )
  } else {
    format!(
      "public, max-age=30, s-maxage={}, stale-while-revalidate={}",
      versioned_duration.0, versioned_swr
    )
  })
  .unwrap();
  let private_latest_value = header::HeaderValue::from_str(&format!(
    "private, max-age={}",
    latest_duration.0
  ))
  .unwrap();
  let private_versioned_value =
    header::HeaderValue::from_str(&if versioned_duration.0
      >= CacheDuration::FOREVER.0
    {
      format!("private, max-age={}, immutable", versioned_duration.0)
    } else {
      format!("private, max-age={}", versioned_duration.0)
    })
    .unwrap();
  let handler = Arc::new(handler);
  move |req: Request<Body>| {
    let handler = handler.clone();
    let latest_value = latest_value.clone();
    let versioned_value = versioned_value.clone();
    let private_latest_value = private_latest_value.clone();
    let private_versioned_value = private_versioned_value.clone();
    async move {
      // Shared routes are identity-independent ⇒ `public` for every caller.
      let public = shared || req.iam().is_anonymous();
      let is_latest =
        req.param("version").map(|v| v == "latest").unwrap_or(true);
      let mut res = match handler(req).await {
        Ok(res) => res,
        Err(err) if is_cacheable_error_status(err.status_code()) => {
          let long_lived = || {
            if public {
              versioned_value
            } else {
              private_versioned_value
            }
          };
          let cc = immutable_miss_cache_control(&err, is_latest, long_lived)
            .unwrap_or_else(|| short_negative_cache_control(public));
          return Ok(error_response(&err, cc, shared));
        }
        Err(err) => return Err(err),
      };
      let value = match (public, is_latest) {
        (true, true) => latest_value,
        (true, false) => versioned_value,
        (false, true) => private_latest_value,
        (false, false) => private_versioned_value,
      };
      res
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert(value);
      mark_shared(&mut res, shared);
      Ok(res)
    }
    .boxed()
  }
}

#[instrument(name = "auth", skip(req), err, fields(token.kind, user.id, repo.id))]
pub async fn auth_middleware(
  mut req: Request<Body>,
) -> ApiResult<Request<Body>> {
  let db = req.data::<Database>().unwrap();
  let token = extract_token_and_sudo(&req);

  let span = Span::current();

  let iam_info = match token {
    Some((AuthorizationToken::Bearer(token), sudo)) => {
      span.record("token.kind", field::display("bearer"));
      if let Some(token) =
        db.get_token_by_hash(&crate::token::hash(token)).await?
      {
        if let Some(expires_at) = token.expires_at
          && expires_at < chrono::Utc::now()
        {
          return Err(ApiError::InvalidBearerToken);
        }

        let user = db.get_user(token.user_id).await?.unwrap();
        span.record("user.id", field::display(user.id));

        if user.is_blocked {
          return Err(ApiError::Blocked);
        }

        IamInfo::from((token, user, sudo))
      } else {
        return Err(ApiError::InvalidBearerToken);
      }
    }
    Some((AuthorizationToken::Oidc { kind, token }, _)) => {
      span.record("token.kind", field::display(kind.span_kind()));
      match kind {
        OidcProviderKind::GitHub => {
          github::build_iam_info(db, token, &span).await?
        }
      }
    }
    None => IamInfo::anonymous(),
  };

  req.extensions_mut().insert(iam_info);

  Ok(req)
}

pub async fn auth_layer(req: Request<Body>, next: Next) -> Response<Body> {
  match auth_middleware(req).await {
    Ok(req) => next.run(req).await,
    Err(err) => err.into_response(),
  }
}

pub fn full_auth<H, HF>(handler: H) -> impl RouteHandler
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  let handler = Arc::new(auth(handler));

  move |req: Request<Body>| {
    let handler = handler.clone();

    async move {
      let req = auth_middleware(req).await?;
      let res = handler(req).await?;
      Ok(res)
    }
    .boxed()
  }
}

enum AuthorizationToken<'s> {
  Bearer(&'s str),
  Oidc {
    kind: OidcProviderKind,
    token: &'s str,
  },
}

static X_JSR_SUDO: HeaderName = header::HeaderName::from_static("x-jsr-sudo");

fn extract_token_and_sudo(
  req: &'_ Request<Body>,
) -> Option<(AuthorizationToken<'_>, bool)> {
  let headers = req.headers();

  let mut sudo = headers
    .get(&X_JSR_SUDO)
    .map(|v| v == "true")
    .unwrap_or(false);

  for cookie in headers.get_all(COOKIE) {
    let mut return_val = Option::<AuthorizationToken>::None;
    if let Ok(cookie) = cookie.to_str() {
      for cookie in cookie.split(';') {
        if let Some(token) = cookie.trim().strip_prefix("token=") {
          return_val = Some(AuthorizationToken::Bearer(token));
        }
        if cookie.trim() == "sudo=1" {
          sudo = true;
        }
      }
    }
    if let Some(token) = return_val {
      return Some((token, sudo));
    }
  }

  if let Some(auth) = headers.get(header::AUTHORIZATION)
    && let Ok(auth) = auth.to_str()
  {
    if let Some(token) = auth.strip_prefix("Bearer ") {
      return Some((AuthorizationToken::Bearer(token), sudo));
    }
    for kind in OidcProviderKind::all() {
      if let Some(token) = auth.strip_prefix(kind.auth_prefix()) {
        return Some((AuthorizationToken::Oidc { kind: *kind, token }, sudo));
      }
    }
  }

  None
}

pub fn search(req: &Request<Body>) -> Option<&str> {
  req.query("query").map(|q| q.as_str())
}

pub fn sort(req: &Request<Body>) -> Option<&str> {
  req.query("sortBy").map(|q| q.as_str())
}

pub fn pagination(req: &Request<Body>) -> (i64, i64) {
  let limit = req
    .query("limit")
    .and_then(|page| page.parse::<i64>().ok())
    .unwrap_or(100)
    .clamp(1, 100);
  let page = req
    .query("page")
    .and_then(|page| page.parse::<i64>().ok())
    .unwrap_or(1)
    .max(1);

  let start = (page * limit) - limit;

  (start, limit)
}

pub struct DocsQueries<'a> {
  pub all_symbols: bool,
  pub entrypoint: Option<&'a str>,
  pub symbol: Option<std::borrow::Cow<'a, str>>,
}

pub fn docs_queries(req: &Request<Body>) -> Result<DocsQueries<'_>, ApiError> {
  let all_symbols = req.query("all_symbols").is_some();
  let entrypoint = req.query("entrypoint").map(|s| match s.as_str() {
    "" => ".",
    s => s,
  });

  let symbol = req
    .query("symbol")
    .and_then(|s| match s.as_str() {
      "" => None,
      s => Some(urlencoding::decode(s)),
    })
    .transpose()?;

  if all_symbols && (entrypoint.is_some() || symbol.is_some()) {
    return Err(
      SharedError::MalformedRequest {
        msg: "Cannot specify both all_symbols and entrypoint".into(),
      }
      .into(),
    );
  }

  Ok(DocsQueries {
    all_symbols,
    entrypoint,
    symbol,
  })
}

// Sanitize redirect urls
// - Remove origin from Url: https://evil.com -> /
// - Replace multiple slashes with one slash to remove prevent
//   relative url bypass: //evil.com/foo -> /foo
// - Remove /../ and /./ from path segments
pub fn sanitize_redirect_url(raw: &str) -> String {
  let base = Url::parse("http://localhost/").unwrap();
  let url = base.join(raw).unwrap_or(base.clone());

  let mut sanitized = "".to_string();

  if let Some(segments) = url.path_segments() {
    for seg in segments {
      if seg.is_empty() {
        continue;
      }

      sanitized.push_str(&format!("/{}", seg));
    }
  }

  if let Some(query) = url.query() {
    sanitized.push_str(&format!("?{}", query));
  }

  if sanitized.is_empty() {
    "/".to_string()
  } else {
    sanitized
  }
}

pub trait RequestIdExt {
  fn param_uuid(&self, name: &str) -> Result<Uuid, ApiError>;
  fn param_scope(&self) -> Result<ScopeName, ApiError>;
  fn param_package(&self) -> Result<PackageName, ApiError>;
  fn param_version(&self) -> Result<Version, ApiError>;
  fn param_version_or_latest(&self) -> Result<VersionOrLatest, ApiError>;
}

pub fn param<'a>(
  req: &'a Request<Body>,
  name: &str,
) -> Result<&'a String, ApiError> {
  req.param(name).ok_or_else(|| {
    let msg = format!("missing path parameter '{name}'").into();
    ApiError::MalformedRequest { msg }
  })
}

#[derive(Eq, PartialEq)]
pub enum VersionOrLatest {
  Version(Version),
  Latest,
}

impl std::fmt::Display for VersionOrLatest {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VersionOrLatest::Version(version) => std::fmt::Display::fmt(version, f),
      VersionOrLatest::Latest => f.write_str("latest"),
    }
  }
}

impl RequestIdExt for Request<Body> {
  fn param_uuid(&self, name: &str) -> Result<Uuid, ApiError> {
    let value = param(self, name)?;
    Uuid::parse_str(value).map_err(|err| {
      let msg =
        format!("failed to parse path parameter '{name}' as uuid: {err}")
          .into();
      ApiError::MalformedRequest { msg }
    })
  }

  fn param_scope(&self) -> Result<ScopeName, ApiError> {
    let value = param(self, "scope")?;
    ScopeName::try_from(value.as_str()).map_err(|err| {
      let msg = format!("failed to parse path parameter 'scope': {err}").into();
      ApiError::MalformedRequest { msg }
    })
  }

  fn param_package(&self) -> Result<PackageName, ApiError> {
    let value = param(self, "package")?;
    PackageName::try_from(value.as_str()).map_err(|err| {
      let msg =
        format!("failed to parse path parameter 'package': {err}").into();
      ApiError::MalformedRequest { msg }
    })
  }

  fn param_version(&self) -> Result<Version, ApiError> {
    let value = param(self, "version")?;
    Version::try_from(value.as_str()).map_err(|err| {
      let msg =
        format!("failed to parse path parameter 'version': {err}").into();
      ApiError::MalformedRequest { msg }
    })
  }

  fn param_version_or_latest(&self) -> Result<VersionOrLatest, ApiError> {
    let value = param(self, "version")?;
    if value == "latest" {
      Ok(VersionOrLatest::Latest)
    } else {
      let version = Version::try_from(value.as_str()).map_err(|err| {
        let msg =
          format!("failed to parse path parameter 'version': {err}").into();
        ApiError::MalformedRequest { msg }
      })?;
      Ok(VersionOrLatest::Version(version))
    }
  }
}

#[cfg(test)]
pub mod test {
  use crate::MainRouterOptions;
  use bytes::Bytes;
  use shared::db::CreatePackageResult;
  use shared::db::CreatePublishingTaskResult;
  use shared::db::Database;
  use shared::errors_internal::ErrorStruct;
  use shared::ids::PackageName;
  use shared::ids::PackagePath;
  use shared::ids::Version;
  use shared::models::NewPublishingTask;
  use shared::models::PublishingTask;
  use shared::models::User;
  use shared::s3::S3UploadOptions;
  use shared::s3::UploadTaskBody;
  use shared::s3_paths::bucket_tarball_path;

  /// The password the inbound email webhook accepts under test.
  pub const TEST_POSTMARK_WEBHOOK_PASSWORD: &str = "test-webhook-password";

  /// The upstream whose `Authentication-Results` the inbound webhook trusts
  /// under test.
  pub const TEST_TRUSTED_AUTHSERV_ID: &str = "mx.test.example";

  use crate::util::sanitize_redirect_url;
  use axum::body::Body;
  use hyper::HeaderMap;
  use hyper::Response;
  use hyper::StatusCode;
  use hyper::http::HeaderName;
  use hyper::http::HeaderValue;
  use serde::de::DeserializeOwned;
  use shared::router::App;
  use std::convert::Infallible;
  use tower::ServiceExt;
  use url::Url;

  #[derive(Debug)]
  pub struct TestUser {
    pub user: User,
    pub token: String,
    pub github_name: String,
  }

  pub use shared::test::FakeFallbackRegistry;

  /// The tasks service for one [`TestSetup`]: the workspace's own
  /// `registry_api_tasks` binary, run against the harness's database and
  /// buckets, so the api's task dispatch goes over HTTP exactly as it does in
  /// production.
  pub struct TasksProcess {
    child: std::process::Child,
    pub url: Url,
  }

  static TASKS_BINARY: std::sync::OnceLock<std::path::PathBuf> =
    std::sync::OnceLock::new();

  impl TasksProcess {
    /// The `registry_api_tasks` binary, built if it is not there yet.
    /// `cargo test --workspace` (what CI runs) builds it as a matter of
    /// course; `cargo test -p registry_api` on its own does not.
    fn binary() -> &'static std::path::Path {
      TASKS_BINARY.get_or_init(|| {
        let workspace_root =
          std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let target_dir = std::env::var_os("CARGO_TARGET_DIR")
          .map(std::path::PathBuf::from)
          .unwrap_or_else(|| workspace_root.join("target"));
        let profile = if cfg!(debug_assertions) {
          "debug"
        } else {
          "release"
        };
        let binary = target_dir.join(profile).join(format!(
          "registry_api_tasks{}",
          std::env::consts::EXE_SUFFIX
        ));
        if !binary.exists() {
          println!("building registry_api_tasks for the test harness");
          let mut cmd = std::process::Command::new(env!("CARGO"));
          cmd.args(["build", "-p", "registry_api_tasks"]);
          if profile == "release" {
            cmd.arg("--release");
          }
          let status = cmd.current_dir(&workspace_root).status().unwrap();
          assert!(status.success(), "failed to build registry_api_tasks");
        }
        binary
      })
    }

    fn start(inner: &shared::test::TestSetup, registry_url: &Url) -> Self {
      use std::io::BufRead;

      let mut cmd = std::process::Command::new(Self::binary());
      // Nothing from the test process's environment (which includes whatever
      // `.env` holds) should leak in: every setting is passed explicitly.
      cmd
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        // An empty directory, so no `.env` file is picked up either.
        .current_dir(std::env::temp_dir())
        .args(["--port=0", "--database_pool_size=1"])
        .env("DATABASE_URL", inner.database_url())
        .env("S3_REGION", "us-east-1")
        .env("S3_ENDPOINT", &inner.s3_endpoint)
        .env("S3_ACCESS_KEY", "minioadmin")
        .env("S3_SECRET_KEY", "minioadmin")
        .env(
          "PUBLISHING_BUCKET",
          inner.buckets.publishing_bucket.bucket.name(),
        )
        .env("MODULES_BUCKET", inner.buckets.modules_bucket.bucket.name())
        .env("DOCS_BUCKET", inner.buckets.docs_bucket.bucket.name())
        .env("NPM_BUCKET", inner.buckets.npm_bucket.bucket.name())
        .env(
          "TICKET_ATTACHMENTS_BUCKET",
          inner.buckets.ticket_attachments_bucket.bucket.name(),
        )
        .env("REGISTRY_URL", registry_url.as_str())
        .env("NPM_URL", inner.npm_url().as_str())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
      if let Some(url) = &inner.fallback_registry_url {
        cmd.env("FALLBACK_REGISTRY_URL", url.as_str());
      }
      let mut child = cmd.spawn().expect("failed to start registry_api_tasks");

      // Wait for the bound port, which the binary prints once it is serving.
      let stdout = child.stdout.take().unwrap();
      let mut stdout = std::io::BufReader::new(stdout);
      let port = loop {
        let mut line = String::new();
        if stdout.read_line(&mut line).unwrap() == 0 {
          let _ = child.kill();
          let mut stderr = String::new();
          if let Some(mut err) = child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut err, &mut stderr);
          }
          panic!("registry_api_tasks exited before serving:\n{stderr}");
        }
        if let Some(addr) =
          line.trim().strip_prefix("Tasks server is running on: ")
        {
          break addr.rsplit(':').next().unwrap().parse::<u16>().unwrap();
        }
      };
      // Keep draining its output so it never blocks on a full pipe.
      std::thread::spawn(move || {
        std::io::copy(&mut stdout.into_inner(), &mut std::io::sink()).ok();
      });
      let mut stderr = child.stderr.take().unwrap();
      std::thread::spawn(move || {
        std::io::copy(&mut stderr, &mut std::io::sink()).ok();
      });

      Self {
        child,
        url: Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
      }
    }
  }

  impl Drop for TasksProcess {
    fn drop(&mut self) {
      let _ = self.child.kill();
      let _ = self.child.wait();
    }
  }

  /// The shared harness (database, buckets, users, scope) plus the api router,
  /// a tasks service and a web token for each user. Derefs to the inner harness, so
  /// `t.db()`, `t.scope` and friends work as before, and `&t` can be handed to
  /// the publish test helpers.
  pub struct TestSetup {
    // Declared first so it is shut down before the database it uses is
    // dropped.
    _tasks_process: TasksProcess,
    inner: shared::test::TestSetup,
    pub user1: TestUser,
    pub user2: TestUser,
    pub user3: TestUser,
    pub staff_user: TestUser,
    #[allow(dead_code)]
    pub github_oauth2_client: crate::auth::github::Oauth2Client,
    #[allow(dead_code)]
    pub gitlab_oauth2_client: crate::auth::gitlab::Oauth2Client,
    pub service: App,
    pub task_dispatch: crate::task_dispatch::TaskDispatch,
  }

  impl std::ops::Deref for TestSetup {
    type Target = shared::test::TestSetup;

    fn deref(&self) -> &Self::Target {
      &self.inner
    }
  }

  impl TestSetup {
    pub async fn new() -> Self {
      Self::with_fallback_registry(None).await
    }

    /// Like [`TestSetup::new`], but with a fallback registry configured. The URL
    /// has to be passed here rather than assigned to `fallback_registry_url`
    /// afterwards: the router is built once, and handlers read the URL out of
    /// its request data.
    pub async fn with_fallback_registry(
      fallback_registry_url: Option<Url>,
    ) -> Self {
      let inner = shared::test::TestSetup::with_fallback_registry(
        fallback_registry_url.clone(),
      )
      .await;
      let db = inner.db();

      let registry_url = inner.registry_url();
      let github_oauth2_client = crate::auth::github::Oauth2Client::new(
        &registry_url,
        "".to_string(),
        "".to_string(),
      );
      let gitlab_oauth2_client = crate::auth::gitlab::Oauth2Client::new(
        &registry_url,
        "".to_string(),
        "".to_string(),
      );

      let user1 = Self::with_token(&db, &inner.user1).await;
      let user2 = Self::with_token(&db, &inner.user2).await;
      let user3 = Self::with_token(&db, &inner.user3).await;
      let staff_user = Self::with_token(&db, &inner.staff_user).await;

      let tasks_process = TasksProcess::start(&inner, &registry_url);
      let task_dispatch =
        crate::task_dispatch::TaskDispatch::new(tasks_process.url.clone());

      let service = crate::main_router(MainRouterOptions {
        database: db,
        buckets: inner.buckets(),
        generate_ctx_cache: crate::docs::GenerateCtxCache::new(),
        object_cache: crate::object_cache::ObjectCache::new(),
        registry_metadata_cache: crate::api::RegistryMetadataCache::new(),
        github_client: github_oauth2_client.clone(),
        gitlab_client: gitlab_oauth2_client.clone(),
        algolia_client: None,
        email_sender: None,
        license_store: inner.license_store(),
        registry_url,
        npm_url: inner.npm_url(),
        fallback_registry_url,
        publish_queue: None, // no queue locally
        // No Cloud Tasks locally, so queued email is delivered inline.
        email_queue: None,
        cache_purge_client: None, // no Cloudflare purge locally
        // No secret key, so the login captcha is not verified in tests.
        turnstile: shared::external::cloudflare::Turnstile(None),
        postmark_webhook_password: crate::api::PostmarkWebhookPassword(Some(
          TEST_POSTMARK_WEBHOOK_PASSWORD.to_owned(),
        )),
        inbound_trusted_authserv_id: crate::api::InboundTrustedAuthservId(
          Some(TEST_TRUSTED_AUTHSERV_ID.to_owned()),
        ),
        task_dispatch: task_dispatch.clone(),
      });

      Self {
        _tasks_process: tasks_process,
        inner,
        user1,
        user2,
        user3,
        staff_user,
        github_oauth2_client,
        gitlab_oauth2_client,
        service,
        task_dispatch,
      }
    }

    async fn with_token(
      db: &Database,
      user: &shared::test::TestUser,
    ) -> TestUser {
      let token = crate::token::create_token(
        db,
        user.user.id,
        shared::models::TokenType::Web,
        None,
        Some(chrono::Utc::now() + chrono::Duration::try_days(7).unwrap()),
        None,
      )
      .await
      .unwrap();

      TestUser {
        user: user.user.clone(),
        token,
        github_name: user.github_name.clone(),
      }
    }

    pub fn http(&'_ mut self) -> TestHttpClient<'_, '_> {
      TestHttpClient {
        service: &mut self.service,
        auth: Some(&self.user1.token),
      }
    }

    pub fn unauthed_http(&'_ mut self) -> TestHttpClient<'_, '_> {
      TestHttpClient {
        service: &mut self.service,
        auth: None,
      }
    }
  }

  /// Publishes `tarball_data` as `@scope/foo@1.2.3` and returns the finished
  /// publishing task.
  pub async fn process_tarball_setup(
    t: &TestSetup,
    tarball_data: Bytes,
  ) -> PublishingTask {
    let package_name = PackageName::try_from("foo").unwrap();
    let version = Version::try_from("1.2.3").unwrap();
    process_tarball_setup2(t, tarball_data, &package_name, &version, false)
      .await
  }

  /// Publishes `tarball_data` as the given version the way the upload
  /// endpoint does: a publishing task row, the tarball in the publishing
  /// bucket, then the publish run through the api's task dispatch.
  pub async fn process_tarball_setup2(
    t: &TestSetup,
    tarball_data: Bytes,
    package_name: &PackageName,
    version: &Version,
    jsonc: bool,
  ) -> PublishingTask {
    let scope_name = "scope".try_into().unwrap();

    let res = t
      .db()
      .create_package(&scope_name, package_name)
      .await
      .unwrap();
    assert!(
      matches!(res, CreatePackageResult::Ok(_))
        || matches!(res, CreatePackageResult::AlreadyExists)
    );

    let CreatePublishingTaskResult::Created(task) = t
      .db()
      .create_publishing_task(NewPublishingTask {
        user_id: Some(t.user1.user.id),
        package_scope: &scope_name,
        package_name,
        package_version: version,
        config_file: &PackagePath::try_from(format!(
          "/jsr.json{}",
          if jsonc { "c" } else { "" }
        ))
        .unwrap(),
      })
      .await
      .unwrap()
    else {
      unreachable!()
    };

    let tarball_path = bucket_tarball_path(task.0.id);
    t.buckets
      .publishing_bucket
      .upload(
        tarball_path.into(),
        UploadTaskBody::Bytes(tarball_data),
        S3UploadOptions {
          content_type: Some("application/x-tar".into()),
          cache_control: None,
          gzip_encoded: true,
        },
      )
      .await
      .unwrap();

    t.task_dispatch.publish(task.0.id).await.unwrap();

    t.db()
      .get_publishing_task(task.0.id)
      .await
      .unwrap()
      .unwrap()
      .0
  }

  pub struct TestHttpClient<'s, 't> {
    service: &'s mut App,
    auth: Option<&'t str>,
  }

  impl<'s> TestHttpClient<'s, '_> {
    pub fn get<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "GET",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    pub fn post<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "POST",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    pub fn delete<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "DELETE",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    pub fn put<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "PUT",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    pub fn patch<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "PATCH",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    pub fn options<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "OPTIONS",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
  }

  pub struct TestHttpCall<'s> {
    service: &'s mut App,
    method: &'static str,
    uri: String,
    body: Body,
    headers: HeaderMap,
    token: Option<&'s str>,
    sudo: bool,
  }

  impl<'s> TestHttpCall<'s> {
    fn new(
      service: &'s mut App,
      method: &'static str,
      uri: String,
      token: Option<&'s str>,
    ) -> Self {
      Self {
        service,
        method,
        uri,
        body: Body::empty(),
        headers: HeaderMap::default(),
        token,
        sudo: false,
      }
    }

    /// overwrite the default user token for authentication
    pub fn token(mut self, token: Option<&'s str>) -> Self {
      self.token = token;
      self
    }

    /// overwrite the default user token for authentication
    pub fn sudo(mut self, sudo: bool) -> Self {
      self.sudo = sudo;
      self
    }

    pub fn body_json(mut self, body: serde_json::Value) -> Self {
      self.body = body.to_string().into();
      self
    }

    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
      let prev = self.headers.insert(name.clone(), value);
      assert!(prev.is_none(), "{} already present in the header map", name);
      self
    }

    pub fn gzip(self) -> Self {
      self.header(hyper::header::CONTENT_ENCODING, "gzip".try_into().unwrap())
    }

    pub fn body(mut self, body: Body) -> Self {
      self.body = body;
      self
    }

    pub async fn call(self) -> Result<hyper::Response<Body>, Infallible> {
      let mut req = hyper::Request::builder().method(self.method).uri(self.uri);

      for (key, value) in self.headers.into_iter() {
        req = req.header(key.unwrap(), value)
      }
      if let Some(token) = self.token {
        req =
          req.header(hyper::header::AUTHORIZATION, format!("Bearer {}", token));
      }

      if self.sudo {
        req = req
          .header(hyper::header::HeaderName::from_static("x-jsr-sudo"), "true");
      }

      let req = req.body(self.body).unwrap();
      self.service.clone().oneshot(req).await
    }
  }

  #[async_trait::async_trait]
  pub trait ApiResultExt {
    #[track_caller]
    async fn expect_json<T: DeserializeOwned>(
      &mut self,
      status: StatusCode,
    ) -> T;

    #[track_caller]
    async fn expect_ok<T: DeserializeOwned>(&mut self) -> T {
      self.expect_json(StatusCode::OK).await
    }

    #[track_caller]
    async fn expect_ok_no_content(&mut self);

    #[track_caller]
    async fn expect_err(&mut self, status: StatusCode) -> ErrorStruct {
      self.expect_json(status).await
    }

    #[track_caller]
    async fn expect_err_code(
      &mut self,
      status: StatusCode,
      code: &str,
    ) -> ErrorStruct {
      let err: ErrorStruct = self.expect_json(status).await;
      assert_eq!(err.code, code, "{}", err.message);
      err
    }
  }

  #[async_trait::async_trait]
  impl ApiResultExt for Response<Body> {
    #[track_caller]
    async fn expect_json<T: DeserializeOwned>(
      &mut self,
      status: StatusCode,
    ) -> T {
      let bytes =
        axum::body::to_bytes(std::mem::take(self.body_mut()), usize::MAX)
          .await
          .unwrap();
      let body = std::str::from_utf8(&bytes).expect("invalid utf8");
      assert_eq!(
        self.status(),
        status,
        "expected {} response, got {} with body: {}",
        status,
        self.status(),
        body
      );
      serde_json::from_str(body).unwrap()
    }

    #[track_caller]
    async fn expect_ok_no_content(&mut self) {
      let bytes =
        axum::body::to_bytes(std::mem::take(self.body_mut()), usize::MAX)
          .await
          .unwrap();
      let body = std::str::from_utf8(&bytes).expect("invalid utf8");
      assert_eq!(
        self.status(),
        StatusCode::NO_CONTENT,
        "expected {} response, got {} with body: {}",
        StatusCode::NO_CONTENT,
        self.status(),
        body
      );
    }
  }

  #[tokio::test]
  async fn harness_expectations() {
    let t = TestSetup::new().await;
    // This permissions set up is important - the scope member management tests
    // rely on it.
    assert!(!t.user1.user.is_staff);
    assert!(!t.user2.user.is_staff);
    assert!(!t.user3.user.is_staff);
    assert!(t.staff_user.user.is_staff);
  }

  #[test]
  fn sanitize_url_test() {
    assert_eq!(sanitize_redirect_url("/foo"), "/foo");
    assert_eq!(sanitize_redirect_url("//evil.com/bar"), "/bar");
    assert_eq!(
      sanitize_redirect_url("//evil.com//bar?foo=bar"),
      "/bar?foo=bar"
    );
    assert_eq!(sanitize_redirect_url("https://evil.com"), "/");
    assert_eq!(sanitize_redirect_url("/../foo"), "/foo");
    assert_eq!(sanitize_redirect_url("/../foo/../bar"), "/bar");
    assert_eq!(sanitize_redirect_url("/foo/./bar"), "/foo/bar");
  }
}
