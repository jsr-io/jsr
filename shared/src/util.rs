// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use crate::errors::SharedError;
use futures::FutureExt;
use hyper::{Body, Request, Response, StatusCode, body, header};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::pin::Pin;
use std::sync::Arc;

pub type HandlerFuture<D, E> =
  Pin<Box<dyn Future<Output = Result<D, E>> + Send>>;

/// Wrap an endpoint handler converting it's success return value into a JSON response
///
/// Uses [`respond_json`] under the hood, with a 200 status code.
pub fn json<D, H, HF, E>(
  handler: H,
) -> impl Fn(Request<Body>) -> HandlerFuture<Response<Body>, E>
where
  D: Serialize,
  H: Fn(Request<Body>) -> HF,
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

pub async fn decode_json<T>(req: &mut Request<Body>) -> Result<T, SharedError>
where
  T: DeserializeOwned,
{
  let bytes = body::to_bytes(req.body_mut())
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

#[derive(Clone)]
pub struct LicenseStore(pub Arc<askalono::Store>);

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
  let store = askalono::Store::from_cache(CACHE).unwrap();
  LicenseStore(Arc::new(store))
}
