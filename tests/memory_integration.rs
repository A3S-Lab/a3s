//! Memory provider integration tests
//!
//! End-to-end tests exercising the full EventBus lifecycle with the
//! in-memory provider. Covers publish/subscribe, history, encryption,
//! schema validation, DLQ, state persistence, metrics, and concurrency.

use a3s_event::{
    Aes256GcmEncryptor, DeadLetterEvent, DlqHandler, Event, EventBus, MemoryDlqHandler,
    MemorySchemaRegistry, MemoryStateStore, EventSchema, SchemaRegistry, StateStore,
    SubscriptionFilter,
};
use std::sync::Arc;

fn test_bus() -> EventBus {
    EventBus::new(a3s_event::MemoryProvider::default())
}

// ─── Publish & History ───────────────────────────────────────────

#[tokio::test]
async fn test_publish_and_history_roundtrip() {
    let bus = test_bus();

    let event = bus
        .publish(
            "market",
            "forex.usd_cny",
            "USD/CNY broke through 7.35",
            "reuters",
            serde_json::json!({"rate": 7.3521, "direction": "up"}),
        )
        .await
        .unwrap();

    assert!(event.id.starts_with("evt-"));
    assert_eq!(event.category, "market");
    assert_eq!(event.subject, "events.market.forex.usd_cny");
    assert_eq!(event.source, "reuters");
    assert_eq!(event.payload["rate"], 7.3521);

    let events = bus.list_events(Some("market"), 10).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, event.id);
    assert_eq!(events[0].payload["direction"], "up");
}

#[tokio::test]
async fn test_multiple_categories_and_filtering() {
    let bus = test_bus();

    bus.publish("market", "forex", "A", "src", serde_json::json!({}))
        .await.unwrap();
    bus.publish("system", "deploy", "B", "src", serde_json::json!({}))
        .await.unwrap();
    bus.publish("market", "crypto", "C", "src", serde_json::json!({}))
        .await.unwrap();
    bus.publish("compliance", "audit", "D", "src", serde_json::json!({}))
        .await.unwrap();

    assert_eq!(bus.list_events(Some("market"), 100).await.unwrap().len(), 2);
    assert_eq!(bus.list_events(Some("system"), 100).await.unwrap().len(), 1);
    assert_eq!(bus.list_events(Some("compliance"), 100).await.unwrap().len(), 1);
    assert_eq!(bus.list_events(None, 100).await.unwrap().len(), 4);

    let counts = bus.counts(100).await.unwrap();
    assert_eq!(counts.total, 4);
    assert_eq!(counts.categories["market"], 2);
}

#[tokio::test]
async fn test_publish_prebuilt_event() {
    let bus = test_bus();
    let event = Event::new(
        "events.task.completed",
        "task",
        "Task finished",
        "scheduler",
        serde_json::json!({"task_id": "t-123", "duration_ms": 450}),
    );

    let seq = bus.publish_event(&event).await.unwrap();
    assert!(seq > 0);

    let events = bus.list_events(Some("task"), 10).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["task_id"], "t-123");
}

#[tokio::test]
async fn test_history_limit() {
    let bus = test_bus();
    for i in 0..20 {
        bus.publish("test", "topic", &format!("E{}", i), "src", serde_json::json!({"i": i}))
            .await.unwrap();
    }

    let limited = bus.list_events(None, 5).await.unwrap();
    assert_eq!(limited.len(), 5);

    let all = bus.list_events(None, 100).await.unwrap();
    assert_eq!(all.len(), 20);
}

// ─── Subscription Lifecycle ──────────────────────────────────────

