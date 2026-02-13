//! High-level event bus built on pluggable providers
//!
//! `EventBus` provides a convenient API for event publishing, querying,
//! and subscription management on top of any `EventProvider` implementation.

use crate::error::{EventError, Result};
use crate::provider::{EventProvider, ProviderInfo, Subscription};
use crate::schema::SchemaRegistry;
use crate::types::{Event, EventCounts, PublishOptions, SubscriptionFilter};
use crate::dlq::DlqHandler;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// High-level event bus backed by a pluggable provider
///
/// Wraps any `EventProvider` with subscription tracking and convenience
/// methods. Thread-safe via internal locks.
///
/// Optionally validates events against a `SchemaRegistry` before publishing.
/// Optionally routes failed events to a `DlqHandler`.
pub struct EventBus {
    provider: Box<dyn EventProvider>,

    /// Tracked subscriptions (subscriber_id → filter)
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionFilter>>>,

    /// Optional schema registry for publish-time validation
    schema_registry: Option<Arc<dyn SchemaRegistry>>,

    /// Optional dead letter queue handler
    dlq_handler: Option<Arc<dyn DlqHandler>>,
}

impl EventBus {
    /// Create a new event bus from a provider
    pub fn new(provider: impl EventProvider + 'static) -> Self {
        Self {
            provider: Box::new(provider),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            schema_registry: None,
            dlq_handler: None,
        }
    }

    /// Create a new event bus with schema validation
    pub fn with_schema_registry(
        provider: impl EventProvider + 'static,
        registry: Arc<dyn SchemaRegistry>,
    ) -> Self {
        Self {
            provider: Box::new(provider),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            schema_registry: Some(registry),
            dlq_handler: None,
        }
    }

    /// Set the dead letter queue handler
    pub fn set_dlq_handler(&mut self, handler: Arc<dyn DlqHandler>) {
        self.dlq_handler = Some(handler);
    }

    /// Get the DLQ handler (if configured)
    pub fn dlq_handler(&self) -> Option<&dyn DlqHandler> {
        self.dlq_handler.as_deref()
    }

    /// Get the schema registry (if configured)
    pub fn schema_registry(&self) -> Option<&dyn SchemaRegistry> {
        self.schema_registry.as_deref()
    }

    /// Get the provider name
    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    /// Publish an event with convenience parameters
    pub async fn publish(
        &self,
        category: &str,
        topic: &str,
        summary: &str,
        source: &str,
        payload: serde_json::Value,
    ) -> Result<Event> {
        let subject = self.provider.build_subject(category, topic);
        let event = Event::new(subject, category, summary, source, payload);
        self.validate_if_configured(&event)?;

        let _span = tracing::info_span!(
            "event.publish",
            event_id = %event.id,
            subject = %event.subject,
            category = category,
            provider = self.provider.name(),
        )
        .entered();

        self.provider.publish(&event).await?;
        Ok(event)
    }

    /// Publish a pre-built event
    pub async fn publish_event(&self, event: &Event) -> Result<u64> {
        self.validate_if_configured(event)?;

        let _span = tracing::info_span!(
            "event.publish",
            event_id = %event.id,
            subject = %event.subject,
            category = %event.category,
            provider = self.provider.name(),
        )
        .entered();

        self.provider.publish(event).await
    }

    /// Publish a pre-built event with provider-specific options
    pub async fn publish_event_with_options(
        &self,
        event: &Event,
        opts: &PublishOptions,
    ) -> Result<u64> {
        self.validate_if_configured(event)?;

        let _span = tracing::info_span!(
            "event.publish",
            event_id = %event.id,
            subject = %event.subject,
            category = %event.category,
            provider = self.provider.name(),
            msg_id = ?opts.msg_id,
        )
        .entered();

        self.provider.publish_with_options(event, opts).await
    }

