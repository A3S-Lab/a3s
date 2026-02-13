# Custom Provider Guide

This guide walks through implementing a custom `EventProvider` for A3S Event. By implementing the provider trait, your backend integrates seamlessly with `EventBus` and all higher-level features (schema validation, DLQ, tracing).

## Architecture

```text
EventBus
  └── dyn EventProvider        ← you implement this
        ├── publish()
        ├── subscribe() / subscribe_durable()
        ├── history()
        ├── unsubscribe()
        ├── info()
        └── health()
```

## Step 1: Implement EventProvider

```rust
use a3s_event::provider::{EventProvider, Subscription, PendingEvent, ProviderInfo};
use a3s_event::types::{Event, ReceivedEvent, PublishOptions, SubscribeOptions};
use a3s_event::error::{EventError, Result};
use async_trait::async_trait;

pub struct RedisProvider {
    client: redis::Client,
    stream_key: String,
    prefix: String,
}

#[async_trait]
impl EventProvider for RedisProvider {
    /// Publish an event. Return a sequence number (or monotonic counter).
    async fn publish(&self, event: &Event) -> Result<u64> {
        let payload = serde_json::to_string(event)?;
        let id: String = redis::cmd("XADD")
            .arg(&self.stream_key)
            .arg("*")
            .arg("data")
            .arg(&payload)
            .query_async(&mut self.client.get_async_connection().await
                .map_err(|e| EventError::Connection(e.to_string()))?)
            .await
            .map_err(|e| EventError::Publish {
                subject: event.subject.clone(),
                reason: e.to_string(),
            })?;

        // Parse Redis stream ID "timestamp-seq" → sequence
        let seq: u64 = id.split('-').last()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        Ok(seq)
    }

    /// Create a durable subscription (e.g., Redis consumer group).
    async fn subscribe_durable(
        &self,
        consumer_name: &str,
        filter_subject: &str,
    ) -> Result<Box<dyn Subscription>> {
        // Create consumer group if not exists
        // Return a RedisSubscription that reads from the group
        todo!()
    }

    /// Create an ephemeral subscription.
    async fn subscribe(
        &self,
        filter_subject: &str,
    ) -> Result<Box<dyn Subscription>> {
        // Use XREAD without consumer group
        todo!()
    }

    /// Fetch historical events.
    async fn history(
        &self,
        filter_subject: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Event>> {
        // Use XREVRANGE to fetch recent events
        // Filter by subject if provided
        todo!()
    }

    /// Delete a consumer group / subscription.
    async fn unsubscribe(&self, consumer_name: &str) -> Result<()> {
        // XGROUP DESTROY
        todo!()
    }

    /// Return provider status.
    async fn info(&self) -> Result<ProviderInfo> {
        // XINFO STREAM
        Ok(ProviderInfo {
            provider: self.name().to_string(),
            messages: 0,  // from XINFO
            bytes: 0,
            consumers: 0,
        })
    }

    /// Build a full subject from category and topic.
    fn build_subject(&self, category: &str, topic: &str) -> String {
        format!("{}.{}.{}", self.prefix, category, topic)
    }

    /// Build a wildcard subject for a category.
    fn category_subject(&self, category: &str) -> String {
        format!("{}.{}.>", self.prefix, category)
    }

    /// Provider name for logging and identification.
    fn name(&self) -> &str {
        "redis"
    }
}
```

## Step 2: Implement Subscription

```rust
use a3s_event::provider::{Subscription, PendingEvent};
use a3s_event::types::ReceivedEvent;
use a3s_event::error::Result;
use async_trait::async_trait;

pub struct RedisSubscription {
    // Your consumer state
}

#[async_trait]
impl Subscription for RedisSubscription {
    /// Auto-ack mode: return the next event.
    async fn next(&mut self) -> Result<Option<ReceivedEvent>> {
        // Read next message from stream
        // Acknowledge immediately
        // Return ReceivedEvent { event, sequence, num_delivered, stream }
        todo!()
    }

    /// Manual ack mode: return a PendingEvent with ack/nak callbacks.
    async fn next_manual_ack(&mut self) -> Result<Option<PendingEvent>> {
        // Read next message WITHOUT acknowledging
        // Create PendingEvent with closures that XACK or re-queue
        //
        // Example:
        // let msg_id = ...;
        // let client = self.client.clone();
        // let stream = self.stream_key.clone();
        // let group = self.group.clone();
        //
        // PendingEvent::new(
        //     received,
        //     move || Box::pin(async move {
        //         // XACK
        //         Ok(())
        //     }),
        //     move || Box::pin(async move {
        //         // XCLAIM or re-add to stream
        //         Ok(())
        //     }),
        // )
        todo!()
    }
}
```

## Step 3: Optional — Override Options Methods

The `EventProvider` trait provides default implementations for `_with_options` methods that ignore options and delegate to the base methods. Override them if your backend supports the features:

```rust
#[async_trait]
impl EventProvider for RedisProvider {
    // ... required methods above ...

    /// Override if your backend supports dedup or expected sequence.
    async fn publish_with_options(
        &self,
        event: &Event,
        opts: &PublishOptions,
    ) -> Result<u64> {
        if let Some(ref msg_id) = opts.msg_id {
            // Check for duplicate msg_id before publishing
        }
        self.publish(event).await
    }

    /// Override if your backend supports max_deliver, backoff, etc.
    async fn subscribe_durable_with_options(
        &self,
        consumer_name: &str,
        filter_subject: &str,
        opts: &SubscribeOptions,
    ) -> Result<Box<dyn Subscription>> {
        // Use opts.max_deliver, opts.backoff_secs, etc.
        self.subscribe_durable(consumer_name, filter_subject).await
    }

    /// Override for custom health checks.
    async fn health(&self) -> Result<bool> {
        // PING the Redis server
        Ok(true)
    }
}
```

## Step 4: Use It

```rust
use a3s_event::EventBus;

let provider = RedisProvider::new(config);
let bus = EventBus::new(provider);

// Everything works: publish, subscribe, schema validation, DLQ, tracing
bus.publish("market", "forex", "Rate change", "source", payload).await?;
```

## Checklist

Before considering your provider complete:

- [ ] All 6 required `EventProvider` methods implemented
- [ ] `Subscription::next()` returns `ReceivedEvent` with correct fields
- [ ] `Subscription::next_manual_ack()` returns working ack/nak callbacks
- [ ] `build_subject()` and `category_subject()` follow the dot-separated convention
- [ ] Error types use `EventError` variants with descriptive messages
- [ ] Provider is `Send + Sync` (required by the trait)
- [ ] Unit tests for publish, subscribe, history, unsubscribe
- [ ] Integration test with real backend (skipped if unavailable)

## Reference Implementations

- `MemoryProvider` (`src/provider/memory.rs`) — simplest implementation, good starting point
- `NatsProvider` (`src/provider/nats/`) — full-featured with options support
