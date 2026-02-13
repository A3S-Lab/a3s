//! NATS JetStream client — connect, publish, subscribe, query

use super::config::{NatsConfig, StorageType};
use super::subscriber::NatsSubscription;
use crate::error::{EventError, Result};
use crate::types::{DeliverPolicy, Event, PublishOptions, SubscribeOptions};
use async_nats::jetstream;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// NATS JetStream client
///
/// Low-level client for publishing and subscribing to events via NATS.
/// Manages the connection and JetStream stream lifecycle.
pub struct NatsClient {
    /// NATS client connection
    client: async_nats::Client,

    /// JetStream context
    jetstream: jetstream::Context,

    /// JetStream stream handle (Mutex for methods requiring &mut self)
    stream: Mutex<jetstream::stream::Stream>,

    /// Configuration
    config: Arc<NatsConfig>,
}

impl NatsClient {
    /// Connect to NATS and initialize the JetStream stream
    pub async fn connect(config: NatsConfig) -> Result<Self> {
        let connect_opts = build_connect_options(&config);

        let client = connect_opts
            .connect(&config.url)
            .await
            .map_err(|e| EventError::Connection(format!("{}: {}", config.url, e)))?;

        tracing::info!(url = %config.url, "Connected to NATS");

        let jetstream = jetstream::new(client.clone());
        let stream = ensure_stream(&jetstream, &config).await?;

        Ok(Self {
            client,
            jetstream,
            stream: Mutex::new(stream),
            config: Arc::new(config),
        })
    }

    /// Publish an event, returning the JetStream sequence number
    pub async fn publish(&self, event: &Event) -> Result<u64> {
        let payload = serde_json::to_vec(event)?;

        let ack = self
            .jetstream
            .publish(event.subject.clone(), payload.into())
            .await
            .map_err(|e| EventError::Publish {
                subject: event.subject.clone(),
                reason: e.to_string(),
            })?
            .await
            .map_err(|e| EventError::Publish {
                subject: event.subject.clone(),
                reason: format!("ack failed: {}", e),
            })?;

        tracing::debug!(
            event_id = %event.id,
            subject = %event.subject,
            sequence = ack.sequence,
            "Event published"
        );

        Ok(ack.sequence)
    }

    /// Publish an event with options (dedup, expected sequence, timeout)
    pub async fn publish_with_options(
        &self,
        event: &Event,
        opts: &PublishOptions,
    ) -> Result<u64> {
        let payload = serde_json::to_vec(event)?;

        let mut headers = async_nats::HeaderMap::new();

        if let Some(ref msg_id) = opts.msg_id {
            headers.insert("Nats-Msg-Id", msg_id.as_str());
        }

        if let Some(seq) = opts.expected_sequence {
            headers.insert(
                "Nats-Expected-Last-Sequence",
                seq.to_string().as_str(),
            );
        }

        let ack_fut = if headers.is_empty() {
            self.jetstream
                .publish(event.subject.clone(), payload.into())
                .await
        } else {
            self.jetstream
                .publish_with_headers(event.subject.clone(), headers, payload.into())
                .await
        }
        .map_err(|e| EventError::Publish {
            subject: event.subject.clone(),
            reason: e.to_string(),
        })?;

        let ack = if let Some(timeout_secs) = opts.timeout_secs {
            tokio::time::timeout(Duration::from_secs(timeout_secs), ack_fut)
                .await
                .map_err(|_| EventError::Timeout(format!(
                    "Publish ack timed out after {}s for subject '{}'",
                    timeout_secs, event.subject
                )))?
        } else {
            ack_fut.await
        }
        .map_err(|e| EventError::Publish {
            subject: event.subject.clone(),
            reason: format!("ack failed: {}", e),
        })?;

        tracing::debug!(
            event_id = %event.id,
            subject = %event.subject,
            sequence = ack.sequence,
            msg_id = ?opts.msg_id,
            "Event published with options"
        );

        Ok(ack.sequence)
    }

    /// Create a durable pull consumer with options
    pub async fn subscribe_durable_with_options(
        &self,
        consumer_name: &str,
        filter_subject: &str,
        opts: &SubscribeOptions,
    ) -> Result<NatsSubscription> {
        let config = build_consumer_config(
            filter_subject,
            Some(consumer_name),
            opts,
        );

        let consumer = self
            .stream
            .lock()
            .await
            .get_or_create_consumer(consumer_name, config)
            .await
            .map_err(|e| EventError::Consumer(format!(
                "Failed to create durable consumer '{}': {}",
                consumer_name, e
            )))?;

        let messages = consumer.messages().await.map_err(|e| {
            EventError::Subscribe {
                subject: filter_subject.to_string(),
                reason: e.to_string(),
            }
        })?;

        tracing::info!(
            consumer = consumer_name,
            filter = filter_subject,
            max_deliver = ?opts.max_deliver,
            max_ack_pending = ?opts.max_ack_pending,
            "Durable subscription created with options"
        );

        Ok(NatsSubscription::new(
            messages,
            self.config.stream_name.clone(),
        ))
    }

