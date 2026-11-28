// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Mirrors `tracing` spans into Workers custom trace spans, so they show up in
//! the request's trace next to the runtime's own `fetch` and binding spans.

use std::fmt::Debug;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::task::Context as TaskContext;
use std::task::Poll;

use pin_project::pin_project;
use tracing::Subscriber;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::span;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::ScopedClosure;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::worker_js::AssertSend;
use crate::worker_js::js_error;

// Every binding is `catch`: the runtime can throw, for example when a span is
// used from the async context of another request. An exception from an
// import without `catch` unwinds through the wasm frames and can skip Rust
// destructors on the way, which can leave borrows, locks, and the event
// loop's state behind for every later request on the isolate.
#[wasm_bindgen(inline_js = "
import { tracing } from 'cloudflare:workers';
export function startSpan(name) { return tracing.startSpan(name); }
export function getActiveSpan() { return tracing.getActiveSpan(); }
export function withActiveSpan(span, cb) { tracing.withActiveSpan(span, cb); }
export function isTraced(span) { return span.isTraced; }
export function traceId(span) { return span.spanContext().traceId; }
export function setAttribute(span, key, value) { span.setAttribute(key, value); }
export function setErrorStatus(span) { span.setStatus({ code: 'error' }); }
export function endSpan(span) { span.end(); }
")]
extern "C" {
  #[wasm_bindgen(catch, js_name = startSpan)]
  fn start_span(name: &str) -> Result<JsValue, JsValue>;
  #[wasm_bindgen(catch, js_name = getActiveSpan)]
  fn get_active_span() -> Result<JsValue, JsValue>;
  #[wasm_bindgen(catch, js_name = withActiveSpan)]
  fn with_active_span(
    span: &JsValue,
    cb: &ScopedClosure<dyn FnMut()>,
  ) -> Result<(), JsValue>;
  #[wasm_bindgen(catch, js_name = isTraced)]
  fn is_traced(span: &JsValue) -> Result<bool, JsValue>;
  #[wasm_bindgen(catch, js_name = traceId)]
  fn trace_id(span: &JsValue) -> Result<String, JsValue>;
  #[wasm_bindgen(catch, js_name = setAttribute)]
  fn set_attribute(
    span: &JsValue,
    key: &str,
    value: &JsValue,
  ) -> Result<(), JsValue>;
  #[wasm_bindgen(catch, js_name = setErrorStatus)]
  fn set_error_status(span: &JsValue) -> Result<(), JsValue>;
  #[wasm_bindgen(catch, js_name = endSpan)]
  fn end_span(span: &JsValue) -> Result<(), JsValue>;
}

/// Logs a failed span call. Tracing is best effort, so the caller carries on
/// without the span. The log goes to stderr because the layer can't emit
/// `tracing` events from its own callbacks, and is limited to every
/// power-of-two failure because a request polled in the wrong context fails
/// on every poll.
fn log_failure(call: &str, err: JsValue) {
  static FAILURES: AtomicU64 = AtomicU64::new(0);
  let failures = FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
  if failures.is_power_of_two() {
    eprintln!(
      "worker span: {call} failed ({failures} failures on this isolate): {}",
      js_error(err)
    );
  }
}

/// A Workers custom trace span. Calls on a span that has ended are ignored.
#[derive(Clone)]
struct Span(JsValue);

impl Span {
  /// Starts a span under the active one, without making it active.
  fn start(name: &str) -> Option<Self> {
    start_span(name)
      .map_err(|err| log_failure("startSpan", err))
      .ok()
      .map(Self)
  }

  /// The span that is active on the current async context.
  fn active() -> Option<Self> {
    get_active_span()
      .ok()
      .filter(|span| !span.is_undefined() && !span.is_null())
      .map(Self)
  }

  /// Whether the span is active. `false` when the runtime can't tell.
  fn is_active(&self) -> bool {
    get_active_span().is_ok_and(|active| active == self.0)
  }

  /// Runs `f` with the span active, so the spans and runtime operations that
  /// `f` starts nest under it. Runs `f` without the span if the runtime
  /// refuses to activate it.
  fn in_scope<R>(&self, f: impl FnOnce() -> R) -> R {
    let mut f = Some(f);
    let mut out = None;
    // `withActiveSpan` invokes the callback once, synchronously. A panic
    // must not unwind into JS, so it's caught here and resumed below.
    let mut cb = || {
      if let Some(f) = f.take() {
        out = Some(std::panic::catch_unwind(AssertUnwindSafe(f)));
      }
    };
    let cb = ScopedClosure::<dyn FnMut()>::borrow_mut_aborting(&mut cb);
    let result = with_active_span(&self.0, &cb);
    drop(cb);
    match (out, f, result) {
      (Some(Ok(value)), _, _) => value,
      (Some(Err(payload)), _, _) => std::panic::resume_unwind(payload),
      (None, Some(f), result) => {
        // The callback never ran.
        if let Err(err) = result {
          log_failure("withActiveSpan", err);
        }
        f()
      }
      // A JS exception unwound through `f`, so there is no value to return.
      (None, None, Err(err)) => wasm_bindgen::throw_val(err),
      (None, None, Ok(())) => unreachable!("the callback returned no value"),
    }
  }

  /// Whether the span is recorded: `false` when the request isn't sampled or
  /// the span has ended.
  fn is_traced(&self) -> bool {
    is_traced(&self.0).unwrap_or(false)
  }

