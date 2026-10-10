// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Workers can't open TCP sockets to Cloudflare's IP ranges (R2, the
//! Cloudflare API, Turnstile), so on the worker outbound HTTP goes through
//! `fetch` and R2 through bucket bindings.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use bytes::Bytes;
use js_sys::Reflect;
use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

thread_local! {
  static ENV: RefCell<Option<JsValue>> = const { RefCell::new(None) };
}

pub fn set_env(env: JsValue) {
  ENV.with(|cell| *cell.borrow_mut() = Some(env));
}

/// SAFETY: the worker is single-threaded (linked without `-pthread`), so
/// nothing wrapped here can reach another thread.
#[derive(Clone)]
struct AssertSend<T>(T);
unsafe impl<T> Send for AssertSend<T> {}
unsafe impl<T> Sync for AssertSend<T> {}

impl<F: Future> Future for AssertSend<F> {
  type Output = F::Output;
  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
    unsafe { self.map_unchecked_mut(|this| &mut this.0) }.poll(cx)
  }
}

fn js_error(err: JsValue) -> String {
  err
    .dyn_ref::<js_sys::Error>()
    .map(|err| String::from(err.message()))
    .unwrap_or_else(|| format!("{err:?}"))
}

async fn resolve(
  promise: Result<js_sys::Promise, JsValue>,
) -> Result<JsValue, String> {
  JsFuture::from(promise.map_err(js_error)?)
    .await
    .map_err(js_error)
}

fn object(entries: &[(&str, JsValue)]) -> Result<JsValue, String> {
  let object = js_sys::Object::new();
  for (key, value) in entries {
    Reflect::set(&object, &(*key).into(), value).map_err(js_error)?;
  }
  Ok(object.into())
}

/// Sends `request` with the runtime's `fetch`. A failure to get any response is
/// returned as a `502 Bad Gateway`, so callers treat it like an upstream error.
pub async fn fetch(
  request: reqwest::Request,
  follow_redirects: bool,
) -> reqwest::Response {
  match AssertSend(fetch_inner(request, follow_redirects)).await {
    Ok(response) => response,
    Err(err) => {
      tracing::error!("fetch failed: {err}");
      hyper::http::Response::builder()
        .status(hyper::StatusCode::BAD_GATEWAY)
        .body(err)
        .unwrap()
        .into()
    }
  }
}

async fn fetch_inner(
  request: reqwest::Request,
  follow_redirects: bool,
) -> Result<reqwest::Response, String> {
  let headers = web_sys::Headers::new().map_err(js_error)?;
  for (name, value) in request.headers() {
    if let Ok(value) = value.to_str() {
      headers.append(name.as_str(), value).map_err(js_error)?;
    }
  }
  if !request.headers().contains_key(hyper::header::USER_AGENT) {
    headers
      .append("user-agent", crate::util::USER_AGENT)
      .map_err(js_error)?;
  }
  let init = web_sys::RequestInit::new();
  init.set_method(request.method().as_str());
  init.set_headers(&headers);
  if !follow_redirects {
    init.set_redirect(web_sys::RequestRedirect::Manual);
  }
  if let Some(body) = request.body().and_then(|body| body.as_bytes()) {
    init.set_body(&Uint8Array::from(body));
  }
  let global: web_sys::WorkerGlobalScope = js_sys::global().unchecked_into();
  let response: web_sys::Response = resolve(Ok(
    global.fetch_with_str_and_init(request.url().as_str(), &init),
  ))
  .await?
  .unchecked_into();
  let body = resolve(response.array_buffer()).await?;

  let mut builder = hyper::http::Response::builder().status(response.status());
  let entries = js_sys::try_iter(&response.headers())
    .map_err(js_error)?
    .into_iter()
    .flatten();
  for entry in entries {
    let pair = js_sys::Array::from(&entry.map_err(js_error)?);
    if let (Some(name), Some(value)) =
      (pair.get(0).as_string(), pair.get(1).as_string())
    {
      // `fetch` has already decoded the body.
      if name != "content-encoding" && name != "content-length" {
        builder = builder.header(name, value);
      }
    }
  }
  let body = Bytes::from(Uint8Array::new(&body).to_vec());
  Ok(builder.body(body).map_err(|err| err.to_string())?.into())
}

/// The R2 binding for a bucket, named `R2_<bucket name with - as _>`.
#[derive(Clone)]
pub struct R2Bucket(AssertSend<worker_sys::R2Bucket>);

impl R2Bucket {
  pub fn new(bucket_name: &str) -> Option<Self> {
    let binding = format!("R2_{}", bucket_name.replace('-', "_"));
    ENV.with(|env| {
      let value = Reflect::get(env.borrow().as_ref()?, &binding.into()).ok()?;
      (!value.is_undefined()).then(|| Self(AssertSend(value.unchecked_into())))
    })
  }

  pub async fn get(
    &self,
    key: &str,
    offset: Option<usize>,
  ) -> Result<Option<Bytes>, String> {
    AssertSend(async {
      let options = match offset {
        Some(offset) => {
          object(&[("range", object(&[("offset", (offset as f64).into())])?)])?
        }
        None => JsValue::UNDEFINED,
      };
      let body = resolve(self.0.0.get(key.into(), options)).await?;
      if body.is_null() {
        return Ok(None);
      }
      let body: worker_sys::R2ObjectBody = body.unchecked_into();
      let bytes = resolve(body.array_buffer()).await?;
      Ok(Some(Bytes::from(Uint8Array::new(&bytes).to_vec())))
    })
    .await
  }

  pub async fn put(
    &self,
    key: &str,
    data: &[u8],
    content_type: Option<&str>,
    cache_control: Option<&str>,
    content_encoding: &str,
  ) -> Result<(), String> {
    AssertSend(async {
      let mut metadata = vec![("contentEncoding", content_encoding.into())];
      if let Some(content_type) = content_type {
        metadata.push(("contentType", content_type.into()));
      }
      if let Some(cache_control) = cache_control {
        metadata.push(("cacheControl", cache_control.into()));
      }
      let options = object(&[("httpMetadata", object(&metadata)?)])?;
      let value = Uint8Array::from(data).into();
      resolve(self.0.0.put(key.into(), value, options)).await?;
      Ok(())
    })
    .await
  }

  pub async fn delete(&self, key: &str) -> Result<(), String> {
    AssertSend(async {
      resolve(self.0.0.delete(key.into())).await?;
      Ok(())
    })
    .await
  }

  pub async fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
    AssertSend(async {
      let mut keys = Vec::new();
      let mut cursor: Option<String> = None;
      loop {
        let mut options = vec![("prefix", prefix.into())];
        if let Some(cursor) = &cursor {
          options.push(("cursor", cursor.as_str().into()));
        }
        let page: worker_sys::R2Objects =
          resolve(self.0.0.list(object(&options)?))
            .await?
            .unchecked_into();
        for object in page.objects().map_err(js_error)? {
          keys.push(object.key().map_err(js_error)?);
        }
        if !page.truncated().map_err(js_error)? {
          return Ok(keys);
        }
        cursor = page.cursor().map_err(js_error)?;
      }
    })
    .await
  }
}
