# A3S Event

<p align="center">
  <strong>Pluggable Event System for A3S</strong>
</p>

<p align="center">
  <em>Provider-agnostic event publish, subscribe, and persistence — swap backends without changing application code</em>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#providers">Providers</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#api-reference">API Reference</a> •
  <a href="#custom-providers">Custom Providers</a> •
  <a href="#development">Development</a>
</p>

---

## Overview

**A3S Event** provides a provider-agnostic API for event subscription, dispatch, and persistence across the A3S ecosystem. All backends implement the `EventProvider` trait, so you can swap between NATS JetStream, in-memory, or any custom provider without changing application code.

### Basic Usage

```rust
use a3s_event::{EventBus, Event};
use a3s_event::provider::memory::MemoryProvider;

#[tokio::main]
async fn main() -> a3s_event::Result<()> {
    // Create an event bus with any provider
    let bus = EventBus::new(MemoryProvider::default());

    // Publish an event
    let event = bus.publish(
        "market",
        "forex.usd_cny",
        "USD/CNY broke through 7.35",
        "reuters",
        serde_json::json!({"rate": 7.3521}),
    ).await?;

    println!("Published: {}", event.id);

    // Query history
    let events = bus.list_events(Some("market"), 50).await?;
    println!("Market events: {}", events.len());

    Ok(())
}
```

## Features

- **Provider-Agnostic API**: `EventProvider` trait abstracts all backends — publish, subscribe, query with a single interface
- **Pluggable Backends**: Swap providers (NATS, in-memory, Redis, Kafka, etc.) without changing application code
- **Publish/Subscribe**: Dot-separated subject hierarchy (`events.<category>.<topic>`)
- **Durable Subscriptions**: Consumers survive disconnects and server restarts (provider-dependent)
- **At-Least-Once Delivery**: Explicit ack/nak via `PendingEvent` with automatic redelivery on failure
- **Event History**: Query past events with subject filtering from any provider
- **High-Level EventBus**: Wraps any provider with subscription management and convenience methods
- **Manual or Auto Ack**: Choose between auto-ack or manual ack for precise delivery control
- **Category-Based Routing**: Subscribe to all events in a category with wildcard subjects (`events.market.>`)
- **In-Memory Provider**: Zero-dependency provider for testing and single-process deployments
- **NATS JetStream Provider**: Distributed, persistent event streaming with configurable retention
- **Payload Encryption**: AES-256-GCM encrypt/decrypt with key rotation — protect sensitive payloads at the application layer
- **State Persistence**: Subscription filters survive restarts via pluggable `StateStore` (JSON file or custom)

## Providers

| Provider | Use Case | Persistence | Distribution |
|----------|----------|-------------|--------------|
| `MemoryProvider` | Testing, development, single-process | In-process only | Single process |
| `NatsProvider` | Production, multi-service | JetStream (file/memory) | Distributed |

### Memory Provider

Zero-dependency, in-process event bus using `tokio::sync::broadcast`. Events are lost on restart.

```rust
use a3s_event::provider::memory::{MemoryProvider, MemoryConfig};

let provider = MemoryProvider::new(MemoryConfig {
    subject_prefix: "events".to_string(),
    max_events: 100_000,
    channel_capacity: 10_000,
});

// Or use defaults
let provider = MemoryProvider::default();
```

### NATS JetStream Provider

Distributed event streaming with persistent storage, durable consumers, and at-least-once delivery.

```rust
use a3s_event::provider::nats::{NatsProvider, NatsConfig, StorageType};

let provider = NatsProvider::connect(NatsConfig {
    url: "nats://127.0.0.1:4222".to_string(),
    stream_name: "A3S_EVENTS".to_string(),
    subject_prefix: "events".to_string(),
    storage: StorageType::File,
    max_events: 100_000,
    max_age_secs: 604_800,  // 7 days
    ..Default::default()
}).await?;
```

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                        EventBus                             │
│  High-level API: publish, subscribe, history, manage subs   │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              dyn EventProvider                        │  │
│  │  publish() | subscribe() | history() | info()        │  │
│  └───────────────────────────────────────────────────────┘  │
│         │                │                │                  │
│  ┌──────┴──────┐  ┌──────┴──────┐  ┌──────┴──────┐        │
│  │   Memory    │  │    NATS     │  │   Custom    │        │
│  │  Provider   │  │  Provider   │  │  Provider   │        │
│  │ (broadcast) │  │ (JetStream) │  │ (your impl) │        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────┘
        ▲                │               │
        │ publish        │ subscribe     │ subscribe
┌───────┴───────┐  ┌─────▼─────┐  ┌─────▼─────┐
│   SafeClaw    │  │  Persona   │  │  Gateway   │
│   Backend     │  │  Agent     │  │  Monitor   │
└───────────────┘  └───────────┘  └───────────┘
```

### Subject Hierarchy

Events follow a dot-separated naming convention:

```
events.<category>.<topic>[.<subtopic>...]