#[tokio::test]
async fn test_subscription_crud() {
    let bus = test_bus();

    // Create
    bus.update_subscription(SubscriptionFilter {
        subscriber_id: "analyst".to_string(),
        subjects: vec!["events.market.>".to_string()],
        durable: true,
        options: None,
    }).await.unwrap();

    bus.update_subscription(SubscriptionFilter {
        subscriber_id: "monitor".to_string(),
        subjects: vec!["events.system.>".to_string()],
        durable: false,
        options: None,
    }).await.unwrap();

    // Read
    let analyst = bus.get_subscription("analyst").await.unwrap();
    assert_eq!(analyst.subjects, vec!["events.market.>"]);
    assert!(analyst.durable);

    let all = bus.list_subscriptions().await;
    assert_eq!(all.len(), 2);

    // Update (overwrite)
    bus.update_subscription(SubscriptionFilter {
        subscriber_id: "analyst".to_string(),
        subjects: vec!["events.market.>".to_string(), "events.compliance.>".to_string()],
        durable: true,
        options: None,
    }).await.unwrap();

    let updated = bus.get_subscription("analyst").await.unwrap();
    assert_eq!(updated.subjects.len(), 2);

    // Delete
    bus.remove_subscription("analyst").await.unwrap();
    assert!(bus.get_subscription("analyst").await.is_none());
    assert_eq!(bus.list_subscriptions().await.len(), 1);

    bus.remove_subscription("monitor").await.unwrap();
    assert!(bus.list_subscriptions().await.is_empty());
}

#[tokio::test]
async fn test_subscribe_and_receive_events() {
    let bus = Arc::new(test_bus());

    bus.update_subscription(SubscriptionFilter {
        subscriber_id: "listener".to_string(),
        subjects: vec!["events.market.>".to_string()],
        durable: false,
        options: None,
    }).await.unwrap();

    let mut subs = bus.create_subscriber("listener").await.unwrap();
    assert_eq!(subs.len(), 1);

    // Publish after subscribing
    let bus_clone = bus.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        bus_clone
            .publish("market", "forex", "Rate", "test", serde_json::json!({"rate": 7.35}))
            .await
            .unwrap();
    });

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        subs[0].next(),
    ).await;

    if let Ok(Ok(Some(received))) = result {
        assert_eq!(received.event.category, "market");
        assert_eq!(received.event.payload["rate"], 7.35);
    }

    bus.remove_subscription("listener").await.unwrap();
}

#[tokio::test]
async fn test_manual_ack_flow() {
    let bus = Arc::new(test_bus());

    bus.update_subscription(SubscriptionFilter {
        subscriber_id: "acker".to_string(),
        subjects: vec!["events.task.>".to_string()],
        durable: false,
        options: None,
    }).await.unwrap();

    let mut subs = bus.create_subscriber("acker").await.unwrap();

    let bus_clone = bus.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        bus_clone
            .publish("task", "completed", "Done", "worker", serde_json::json!({"task": "t-1"}))
            .await
            .unwrap();
    });

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        subs[0].next_manual_ack(),
    ).await;

    if let Ok(Ok(Some(pending))) = result {
        assert_eq!(pending.received.event.summary, "Done");
        // Ack should succeed (no-op for memory provider)
        pending.ack().await.unwrap();
    }

    bus.remove_subscription("acker").await.unwrap();
}

#[tokio::test]
async fn test_create_subscriber_not_found() {
    let bus = test_bus();
    let result = bus.create_subscriber("ghost").await;
    assert!(result.is_err());
}

// ─── Encryption End-to-End ───────────────────────────────────────

#[tokio::test]
async fn test_encrypted_publish_decrypt_on_list() {
    let enc = Arc::new(Aes256GcmEncryptor::new("primary", &[0xAB; 32]));
    let mut bus = test_bus();
    bus.set_encryptor(enc);

    let event = bus
        .publish(
            "compliance",
            "audit.login",
            "User login",
            "auth-service",
            serde_json::json!({"user": "alice", "ip": "10.0.0.1"}),
        )
        .await
        .unwrap();

    // Returned event has encrypted payload
    assert!(a3s_event::EncryptedPayload::is_encrypted(&event.payload));

    // list_events auto-decrypts
    let events = bus.list_events(Some("compliance"), 10).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["user"], "alice");
    assert_eq!(events[0].payload["ip"], "10.0.0.1");
}

