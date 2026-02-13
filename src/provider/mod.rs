//! Event provider trait — the core abstraction for event backends
//!
//! All event backends (NATS, Redis, Kafka, in-memory, etc.) implement
//! `EventProvider` to provide a uniform API for publish, subscribe, and query.

use crate::error::Result;
use crate::types::{Event, PublishOptions, ReceivedEvent, SubscribeOptions};
use async_trait::async_trait;

pub mod memory;
pub mod nats;

/// Core trait for event backends
///
/// Implementations handle the transport-specific details of event
/// publishing, subscription, and persistence. The `EventBus` uses
/// a provider to perform all operations.
#[async_trait]
pub trait EventProvider: Send + Sync {
    /// Publish an event, returning the provider-assigned sequence number
    async fn publish(&self, event: &Event) -> Result<u64>;

    /// Create a durable subscription (survives reconnects)
    ///
    /// Returns a `Subscription` handle for receiving events.
    async fn subscribe_durable(
        &self,
        consumer_name: &str,
        filter_subject: &str,
    ) -> Result<Box<dyn Subscription>>;

    /// Create an ephemeral subscription (cleaned up on disconnect)
    async fn subscribe(
        &self,
        filter_subject: &str,
    ) -> Result<Box<dyn Subscription>>;

    /// Fetch historical events from the backend
    async fn history(
        &self,
        filter_subject: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Event>>;

    /// Delete a durable subscription by consumer name
    async fn unsubscribe(&self, consumer_name: &str) -> Result<()>;

    /// Get provider info (message count, etc.)
    async fn info(&self) -> Result<ProviderInfo>;

    /// Build a full subject from category and topic
    fn build_subject(&self, category: &str, topic: &str) -> String;

    /// Build a wildcard subject for a category
    fn category_subject(&self, category: &str) -> String;

    /// Provider name (e.g., "nats", "memory", "redis")
    fn name(&self) -> &str;

    /// Publish an event with provider-specific options
    ///
    /// Default implementation ignores options and delegates to `publish()`.
    /// Providers that support deduplication, expected sequence, or custom
    /// timeouts should override this.
    async fn publish_with_options(&self, event: &Event, _opts: &PublishOptions) -> Result<u64> {
        self.publish(event).await
    }

    /// Create a durable subscription with provider-specific options
    ///
    /// Default implementation ignores options and delegates to `subscribe_durable()`.
    /// Providers that support max_deliver, backoff, max_ack_pending, or
    /// deliver_policy should override this.
    async fn subscribe_durable_with_options(
        &self,
        consumer_name: &str,
        filter_subject: &str,
        _opts: &SubscribeOptions,
    ) -> Result<Box<dyn Subscription>> {
        self.subscribe_durable(consumer_name, filter_subject).await
    }

    /// Create an ephemeral subscription with provider-specific options
    ///
    /// Default implementation ignores options and delegates to `subscribe()`.
    async fn subscribe_with_options(
        &self,
        filter_subject: &str,
        _opts: &SubscribeOptions,
    ) -> Result<Box<dyn Subscription>> {
        self.subscribe(filter_subject).await
    }

    /// Health check — returns true if the provider is connected and operational
    ///
    /// Default implementation delegates to `info()` and returns true if it succeeds.
    /// Providers may override for more specific health checks.
    async fn health(&self) -> Result<bool> {
        self.info().await.map(|_| true)
    }
}

/// Async subscription handle for receiving events
///
/// Provider-agnostic interface for consuming events from any backend.
#[async_trait]
pub trait Subscription: Send + Sync {
    /// Receive the next event (auto-ack)
    async fn next(&mut self) -> Result<Option<ReceivedEvent>>;

    /// Receive the next event with manual ack control
    async fn next_manual_ack(&mut self) -> Result<Option<PendingEvent>>;
}

/// An event pending acknowledgement
pub struct PendingEvent {
    /// The received event
    pub received: ReceivedEvent,

    /// Ack callback — call to confirm processing
    ack_fn: Box<dyn FnOnce() -> futures::future::BoxFuture<'static, Result<()>> + Send>,

    /// Nak callback — call to request redelivery
    nak_fn: Box<dyn FnOnce() -> futures::future::BoxFuture<'static, Result<()>> + Send>,
}

impl PendingEvent {
    /// Create a new pending event with ack/nak callbacks
    pub fn new(
        received: ReceivedEvent,
        ack_fn: impl FnOnce() -> futures::future::BoxFuture<'static, Result<()>> + Send + 'static,
        nak_fn: impl FnOnce() -> futures::future::BoxFuture<'static, Result<()>> + Send + 'static,
    ) -> Self {
        Self {
            received,
            ack_fn: Box::new(ack_fn),
            nak_fn: Box::new(nak_fn),
        }
    }

    /// Acknowledge successful processing
    pub async fn ack(self) -> Result<()> {
        (self.ack_fn)().await
    }

    /// Negative-acknowledge (request redelivery)
    pub async fn nak(self) -> Result<()> {
        (self.nak_fn)().await
    }
}

/// Provider status information
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// Provider name
    pub provider: String,
    /// Total messages stored
    pub messages: u64,
    /// Total bytes used
    pub bytes: u64,
    /// Number of active consumers/subscribers
    pub consumers: usize,
}
