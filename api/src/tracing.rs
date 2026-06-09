// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::TraceId;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::LoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::runtime;
use opentelemetry_sdk::trace::TracerProvider;
use tracing_opentelemetry::OtelData;
use tracing_subscriber::Layer;
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

/// Append an OTLP signal subpath to the configured base endpoint, OTEL
/// `OTEL_EXPORTER_OTLP_ENDPOINT` style: the endpoint is the base (e.g. Grafana
/// Cloud's `.../otlp`) and each signal posts to its own path (`/v1/traces` for
/// spans, `/v1/logs` for logs). The opentelemetry-otlp HTTP exporter uses the
/// endpoint verbatim and does NOT do this itself, so posting to the bare base
/// 404s. Tolerates a trailing slash and an endpoint that already carries the
/// signal path.
fn otlp_signal_endpoint(base: &str, signal_path: &str) -> String {
  let base = base.trim_end_matches('/');
  if base.ends_with(signal_path) {
    base.to_string()
  } else {
    format!("{base}{signal_path}")
  }
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
  deployment_environment: Option<String>,
) -> (LogFilterHandle, String) {
  let mut resource = vec![
    KeyValue::new("service.name", name),
    KeyValue::new("service.namespace", "registry"),
  ];
  // Distinguishes staging from prod telemetry when both export to the same
  // backend. Empty/unset omits it rather than reporting a blank environment.
  if let Some(env) = deployment_environment.filter(|s| !s.trim().is_empty()) {
    resource.push(KeyValue::new("deployment.environment", env));
  }
  let resource = Resource::new(resource);

  // OTLP/HTTP (protobuf), not gRPC: the managed Grafana Cloud gateway only
  // accepts HTTP, and it also works directly from the Cloudflare Container.
  // `endpoint` is the base; each signal's subpath is appended here. `headers`
  // carries the backend auth, e.g. `Authorization: Basic <base64>` for Grafana
  // Cloud. Traces export as spans (`/v1/traces`) and `tracing` events are
  // bridged into OpenTelemetry log records and exported alongside them
  // (`/v1/logs`), so the same logs we print to stdout also land in Grafana.
  //
  // Each exporter's provider is kept alive past this function: the tracer
  // provider by the global registration below, and the logger provider by the
  // appender layer (its `Logger` holds an `Arc` to the provider's batch
  // processor), so dropping the local handles here does not stop export.
  let mut export_layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> =
    Vec::new();
  match export_target {
    TracingExportTarget::Otlp { endpoint, headers } => {
      let span_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(otlp_signal_endpoint(&endpoint, "/v1/traces"))
        .with_protocol(Protocol::HttpBinary)
        .with_headers(headers.clone())
        .build()
        .unwrap();
      let tracer_provider = TracerProvider::builder()
        .with_batch_exporter(span_exporter, runtime::Tokio)
        .with_resource(resource.clone())
        .build();
      let tracer = tracer_provider.tracer(name);
      global::set_tracer_provider(tracer_provider);
      export_layers
        .push(tracing_opentelemetry::layer().with_tracer(tracer).boxed());

      let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_endpoint(otlp_signal_endpoint(&endpoint, "/v1/logs"))
        .with_protocol(Protocol::HttpBinary)
        .with_headers(headers)
        .build()
        .unwrap();
      let logger_provider = LoggerProvider::builder()
        .with_batch_exporter(log_exporter, runtime::Tokio)
        .with_resource(resource)
        .build();
      export_layers
        .push(OpenTelemetryTracingBridge::new(&logger_provider).boxed());
    }
    TracingExportTarget::None => {}
  };

  let base_filter = EnvFilter::builder()
    .with_default_directive(DEFAULT_LOG_LEVEL_FILTER.into())
    .from_env_lossy()
    .add_directive("swc_ecma_codegen=off".parse().unwrap());
  let default_filter_directive = base_filter.to_string();
  let (filter, reload_handle) = reload::Layer::new(base_filter);
  let fmt = tracing_subscriber::fmt::layer()
    .with_ansi(false)
    .event_format(FullOutputWithTraceId);
  let subscriber = Registry::default()
    .with(export_layers)
    .with(filter)
    .with(fmt);
  tracing::subscriber::set_global_default(subscriber).unwrap();

  global::set_text_map_propagator(TraceContextPropagator::new());
  (reload_handle, default_filter_directive)
}

/// Handle to the log-level filter within tracing infrastructure.
pub type LogFilterHandle = reload::Handle<
  EnvFilter,
  Layered<Vec<Box<dyn Layer<Registry> + Send + Sync>>, Registry>,
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
  use super::otlp_signal_endpoint;
  use super::parse_otlp_headers;

  #[test]
  fn appends_signal_path_to_base() {
    assert_eq!(
      otlp_signal_endpoint("https://x.grafana.net/otlp", "/v1/traces"),
      "https://x.grafana.net/otlp/v1/traces"
    );
    assert_eq!(
      otlp_signal_endpoint("https://x.grafana.net/otlp", "/v1/logs"),
      "https://x.grafana.net/otlp/v1/logs"
    );
  }

  #[test]
  fn tolerates_trailing_slash_and_existing_path() {
    assert_eq!(
      otlp_signal_endpoint("https://x.grafana.net/otlp/", "/v1/traces"),
      "https://x.grafana.net/otlp/v1/traces"
    );
    assert_eq!(
      otlp_signal_endpoint(
        "https://x.grafana.net/otlp/v1/traces",
        "/v1/traces"
      ),
      "https://x.grafana.net/otlp/v1/traces"
    );
  }

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