#[tokio::test]
async fn test_encryption_with_key_rotation() {
    let enc = Aes256GcmEncryptor::new("key-v1", &[0x11; 32]);
    enc.add_key("key-v2", &[0x22; 32]).unwrap();

    let enc = Arc::new(enc);
    let mut bus = test_bus();
    bus.set_encryptor(enc);

    // Publish with key-v1
    bus.publish("secret", "data", "V1 event", "src", serde_json::json!({"v": 1}))
        .await.unwrap();

    // Rotate to key-v2: need a new encryptor with v2 active
    let enc2 = Aes256GcmEncryptor::new("key-v2", &[0x22; 32]);
    enc2.add_key("key-v1", &[0x11; 32]).unwrap();
    let enc2 = Arc::new(enc2);
    bus.set_encryptor(enc2);

    // Publish with key-v2
    bus.publish("secret", "data", "V2 event", "src", serde_json::json!({"v": 2}))
        .await.unwrap();

    // Both should decrypt (both keys registered in enc2)
    let events = bus.list_events(Some("secret"), 10).await.unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().any(|e| e.payload["v"] == 1));
    assert!(events.iter().any(|e| e.payload["v"] == 2));
}

#[tokio::test]
async fn test_publish_event_prebuilt_does_not_mutate_original() {
    let enc = Arc::new(Aes256GcmEncryptor::new("k1", &[0x42; 32]));
    let mut bus = test_bus();
    bus.set_encryptor(enc);

    let original = Event::new(
        "events.secret.data",
        "secret",
        "Sensitive",
        "src",
        serde_json::json!({"ssn": "123-45-6789"}),
    );

    bus.publish_event(&original).await.unwrap();

    // Original event must NOT be mutated
    assert_eq!(original.payload["ssn"], "123-45-6789");
    assert!(!a3s_event::EncryptedPayload::is_encrypted(&original.payload));
}

// ─── Schema Validation End-to-End ────────────────────────────────

#[tokio::test]
async fn test_schema_validation_rejects_invalid_event() {
    let registry = Arc::new(MemorySchemaRegistry::new());
    registry
        .register(EventSchema {
            event_type: "trade.executed".to_string(),
            version: 1,
            required_fields: vec!["symbol".to_string(), "quantity".to_string(), "price".to_string()],
            description: "Trade execution event".to_string(),
        })
        .unwrap();

    let bus = EventBus::with_schema_registry(
        a3s_event::MemoryProvider::default(),
        registry,
    );

    // Valid typed event
    let valid = Event::typed(
        "events.market.trade",
        "market",
        "trade.executed",
        1,
        "AAPL buy",
        "trading-engine",
        serde_json::json!({"symbol": "AAPL", "quantity": 100, "price": 150.25}),
    );
    assert!(bus.publish_event(&valid).await.is_ok());

    // Invalid typed event (missing required fields)
    let invalid = Event::typed(
        "events.market.trade",
        "market",
        "trade.executed",
        1,
        "Bad trade",
        "trading-engine",
        serde_json::json!({"symbol": "AAPL"}),
    );
    assert!(bus.publish_event(&invalid).await.is_err());

    // Untyped event bypasses validation
    let untyped = bus
        .publish("market", "trade", "Untyped", "src", serde_json::json!({}))
        .await;
    assert!(untyped.is_ok());
}

#[tokio::test]
async fn test_schema_validation_with_encryption() {
    let registry = Arc::new(MemorySchemaRegistry::new());
    registry
        .register(EventSchema {
            event_type: "user.created".to_string(),
            version: 1,
            required_fields: vec!["username".to_string()],
            description: String::new(),
        })
        .unwrap();

    let mut bus = EventBus::with_schema_registry(
        a3s_event::MemoryProvider::default(),
        registry,
    );
    bus.set_encryptor(Arc::new(Aes256GcmEncryptor::new("k1", &[0x55; 32])));

    // Valid: passes validation, then gets encrypted
    let valid = Event::typed(
        "events.user.created",
        "user",
        "user.created",
        1,
        "New user",
        "auth",
        serde_json::json!({"username": "bob"}),
    );
    let seq = bus.publish_event(&valid).await.unwrap();
    assert!(seq > 0);

    // Decrypted on read
    let events = bus.list_events(Some("user"), 10).await.unwrap();
    assert_eq!(events[0].payload["username"], "bob");

    // Invalid: fails validation before encryption
    let invalid = Event::typed(
        "events.user.created",
        "user",
        "user.created",
        1,
        "Bad",
        "auth",
        serde_json::json!({"email": "bob@test.com"}),
    );
    assert!(bus.publish_event(&invalid).await.is_err());
}

