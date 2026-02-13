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

        let span = tracing::info_span!(
            "event.publish",
            event_id = %event.id,
            subject = %event.subject,
            category = category,
            provider = self.provider.name(),
        );
        let _guard = span.enter();
        drop(_guard);

        self.provider.publish(&event).await?;
        Ok(event)
    }

    /// Publish a pre-built event
    pub async fn publish_event(&self, event: &Event) -> Result<u64> {
        self.validate_if_configured(event)?;

        let span = tracing::info_span!(
            "event.publish",
            event_id = %event.id,
            subject = %event.subject,
            category = %event.category,
            provider = self.provider.name(),
        );
        let _guard = span.enter();
        drop(_guard);

        self.provider.publish(event).await
    }

    /// Publish a pre-built event with provider-specific options
    pub async fn publish_event_with_options(
        &self,
        event: &Event,
        opts: &PublishOptions,
    ) -> Result<u64> {
        self.validate_if_configured(event)?;

        let span = tracing::info_span!(
            "event.publish",
            event_id = %event.id,
            subject = %event.subject,
            category = %event.category,
            provider = self.provider.name(),
            msg_id = ?opts.msg_id,
        );
        let _guard = span.enter();
        drop(_guard);

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

        let span = tracing::info_span!(
            "event.subscribe",
            subscriber = subscriber_id,
            subjects = ?filter.subjects,
            durable = filter.durable,
            provider = self.provider.name(),
        );
        let _guard = span.enter();
        drop(_guard);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dlq::{DeadLetterEvent, MemoryDlqHandler};
    use crate::provider::memory::MemoryProvider;
    use crate::schema::{EventSchema, MemorySchemaRegistry};
    use crate::types::Event;

    fn test_bus() -> EventBus {
        EventBus::new(MemoryProvider::default())
    }

    #[tokio::test]
    async fn test_publish_and_list() {
        let bus = test_bus();
        let event = bus
            .publish("market", "forex", "Rate change", "reuters", serde_json::json!({"rate": 7.35}))
            .await
            .unwrap();

        assert!(event.id.starts_with("evt-"));
        assert_eq!(event.subject, "events.market.forex");
        assert_eq!(event.category, "market");

        let events = bus.list_events(Some("market"), 10).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event.id);
    }

    #[tokio::test]
    async fn test_publish_event_prebuilt() {
        let bus = test_bus();
        let event = Event::new("events.test.a", "test", "Test", "test", serde_json::json!({}));
        let seq = bus.publish_event(&event).await.unwrap();
        assert!(seq > 0);

        let events = bus.list_events(None, 10).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_list_events_by_category() {
        let bus = test_bus();
        bus.publish("market", "forex", "A", "test", serde_json::json!({})).await.unwrap();
        bus.publish("system", "deploy", "B", "test", serde_json::json!({})).await.unwrap();
        bus.publish("market", "crypto", "C", "test", serde_json::json!({})).await.unwrap();

        let market = bus.list_events(Some("market"), 10).await.unwrap();
        assert_eq!(market.len(), 2);

        let system = bus.list_events(Some("system"), 10).await.unwrap();
        assert_eq!(system.len(), 1);

        let all = bus.list_events(None, 10).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_counts() {
        let bus = test_bus();
        bus.publish("market", "forex", "A", "test", serde_json::json!({})).await.unwrap();
        bus.publish("market", "crypto", "B", "test", serde_json::json!({})).await.unwrap();
        bus.publish("system", "deploy", "C", "test", serde_json::json!({})).await.unwrap();

        let counts = bus.counts(100).await.unwrap();
        assert_eq!(counts.total, 3);
        assert_eq!(counts.categories["market"], 2);
        assert_eq!(counts.categories["system"], 1);
    }

    #[tokio::test]
    async fn test_subscription_lifecycle() {
        let bus = test_bus();

        let filter = SubscriptionFilter {
            subscriber_id: "analyst".to_string(),
            subjects: vec!["events.market.>".to_string()],
            durable: false,
            options: None,
        };

        bus.update_subscription(filter).await.unwrap();

        let sub = bus.get_subscription("analyst").await;
        assert!(sub.is_some());
        assert_eq!(sub.unwrap().subjects, vec!["events.market.>"]);

        let subs = bus.list_subscriptions().await;
        assert_eq!(subs.len(), 1);

        bus.remove_subscription("analyst").await.unwrap();
        assert!(bus.get_subscription("analyst").await.is_none());
        assert!(bus.list_subscriptions().await.is_empty());
    }

    #[tokio::test]
    async fn test_create_subscriber_not_found() {
        let bus = test_bus();
        let result = bus.create_subscriber("nonexistent").await;
        assert!(matches!(result, Err(EventError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_provider_name() {
        let bus = test_bus();
        assert_eq!(bus.provider_name(), "memory");
    }

    #[tokio::test]
    async fn test_info() {
        let bus = test_bus();
        bus.publish("test", "a", "A", "test", serde_json::json!({})).await.unwrap();

        let info = bus.info().await.unwrap();
        assert_eq!(info.provider, "memory");
        assert_eq!(info.messages, 1);
    }

    #[tokio::test]
    async fn test_health() {
        let bus = test_bus();
        assert!(bus.health().await.unwrap());
    }

    #[tokio::test]
    async fn test_schema_validation_on_publish() {
        let registry = Arc::new(MemorySchemaRegistry::new());
        registry
            .register(EventSchema {
                event_type: "forex.rate".to_string(),
                version: 1,
                required_fields: vec!["rate".to_string()],
                description: String::new(),
            })
            .unwrap();

        let bus = EventBus::with_schema_registry(MemoryProvider::default(), registry);

        // Valid typed event
        let event = Event::typed(
            "events.market.forex",
            "market",
            "forex.rate",
            1,
            "Rate",
            "test",
            serde_json::json!({"rate": 7.35}),
        );
        assert!(bus.publish_event(&event).await.is_ok());

        // Invalid typed event (missing required field)
        let bad_event = Event::typed(
            "events.market.forex",
            "market",
            "forex.rate",
            1,
            "Rate",
            "test",
            serde_json::json!({"currency": "USD"}),
        );
        let err = bus.publish_event(&bad_event).await.unwrap_err();
        assert!(matches!(err, EventError::SchemaValidation { .. }));
    }

    #[tokio::test]
    async fn test_untyped_event_skips_validation() {
        let registry = Arc::new(MemorySchemaRegistry::new());
        registry
            .register(EventSchema {
                event_type: "forex.rate".to_string(),
                version: 1,
                required_fields: vec!["rate".to_string()],
                description: String::new(),
            })
            .unwrap();

        let bus = EventBus::with_schema_registry(MemoryProvider::default(), registry);

        // Untyped event should pass even without required fields
        let event = bus
            .publish("market", "forex", "Rate", "test", serde_json::json!({}))
            .await;
        assert!(event.is_ok());
    }

    #[tokio::test]
    async fn test_dlq_handler_integration() {
        let dlq = Arc::new(MemoryDlqHandler::default());
        let mut bus = test_bus();
        bus.set_dlq_handler(dlq.clone());

        assert!(bus.dlq_handler().is_some());

        // Manually route an event to DLQ
        let received = crate::types::ReceivedEvent {
            event: Event::new("events.test.a", "test", "Test", "test", serde_json::json!({})),
            sequence: 1,
            num_delivered: 5,
            stream: "memory".to_string(),
        };
        let dle = DeadLetterEvent::new(received, "Max retries exceeded");
        dlq.handle(dle).await.unwrap();

        assert_eq!(dlq.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_publish_with_options() {
        let bus = test_bus();
        let event = Event::new("events.test.a", "test", "Test", "test", serde_json::json!({}));
        let opts = PublishOptions {
            msg_id: Some("dedup-1".to_string()),
            ..Default::default()
        };

        // MemoryProvider ignores options but should still succeed
        let seq = bus.publish_event_with_options(&event, &opts).await.unwrap();
        assert!(seq > 0);
    }

    #[tokio::test]
    async fn test_concurrent_publish() {
        let bus = Arc::new(test_bus());
        let mut handles = Vec::new();

        for i in 0..50 {
            let bus = bus.clone();
            handles.push(tokio::spawn(async move {
                bus.publish(
                    "test",
                    &format!("topic.{}", i),
                    &format!("Event {}", i),
                    "test",
                    serde_json::json!({"index": i}),
                )
                .await
                .unwrap()
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let events = bus.list_events(None, 100).await.unwrap();
        assert_eq!(events.len(), 50);
    }

    #[tokio::test]
    async fn test_remove_nonexistent_subscription() {
        let bus = test_bus();
        // Should not error — just a no-op
        assert!(bus.remove_subscription("nonexistent").await.is_ok());
    }

    #[tokio::test]
    async fn test_update_subscription_overwrites() {
        let bus = test_bus();

        let filter1 = SubscriptionFilter {
            subscriber_id: "analyst".to_string(),
            subjects: vec!["events.market.>".to_string()],
            durable: false,
            options: None,
        };
        bus.update_subscription(filter1).await.unwrap();

        let filter2 = SubscriptionFilter {
            subscriber_id: "analyst".to_string(),
            subjects: vec!["events.system.>".to_string()],
            durable: true,
            options: None,
        };
        bus.update_subscription(filter2).await.unwrap();

        let sub = bus.get_subscription("analyst").await.unwrap();
        assert_eq!(sub.subjects, vec!["events.system.>"]);
        assert!(sub.durable);
        assert_eq!(bus.list_subscriptions().await.len(), 1);
    }
}