Examples:
  events.market.forex.usd_cny     — forex rate change
  events.system.deploy.gateway    — service deployment
  events.task.completed           — task completion
  events.compliance.audit.login   — audit event
```

Wildcard patterns:
- `events.market.>` — all market events (any depth)
- `events.*.forex` — forex events from any category

### Core Types

| Type | Description |
|------|-------------|
| `EventProvider` | Core trait — all backends implement this |
| `Subscription` | Async event stream from any provider |
| `PendingEvent` | Event with ack/nak callbacks for manual acknowledgement |
| `EventBus` | High-level API with subscription management |
| `Event` | Provider-agnostic message envelope (id, subject, category, payload) |
| `ReceivedEvent` | Event with delivery context (sequence, num_delivered, stream) |
| `ProviderInfo` | Backend status (message count, bytes, consumers) |
| `EventEncryptor` | Trait for payload encrypt/decrypt |
| `Aes256GcmEncryptor` | AES-256-GCM encryptor with key rotation |
| `EncryptedPayload` | Encrypted envelope (key_id, nonce, ciphertext) |
| `StateStore` | Trait for persisting subscription state |
| `FileStateStore` | JSON file-based state persistence |

## API Reference

### EventBus

```rust
use a3s_event::{EventBus, SubscriptionFilter};
use a3s_event::provider::memory::MemoryProvider;

// Create bus with any provider
let bus = EventBus::new(MemoryProvider::default());

// Publish with convenience parameters
let event = bus.publish("market", "forex", "Rate change", "reuters", payload).await?;

// Publish a pre-built event
let seq = bus.publish_event(&event).await?;

// List events (optionally filtered by category)
let events = bus.list_events(Some("market"), 50).await?;

// Get event counts by category
let counts = bus.counts(1000).await?;

// Manage subscriptions
bus.update_subscription(SubscriptionFilter {
    subscriber_id: "analyst".to_string(),
    subjects: vec!["events.market.>".to_string()],
    durable: true,
}).await?;

let subs = bus.create_subscriber("analyst").await?;
bus.remove_subscription("analyst").await?;

// Provider info
let info = bus.info().await?;
println!("{}: {} messages", info.provider, info.messages);
```

### EventProvider Trait

```rust
use a3s_event::provider::EventProvider;

// All providers implement:
provider.publish(&event).await?;
provider.subscribe("events.market.>").await?;
provider.subscribe_durable("consumer-1", "events.market.>").await?;
provider.history(Some("events.market.>"), 100).await?;
provider.unsubscribe("consumer-1").await?;
provider.info().await?;
provider.build_subject("market", "forex.usd");  // → "events.market.forex.usd"
provider.category_subject("market");             // → "events.market.>"
provider.name();                                 // → "memory" | "nats"
```

### Subscription

```rust
// Auto-ack mode
let mut sub = provider.subscribe("events.>").await?;
while let Some(received) = sub.next().await? {
    println!("{}: {}", received.event.id, received.event.summary);
}

// Manual ack mode
while let Some(pending) = sub.next_manual_ack().await? {
    match process(&pending.received.event) {
        Ok(_) => pending.ack().await?,
        Err(_) => pending.nak().await?,  // request redelivery
    }
}
```

### NATS Configuration

```rust
let config = NatsConfig {
    url: "nats://127.0.0.1:4222".to_string(),
    token: None,
    credentials_path: None,
    stream_name: "A3S_EVENTS".to_string(),
    subject_prefix: "events".to_string(),
    storage: StorageType::File,
    max_events: 100_000,
    max_age_secs: 604_800,  // 7 days
    max_bytes: 0,            // unlimited
    connect_timeout_secs: 5,
    request_timeout_secs: 10,
};
```

## Custom Providers

Implement `EventProvider` and `Subscription` to add any backend:

```rust
use a3s_event::provider::{EventProvider, Subscription, PendingEvent, ProviderInfo};
use a3s_event::types::{Event, ReceivedEvent};
use a3s_event::Result;
use async_trait::async_trait;

pub struct RedisProvider { /* ... */ }

#[async_trait]
impl EventProvider for RedisProvider {
    async fn publish(&self, event: &Event) -> Result<u64> {
        // Publish to Redis Streams
        todo!()
    }

    async fn subscribe_durable(
        &self,
        consumer_name: &str,
        filter_subject: &str,
    ) -> Result<Box<dyn Subscription>> {
        // Create Redis consumer group
        todo!()
    }

    async fn subscribe(&self, filter_subject: &str) -> Result<Box<dyn Subscription>> {
        todo!()
    }

