# Deployment Guide

## Prerequisites

- Rust 1.75+
- NATS Server 2.10+ with JetStream enabled (for production)

## Provider Selection

| Scenario | Provider | Notes |
|----------|----------|-------|
| Unit tests, CI | `MemoryProvider` | Zero dependencies, in-process |
| Single service, dev | `MemoryProvider` | Fast, no setup required |
| Multi-service, staging | `NatsProvider` | Requires NATS server |
| Production | `NatsProvider` | Persistent, distributed |

## NATS Server Setup

### Local Development

```bash
# Install
brew install nats-server    # macOS
# or download from https://nats.io/download/

# Start with JetStream
nats-server -js
```

### Docker

```bash
docker run -d --name nats \
  -p 4222:4222 \
  -p 8222:8222 \
  nats:latest -js
```

### Docker Compose

```yaml
services:
  nats:
    image: nats:latest
    command: ["-js", "-m", "8222"]
    ports:
      - "4222:4222"   # Client
      - "8222:8222"   # Monitoring
    volumes:
      - nats-data:/data
    restart: unless-stopped

volumes:
  nats-data:
```

### Production NATS

For production deployments, consider:

- **Clustering**: 3+ node cluster for high availability
- **TLS**: Enable TLS for all client and cluster connections
- **Authentication**: Use token or NKey authentication
- **Storage**: File-based storage with adequate disk space
- **Monitoring**: Enable the HTTP monitoring port (8222)

```bash
# nats-server.conf
jetstream {
  store_dir: /data/jetstream
  max_mem: 1G
  max_file: 10G
}

cluster {
  name: a3s-events
  listen: 0.0.0.0:6222
  routes: [
    nats-route://node1:6222
    nats-route://node2:6222
    nats-route://node3:6222
  ]
}

tls {
  cert_file: /etc/nats/server-cert.pem
  key_file: /etc/nats/server-key.pem
  ca_file: /etc/nats/ca.pem
}
```

## Configuration Reference

### NatsConfig

```rust
use a3s_event::provider::nats::{NatsConfig, StorageType};

let config = NatsConfig {
    // NATS server URL
    url: "nats://127.0.0.1:4222".to_string(),

    // Authentication token (optional)
    token: None,

    // Path to credentials file (optional)
    credentials_path: None,

    // JetStream stream name
    stream_name: "A3S_EVENTS".to_string(),

    // Subject prefix for all events
    subject_prefix: "events".to_string(),

    // Storage backend: File (persistent) or Memory (fast, volatile)
    storage: StorageType::File,

    // Maximum number of events to retain
    max_events: 100_000,

    // Maximum age of events in seconds (0 = unlimited)
    max_age_secs: 604_800,  // 7 days

    // Maximum bytes for the stream (0 = unlimited)
    max_bytes: 0,

    // Connection timeout
    connect_timeout_secs: 5,

    // Request timeout for JetStream operations
    request_timeout_secs: 10,
};
```

### MemoryConfig

```rust
use a3s_event::provider::memory::MemoryConfig;

let config = MemoryConfig {
    // Subject prefix (default: "events")
    subject_prefix: "events".to_string(),

    // Maximum events to retain in memory (default: 100_000)
    max_events: 100_000,

    // Broadcast channel capacity (default: 10_000)
    channel_capacity: 10_000,
};
```

### PublishOptions

```rust
use a3s_event::PublishOptions;

let opts = PublishOptions {
    // Message ID for deduplication (NATS: Nats-Msg-Id header)
    msg_id: Some("unique-id-123".to_string()),

    // Expected last sequence for optimistic concurrency
    expected_sequence: None,

    // Publish ack timeout in seconds
    timeout_secs: Some(5),
};
```

### SubscribeOptions

```rust
use a3s_event::{SubscribeOptions, DeliverPolicy};

let opts = SubscribeOptions {
    // Maximum delivery attempts before giving up (0 = unlimited)
    max_deliver: Some(5),

    // Backoff intervals between redeliveries
    backoff_secs: vec![1, 5, 30],

    // Maximum unacknowledged messages (0 = unlimited)
    max_ack_pending: Some(1000),

    // Where to start consuming
    deliver_policy: DeliverPolicy::New,

    // Time to wait for ack before redelivery
    ack_wait_secs: Some(30),
};
```

## Operational Recommendations

### Stream Sizing

| Workload | max_events | max_age_secs | max_bytes | storage |
|----------|-----------|--------------|-----------|---------|
| Dev/test | 10,000 | 3,600 (1h) | 0 | Memory |
| Staging | 100,000 | 86,400 (1d) | 1 GB | File |
| Production | 1,000,000 | 604,800 (7d) | 10 GB | File |

### Consumer Configuration

- **At-least-once**: Use `max_deliver: 5` with `backoff_secs: [1, 5, 30, 120]`
- **Low latency**: Use `max_ack_pending: 1000` with `ack_wait_secs: 10`
- **Replay**: Use `DeliverPolicy::ByStartTime` or `ByStartSequence`

### Dead Letter Queue

Configure DLQ for events that exceed max delivery attempts:

```rust
use a3s_event::{EventBus, MemoryDlqHandler};
use std::sync::Arc;

let dlq = Arc::new(MemoryDlqHandler::default());
let mut bus = EventBus::new(provider);
bus.set_dlq_handler(dlq.clone());

// Check DLQ periodically
let count = dlq.count().await?;
let failed = dlq.list(0, 100).await?;
```

### Health Checks

```rust
// Liveness probe
let healthy = bus.health().await?;

// Readiness probe (includes provider info)
let info = bus.info().await?;
println!("Provider: {}, Messages: {}", info.provider, info.messages);
```

### Monitoring

NATS exposes metrics at `http://localhost:8222/`:

- `/varz` — server info
- `/jsz` — JetStream info
- `/connz` — connections
- `/subsz` — subscriptions

A3S Event adds tracing spans on publish and subscribe operations. Integrate with your tracing subscriber (e.g., `tracing-opentelemetry`) for distributed tracing.
