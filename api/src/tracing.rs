// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright Deno Land Inc. All Rights Reserved. Proprietary and confidential.

use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::sdk::Resource;
use opentelemetry::sdk::propagation::TraceContextPropagator;
use opentelemetry::sdk::trace;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::TraceId;
use opentelemetry_otlp::WithExportConfig;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_opentelemetry::OtelData;
use tracing_subscriber::Registry;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::FormatFields;
use tracing_subscriber::layer::Layered;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::reload;

pub enum TracingExportTarget {
  Otlp {
    endpoint: String,
    headers: std::collections::HashMap<String, String>,
  },
  None,
}

/// Parse the `OTLP_HEADERS` value (`key1=value1,key2=value2`, the OpenTelemetry
/// `OTEL_EXPORTER_OTLP_HEADERS` format) into a header map. Splits each pair on
/// its first `=` only, so values containing `=` (e.g. base64 padding in a
/// `Basic` auth header) survive intact.
pub fn parse_otlp_headers(
  raw: Option<&str>,
) -> std::collections::HashMap<String, String> {
  raw
    .into_iter()
    .flat_map(|s| s.split(','))
    .filter_map(|pair| pair.split_once('='))
    .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
    .collect()
}

/// Initialize tracing infrastructure.
///
/// `tracing` has a three core concepts. These are Spans, Events, and subscribers.
///  - A Span represents a period of time with a beginning and an end. They record
///    the flow of execution.
///  - An Event represents a moment in time. It signifies something
///    that happened while a trace was being recorded.
///  - As Spans and Events occur, they are recorded or aggregated by implementations
///    of the Subscriber trait. Subscribers are notified when an Event takes place
///    and when a Span is entered or exited
pub async fn setup_tracing(
  name: &'static str,
  export_target: TracingExportTarget,
) -> (LogFilterHandle, String) {
  let trace_config = trace::config().with_resource(Resource::new(vec![
    KeyValue::new("service.name", name),
    KeyValue::new("service.namespace", "registry"),
  ]));

  let tracer = match export_target {
    TracingExportTarget::Otlp { endpoint, headers } => {
      // OTLP/HTTP (protobuf), not gRPC: the managed Grafana Cloud gateway only
      // accepts HTTP, and it also works directly from the Cloudflare Container.
      // The endpoint is used verbatim (no `/v1/traces` is appended), so it must
      // already include the path. `headers` carries the backend auth, e.g.
      // `Authorization: Basic <base64>` for Grafana Cloud.
      let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_endpoint(endpoint)
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .with_headers(headers);
      let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_trace_config(trace_config)
        .with_exporter(exporter)
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();
      Some(tracer)
    }
    TracingExportTarget::None => None,
  };

  let telemetry =
    tracer.map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer));

  let base_filter = EnvFilter::builder()
    .with_default_directive(DEFAULT_LOG_LEVEL_FILTER.into())
    .from_env_lossy()
    .add_directive("swc_ecma_codegen=off".parse().unwrap());
  let default_filter_directive = base_filter.to_string();
  let (filter, reload_handle) = reload::Layer::new(base_filter);
  let fmt = tracing_subscriber::fmt::layer()
    .with_ansi(false)
    .event_format(FullOutputWithTraceId);
  let subscriber = Registry::default().with(telemetry).with(filter).with(fmt);
  tracing::subscriber::set_global_default(subscriber).unwrap();

  global::set_text_map_propagator(TraceContextPropagator::new());
  (reload_handle, default_filter_directive)
}

/// Handle to the log-level filter within tracing infrastructure.
pub type LogFilterHandle = reload::Handle<
  EnvFilter,
  Layered<Option<OpenTelemetryLayer<Registry, trace::Tracer>>, Registry>,
>;

/// Default log level filter, used if `RUST_LOG` is missing or invalid.
const DEFAULT_LOG_LEVEL_FILTER: LevelFilter = LevelFilter::INFO;

struct FullOutputWithTraceId;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for FullOutputWithTraceId
where
  S: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
  N: for<'writer> FormatFields<'writer> + 'static,
{
  fn format_event(
    &self,
    ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
    mut writer: tracing_subscriber::fmt::format::Writer<'_>,
    event: &tracing::Event<'_>,
  ) -> std::fmt::Result {
    // tracing_subscriber will likely provide a way to add extra fields when
    // formatting events (logs): https://github.com/tokio-rs/tracing/pull/2664
    // Once this patch lands we can take advantage of it.
    //
    // Also, there's another pending PR implementing a OpenTelemetry-aware
    // formatter that does more than what we implement here. We'll watch the
    // status of it: https://github.com/tokio-rs/tracing/pull/1304
    if let Some(trace_id) = get_trace_id(ctx, event) {
      write!(writer, "trace_id={trace_id} ")?;
    }

    tracing_subscriber::fmt::format::Format::default()
      .format_event(ctx, writer, event)
  }
}

fn get_trace_id<S, N>(
  ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
  event: &tracing::Event<'_>,
) -> Option<TraceId>
where
  S: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
  N: for<'writer> FormatFields<'writer> + 'static,
{
  let current_span = event
    .parent()
    .and_then(|id| ctx.span(id))
    .or_else(|| ctx.lookup_current())?;
  let extensions = current_span.extensions();
  let otel_data = extensions.get::<OtelData>()?;
  Some(otel_data.parent_cx.span().span_context().trace_id())
}

#[cfg(test)]
mod tests {
  use super::parse_otlp_headers;

  #[test]
  fn none_is_empty() {
    assert!(parse_otlp_headers(None).is_empty());
    assert!(parse_otlp_headers(Some("")).is_empty());
  }

  #[test]
  fn keeps_equals_in_value() {
    // A `Basic` auth header's base64 value can contain `=` padding; only the
    // first `=` of each pair separates key from value.
    let headers =
      parse_otlp_headers(Some("Authorization=Basic dXNlcjpwYXNz=="));
    assert_eq!(headers.len(), 1);
    assert_eq!(headers["Authorization"], "Basic dXNlcjpwYXNz==");
  }

  #[test]
  fn multiple_pairs_are_trimmed() {
    let headers = parse_otlp_headers(Some("a=1, b=2"));
    assert_eq!(headers["a"], "1");
    assert_eq!(headers["b"], "2");
  }
}