  fn trace_id(&self) -> Option<String> {
    trace_id(&self.0).ok()
  }

  fn set_attribute(&self, key: &str, value: impl Into<JsValue>) {
    if let Err(err) = set_attribute(&self.0, key, &value.into()) {
      log_failure("setAttribute", err);
    }
  }

  fn set_error_status(&self) {
    if let Err(err) = set_error_status(&self.0) {
      log_failure("setStatus", err);
    }
  }

  fn end(&self) {
    if let Err(err) = end_span(&self.0) {
      log_failure("end", err);
    }
  }
}

/// A future that is polled with a span active, see [`in_active_span`].
#[pin_project]
pub struct Instrumented<F> {
  span: Option<AssertSend<Span>>,
  #[pin]
  inner: F,
}

impl<F: Future> Future for Instrumented<F> {
  type Output = F::Output;

  fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<F::Output> {
    let this = self.project();
    match this.span {
      // The poll usually runs in a context that already carries the span.
      Some(AssertSend(span)) if !span.is_active() => {
        span.in_scope(|| this.inner.poll(cx))
      }
      _ => this.inner.poll(cx),
    }
  }
}

/// Wraps `future` so that the span that is active now is active on every poll.
/// The event loop polls a future in the context of whatever woke it, and the
/// spans that the future starts are parented under the active span.
pub fn in_active_span<F: Future>(future: F) -> Instrumented<F> {
  Instrumented {
    span: Span::active().map(AssertSend),
    inner: future,
  }
}

/// The Workers span of a `tracing` span, kept in its extensions.
struct WorkerSpan(AssertSend<Span>);

pub struct WorkerSpanLayer;

impl<S> Layer<S> for WorkerSpanLayer
where
  S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
  fn on_new_span(
    &self,
    attrs: &span::Attributes<'_>,
    id: &span::Id,
    ctx: Context<'_, S>,
  ) {
    let Some(span) = ctx.span(id) else { return };
    let parent = span.parent().and_then(|parent| {
      let extensions = parent.extensions();
      extensions.get::<WorkerSpan>().map(|span| span.0.0.clone())
    });
    // The runtime parents a new span under the active one. A root span is
    // started under the invocation's span, see `fetch` in `main.rs`.
    let name = attrs.metadata().name();
    let worker_span = match parent {
      Some(parent) => parent.in_scope(|| Span::start(name)),
      None => Span::start(name),
    };
    let Some(worker_span) = worker_span else {
      return;
    };
    if worker_span.is_traced() {
      attrs.record(&mut Attributes(&worker_span));
    }
    span
      .extensions_mut()
      .insert(WorkerSpan(AssertSend(worker_span)));
  }

  fn on_record(
    &self,
    id: &span::Id,
    values: &span::Record<'_>,
    ctx: Context<'_, S>,
  ) {
    let Some(span) = ctx.span(id) else { return };
    if let Some(WorkerSpan(AssertSend(worker_span))) = span.extensions().get()
      && worker_span.is_traced()
    {
      values.record(&mut Attributes(worker_span));
    }
  }

  fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
    let Some(span) = ctx.span(&id) else { return };
    if let Some(WorkerSpan(AssertSend(worker_span))) = span.extensions().get() {
      worker_span.end();
    }
  }
}

/// Sets recorded fields as attributes of a Workers span.
struct Attributes<'a>(&'a Span);

impl Visit for Attributes<'_> {
  fn record_str(&mut self, field: &Field, value: &str) {
    match field.name() {
      // An `ok` status is final, and the request's span starts out `ok`, so
      // only errors are passed on.
      "otel.status_code" => {
        if value == "error" {
          self.0.set_error_status();
        }
      }
      "otel.kind" => {}
      name => {
        self.0.set_attribute(name, value);
      }
    }
  }

  fn record_bool(&mut self, field: &Field, value: bool) {
    self.0.set_attribute(field.name(), value);
  }

  // Integers become a JS `number`, so values beyond 2^53 lose precision.
  fn record_i64(&mut self, field: &Field, value: i64) {
    self.0.set_attribute(field.name(), value as f64);
  }

  fn record_u64(&mut self, field: &Field, value: u64) {
    self.0.set_attribute(field.name(), value as f64);
  }

  fn record_f64(&mut self, field: &Field, value: f64) {
    self.0.set_attribute(field.name(), value);
  }

  fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
    self.0.set_attribute(field.name(), format!("{value:?}"));
  }
}

/// The Workers span of the current `tracing` span.
fn current() -> Option<Span> {
  tracing::Span::current()
    .with_subscriber(|(id, dispatch)| {
      let span = dispatch.downcast_ref::<Registry>()?.span(id)?;
      let extensions = span.extensions();
      extensions.get::<WorkerSpan>().map(|span| span.0.0.clone())
    })
    .flatten()
}

/// Runs `f` with the current `tracing` span's Workers span active, so the
/// runtime spans of the `fetch` and binding calls `f` makes nest under it.
pub fn in_current<R>(f: impl FnOnce() -> R) -> R {
  match current() {
    Some(span) => span.in_scope(f),
    None => f(),
  }
}

/// The trace id of the current span, if the request is being traced.
pub fn current_trace_id() -> Option<String> {
  current().filter(|span| span.is_traced())?.trace_id()
}
