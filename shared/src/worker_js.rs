// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Workers can't open TCP sockets to Cloudflare's IP ranges (R2, the
//! Cloudflare API, Turnstile), so on the worker outbound HTTP goes through
//! `fetch` and R2 through bucket bindings. Those calls are made with the
//! current span active, so the runtime's spans for them nest under it.

use std::cell::Cell;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use bytes::Bytes;
use js_sys::Function;
use js_sys::Reflect;
use js_sys::Uint8Array;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::worker_spans::in_current;

thread_local! {
  // A `Cell`, not a `RefCell`: a JS exception or emscripten abort that
  // unwinds through wasm frames can skip Rust destructors (imports are
  // nounwind), so a `RefCell` borrow could stay set and make every later
  // `set_env` panic.
  static ENV: Cell<Option<JsValue>> = const { Cell::new(None) };
}

/// The env of the request that most recently started on this isolate.
fn env() -> Option<JsValue> {
  ENV.with(|cell| {
    let env = cell.take();
    cell.set(env.clone());
    env
  })
}

tokio::task_local! {
  static BACKGROUND: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>;
}

pub fn spawn_background(fut: impl Future<Output = ()> + Send + 'static) {
  match BACKGROUND.try_with(Arc::clone) {
    Ok(tasks) => {
      let handle = tokio::spawn(BACKGROUND.scope(tasks.clone(), fut));
      tasks.lock().unwrap().push(handle);
    }
    Err(_) => {
      tokio::spawn(fut);
    }
  }
}

pub async fn with_background<T>(fut: impl Future<Output = T>) -> T {
  let tasks = Arc::<Mutex<Vec<_>>>::default();
  BACKGROUND
    .scope(tasks.clone(), async {
      let output = fut.await;
      loop {
        let handles = std::mem::take(&mut *tasks.lock().unwrap());
        if handles.is_empty() {
          break output;
        }
        for handle in handles {
          let _ = handle.await;
        }
      }
    })
    .await
}

pub fn set_env(env: JsValue) {
  ENV.with(|cell| cell.set(Some(env)));
}

/// The Hyperdrive binding's connection string. Only its credentials and
/// database name are used: connections go through the binding's `connect()`
/// (see [`WorkerConnect`]), never to the host in it.
pub fn hyperdrive_database_url() -> Option<String> {
  let binding = Reflect::get(&env()?, &"HYPERDRIVE".into()).ok()?;
  Reflect::get(&binding, &"connectionString".into())
    .ok()?
    .as_string()
}

/// SAFETY: the worker is single-threaded (linked without `-pthread`), so
/// nothing wrapped here can reach another thread.
#[derive(Clone)]
pub struct AssertSend<T>(pub T);
unsafe impl<T> Send for AssertSend<T> {}
unsafe impl<T> Sync for AssertSend<T> {}

impl<F: Future> Future for AssertSend<F> {
  type Output = F::Output;
  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
    unsafe { self.map_unchecked_mut(|this| &mut this.0) }.poll(cx)
  }
}

pub(crate) fn js_error(err: JsValue) -> String {
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

struct BodyReader(web_sys::ReadableStreamDefaultReader);

impl Drop for BodyReader {
  fn drop(&mut self) {
    self.0.release_lock();
  }
}

pub fn body_stream(
  stream: web_sys::ReadableStream,
) -> impl futures::Stream<Item = io::Result<Bytes>> + Send + 'static {
  let reader = BodyReader(stream.get_reader().unchecked_into());
  futures::stream::try_unfold(AssertSend(reader), |reader| {
    AssertSend(async move {
      let chunk = resolve(Ok(reader.0.0.read()))
        .await
        .map_err(io::Error::other)?;
      let field = |name: &str| {
        Reflect::get(&chunk, &name.into())
          .map_err(|err| io::Error::other(js_error(err)))
      };
      if field("done")?.is_truthy() {
        return Ok(None);
      }
      let bytes = field("value")?.unchecked_into::<Uint8Array>().to_vec();
      Ok(Some((Bytes::from(bytes), reader)))
    })
  })
}