// ─── Dead Letter Queue ───────────────────────────────────────────

#[tokio::test]
async fn test_dlq_integration() {
    let dlq = Arc::new(MemoryDlqHandler::default());
    let mut bus = test_bus();
    bus.set_dlq_handler(dlq.clone());

    assert!(bus.dlq_handler().is_some());

    // Simulate a failed event routed to DLQ
    let received = a3s_event::ReceivedEvent {
        event: Event::new(
            "events.payment.failed",
            "payment",
            "Payment timeout",
            "billing",
            serde_json::json!({"order_id": "ord-999", "amount": 49.99}),
        ),
        sequence: 42,
        num_delivered: 5,
        stream: "memory".to_string(),
    };

    let dle = DeadLetterEvent::new(received, "Max retries exceeded after 5 attempts");
    dlq.handle(dle).await.unwrap();

    assert_eq!(dlq.count().await.unwrap(), 1);

    let dead_events = dlq.list(10).await.unwrap();
    assert_eq!(dead_events.len(), 1);
    assert_eq!(dead_events[0].event.event.category, "payment");
    assert_eq!(dead_events[0].reason, "Max retries exceeded after 5 attempts");
}

// ─── State Persistence ───────────────────────────────────────────

#[tokio::test]
async fn test_state_persistence_across_bus_instances() {
    let store = Arc::new(MemoryStateStore::default());

    // Bus 1: create subscriptions
    {
        let mut bus = test_bus();
        bus.set_state_store(store.clone()).unwrap();

        bus.update_subscription(SubscriptionFilter {
            subscriber_id: "analyst".to_string(),
            subjects: vec!["events.market.>".to_string()],
            durable: true,
            options: None,
        }).await.unwrap();

        bus.update_subscription(SubscriptionFilter {
            subscriber_id: "ops".to_string(),
            subjects: vec!["events.system.>".to_string()],
            durable: false,
            options: None,
        }).await.unwrap();
    }

    // Bus 2: restore from same store
    {
        let mut bus = test_bus();
        bus.set_state_store(store.clone()).unwrap();

        let subs = bus.list_subscriptions().await;
        assert_eq!(subs.len(), 2);

        let analyst = bus.get_subscription("analyst").await.unwrap();
        assert_eq!(analyst.subjects, vec!["events.market.>"]);
        assert!(analyst.durable);

        let ops = bus.get_subscription("ops").await.unwrap();
        assert_eq!(ops.subjects, vec!["events.system.>"]);
    }
}

