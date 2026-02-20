# P0: Telemetry + MCP HTTP/SSE Transport

## P0-A: OpenTelemetry Export (telemetry)

Current state: `telemetry.rs` has span constants, cost tracking, tool metrics via `tracing`.
Missing: OTel subscriber setup so traces/metrics export to OTLP backends (Jaeger, Grafana, etc.)

### Approach
Add optional `telemetry` feature flag with OTel dependencies. Keep core zero-cost when disabled.

1. Add `Cargo.toml` feature: `telemetry = ["opentelemetry", "opentelemetry-otlp", "opentelemetry_sdk", "tracing-opentelemetry"]`
2. New file: `core/src/telemetry_otel.rs` — OTel init/shutdown behind feature gate
   - `init_telemetry(endpoint, service_name)` → sets up OTLP exporter + tracing subscriber
   - `shutdown_telemetry()` → flush and shutdown
   - Metrics: LLM token counter, tool call counter, tool duration histogram
3. Add `SessionOptions::with_telemetry(endpoint)` builder
4. Wire into `AgentSession` — init on create, shutdown on drop
5. Tests (feature-gated)

## P0-B: MCP HTTP+SSE Transport

Current state: `McpTransport` trait + `StdioTransport` impl.
Missing: HTTP+SSE transport for remote MCP servers.

### Approach
Implement `HttpSseTransport` using existing `reqwest` dependency.

1. New file: `core/src/mcp/transport/http_sse.rs`
   - `HttpSseTransport::connect(url, headers?)` — connect to SSE endpoint
   - Requests: POST JSON-RPC to server URL
   - Notifications: received via SSE stream
   - Reconnection with backoff on disconnect
2. Update `core/src/mcp/transport/mod.rs` — add `pub mod http_sse`
3. Update MCP config to support `transport = "http_sse"` with `url` field
4. Tests with mock server