/// Sends `request` with the runtime's `fetch`. A failure to get any response is
/// returned as a `502 Bad Gateway`, so callers treat it like an upstream error.
pub async fn fetch(
  request: reqwest::Request,
  follow_redirects: bool,
  decode_body: bool,
) -> reqwest::Response {
  match AssertSend(fetch_inner(request, follow_redirects, decode_body)).await {
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
  decode_body: bool,
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
  if let Some(body) = request.body() {
    let body = body
      .as_bytes()
      .ok_or_else(|| "request body is not buffered".to_string())?;
    init.set_body(&Uint8Array::from(body));
  }
  if let Some(timeout) = request.timeout() {
    let signal = web_sys::AbortSignal::timeout_with_u32(
      timeout.as_millis().try_into().unwrap_or(u32::MAX),
    );
    init.set_signal(Some(&signal));
  }
  if !decode_body {
    Reflect::set(&init, &"encodeResponseBody".into(), &"manual".into())
      .map_err(js_error)?;
  }
  let global: web_sys::WorkerGlobalScope = js_sys::global().unchecked_into();
  let response: web_sys::Response = resolve(Ok(in_current(|| {
    global.fetch_with_str_and_init(request.url().as_str(), &init)
  })))
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
      if !decode_body
        || (name != "content-encoding" && name != "content-length")
      {
        builder = builder.header(name, value);
      }
    }
  }
  let body = Bytes::from(Uint8Array::new(&body).to_vec());
  Ok(builder.body(body).map_err(|err| err.to_string())?.into())
}

#[derive(Clone)]
pub struct ResponseStream(pub AssertSend<web_sys::ReadableStream>);

async fn retry<T, Fut>(mut op: impl FnMut() -> Fut) -> Result<T, String>
where
  Fut: Future<Output = Result<T, String>>,
{
  let mut delay = std::time::Duration::from_millis(100);
  for _ in 0..2 {
    match op().await {
      Ok(value) => return Ok(value),
      Err(err) => tracing::warn!("R2 binding failed, retrying: {err}"),
    }
    tokio::time::sleep(delay).await;
    delay *= 4;
  }
  op().await
}

/// The R2 binding for a bucket, named `R2_<bucket name with - as _>`.
#[derive(Clone)]
pub struct R2Bucket(AssertSend<worker_sys::R2Bucket>);

impl R2Bucket {
  pub fn new(bucket_name: &str) -> Option<Self> {
    let binding = format!("R2_{}", bucket_name.replace('-', "_"));
    let value = Reflect::get(&env()?, &binding.into()).ok()?;
    (!value.is_undefined()).then(|| Self(AssertSend(value.unchecked_into())))
  }

  pub async fn get(
    &self,
    key: &str,
    offset: Option<usize>,
  ) -> Result<Option<Bytes>, String> {
    retry(move || {
      AssertSend(async move {
        let options = match offset {
          Some(offset) => object(&[(
            "range",
            object(&[("offset", (offset as f64).into())])?,
          )])?,
          None => JsValue::UNDEFINED,
        };
        let body =
          resolve(in_current(|| self.0.0.get(key.into(), options))).await?;
        if body.is_null() {
          return Ok(None);
        }
        let body: worker_sys::R2ObjectBody = body.unchecked_into();
        let bytes = resolve(body.array_buffer()).await?;
        Ok(Some(Bytes::from(Uint8Array::new(&bytes).to_vec())))
      })
    })
    .await
  }

