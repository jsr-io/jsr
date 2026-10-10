// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! This module implements a service that wraps the router and handles
//! tracing. It starts a span for each request, and records successes and
//! failure.

// The OpenTelemetry stack is not compiled into the wasm worker (see
// `tracing.rs`), so trace propagation and `x-deno-ray` are native-only.
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;
use std::convert::Infallible;
use std::task::Context;
use std::task::Poll;

use axum::body::Body;
use futures::FutureExt;
use futures::future::BoxFuture;
use hyper::Request;
use hyper::Response;
#[cfg(not(target_arch = "wasm32"))]
use hyper::header::HeaderValue;
#[cfg(not(target_arch = "wasm32"))]
use opentelemetry::global;
#[cfg(not(target_arch = "wasm32"))]
use opentelemetry::trace::TraceContextExt;
use tower::Service;
use tracing::Instrument;
use tracing::Span;
use tracing::field;
use tracing::info_span;
#[cfg(not(target_arch = "wasm32"))]
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::router::App;

#[derive(Clone)]
pub struct TracedRouterService {
  app: App,
  is_internal: bool,
}

impl TracedRouterService {
  /// `is_internal` determines if the router will respect incoming tracing
  /// headers.
  pub fn new(app: App, is_internal: bool) -> Self {
    Self { app, is_internal }
  }
}

impl Service<Request<Body>> for TracedRouterService {
  type Response = Response<Body>;
  type Error = Infallible;
  type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

  fn poll_ready(
    &mut self,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    self.app.poll_ready(cx)
  }

  fn call(&mut self, req: Request<Body>) -> Self::Future {
    let method = req.method().as_str();
    let uri = req.uri();
    let headers = req.headers();
    let user_agent = headers
      .get("user-agent")
      .map(|v| v.to_str().unwrap_or(""))
      .unwrap_or("");

    let span = info_span!(
      "HTTP",
      "http.method" = method,
      "http.url" = ?uri,
      "http.user_agent" = user_agent,
      "http.status_code" = field::Empty,
      "otel.status_code" = "ok",
      "otel.kind" = "server"
    );

    #[cfg(not(target_arch = "wasm32"))]
    if self.is_internal {
      global::get_text_map_propagator(|propagator| {
        let mut headers = HashMap::new();
        for (k, v) in req.headers() {
          headers.insert(k.to_string(), v.to_str().unwrap().to_string());
        }
        let cx = propagator.extract(&headers);
        // Fails only when no OpenTelemetry layer is installed (export target
        // `None`) or the span has already been started, neither of which
        // matters here: there is nothing to attach the parent to.
        let _ = span.set_parent(cx);
      });
    }

    let fut = self.app.call(req).map(|res| {
      let Ok(mut resp) = res;
      let status = resp.status();
      let span = Span::current();
      #[cfg(not(target_arch = "wasm32"))]
      {
        let ctx = span.context();
        let span_ref = ctx.span();
        let span_ctx = span_ref.span_context();
        let trace_id = span_ctx.trace_id().to_string();
        let headers = resp.headers_mut();
        headers.insert("x-deno-ray", HeaderValue::from_str(&trace_id).unwrap());
      }
      span.record("http.status_code", status.as_u16());
      Ok(resp)
    });

    fut.instrument(span).boxed()
  }
}
