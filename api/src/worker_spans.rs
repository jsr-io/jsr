// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Mirrors `tracing` spans into Workers custom trace spans, so they show up in
//! the request's trace next to the runtime's own `fetch` and binding spans.

use std::fmt::Debug;

use tracing::Subscriber;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::span;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use worker::observability::Span;
use worker::observability::SpanStatus;
use worker::observability::start_span;

use crate::worker_js::AssertSend;

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
      Some(parent) => parent.in_scope(|| start_span(name)),
      None => start_span(name),
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
          self.0.set_status(SpanStatus::Error(None));
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

  fn record_i64(&mut self, field: &Field, value: i64) {
    self.0.set_attribute(field.name(), value);
  }

  fn record_u64(&mut self, field: &Field, value: u64) {
    self.0.set_attribute(field.name(), value);
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
  let span = current().filter(|span| span.is_traced())?;
  Some(span.span_context().trace_id)
}