  pub async fn get_stream(
    &self,
    key: &str,
  ) -> Result<Option<ResponseStream>, String> {
    retry(move || {
      AssertSend(async move {
        let body =
          resolve(in_current(|| self.0.0.get(key.into(), JsValue::UNDEFINED)))
            .await?;
        if body.is_null() {
          return Ok(None);
        }
        let body: worker_sys::R2ObjectBody = body.unchecked_into();
        let stream = body.body().map_err(js_error)?;
        Ok(Some(ResponseStream(AssertSend(stream))))
      })
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
    retry(move || {
      AssertSend(async move {
        let mut metadata = vec![("contentEncoding", content_encoding.into())];
        if let Some(content_type) = content_type {
          metadata.push(("contentType", content_type.into()));
        }
        if let Some(cache_control) = cache_control {
          metadata.push(("cacheControl", cache_control.into()));
        }
        let options = object(&[("httpMetadata", object(&metadata)?)])?;
        let value = Uint8Array::from(data).into();
        resolve(in_current(|| self.0.0.put(key.into(), value, options)))
          .await?;
        Ok(())
      })
    })
    .await
  }

  pub async fn delete(&self, key: &str) -> Result<(), String> {
    retry(move || {
      AssertSend(async move {
        resolve(in_current(|| self.0.0.delete(key.into()))).await?;
        Ok(())
      })
    })
    .await
  }

  pub async fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
    retry(move || {
      AssertSend(async move {
        let mut keys = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
          let mut options = vec![("prefix", prefix.into())];
          if let Some(cursor) = &cursor {
            options.push(("cursor", cursor.as_str().into()));
          }
          let options = object(&options)?;
          let page: worker_sys::R2Objects =
            resolve(in_current(|| self.0.0.list(options)))
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
    })
    .await
  }
}

/// Opens database connections through the Hyperdrive binding's `connect()`,
/// and runs the Postgres protocol over that socket: deployed bindings have no
/// address the worker could reach with a plain socket. Hyperdrive terminates
/// TLS to the database, so this hop is plaintext.
///
/// Without a binding (`wrangler dev` against a local Postgres), connects over
/// TCP to the host in `DATABASE_URL`, which has to be an IP address.
pub struct WorkerConnect;

impl deadpool_postgres::Connect for WorkerConnect {
  fn connect(
    &self,
    pg_config: &tokio_postgres::Config,
  ) -> Pin<
    Box<
      dyn Future<
          Output = Result<
            (tokio_postgres::Client, tokio::task::JoinHandle<()>),
            tokio_postgres::Error,
          >,
        > + Send
        + '_,
    >,
  > {
    let pg_config = pg_config.clone();
    Box::pin(AssertSend(async move {
      let has_binding = env().is_some_and(|env| {
        Reflect::get(&env, &"HYPERDRIVE".into())
          .is_ok_and(|binding| !binding.is_undefined())
      });
      if has_binding {
        let (client, connection) = pg_config
          .connect_raw(HyperdriveSocket::open(), tokio_postgres::NoTls)
          .await?;
        Ok((client, spawn_connection(connection)))
      } else {
        // A failure to connect is returned by the socket's first read, so it
        // surfaces as the connection's error.
        match local_socket(&pg_config).await {
          Ok(stream) => {
            let (client, connection) =
              pg_config.connect_raw(stream, tokio_postgres::NoTls).await?;
            Ok((client, spawn_connection(connection)))
          }
          Err(err) => {
            let (client, connection) = pg_config
              .connect_raw(FailedSocket(err), tokio_postgres::NoTls)
              .await?;
            Ok((client, spawn_connection(connection)))
          }
        }
      }
    }))
  }
}

fn spawn_connection(
  connection: impl Future<Output = Result<(), tokio_postgres::Error>> + 'static,
) -> tokio::task::JoinHandle<()> {
  tokio::spawn(AssertSend(async move {
    if let Err(err) = connection.await {
      tracing::error!("database connection failed: {err}");
    }
  }))
}

/// A TCP socket to the first host in the config.
async fn local_socket(
  pg_config: &tokio_postgres::Config,
) -> Result<tokio::net::TcpStream, String> {
  let Some(tokio_postgres::config::Host::Tcp(host)) =
    pg_config.get_hosts().first()
  else {
    return Err("DATABASE_URL has no TCP host".to_string());
  };
  let port = pg_config.get_ports().first().copied().unwrap_or(5432);
  tokio::net::TcpStream::connect((host.as_str(), port))
    .await
    .map_err(|err| format!("connecting to {host}:{port}: {err}"))
}

/// Fails every read and write with the reason the socket couldn't be opened.
struct FailedSocket(String);

impl AsyncRead for FailedSocket {
  fn poll_read(
    self: Pin<&mut Self>,
    _: &mut Context<'_>,
    _: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    Poll::Ready(Err(io_error(self.0.clone())))
  }
}

impl AsyncWrite for FailedSocket {
  fn poll_write(
    self: Pin<&mut Self>,
    _: &mut Context<'_>,
    _: &[u8],
  ) -> Poll<io::Result<usize>> {
    Poll::Ready(Err(io_error(self.0.clone())))
  }

  fn poll_flush(
    self: Pin<&mut Self>,
    _: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    Poll::Ready(Err(io_error(self.0.clone())))
  }

  fn poll_shutdown(
    self: Pin<&mut Self>,
    _: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    Poll::Ready(Ok(()))
  }
}

/// A socket from the Hyperdrive binding's `connect()`, read and written
/// through its `readable` and `writable` streams.
struct HyperdriveSocket {
  /// Set when the socket could not be opened; returned from the first read or
  /// write, so the failure surfaces as the connection's error.
  error: Option<String>,
  socket: JsValue,
  reader: JsValue,
  writer: JsValue,
  read: Option<JsFuture>,
  /// Bytes of the last chunk read that didn't fit the caller's buffer.
  unread: Vec<u8>,
  write: Option<JsFuture>,
}

fn call(target: &JsValue, method: &str) -> Result<JsValue, JsValue> {
  Reflect::get(target, &method.into())?
    .dyn_into::<Function>()?
    .call0(target)
}

fn call1(
  target: &JsValue,
  method: &str,
  arg: &JsValue,
) -> Result<JsValue, JsValue> {
  Reflect::get(target, &method.into())?
    .dyn_into::<Function>()?
    .call1(target, arg)
}

fn io_error(err: impl Into<String>) -> io::Error {
  io::Error::other(err.into())
}

fn promise(value: Result<JsValue, JsValue>) -> io::Result<JsFuture> {
  let promise = value.map_err(|err| io_error(js_error(err)))?;
  Ok(JsFuture::from(js_sys::Promise::from(promise)))
}

impl HyperdriveSocket {
  /// The socket of a new Hyperdrive connection, with its reader and writer.
  fn connect() -> Result<[JsValue; 3], String> {
    let env = env().ok_or("no worker env")?;
    let binding = Reflect::get(&env, &"HYPERDRIVE".into()).map_err(js_error)?;
    let socket = in_current(|| call(&binding, "connect")).map_err(js_error)?;
    let readable =
      Reflect::get(&socket, &"readable".into()).map_err(js_error)?;
    let writable =
      Reflect::get(&socket, &"writable".into()).map_err(js_error)?;
    let reader = call(&readable, "getReader").map_err(js_error)?;
    let writer = call(&writable, "getWriter").map_err(js_error)?;
    Ok([socket, reader, writer])
  }

  fn open() -> Self {
    let (error, [socket, reader, writer]) = match Self::connect() {
      Ok(socket) => (None, socket),
      Err(err) => (
        Some(format!("Hyperdrive connect: {err}")),
        [JsValue::UNDEFINED, JsValue::UNDEFINED, JsValue::UNDEFINED],
      ),
    };
    Self {
      error,
      socket,
      reader,
      writer,
      read: None,
      unread: Vec::new(),
      write: None,
    }
  }

  fn check(&self) -> io::Result<()> {
    match &self.error {
      Some(err) => Err(io_error(err.clone())),
      None => Ok(()),
    }
  }

  /// Waits for the last write to be accepted by the stream.
  fn poll_write_done(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    if let Some(write) = &mut self.write {
      let result = ready!(Pin::new(write).poll(cx));
      self.write = None;
      result.map_err(|err| io_error(js_error(err)))?;
    }
    Poll::Ready(Ok(()))
  }
}

// A read left pending holds the waker of the task that polled it, and through
// it the request's runtime, so the socket is closed to settle it.
impl Drop for HyperdriveSocket {
  fn drop(&mut self) {
    let _ = call(&self.socket, "close");
  }
}

impl AsyncRead for HyperdriveSocket {
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    self.check()?;
    loop {
      if !self.unread.is_empty() {
        let n = self.unread.len().min(buf.remaining());
        buf.put_slice(&self.unread[..n]);
        self.unread.drain(..n);
        return Poll::Ready(Ok(()));
      }

      if self.read.is_none() {
        self.read = Some(promise(call(&self.reader, "read"))?);
      }
      let result = ready!(Pin::new(self.read.as_mut().unwrap()).poll(cx));
      self.read = None;
      let chunk = result.map_err(|err| io_error(js_error(err)))?;

      let done = Reflect::get(&chunk, &"done".into())
        .map_err(|err| io_error(js_error(err)))?;
      if done.is_truthy() {
        // End of stream: returning without filling `buf` signals EOF.
        return Poll::Ready(Ok(()));
      }
      let value = Reflect::get(&chunk, &"value".into())
        .map_err(|err| io_error(js_error(err)))?;
      self.unread = Uint8Array::new(&value).to_vec();
    }
  }
}

impl AsyncWrite for HyperdriveSocket {
  fn poll_write(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    self.check()?;
    ready!(self.poll_write_done(cx))?;
    // `Uint8Array::from` copies `buf`, so the stream never holds a view into
    // wasm memory. Writes queue in order; the next write, flush or shutdown
    // waits for this one.
    let chunk = Uint8Array::from(buf);
    self.write = Some(promise(call1(&self.writer, "write", &chunk))?);
    Poll::Ready(Ok(buf.len()))
  }

  fn poll_flush(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    self.check()?;
    self.poll_write_done(cx)
  }

  fn poll_shutdown(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    self.check()?;
    ready!(self.poll_write_done(cx))?;
    // Closing is best-effort: the connection is done either way.
    let _ = call(&self.writer, "close");
    Poll::Ready(Ok(()))
  }
}
