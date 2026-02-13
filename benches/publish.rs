//! Performance benchmarks for a3s-event
//!
//! Run with: cargo bench
//! Or via justfile: just bench

use a3s_event::provider::memory::MemoryProvider;
use a3s_event::{Event, EventBus};
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_event_creation(c: &mut Criterion) {
    c.bench_function("Event::new", |b| {
        b.iter(|| {
            Event::new(
                "events.market.forex",
                "market",
                "Rate change",
                "reuters",
                serde_json::json!({"rate": 7.35}),
            )
        });
    });

    c.bench_function("Event::typed", |b| {
        b.iter(|| {
            Event::typed(
                "events.market.forex",
                "market",
                "forex.rate",
                1,
                "Rate change",
                "reuters",
                serde_json::json!({"rate": 7.35}),
            )
        });
    });
}

fn bench_event_serialization(c: &mut Criterion) {
    let event = Event::new(
        "events.market.forex",
        "market",
        "Rate change",
        "reuters",
        serde_json::json!({"rate": 7.35, "currency": "USD/CNY", "source": "reuters"}),
    );

    c.bench_function("Event serialize", |b| {
        b.iter(|| serde_json::to_vec(&event).unwrap());
    });

    let bytes = serde_json::to_vec(&event).unwrap();
    c.bench_function("Event deserialize", |b| {
        b.iter(|| serde_json::from_slice::<Event>(&bytes).unwrap());
    });
}

fn bench_memory_publish(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("MemoryProvider publish", |b| {
        b.to_async(&rt).iter(|| async {
            let bus = EventBus::new(MemoryProvider::default());
            bus.publish(
                "market",
                "forex",
                "Rate change",
                "reuters",
                serde_json::json!({"rate": 7.35}),
            )
            .await
            .unwrap()
        });
    });
}

fn bench_memory_publish_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("publish_throughput");
    for count in [10, 100, 1000] {
        group.bench_function(format!("{} events", count), |b| {
            b.to_async(&rt).iter(|| async {
                let bus = EventBus::new(MemoryProvider::default());
                for i in 0..count {
                    bus.publish(
                        "market",
                        &format!("topic.{}", i),
                        "Event",
                        "test",
                        serde_json::json!({"i": i}),
                    )
                    .await
                    .unwrap();
                }
            });
        });
    }
    group.finish();
}

fn bench_memory_history(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Pre-populate
    let bus = rt.block_on(async {
        let bus = EventBus::new(MemoryProvider::default());
        for i in 0..1000 {
            bus.publish(
                "market",
                &format!("topic.{}", i % 10),
                "Event",
                "test",
                serde_json::json!({"i": i}),
            )
            .await
            .unwrap();
        }
        bus
    });

    c.bench_function("history (all, limit 100)", |b| {
        b.to_async(&rt)
            .iter(|| async { bus.list_events(None, 100).await.unwrap() });
    });

    c.bench_function("history (filtered, limit 100)", |b| {
        b.to_async(&rt)
            .iter(|| async { bus.list_events(Some("market"), 100).await.unwrap() });
    });
}

criterion_group!(
    benches,
    bench_event_creation,
    bench_event_serialization,
    bench_memory_publish,
    bench_memory_publish_throughput,
    bench_memory_history,
);
criterion_main!(benches);
