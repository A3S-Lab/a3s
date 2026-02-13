//! NATS JetStream event provider
//!
//! Implements `EventProvider` using NATS JetStream for persistent,
//! distributed event pub/sub with at-least-once delivery.

mod client;
mod config;
mod subscriber;

pub use client::NatsClient;
pub use config::{NatsConfig, StorageType};
pub use subscriber::NatsSubscription;

use crate::error::Result;
use crate::provider::{EventProvider, ProviderInfo, Subscription};
use crate::types::{Event, PublishOptions, SubscribeOptions};
use async_trait::async_trait;

/// NATS JetStream event provider
///
/// Wraps `NatsClient` and implements the `EventProvider` trait.
pub struct NatsProvider {
    client: NatsClient,
}

impl NatsProvider {
    /// Connect to NATS and initialize the JetStream stream
    pub async fn connect(config: NatsConfig) -> Result<Self> {
        let client = NatsClient::connect(config).await?;
        Ok(Self { client })
    }

    /// Get the underlying NATS client for advanced usage
    pub fn client(&self) -> &NatsClient {
        &self.client
    }
}

#[async_trait]
impl EventProvider for NatsProvider {
    async fn publish(&self, event: &Event) -> Result<u64> {
        self.client.publish(event).await
    }

    async fn subscribe_durable(
        &self,
        consumer_name: &str,
        filter_subject: &str,
    ) -> Result<Box<dyn Subscription>> {
        let sub = self.client.subscribe_durable(consumer_name, filter_subject).await?;
        Ok(Box::new(sub))
    }

    async fn subscribe(
        &self,
        filter_subject: &str,
    ) -> Result<Box<dyn Subscription>> {
        let sub = self.client.subscribe(filter_subject).await?;
        Ok(Box::new(sub))
    }

    async fn history(
        &self,
        filter_subject: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Event>> {
        self.client.history(filter_subject, limit).await
    }

    async fn unsubscribe(&self, consumer_name: &str) -> Result<()> {
        self.client.unsubscribe(consumer_name).await
    }

    async fn info(&self) -> Result<ProviderInfo> {
        let info = self.client.stream_info().await?;
        Ok(ProviderInfo {
            provider: "nats".to_string(),
            messages: info.messages,
            bytes: info.bytes,
            consumers: info.consumer_count,
        })
    }

    fn build_subject(&self, category: &str, topic: &str) -> String {
        self.client.config().build_subject(category, topic)
    }

    fn category_subject(&self, category: &str) -> String {
        self.client.config().category_subject(category)
    }

    fn name(&self) -> &str {
        "nats"
    }

    async fn publish_with_options(&self, event: &Event, opts: &PublishOptions) -> Result<u64> {
        self.client.publish_with_options(event, opts).await
    }

    async fn subscribe_durable_with_options(
        &self,
        consumer_name: &str,
        filter_subject: &str,
        opts: &SubscribeOptions,
    ) -> Result<Box<dyn Subscription>> {
        let sub = self
            .client
            .subscribe_durable_with_options(consumer_name, filter_subject, opts)
            .await?;
        Ok(Box::new(sub))
    }

    async fn subscribe_with_options(
        &self,
        filter_subject: &str,
        opts: &SubscribeOptions,
    ) -> Result<Box<dyn Subscription>> {
        let sub = self
            .client
            .subscribe_with_options(filter_subject, opts)
            .await?;
        Ok(Box::new(sub))
    }
}