#[tokio::test]
async fn test_state_persistence_with_file_store() {
    let dir = std::env::temp_dir().join(format!("a3s-mem-integ-{}", uuid::Uuid::new_v4()));
    let path = dir.join("state.json");
    let store = Arc::new(a3s_event::FileStateStore::new(&path));

    // Bus 1: add subscription, persist to file
    {
        let mut bus = test_bus();
        bus.set_state_store(store.clone()).unwrap();

        bus.update_subscription(SubscriptionFilter {
            subscriber_id: "file-sub".to_string(),
            subjects: vec!["events.>".to_string()],
            durable: true,
            options: None,
        }).await.unwrap();
    }

    assert!(path.exists());

    // Bus 2: restore from file
    {
        let mut bus = test_bus();
        bus.set_state_store(store).unwrap();

        let sub = bus.get_subscription("file-sub").await.unwrap();
        assert_eq!(sub.subjects, vec!["events.>"]);
    }

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn test_remove_subscription_persists() {
    let store = Arc::new(MemoryStateStore::default());
    let mut bus = test_bus();
    bus.set_state_store(store.clone()).unwrap();

    bus.update_subscription(SubscriptionFilter {
        subscriber_id: "temp".to_string(),
        subjects: vec!["events.>".to_string()],
        durable: false,
        options: None,
    }).await.unwrap();

    assert_eq!(store.load().unwrap().len(), 1);

    bus.remove_subscription("temp").await.unwrap();
    assert!(store.load().unwrap().is_empty());
}

// ─── Metrics End-to-End ──────────────────────────────────────────

#[tokio::test]
async fn test_metrics_full_lifecycle() {
    let registry = Arc::new(MemorySchemaRegistry::new());
    registry
        .register(EventSchema {
            event_type: "strict.event".to_string(),
            version: 1,
            required_fields: vec!["required".to_string()],
            description: String::new(),
        })
        .unwrap();

    let mut bus = EventBus::with_schema_registry(
        a3s_event::MemoryProvider::default(),
        registry,
    );
    bus.set_encryptor(Arc::new(Aes256GcmEncryptor::new("k1", &[0x77; 32])));

    // Successful publishes
    bus.publish("test", "a", "A", "src", serde_json::json!({"data": 1}))
        .await.unwrap();
    bus.publish("test", "b", "B", "src", serde_json::json!({"data": 2}))
        .await.unwrap();

    // Validation error (typed event missing required field)
    let bad = Event::typed(
        "events.test.c", "test", "strict.event", 1,
        "Bad", "src", serde_json::json!({}),
    );
    assert!(bus.publish_event(&bad).await.is_err());

    // Subscribe + unsubscribe
    bus.update_subscription(SubscriptionFilter {
        subscriber_id: "m".to_string(),
        subjects: vec!["events.>".to_string()],
        durable: false,
        options: None,
    }).await.unwrap();
    bus.remove_subscription("m").await.unwrap();

    // Decrypt via list_events
    let events = bus.list_events(None, 100).await.unwrap();
    assert_eq!(events.len(), 2);

    let snap = bus.metrics().snapshot();
    assert_eq!(snap.publish_count, 2);
    assert_eq!(snap.publish_errors, 0);
    assert_eq!(snap.validation_errors, 1);
    assert_eq!(snap.encrypt_count, 2);
    assert_eq!(snap.decrypt_count, 2);
    assert_eq!(snap.subscribe_count, 1);
    assert_eq!(snap.unsubscribe_count, 1);
    assert!(snap.avg_publish_latency_us < 1_000_000);

    // Serializable
    let json = serde_json::to_string(&snap).unwrap();
    assert!(json.contains("publishCount"));
    assert!(json.contains("encryptCount"));
}

#[tokio::test]
async fn test_metrics_reset() {
    let bus = test_bus();
    bus.publish("test", "a", "A", "src", serde_json::json!({}))
        .await.unwrap();

    assert_eq!(bus.metrics().snapshot().publish_count, 1);
    bus.metrics().reset();
    assert_eq!(bus.metrics().snapshot().publish_count, 0);
}

// ─── Provider Info & Health ──────────────────────────────────────

#[tokio::test]
async fn test_provider_info() {
    let bus = test_bus();

    bus.publish("market", "forex", "A", "src", serde_json::json!({"rate": 7.35}))
        .await.unwrap();
    bus.publish("system", "deploy", "B", "src", serde_json::json!({}))
        .await.unwrap();

    let info = bus.info().await.unwrap();
    assert_eq!(info.provider, "memory");
    assert_eq!(info.messages, 2);
    assert!(info.bytes > 0);
    assert_eq!(bus.provider_name(), "memory");
}

#[tokio::test]
async fn test_health_check() {
    let bus = test_bus();
    assert!(bus.health().await.unwrap());
}

// ─── Concurrency ─────────────────────────────────────────────────

#[tokio::test]
async fn test_concurrent_publish_50_tasks() {
    let bus = Arc::new(test_bus());
    let mut handles = Vec::new();

    for i in 0..50 {
        let bus = bus.clone();
        handles.push(tokio::spawn(async move {
            bus.publish(
                "load",
                &format!("topic.{}", i),
                &format!("Event {}", i),
                "stress-test",
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

    let snap = bus.metrics().snapshot();
    assert_eq!(snap.publish_count, 50);
    assert_eq!(snap.publish_errors, 0);
}

#[tokio::test]
async fn test_concurrent_publish_with_encryption() {
    let enc = Arc::new(Aes256GcmEncryptor::new("k1", &[0x99; 32]));
    let mut bus = test_bus();
    bus.set_encryptor(enc);
    let bus = Arc::new(bus);

    let mut handles = Vec::new();
    for i in 0..20 {
        let bus = bus.clone();
        handles.push(tokio::spawn(async move {
            bus.publish(
                "secret",
                &format!("data.{}", i),
                &format!("Secret {}", i),
                "src",
                serde_json::json!({"i": i, "sensitive": true}),
            )
            .await
            .unwrap()
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // All should decrypt correctly
    let events = bus.list_events(None, 100).await.unwrap();
    assert_eq!(events.len(), 20);
    for event in &events {
        assert!(!a3s_event::EncryptedPayload::is_encrypted(&event.payload));
        assert_eq!(event.payload["sensitive"], true);
    }

    let snap = bus.metrics().snapshot();
    assert_eq!(snap.publish_count, 20);
    assert_eq!(snap.encrypt_count, 20);
    assert_eq!(snap.decrypt_count, 20);
}

// ─── Full Stack: All Features Combined ───────────────────────────

#[tokio::test]
async fn test_full_stack_all_features() {
    let registry = Arc::new(MemorySchemaRegistry::new());
    registry
        .register(EventSchema {
            event_type: "order.placed".to_string(),
            version: 1,
            required_fields: vec!["order_id".to_string(), "total".to_string()],
            description: "Order placement event".to_string(),
        })
        .unwrap();

    let store = Arc::new(MemoryStateStore::default());
    let dlq = Arc::new(MemoryDlqHandler::default());
    let enc = Arc::new(Aes256GcmEncryptor::new("prod-key", &[0xDE; 32]));

    let mut bus = EventBus::with_schema_registry(
        a3s_event::MemoryProvider::default(),
        registry,
    );
    bus.set_state_store(store.clone()).unwrap();
    bus.set_dlq_handler(dlq.clone());
    bus.set_encryptor(enc);

    // 1. Register subscription (persisted)
    bus.update_subscription(SubscriptionFilter {
        subscriber_id: "order-processor".to_string(),
        subjects: vec!["events.commerce.>".to_string()],
        durable: true,
        options: None,
    }).await.unwrap();

    // 2. Publish valid typed event (validated → encrypted → published)
    let order = Event::typed(
        "events.commerce.order",
        "commerce",
        "order.placed",
        1,
        "New order",
        "checkout",
        serde_json::json!({"order_id": "ORD-001", "total": 99.99, "items": 3}),
    );
    bus.publish_event(&order).await.unwrap();

    // 3. Publish invalid typed event (validation fails)
    let bad_order = Event::typed(
        "events.commerce.order",
        "commerce",
        "order.placed",
        1,
        "Bad order",
        "checkout",
        serde_json::json!({"items": 1}),
    );
    assert!(bus.publish_event(&bad_order).await.is_err());

    // 4. Publish untyped event (bypasses validation, still encrypted)
    bus.publish("commerce", "refund", "Refund issued", "billing",
        serde_json::json!({"order_id": "ORD-001", "amount": 99.99}),
    ).await.unwrap();

    // 5. Route a failed event to DLQ
    let failed = a3s_event::ReceivedEvent {
        event: Event::new(
            "events.commerce.order",
            "commerce",
            "Stuck order",
            "checkout",
            serde_json::json!({"order_id": "ORD-ERR"}),
        ),
        sequence: 99,
        num_delivered: 10,
        stream: "memory".to_string(),
    };
    dlq.handle(DeadLetterEvent::new(failed, "Processing timeout"))
        .await.unwrap();

    // ── Verify everything ──

    // History: 2 events, both decrypted
    let events = bus.list_events(Some("commerce"), 100).await.unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().any(|e| e.payload["order_id"] == "ORD-001" && e.payload["total"] == 99.99));
    assert!(events.iter().any(|e| e.payload["amount"] == 99.99));

    // DLQ: 1 dead letter
    assert_eq!(dlq.count().await.unwrap(), 1);

    // State: subscription persisted
    assert!(store.load().unwrap().contains_key("order-processor"));

    // Metrics: full picture
    let snap = bus.metrics().snapshot();
    assert_eq!(snap.publish_count, 2);
    assert_eq!(snap.validation_errors, 1);
    assert_eq!(snap.encrypt_count, 2);
    assert_eq!(snap.decrypt_count, 2);
    assert_eq!(snap.subscribe_count, 1);

    // Health
    assert!(bus.health().await.unwrap());

    // Provider info
    let info = bus.info().await.unwrap();
    assert_eq!(info.provider, "memory");
    assert_eq!(info.messages, 2);
}
