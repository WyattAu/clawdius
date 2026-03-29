//! OpenTelemetry Integration (Optional)
//!
//! Provides an OTel-compatible tracing layer that exports spans and metrics
//! to an OpenTelemetry Protocol (OTLP) collector (e.g., Jaeger, Tempo, Grafana
//! Tempo, or any OTLP-compatible backend).
//!
//! # Feature Gate
//!
//! Enabled with `--features otel` on `clawdius-server`.
//!
//! # Configuration
//!
//! Set environment variables before starting the server:
//! - `OTEL_EXPORTER_OTLP_ENDPOINT` — collector URL (default: `http://localhost:4317`)
//! - `OTEL_SERVICE_NAME` — service name (default: `clawdius-server`)
//!
//! When the feature is enabled but no collector is reachable, traces still
//! flow to the console (via the existing `tracing_subscriber` fmt layer) —
//! OTel export degrades gracefully.

#![deny(unsafe_code)]

use opentelemetry::trace::TracerProvider as _;
use tracing_subscriber::prelude::*;

/// Initialise the OpenTelemetry tracing pipeline.
///
/// Creates an OTLP gRPC trace exporter and installs it as a `tracing`
/// subscriber layer. If the collector endpoint is unreachable at startup,
/// the layer is still installed — OTel will retry in the background.
///
/// The exporter reads `OTEL_EXPORTER_OTLP_ENDPOINT` from the environment
/// (standard OTel env var). Falls back to `http://localhost:4317`.
///
/// Returns the `TracerProvider` on success (for later shutdown), or `None`
/// if OTel could not be initialised (degrades gracefully).
pub fn init_otel_tracing() -> Option<opentelemetry_sdk::trace::TracerProvider> {
    // Set default endpoint if not configured via env
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_err() {
        std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4317");
    }

    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "clawdius-server".to_string());

    // Build OTLP exporter (gRPC) — reads OTEL_EXPORTER_OTLP_ENDPOINT from env
    let exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()
    {
        Ok(e) => e,
        Err(e) => {
            tracing_eprintln!(
                error = %e,
                "Failed to create OTLP exporter — OTel tracing disabled"
            );
            return None;
        }
    };

    // Build tracer provider with batch export and tokio runtime
    let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(opentelemetry_sdk::Resource::new(vec![
            opentelemetry::KeyValue::new(
                opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                service_name,
            ),
        ]))
        .build();

    // Install as global tracer provider
    let _ = opentelemetry::global::set_tracer_provider(tracer_provider.clone());

    // Build the tracing layer using a concrete SdkTracer (not BoxedTracer,
    // which doesn't implement PreSampledTracer).
    let tracer = tracer_provider.tracer("clawdius-server");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Install as a global tracing subscriber alongside the fmt layer.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    match tracing_subscriber::registry()
        .with(env_filter)
        .with(telemetry)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
    {
        Ok(()) => {
            let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string());
            tracing::info!(
                endpoint = %endpoint,
                "OTel tracing initialised (OTLP gRPC)"
            );
            Some(tracer_provider)
        }
        Err(_) => {
            eprintln!(
                "Global tracing subscriber already set — OTel layer not added \
                 (call init_otel_tracing before other tracing init)"
            );
            None
        }
    }
}

/// Shut down the OTel tracer provider, flushing any pending spans.
///
/// Call this during graceful shutdown to ensure all spans are exported.
/// This is synchronous in opentelemetry-sdk 0.27 — it blocks briefly
/// to drain the batch processor.
pub fn shutdown(provider: Option<opentelemetry_sdk::trace::TracerProvider>) {
    if let Some(provider) = provider {
        if let Err(e) = provider.shutdown() {
            tracing::warn!(error = %e, "OTel tracer shutdown failed");
        }
    }
}

/// Print to stderr (used before tracing subscriber is initialised).
macro_rules! tracing_eprintln {
    ($($arg:tt)*) => {
        eprintln!($($arg)*)
    };
}