    /// Create an ephemeral pull consumer with options
    pub async fn subscribe_with_options(
        &self,
        filter_subject: &str,
        opts: &SubscribeOptions,
    ) -> Result<NatsSubscription> {
        let config = build_consumer_config(filter_subject, None, opts);

        let consumer = self
            .stream
            .lock()
            .await
            .create_consumer(config)
            .await
            .map_err(|e| EventError::Consumer(format!(
                "Failed to create ephemeral consumer: {}",
                e
            )))?;

        let messages = consumer.messages().await.map_err(|e| {
            EventError::Subscribe {
                subject: filter_subject.to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(NatsSubscription::new(
            messages,
            self.config.stream_name.clone(),
        ))
    }

    /// Create a durable pull consumer and return a subscription
    pub async fn subscribe_durable(
        &self,
        consumer_name: &str,
        filter_subject: &str,
    ) -> Result<NatsSubscription> {
        let consumer = self
            .stream
            .lock()
            .await
            .get_or_create_consumer(
                consumer_name,
                jetstream::consumer::pull::Config {
                    durable_name: Some(consumer_name.to_string()),
                    filter_subject: filter_subject.to_string(),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| EventError::Consumer(format!(
                "Failed to create durable consumer '{}': {}",
                consumer_name, e
            )))?;

        let messages = consumer.messages().await.map_err(|e| {
            EventError::Subscribe {
                subject: filter_subject.to_string(),
                reason: e.to_string(),
            }
        })?;

        tracing::info!(
            consumer = consumer_name,
            filter = filter_subject,
            "Durable subscription created"
        );

        Ok(NatsSubscription::new(
            messages,
            self.config.stream_name.clone(),
        ))
    }

    /// Create an ephemeral pull consumer
    pub async fn subscribe(&self, filter_subject: &str) -> Result<NatsSubscription> {
        let consumer = self
            .stream
            .lock()
            .await
            .create_consumer(jetstream::consumer::pull::Config {
                filter_subject: filter_subject.to_string(),
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                ..Default::default()
            })
            .await
            .map_err(|e| EventError::Consumer(format!(
                "Failed to create ephemeral consumer: {}",
                e
            )))?;

        let messages = consumer.messages().await.map_err(|e| {
            EventError::Subscribe {
                subject: filter_subject.to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(NatsSubscription::new(
            messages,
            self.config.stream_name.clone(),
        ))
    }

    /// Fetch historical events from the stream
    pub async fn history(&self, filter_subject: Option<&str>, limit: usize) -> Result<Vec<Event>> {
        let mut config = jetstream::consumer::pull::Config {
            deliver_policy: jetstream::consumer::DeliverPolicy::Last,
            ack_policy: jetstream::consumer::AckPolicy::None,
            ..Default::default()
        };

        if let Some(subject) = filter_subject {
            config.filter_subject = subject.to_string();
        }

        let consumer = self
            .stream
            .lock()
            .await
            .create_consumer(config)
            .await
            .map_err(|e| EventError::Consumer(format!("Failed to create history consumer: {}", e)))?;

        let mut events = Vec::with_capacity(limit);
        let batch = consumer
            .fetch()
            .max_messages(limit)
            .expires(Duration::from_secs(self.config.request_timeout_secs))
            .messages()
            .await
            .map_err(|e| EventError::JetStream(format!("Failed to fetch history: {}", e)))?;

        use futures::StreamExt;
        let mut batch = std::pin::pin!(batch);
        while let Some(msg) = batch.next().await {
            match msg {
                Ok(msg) => {
                    if let Ok(event) = serde_json::from_slice::<Event>(&msg.payload) {
                        events.push(event);
                    }
                    if events.len() >= limit {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Error fetching history message: {}", e);
                    break;
                }
            }
        }

        Ok(events)
    }

    /// Delete a durable consumer
    pub async fn unsubscribe(&self, consumer_name: &str) -> Result<()> {
        self.stream
            .lock()
            .await
            .delete_consumer(consumer_name)
            .await
            .map_err(|e| EventError::Consumer(format!(
                "Failed to delete consumer '{}': {}",
                consumer_name, e
            )))?;

        tracing::info!(consumer = consumer_name, "Consumer deleted");
        Ok(())
    }

    /// Get stream info
    pub async fn stream_info(&self) -> Result<StreamInfo> {
        let mut stream = self.stream.lock().await;
        let info = stream
            .info()
            .await
            .map_err(|e| EventError::Stream(format!("Failed to get stream info: {}", e)))?;

        Ok(StreamInfo {
            messages: info.state.messages,
            bytes: info.state.bytes,
            first_sequence: info.state.first_sequence,
            last_sequence: info.state.last_sequence,
            consumer_count: info.state.consumer_count,
        })
    }

    /// Get the underlying NATS client
    pub fn nats_client(&self) -> &async_nats::Client {
        &self.client
    }

    /// Get the JetStream context
    pub fn jetstream_context(&self) -> &jetstream::Context {
        &self.jetstream
    }

    /// Get the configuration
    pub fn config(&self) -> &NatsConfig {
        &self.config
    }
}

/// Summary of stream state
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub messages: u64,
    pub bytes: u64,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub consumer_count: usize,
}

/// Build a JetStream pull consumer config from SubscribeOptions
fn build_consumer_config(
    filter_subject: &str,
    durable_name: Option<&str>,
    opts: &SubscribeOptions,
) -> jetstream::consumer::pull::Config {
    let deliver_policy = match &opts.deliver_policy {
        DeliverPolicy::All => jetstream::consumer::DeliverPolicy::All,
        DeliverPolicy::Last => jetstream::consumer::DeliverPolicy::Last,
        DeliverPolicy::New => jetstream::consumer::DeliverPolicy::New,
        DeliverPolicy::ByStartSequence { sequence } => {
            jetstream::consumer::DeliverPolicy::ByStartSequence {
                start_sequence: *sequence,
            }
        }
        DeliverPolicy::ByStartTime { timestamp } => {
            let secs = *timestamp / 1000;
            let nanos = ((*timestamp % 1000) * 1_000_000) as u32;
            let time = time::OffsetDateTime::from_unix_timestamp(secs as i64)
                .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
                + time::Duration::nanoseconds(nanos as i64);
            jetstream::consumer::DeliverPolicy::ByStartTime { start_time: time }
        }
        DeliverPolicy::LastPerSubject => {
            jetstream::consumer::DeliverPolicy::LastPerSubject
        }
    };

    let backoff: Vec<Duration> = opts
        .backoff_secs
        .iter()
        .map(|s| Duration::from_secs(*s))
        .collect();

    jetstream::consumer::pull::Config {
        durable_name: durable_name.map(|s| s.to_string()),
        filter_subject: filter_subject.to_string(),
        ack_policy: jetstream::consumer::AckPolicy::Explicit,
        deliver_policy,
        max_deliver: opts.max_deliver.unwrap_or(0),
        max_ack_pending: opts.max_ack_pending.unwrap_or(0),
        ack_wait: opts
            .ack_wait_secs
            .map(Duration::from_secs)
            .unwrap_or_default(),
        backoff,
        ..Default::default()
    }
}

/// Build NATS connect options from config
fn build_connect_options(config: &NatsConfig) -> async_nats::ConnectOptions {
    let mut opts = async_nats::ConnectOptions::new()
        .connection_timeout(Duration::from_secs(config.connect_timeout_secs))
        .request_timeout(Some(Duration::from_secs(config.request_timeout_secs)));

    if let Some(ref token) = config.token {
        opts = opts.token(token.clone());
    }

    opts
}

/// Ensure the JetStream stream exists with the correct configuration
async fn ensure_stream(
    js: &jetstream::Context,
    config: &NatsConfig,
) -> Result<jetstream::stream::Stream> {
    let storage = match config.storage {
        StorageType::File => jetstream::stream::StorageType::File,
        StorageType::Memory => jetstream::stream::StorageType::Memory,
    };

    let max_age = if config.max_age_secs > 0 {
        Duration::from_secs(config.max_age_secs)
    } else {
        Duration::ZERO
    };

    let stream_config = jetstream::stream::Config {
        name: config.stream_name.clone(),
        subjects: config.stream_subjects(),
        storage,
        max_messages: config.max_events,
        max_age,
        max_bytes: config.max_bytes,
        retention: jetstream::stream::RetentionPolicy::Limits,
        ..Default::default()
    };

    let stream = js
        .get_or_create_stream(stream_config)
        .await
        .map_err(|e| EventError::Stream(format!(
            "Failed to create/get stream '{}': {}",
            config.stream_name, e
        )))?;

    tracing::info!(
        stream = %config.stream_name,
        subjects = ?config.stream_subjects(),
        "JetStream stream ready"
    );

    Ok(stream)
}
