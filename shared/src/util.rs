// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use crate::errors::SharedError;
use axum::body::Body;
use futures::FutureExt;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::header;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::future::Future;
use std::pin::Pin;
use std::sync::LazyLock;

pub type HandlerFuture<D, E> =
  Pin<Box<dyn Future<Output = Result<D, E>> + Send>>;

/// Wrap an endpoint handler converting it's success return value into a JSON response
///
/// Uses [`respond_json`] under the hood, with a 200 status code.
pub fn json<D, H, HF, E>(
  handler: H,
) -> impl Fn(Request<Body>) -> HandlerFuture<Response<Body>, E>
+ Clone
+ Send
+ Sync
+ 'static
where
  D: Serialize,
  H: Fn(Request<Body>) -> HF + Clone + Send + Sync + 'static,
  HF: Future<Output = Result<D, E>> + Send + 'static,
{
  move |req: Request<Body>| {
    let fut = handler(req);
    async move {
      let data = fut.await?;
      Ok::<Response<Body>, E>(respond_json(&data, StatusCode::OK))
    }
    .boxed()
  }
}

pub fn respond_json<V: ?Sized + Serialize>(
  value: &V,
  status_code: StatusCode,
) -> Response<Body> {
  let body = serde_json::to_string(value).unwrap();
  create_response(status_code, "application/json", body)
}

pub fn create_response<B>(
  status: StatusCode,
  mime: &str, // TODO mime::Mime,
  body: B,
) -> Response<Body>
where
  B: Into<Body>,
{
  Response::builder()
    .status(status)
    .header(header::CONTENT_TYPE, mime)
    .body(body.into())
    .expect("expected to be able to create response")
}

/// Wrap a handler so its response is explicitly uncacheable
pub async fn decode_json<T>(req: &mut Request<Body>) -> Result<T, SharedError>
where
  T: DeserializeOwned,
{
  let bytes = axum::body::to_bytes(std::mem::take(req.body_mut()), usize::MAX)
    .await
    .map_err(anyhow::Error::from)?;
  let data = serde_json::from_slice(&bytes).map_err(|error| {
    SharedError::MalformedRequest {
      msg: error.to_string().into(),
    }
  })?;
  Ok(data)
}

pub const USER_AGENT: &str = "JSR";

/// Sends requests through the runtime's `fetch` on the worker, which can't open
/// sockets to Cloudflare's IP ranges.
pub trait Fetch {
  fn fetch(
    self,
  ) -> impl Future<Output = reqwest::Result<reqwest::Response>> + Send;
}

impl Fetch for reqwest::RequestBuilder {
  async fn fetch(self) -> reqwest::Result<reqwest::Response> {
    #[cfg(target_arch = "wasm32")]
    return Ok(crate::worker_js::fetch(self.build()?, true, true).await);
    #[cfg(not(target_arch = "wasm32"))]
    self.send().await
  }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn spawn_blocking<T: Send + 'static>(
  f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, tokio::task::JoinError> {
  tokio::task::spawn_blocking(f).await
}

#[cfg(target_arch = "wasm32")]
pub async fn spawn_blocking<Fut: Future>(
  f: impl FnOnce() -> Fut,
) -> Result<Fut::Output, tokio::task::JoinError> {
  Ok(crate::worker_js::AssertSend(f()).await)
}

pub fn spawn_background<F>(fut: F)
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  #[cfg(target_arch = "wasm32")]
  crate::worker_js::spawn_background(async move {
    fut.await;
  });
  #[cfg(not(target_arch = "wasm32"))]
  tokio::spawn(fut);
}

/// A shared `reqwest::Client` for all outbound HTTP requests. Reusing a single
/// client avoids per-request connection pool and TLS session allocation.
pub fn shared_http_client() -> &'static reqwest::Client {
  static CLIENT: std::sync::OnceLock<reqwest::Client> =
    std::sync::OnceLock::new();
  CLIENT.get_or_init(|| {
    reqwest::Client::builder()
      .user_agent(USER_AGENT)
      .connect_timeout(std::time::Duration::from_secs(10))
      .build()
      .expect("failed to build shared reqwest client")
  })
}
/// The SPDX license list. Decoding it takes ~100ms of CPU, and only
/// publishing needs it, so it is decoded once per process, on first use: the
/// worker, which builds the app for every request, never pays for it.
#[derive(Clone)]
pub struct LicenseStore(pub &'static LazyLock<askalono::Store>);

impl LicenseStore {
  /// Check if a license identifier is recognized, either as a primary key
  /// or as an alias of another license (e.g. GPL-3.0-or-later is an alias
  /// of GPL-3.0-only because they share the same license text).
  pub fn is_recognized(&self, name: &str) -> bool {
    if self.0.get_original(name).is_some() {
      return true;
    }
    self.0.licenses().any(|key| {
      self
        .0
        .aliases(key)
        .is_ok_and(|aliases| aliases.iter().any(|a| a == name))
    })
  }
}

pub fn license_store() -> LicenseStore {
  static CACHE: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/license-store.cache"));
  static STORE: LazyLock<askalono::Store> =
    LazyLock::new(|| askalono::Store::from_cache(CACHE).unwrap());
  LicenseStore(&STORE)
}