    async fn history(
        &self,
        filter_subject: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Event>> {
        todo!()
    }

    async fn unsubscribe(&self, consumer_name: &str) -> Result<()> {
        todo!()
    }

    async fn info(&self) -> Result<ProviderInfo> {
        todo!()
    }

    fn build_subject(&self, category: &str, topic: &str) -> String {
        format!("events.{}.{}", category, topic)
    }

    fn category_subject(&self, category: &str) -> String {
        format!("events.{}.>", category)
    }

    fn name(&self) -> &str {
        "redis"
    }
}
```

Then use it like any other provider:

```rust
let bus = EventBus::new(RedisProvider::new(config));
bus.publish("market", "forex", "Rate change", "source", payload).await?;
```

## Development

### Prerequisites

- Rust 1.75+
- NATS Server with JetStream enabled (for NATS provider tests: `nats-server -js`)
- `cargo-llvm-cov` for coverage (`cargo install cargo-llvm-cov`)
- `lcov` for coverage reports (`brew install lcov`)

### Commands

```bash
just build              # Build the project
just test               # Run all tests with progress display
just test-v             # Run tests with verbose output
just test-one NAME      # Run a specific test
just test-integration   # NATS integration tests (requires nats-server -js)
just bench              # Run performance benchmarks
just bench-one NAME     # Run specific benchmark
just test-cov           # Run tests with coverage report
just cov                # Coverage with lcov summary
just cov-html           # Coverage with HTML report (opens browser)
just cov-table          # Coverage with file-by-file table
just cov-ci             # Generate lcov.info for CI
just lint               # Run clippy
just fmt                # Format code
just ci                 # Full CI check (fmt + lint + test)
just doc                # Generate and open docs
```

### Test Modules

| Module | Description |
|--------|-------------|
| `types` | Event creation, serialization, metadata |
| `error` | Error type construction and display |
| `schema` | Schema registry, validation, compatibility |
| `dlq` | Dead letter queue handler |
| `provider::memory` | In-memory provider: publish, subscribe, history, wildcards |
| `provider::nats` | NATS provider: client, config, subscriber (requires NATS) |
| `store` | EventBus high-level operations |
| `nats_integration` | End-to-end NATS tests: publish, dedup, durable sub, manual ack |

### Running Tests

```bash
# Unit tests (no external dependencies)
just test

# NATS integration tests (requires running NATS server)
nats-server -js
just test-integration

# Performance benchmarks
just bench

# Test specific modules
just test-memory     # In-memory provider tests
just test-nats       # NATS provider tests
just test-types      # Event type tests
just test-store      # EventBus tests
```

## Roadmap

A3S Event is the application-level event abstraction. It does NOT re-implement capabilities that providers (NATS, Kafka, etc.) already offer natively. The roadmap focuses on what only the abstraction layer should own.

### Responsibility Boundary

| Capability | Owner | Notes |
|------------|-------|-------|
| Retry / backoff | **Provider** | NATS: `MaxDeliver` + `BackOff`. Kafka: consumer retry topic. |
| Backpressure | **Provider** | NATS: pull consumer + `MaxAckPending`. Kafka: consumer poll. |
| Connection resilience | **Provider** | NATS: async-nats auto-reconnect. Kafka: librdkafka reconnect. |
| Consumer lag monitoring | **Provider** | NATS: `consumer.info().num_pending`. Kafka: consumer group lag. |
| Event replay by timestamp | **Provider** | NATS: `DeliverPolicy::ByStartTime`. Kafka: `offsetsForTimes`. |
| Exactly-once delivery | **Provider** | NATS: `Nats-Msg-Id` dedup + double ack. Kafka: idempotent producer + transactions. |
| Partitioning / sharding | **Provider** | NATS: subject-based routing. Kafka: partition key. |
| Stream mirroring | **Provider** | NATS: Mirror/Source config. Kafka: MirrorMaker. |
| Metrics (server-side) | **Provider** | NATS: `/metrics` endpoint. Kafka: JMX metrics. |
| Transport encryption | **Provider** | NATS/Kafka: TLS configuration. |
| Event versioning / schema | **A3S Event** | Provider-agnostic, application-level concern. |
| Payload encryption | **A3S Event** | Application-level encrypt/decrypt before publish. |
| Dead letter queue | **A3S Event** | Unified DLQ abstraction across providers. |
| EventBus state persistence | **A3S Event** | Subscription filter durability across restarts. |
| Observability integration | **A3S Event** | Bridge provider metrics into app-level tracing/metrics. |
| Provider config passthrough | **A3S Event** | Expose provider-native knobs (MaxDeliver, BackOff, etc.) |
| Integration tests | **A3S Event** | End-to-end verification with real providers. |

### Phase 1: Provider Config Passthrough ✅

Expose provider-native capabilities through the abstraction layer without re-implementing them.

- [x] `SubscribeOptions` struct — `max_deliver`, `backoff`, `max_ack_pending`, `deliver_policy`, `ack_wait`
- [x] `PublishOptions` struct — `msg_id` (dedup), `expected_sequence`, `timeout`
- [x] `DeliverPolicy` enum — `All`, `Last`, `New`, `ByStartSequence`, `ByStartTime`, `LastPerSubject`
- [x] `EventProvider` trait extended with `publish_with_options()`, `subscribe_with_options()`, `subscribe_durable_with_options()` (default impls for backward compatibility)
- [x] NatsProvider maps options to JetStream consumer/publish config (headers, backoff, max_deliver, deliver_policy, etc.)
- [x] MemoryProvider uses default impls (ignores unsupported options gracefully)
- [x] `SubscriptionFilter` carries optional `SubscribeOptions`
- [x] `EventBus` threads options through `create_subscriber()`

### Phase 2: Event Versioning & Schema ✅

Application-level schema management that no provider handles.

- [x] Add `event_type` and `version` fields to `Event` struct (backward-compatible defaults)
- [x] `Event::typed()` constructor for versioned events
- [x] `SchemaRegistry` trait — register, validate, query schemas
- [x] `MemorySchemaRegistry` — in-memory registry for development
- [x] `EventSchema` — required fields validation per event type + version
- [x] Publish-time validation (optional, via `EventBus::with_schema_registry()`)
- [x] `Compatibility` enum — Backward, Forward, Full, None
- [x] Schema evolution checks (`check_compatibility()`) between versions

### Phase 3: Operational Hardening ✅

Production reliability features that live above the provider layer.

- [x] Dead Letter Queue — `DlqHandler` trait + `MemoryDlqHandler` impl
- [x] `DeadLetterEvent` with reason and timestamp, `should_dead_letter()` helper
- [x] `EventBus::set_dlq_handler()` for DLQ integration
- [x] Observability — `tracing::info_span!` on publish and subscribe lifecycle in EventBus
- [x] Health check API — `EventProvider::health()` with default impl, `EventBus::health()`

### Phase 4: Testing & Documentation ✅

Confidence and onboarding.

- [x] Integration tests with real NATS (9 tests — publish, history, dedup, durable subscription, options, concurrent, manual ack, health, info)
- [x] EventBus unit tests (publish, subscribe, lifecycle, schema validation, DLQ integration — 17 tests)
- [x] Concurrent publish/subscribe stress tests (50 concurrent publishers)
- [x] Error path tests (all error variants display, From conversion, not-found, schema validation — 7 tests)
- [x] Performance benchmarks (`criterion` — event creation, serialization, publish throughput, history query)
- [x] Deployment guide and configuration reference (`docs/deployment.md`)
- [x] Provider implementation guide (`docs/custom-providers.md`)

**Test summary: 106 unit tests + 9 integration tests across 9 modules**

### Phase 5: Payload Encryption ✅

Application-level encrypt/decrypt for sensitive event payloads.

- [x] `EventEncryptor` trait — `encrypt(payload) → Value`, `decrypt(encrypted) → Value`
- [x] `Aes256GcmEncryptor` — AES-256-GCM with random nonce per message
- [x] `EncryptedPayload` envelope — key_id, nonce, ciphertext (base64), encrypted marker
- [x] Key rotation — `add_key()`, `rotate_to()`, decrypt with any registered key
- [x] `EventBus::set_encryptor()` — transparent encrypt on publish, decrypt on `list_events()`
- [x] `EncryptedPayload::is_encrypted()` — detect encrypted payloads for selective decryption
- [x] Schema validation runs on plaintext before encryption
- [x] 10 crypto tests + 4 EventBus encryption integration tests

### Phase 6: EventBus State Persistence ✅

Subscription filter durability across restarts.

- [x] `StateStore` trait — save/load subscription filters
- [x] `FileStateStore` — JSON file persistence with atomic writes (temp + rename)
- [x] `MemoryStateStore` — in-memory store for testing
- [x] `EventBus::set_state_store()` — auto-loads persisted subscriptions on setup
- [x] Auto-save on `update_subscription()` and `remove_subscription()`
- [x] 7 state store tests + 5 EventBus persistence integration tests

### Phase 7: Observability Integration 🚧

Bridge provider metrics into application-level tracing/metrics.

- [ ] `EventMetrics` struct — publish count, error count, latency histogram, DLQ depth
- [ ] `EventBus` emits metrics on publish/subscribe/error
- [ ] `metrics()` accessor for scraping
- [ ] Integration with `tracing` and `metrics` crates

## License

MIT License — see [LICENSE](LICENSE) for details.
