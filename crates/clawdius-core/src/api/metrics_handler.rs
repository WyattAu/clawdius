use axum::http::{header, StatusCode};
use axum::response::IntoResponse;

use crate::telemetry::{MetricsDashboard, MetricsSnapshot};
pub async fn metrics_handler() -> impl IntoResponse {
    let dashboard = MetricsDashboard::new();
    let snapshot = dashboard.comprehensive_snapshot();
    let body = format_prometheus(&snapshot);

    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

fn format_prometheus(s: &MetricsSnapshot) -> String {
    let mut out = String::new();

    prom_counter(
        &mut out,
        "clawdius_llm_requests_total",
        "Total LLM requests",
        s.llm.total_requests,
    );
    prom_counter(
        &mut out,
        "clawdius_llm_requests_successful_total",
        "Successful LLM requests",
        s.llm.successful_requests,
    );
    prom_counter(
        &mut out,
        "clawdius_llm_requests_failed_total",
        "Failed LLM requests",
        s.llm.failed_requests,
    );
    prom_counter(
        &mut out,
        "clawdius_llm_tokens_total",
        "Total tokens consumed",
        s.llm.total_tokens,
    );
    prom_counter(
        &mut out,
        "clawdius_llm_prompt_tokens_total",
        "Prompt tokens consumed",
        s.llm.prompt_tokens,
    );
    prom_counter(
        &mut out,
        "clawdius_llm_completion_tokens_total",
        "Completion tokens consumed",
        s.llm.completion_tokens,
    );
    prom_gauge(
        &mut out,
        "clawdius_llm_avg_latency_ms",
        "Average LLM latency in milliseconds",
        s.llm.avg_latency_ms,
    );
    prom_gauge(
        &mut out,
        "clawdius_llm_p50_latency_ms",
        "P50 LLM latency in milliseconds",
        s.llm.p50_latency_ms,
    );
    prom_gauge(
        &mut out,
        "clawdius_llm_p95_latency_ms",
        "P95 LLM latency in milliseconds",
        s.llm.p95_latency_ms,
    );
    prom_gauge(
        &mut out,
        "clawdius_llm_p99_latency_ms",
        "P99 LLM latency in milliseconds",
        s.llm.p99_latency_ms,
    );

    for (provider, count) in &s.llm.requests_by_provider {
        prom_counter_label(
            &mut out,
            "clawdius_llm_requests_by_provider_total",
            "LLM requests by provider",
            "provider",
            provider,
            *count,
        );
    }
    for (provider, tokens) in &s.llm.tokens_by_provider {
        prom_counter_label(
            &mut out,
            "clawdius_llm_tokens_by_provider_total",
            "Tokens by provider",
            "provider",
            provider,
            *tokens,
        );
    }
    for (provider, count) in &s.llm.errors_by_provider {
        prom_counter_label(
            &mut out,
            "clawdius_llm_errors_by_provider_total",
            "LLM errors by provider",
            "provider",
            provider,
            *count,
        );
    }

    prom_counter(
        &mut out,
        "clawdius_tool_invocations_total",
        "Total tool invocations",
        s.tools.total_invocations,
    );
    prom_counter(
        &mut out,
        "clawdius_tool_successful_invocations_total",
        "Successful tool invocations",
        s.tools.successful_invocations,
    );
    prom_counter(
        &mut out,
        "clawdius_tool_failed_invocations_total",
        "Failed tool invocations",
        s.tools.failed_invocations,
    );
    prom_gauge(
        &mut out,
        "clawdius_tool_avg_duration_ms",
        "Average tool duration in milliseconds",
        s.tools.avg_duration_ms,
    );

    for (tool, count) in &s.tools.invocations_by_tool {
        prom_counter_label(
            &mut out,
            "clawdius_tool_invocations_by_tool_total",
            "Tool invocations by tool",
            "tool",
            tool,
            *count,
        );
    }
    for (tool, count) in &s.tools.errors_by_tool {
        prom_counter_label(
            &mut out,
            "clawdius_tool_errors_by_tool_total",
            "Tool errors by tool",
            "tool",
            tool,
            *count,
        );
    }
    for (tool, dur) in &s.tools.avg_duration_by_tool {
        prom_gauge_label(
            &mut out,
            "clawdius_tool_avg_duration_by_tool_ms",
            "Average tool duration by tool",
            "tool",
            tool,
            *dur,
        );
    }

    prom_gauge(
        &mut out,
        "clawdius_sessions_total",
        "Total sessions",
        s.sessions.total_sessions as f64,
    );
    prom_gauge(
        &mut out,
        "clawdius_sessions_active",
        "Currently active sessions",
        s.sessions.active_sessions as f64,
    );
    prom_gauge(
        &mut out,
        "clawdius_sessions_messages_total",
        "Total messages across sessions",
        s.sessions.total_messages as f64,
    );
    prom_gauge(
        &mut out,
        "clawdius_sessions_active",
        "Currently active sessions",
        s.sessions.active_sessions as f64,
    );
    prom_gauge(
        &mut out,
        "clawdius_sessions_messages_total",
        "Total messages across sessions",
        s.sessions.total_messages as f64,
    );
    prom_gauge(
        &mut out,
        "clawdius_sessions_avg_messages_per_session",
        "Average messages per session",
        s.sessions.avg_messages_per_session,
    );
    prom_gauge(
        &mut out,
        "clawdius_sessions_avg_duration_secs",
        "Average session duration in seconds",
        s.sessions.avg_session_duration_secs,
    );

    for (mode, count) in &s.sessions.sessions_by_mode {
        prom_gauge_label(
            &mut out,
            "clawdius_sessions_by_mode",
            "Sessions by mode",
            "mode",
            mode,
            *count as f64,
        );
    }

    prom_gauge(
        &mut out,
        "clawdius_performance_memory_usage_mb",
        "Memory usage in megabytes",
        s.performance.memory_usage_mb,
    );
    prom_gauge(
        &mut out,
        "clawdius_performance_cpu_usage_percent",
        "CPU usage percentage",
        s.performance.cpu_usage_percent,
    );
    prom_gauge(
        &mut out,
        "clawdius_performance_disk_io_read_mb",
        "Disk IO read in megabytes",
        s.performance.disk_io_read_mb,
    );
    prom_gauge(
        &mut out,
        "clawdius_performance_disk_io_write_mb",
        "Disk IO write in megabytes",
        s.performance.disk_io_write_mb,
    );
    prom_gauge(
        &mut out,
        "clawdius_performance_network_rx_mb",
        "Network received in megabytes",
        s.performance.network_rx_mb,
    );
    prom_gauge(
        &mut out,
        "clawdius_performance_network_tx_mb",
        "Network transmitted in megabytes",
        s.performance.network_tx_mb,
    );

    prom_counter(
        &mut out,
        "clawdius_errors_total",
        "Total errors",
        s.errors.total_errors,
    );
    prom_gauge(
        &mut out,
        "clawdius_errors_last_hour",
        "Errors in the last hour",
        s.errors.errors_last_hour as f64,
    );
    prom_gauge(
        &mut out,
        "clawdius_errors_last_day",
        "Errors in the last day",
        s.errors.errors_last_day as f64,
    );

    for (error_type, count) in &s.errors.errors_by_type {
        prom_counter_label(
            &mut out,
            "clawdius_errors_by_type_total",
            "Errors by type",
            "error_type",
            error_type,
            *count,
        );
    }

    out
}

fn prom_counter(out: &mut String, name: &str, help: &str, value: u64) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push_str(" counter\n");
    out.push_str(name);
    out.push(' ');
    out.push_str(&value.to_string());
    out.push_str("\n\n");
}

