# Performance SLA

## Response Time Guarantees

| Operation | P50 | P95 | P99 |
|-----------|-----|-----|-----|
| CLI chat | <500ms | <1s | <2s |
| TUI render | <16ms | <33ms | <50ms |
| File read | <10ms | <50ms | <100ms |
| Symbol search | <50ms | <200ms | <500ms |

## Resource Limits

| Resource | Limit |
|----------|-------|
| Memory (idle) | <100MB |
| Memory (active) | <500MB |
| Binary size | <15MB |
| Startup time | <500ms |
