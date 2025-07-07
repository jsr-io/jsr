// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

//! This module implements a hyper service that wraps routerify and handles
//! tracing. It starts a span for each request, and records successes and
//! failure.

// Adapted from https://github.com/routerify/routerify. Copyright 2020 Rousan Ali. MIT License

use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::future::Ready;
use std::future::ready;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures::FutureExt;
use hyper::Request;
use hyper::Response;
use hyper::body::HttpBody;
use hyper::header::HeaderValue;
use hyper::server::conn::AddrStream;
use hyper::service::Service;
use opentelemetry::global;
use opentelemetry::trace::TraceContextExt;
use routerify::RequestService;
use routerify::RequestServiceBuilder;
use routerify::Router;
use tracing::Instrument;
use tracing::Span;
use tracing::error;
use tracing::field;
use tracing::info_span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug)]
pub struct TracedRouterService<B, E> {
  builder: TracedRequestServiceBuilder<B, E>,
  is_internal: bool,
}

impl<
  B: HttpBody + Send + Sync + 'static,
  E: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
> TracedRouterService<B, E>
{
  /// Creates a new service with the provided router and it's ready to be used with the hyper [`serve`](https://docs.rs/hyper/0.14.4/hyper/server/struct.Builder.html#method.serve)
  /// method. `is_internal` determines if the router will respect incoming tracing headers.
  pub fn new(
    router: Router<B, E>,
    is_internal: bool,
  ) -> routerify::Result<TracedRouterService<B, E>> {
    let builder = TracedRequestServiceBuilder::new(router)?;
    Ok(TracedRouterService {
      builder,
      is_internal,
    })
  }
}

impl<
  B: HttpBody + Send + Sync + 'static,
  E: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
> Service<&AddrStream> for TracedRouterService<B, E>
{
  type Response = TracedRequestService<B, E>;
  type Error = Infallible;
  type Future = Ready<Result<Self::Response, Self::Error>>;

  fn poll_ready(
    &mut self,
    _cx: &mut Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, conn: &AddrStream) -> Self::Future {
    let req_service = self.builder.build(conn.remote_addr(), self.is_internal);

    ready(Ok(req_service))
  }
}

pub struct TracedRequestService<B, E> {
  request_service: RequestService<B, E>,
  is_internal: bool,
}

// Here to please clippy.
type PinBox<T> = Pin<Box<T>>;

impl<
  B: HttpBody + Send + Sync + 'static,
  E: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
> Service<Request<hyper::Body>> for TracedRequestService<B, E>
{
  type Response = Response<B>;
  type Error = routerify::RouteError;
  type Future = PinBox<
    dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static,
  >;

  fn poll_ready(
    &mut self,
    _cx: &mut Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, req: Request<hyper::Body>) -> Self::Future {
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
      "otel.status_code" = "ok"
    );

    if self.is_internal {
      global::get_text_map_propagator(|propagator| {
        let mut headers = HashMap::new();
        for (k, v) in req.headers() {
          headers.insert(k.to_string(), v.to_str().unwrap().to_string());
        }
        let cx = propagator.extract(&headers);
        span.set_parent(cx);
      });
    }

    let fut = self.request_service.call(req).then(|res| async move {
      match res {
        Ok(mut resp) => {
          let status = resp.status();
          let span = Span::current();
          let ctx = span.context();
          let span_ref = ctx.span();
          let span_ctx = span_ref.span_context();
          let trace_id = span_ctx.trace_id().to_string();
          let headers = resp.headers_mut();
          headers
            .insert("x-deno-ray", HeaderValue::from_str(&trace_id).unwrap());
          span.record("http.status_code", status.as_u16());
          Ok(resp)
        }
        Err(err) => {
          error!({ ?err }, "error");
          Err(err)
        }
      }
    });

    fut.instrument(span).boxed()
  }
}

#[derive(Debug)]
pub struct TracedRequestServiceBuilder<B, E> {
  builder: RequestServiceBuilder<B, E>,
}

impl<
  B: HttpBody + Send + Sync + 'static,
  E: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
> TracedRequestServiceBuilder<B, E>
{
  pub fn new(router: Router<B, E>) -> routerify::Result<Self> {
    let builder = RequestServiceBuilder::new(router)?;

    Ok(Self { builder })
  }

  pub fn build(
    &mut self,
    remote_addr: SocketAddr,
    is_internal: bool,
  ) -> TracedRequestService<B, E> {
    let request_service = self.builder.build(remote_addr);
    TracedRequestService {
      request_service,
      is_internal,
    }
  }
}