fn prom_gauge(out: &mut String, name: &str, help: &str, value: f64) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push_str(" gauge\n");
    out.push_str(name);
    out.push(' ');
    out.push_str(&format!("{value}"));
    out.push_str("\n\n");
}

fn prom_counter_label(
    out: &mut String,
    name: &str,
    help: &str,
    label_key: &str,
    label_val: &str,
    value: u64,
) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push_str(" counter\n");
    out.push_str(name);
    out.push_str(&format!("{{{label_key}=\"{label_val}\"}} {value}"));
    out.push_str("\n\n");
}

fn prom_gauge_label(
    out: &mut String,
    name: &str,
    help: &str,
    label_key: &str,
    label_val: &str,
    value: f64,
) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push_str(" gauge\n");
    out.push_str(name);
    out.push_str(&format!("{{{label_key}=\"{label_val}\"}} {value}"));
    out.push_str("\n\n");
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::api::rest::create_router;
    use crate::api::rest::ApiState;
    use crate::session::SessionStore;
    use crate::telemetry::MetricsDashboard;

    fn make_router() -> axum::Router {
        let store = SessionStore::in_memory().unwrap();
        let state = ApiState::new(store);
        create_router(state)
    }

    #[tokio::test]
    async fn test_metrics_returns_200() {
        let app = make_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_contains_prometheus_labels() {
        let app = make_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();

        assert!(text.contains("# HELP clawdius_llm_requests_total"));
        assert!(text.contains("# TYPE clawdius_llm_requests_total counter"));
        assert!(text.contains("# HELP clawdius_sessions_active"));
        assert!(text.contains("# TYPE clawdius_sessions_active gauge"));
        assert!(text.contains("# HELP clawdius_tool_invocations_total"));
        assert!(text.contains("# HELP clawdius_errors_total"));
        assert!(text.contains("# HELP clawdius_performance_memory_usage_mb"));
    }

    #[tokio::test]
    async fn test_metrics_content_type() {
        let app = make_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let ct = resp.headers().get("content-type").unwrap();
        assert_eq!(ct, "text/plain; version=0.0.4; charset=utf-8");
    }

    #[test]
    fn test_format_prometheus_output() {
        let dashboard = MetricsDashboard::new();
        let snapshot = dashboard.comprehensive_snapshot();
        let output = super::format_prometheus(&snapshot);

        assert!(output.contains("clawdius_llm_requests_total "));
        assert!(output.contains("clawdius_sessions_active "));
        assert!(output.contains("clawdius_performance_memory_usage_mb "));
    }
}
