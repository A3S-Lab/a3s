//! # a3s-event
//!
//! Pluggable event subscription, dispatch, and persistence for the A3S ecosystem.
//!
//! ## Overview
//!
//! `a3s-event` provides a provider-agnostic API for publishing, subscribing to,
//! and persisting events. Swap backends (NATS, in-memory, Redis, Kafka, etc.)
//! without changing application code.
//!
//! ## Quick Start
//!
//! ```rust
//! use a3s_event::{EventBus, Event};
//! use a3s_event::provider::memory::MemoryProvider;
//!
//! # async fn example() -> a3s_event::Result<()> {
//! // Create an event bus with the in-memory provider
//! let bus = EventBus::new(MemoryProvider::default());
//!
//! // Publish an event
//! let event = bus.publish(
//!     "market",
//!     "forex.usd_cny",
//!     "USD/CNY broke through 7.35",
//!     "reuters",
//!     serde_json::json!({"rate": 7.3521}),
//! ).await?;
//!
//! println!("Published: {}", event.id);
//! # Ok(())
//! # }
//! ```
//!
//! ## Providers
//!
//! - **memory** — In-memory provider for testing and single-process use
//! - **nats** — NATS JetStream for distributed, persistent event streaming
//!
//! ## Architecture
//!
//! - **EventProvider** trait — core abstraction all backends implement
//! - **EventBus** — high-level API with subscription management
//! - **Subscription** trait — async event stream from any provider
//! - **Event** — provider-agnostic message envelope

pub mod error;
pub mod provider;
pub mod schema;
pub mod store;
pub mod types;

// Re-export core types
pub use error::{EventError, Result};
pub use provider::{EventProvider, PendingEvent, ProviderInfo, Subscription};
pub use schema::{Compatibility, EventSchema, MemorySchemaRegistry, SchemaRegistry};
pub use store::EventBus;
pub use types::{
    DeliverPolicy, Event, EventCounts, PublishOptions, ReceivedEvent, SubscribeOptions,
    SubscriptionFilter,
};

// Re-export providers for convenience
pub use provider::memory::{MemoryConfig, MemoryProvider};
pub use provider::nats::{NatsClient, NatsConfig, NatsProvider, NatsSubscription, StorageType};
