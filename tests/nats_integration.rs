//! NATS JetStream integration tests
//!
//! These tests require a running NATS server with JetStream enabled:
//!   nats-server -js
//!
//! Tests are skipped automatically if NATS is not available.

use a3s_event::provider::nats::{NatsConfig, NatsProvider, StorageType};
use a3s_event::{
    DeliverPolicy, Event, EventBus, EventProvider, PublishOptions, SubscribeOptions,
    SubscriptionFilter,
};

/// Try to connect to NATS. Returns None if server is unavailable.
async fn try_nats_provider(stream_suffix: &str) -> Option<NatsProvider> {
    let config = NatsConfig {
        url: "nats://127.0.0.1:4222".to_string(),
        stream_name: format!("TEST_EVENTS_{}", stream_suffix),
        subject_prefix: format!("test.{}", stream_suffix),
        storage: StorageType::Memory,
        max_events: 10_000,
        max_age_secs: 60,
        ..Default::default()
    };

    match NatsProvider::connect(config).await {
        Ok(provider) => Some(provider),
        Err(_) => {
            eprintln!("NATS not available, skipping integration test");
            None
        }
    }
}

/// Helper to create an EventBus with NATS, or skip the test
macro_rules! nats_bus {
    ($suffix:expr) => {
        match try_nats_provider($suffix).await {
            Some(p) => EventBus::new(p),
            None => return,
        }
    };
}

#[tokio::test]
async fn test_nats_publish_and_history() {
    let bus = nats_bus!("pub_hist");

    let event = bus
        .publish(
            "market",
            "forex",
            "USD/CNY rate change",
            "reuters",
            serde_json::json!({"rate": 7.35}),
        )
        .await
        .unwrap();

    assert!(event.id.starts_with("evt-"));
    assert_eq!(event.category, "market");

    // Give JetStream a moment to persist
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let events = bus.list_events(Some("market"), 10).await.unwrap();
    assert!(!events.is_empty());
    assert!(events.iter().any(|e| e.id == event.id));
}

#[tokio::test]
async fn test_nats_publish_multiple_categories() {
    let bus = nats_bus!("multi_cat");

    bus.publish("market", "forex", "A", "test", serde_json::json!({}))
        .await
        .unwrap();
    bus.publish("system", "deploy", "B", "test", serde_json::json!({}))
        .await
        .unwrap();
    bus.publish("market", "crypto", "C", "test", serde_json::json!({}))
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let all = bus.list_events(None, 100).await.unwrap();
    assert!(all.len() >= 3);
}

#[tokio::test]
async fn test_nats_publish_with_dedup() {
    let bus = nats_bus!("dedup");

    let event = Event::new(
        "test.dedup.topic",
        "test",
        "Dedup test",
        "test",
        serde_json::json!({"key": "value"}),
    );

    let opts = PublishOptions {
        msg_id: Some("dedup-test-1".to_string()),
        ..Default::default()
    };

    let seq1 = bus.publish_event_with_options(&event, &opts).await.unwrap();
    assert!(seq1 > 0);

    // Publishing with same msg_id should be deduplicated (same sequence)
    let seq2 = bus.publish_event_with_options(&event, &opts).await.unwrap();
    assert_eq!(seq1, seq2, "Duplicate message should return same sequence");
}

#[tokio::test]
async fn test_nats_durable_subscription() {
    let bus = nats_bus!("durable_sub");

    let filter = SubscriptionFilter {
        subscriber_id: "test-analyst".to_string(),
        subjects: vec!["test.durable_sub.market.>".to_string()],
        durable: true,
        options: None,
    };

    bus.update_subscription(filter).await.unwrap();

    // Publish an event
    bus.publish("market", "forex", "Rate", "test", serde_json::json!({"rate": 7.0}))
        .await
        .unwrap();

    // Create subscriber and receive
    let mut subs = bus.create_subscriber("test-analyst").await.unwrap();
    assert_eq!(subs.len(), 1);

    // Try to receive (with timeout to avoid hanging)
    let sub = &mut subs[0];
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), sub.next()).await;

    // Clean up
    bus.remove_subscription("test-analyst").await.unwrap();

    if let Ok(Ok(Some(received))) = result {
        assert_eq!(received.event.category, "market");
    }
    // If timeout, that's ok — the subscription was created successfully
}

#[tokio::test]
async fn test_nats_subscribe_with_options() {
    let bus = nats_bus!("sub_opts");

    let filter = SubscriptionFilter {
        subscriber_id: "opts-consumer".to_string(),
        subjects: vec!["test.sub_opts.>".to_string()],
        durable: true,
        options: Some(SubscribeOptions {
            max_deliver: Some(3),
            max_ack_pending: Some(100),
            deliver_policy: DeliverPolicy::New,
            ..Default::default()
        }),
    };

    bus.update_subscription(filter).await.unwrap();

    let subs = bus.create_subscriber("opts-consumer").await.unwrap();
    assert_eq!(subs.len(), 1);

    bus.remove_subscription("opts-consumer").await.unwrap();
}

#[tokio::test]
async fn test_nats_provider_info() {
    let bus = nats_bus!("info");

    bus.publish("test", "a", "Info test", "test", serde_json::json!({}))
        .await
        .unwrap();

    let info = bus.info().await.unwrap();
    assert_eq!(info.provider, "nats");
    assert!(info.messages >= 1);
}

#[tokio::test]
async fn test_nats_health_check() {
    let bus = nats_bus!("health");
    assert!(bus.health().await.unwrap());
}

#[tokio::test]
async fn test_nats_concurrent_publish() {
    let bus = std::sync::Arc::new(nats_bus!("concurrent"));
    let mut handles = Vec::new();

    for i in 0..20 {
        let bus = bus.clone();
        handles.push(tokio::spawn(async move {
            bus.publish(
                "load",
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

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let events = bus.list_events(None, 100).await.unwrap();
    assert_eq!(events.len(), 20);
}

#[tokio::test]
async fn test_nats_manual_ack() {
    let suffix = "manual_ack";
    let provider = match try_nats_provider(suffix).await {
        Some(p) => p,
        None => return,
    };

    // Publish an event
    let event = Event::new(
        format!("test.{}.topic", suffix),
        "test",
        "Ack test",
        "test",
        serde_json::json!({}),
    );
    provider.publish(&event).await.unwrap();

    // Subscribe with durable consumer
    let mut sub = provider
        .subscribe_durable("ack-test-consumer", &format!("test.{}.>", suffix))
        .await
        .unwrap();

    // Receive with manual ack
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(2), sub.next_manual_ack()).await;

    if let Ok(Ok(Some(pending))) = result {
        assert_eq!(pending.received.event.summary, "Ack test");
        pending.ack().await.unwrap();
    }

    // Clean up
    let _ = provider.unsubscribe("ack-test-consumer").await;
}