    /// Fetch recent events, optionally filtered by category
    pub async fn list_events(
        &self,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Event>> {
        let filter = category.map(|c| self.provider.category_subject(c));
        self.provider
            .history(filter.as_deref(), limit)
            .await
    }

    /// Get event counts by category
    pub async fn counts(&self, limit: usize) -> Result<EventCounts> {
        let events = self.provider.history(None, limit).await?;
        let mut counts = EventCounts::default();

        for event in &events {
            *counts.categories.entry(event.category.clone()).or_insert(0) += 1;
            counts.total += 1;
        }

        Ok(counts)
    }

    /// Register or update a subscription
    pub async fn update_subscription(&self, filter: SubscriptionFilter) -> Result<()> {
        let subscriber_id = filter.subscriber_id.clone();

        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(subscriber_id.clone(), filter.clone());
        }

        tracing::info!(
            subscriber = %subscriber_id,
            subjects = ?filter.subjects,
            durable = filter.durable,
            "Subscription updated"
        );

        Ok(())
    }

    /// Create subscribers for a registered subscription
    pub async fn create_subscriber(
        &self,
        subscriber_id: &str,
    ) -> Result<Vec<Box<dyn Subscription>>> {
        let subs = self.subscriptions.read().await;
        let filter = subs.get(subscriber_id).ok_or_else(|| {
            EventError::NotFound(format!("Subscription not found: {}", subscriber_id))
        })?;

        let _span = tracing::info_span!(
            "event.subscribe",
            subscriber = subscriber_id,
            subjects = ?filter.subjects,
            durable = filter.durable,
            provider = self.provider.name(),
        )
        .entered();

        let mut subscribers = Vec::new();
        for subject in &filter.subjects {
            let consumer_name = format!("{}-{}", subscriber_id, subject.replace('.', "-"));
            let sub = match (&filter.options, filter.durable) {
                (Some(opts), true) => {
                    self.provider
                        .subscribe_durable_with_options(&consumer_name, subject, opts)
                        .await?
                }
                (Some(opts), false) => {
                    self.provider
                        .subscribe_with_options(subject, opts)
                        .await?
                }
                (None, true) => {
                    self.provider
                        .subscribe_durable(&consumer_name, subject)
                        .await?
                }
                (None, false) => {
                    self.provider.subscribe(subject).await?
                }
            };
            subscribers.push(sub);
        }

        Ok(subscribers)
    }

    /// Remove a subscription
    pub async fn remove_subscription(&self, subscriber_id: &str) -> Result<()> {
        let filter = {
            let mut subs = self.subscriptions.write().await;
            subs.remove(subscriber_id)
        };

        if let Some(filter) = filter {
            for subject in &filter.subjects {
                let consumer_name = format!("{}-{}", subscriber_id, subject.replace('.', "-"));
                if let Err(e) = self.provider.unsubscribe(&consumer_name).await {
                    tracing::warn!(
                        consumer = %consumer_name,
                        error = %e,
                        "Failed to delete consumer during unsubscribe"
                    );
                }
            }
        }

        Ok(())
    }

    /// Get all registered subscriptions
    pub async fn list_subscriptions(&self) -> Vec<SubscriptionFilter> {
        let subs = self.subscriptions.read().await;
        subs.values().cloned().collect()
    }

    /// Get a specific subscription
    pub async fn get_subscription(&self, subscriber_id: &str) -> Option<SubscriptionFilter> {
        let subs = self.subscriptions.read().await;
        subs.get(subscriber_id).cloned()
    }

    /// Get provider info
    pub async fn info(&self) -> Result<ProviderInfo> {
        self.provider.info().await
    }

    /// Get a reference to the underlying provider
    pub fn provider(&self) -> &dyn EventProvider {
        self.provider.as_ref()
    }

    /// Health check — returns true if the provider is connected and operational
    pub async fn health(&self) -> Result<bool> {
        self.provider.health().await
    }

    /// Validate event against schema registry (if configured)
    fn validate_if_configured(&self, event: &Event) -> Result<()> {
        if let Some(ref registry) = self.schema_registry {
            registry.validate(event)?;
        }
        Ok(())
    }
}
