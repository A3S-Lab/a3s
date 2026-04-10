//! Event Aggregation - Cluster-wide event collection and querying.
//!
//! Events track significant occurrences in the cluster like pod created,
//! node joined, deployment scaled, etc. Events are queryable by type,
//! involved object, involved object name, and time range.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Event type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    Normal,
    Warning,
}

/// Event reason classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventReason {
    /// Resource created
    Created,
    /// Resource updated
    Updated,
    /// Resource deleted
    Deleted,
    /// Resource scheduled
    Scheduled,
    /// Resource started
    Started,
    /// Resource stopped
    Stopped,
    /// Resource restarted
    Restarted,
    /// Health check passed
    HealthCheckPassed,
    /// Health check failed
    HealthCheckFailed,
    /// Rolling update started
    RollingUpdateStarted,
    /// Rolling update completed
    RollingUpdateCompleted,
    /// Rolling update failed
    RollingUpdateFailed,
    /// Scale up occurred
    ScaleUp,
    /// Scale down occurred
    ScaleDown,
    /// Node joined cluster
    NodeJoined,
    /// Node left cluster
    NodeLeft,
    /// Node became ready
    NodeReady,
    /// Node became not ready
    NodeNotReady,
    /// Volume attached
    VolumeAttached,
    /// Volume detached
    VolumeDetached,
    /// Volume bound
    VolumeBound,
    /// Volume unbound
    VolumeUnbound,
    /// Pod scheduled
    PodScheduled,
    /// Pod unscheduled
    PodUnscheduled,
    /// Custom reason
    Custom(String),
}

impl EventReason {
    /// Get the string representation of the reason.
    pub fn as_str(&self) -> &str {
        match self {
            EventReason::Created => "Created",
            EventReason::Updated => "Updated",
            EventReason::Deleted => "Deleted",
            EventReason::Scheduled => "Scheduled",
            EventReason::Started => "Started",
            EventReason::Stopped => "Stopped",
            EventReason::Restarted => "Restarted",
            EventReason::HealthCheckPassed => "HealthCheckPassed",
            EventReason::HealthCheckFailed => "HealthCheckFailed",
            EventReason::RollingUpdateStarted => "RollingUpdateStarted",
            EventReason::RollingUpdateCompleted => "RollingUpdateCompleted",
            EventReason::RollingUpdateFailed => "RollingUpdateFailed",
            EventReason::ScaleUp => "ScaleUp",
            EventReason::ScaleDown => "ScaleDown",
            EventReason::NodeJoined => "NodeJoined",
            EventReason::NodeLeft => "NodeLeft",
            EventReason::NodeReady => "NodeReady",
            EventReason::NodeNotReady => "NodeNotReady",
            EventReason::VolumeAttached => "VolumeAttached",
            EventReason::VolumeDetached => "VolumeDetached",
            EventReason::VolumeBound => "VolumeBound",
            EventReason::VolumeUnbound => "VolumeUnbound",
            EventReason::PodScheduled => "PodScheduled",
            EventReason::PodUnscheduled => "PodUnscheduled",
            EventReason::Custom(s) => s,
        }
    }
}

impl std::fmt::Display for EventReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Involved object reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvolvedObject {
    /// API version of the object.
    pub api_version: String,
    /// Kind of the object.
    pub kind: String,
    /// Name of the object.
    pub name: String,
    /// UID of the object.
    pub uid: String,
    /// Namespace of the object (if applicable).
    pub namespace: Option<String>,
}

/// Cluster event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique identifier for the event.
    pub id: String,
    /// Event type (Normal or Warning).
    pub event_type: EventType,
    /// Reason for the event.
    pub reason: EventReason,
    /// Human-readable message describing the event.
    pub message: String,
    /// Involved object (the object this event is about).
    pub involved_object: InvolvedObject,
    /// Source of the event (component that generated it).
    pub source: EventSource,
    /// First occurrence timestamp.
    pub first_timestamp: DateTime<Utc>,
    /// Last occurrence timestamp.
    pub last_timestamp: DateTime<Utc>,
    /// Number of times this event has occurred.
    pub count: u32,
    /// Whether the event has been reviewed by an administrator.
    pub reviewed: bool,
}

/// Source of the event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSource {
    /// Component that generated the event.
    pub component: String,
    /// Host where the event occurred.
    pub host: Option<String>,
}

/// Event filter for querying.
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// Filter by event type.
    pub event_type: Option<EventType>,
    /// Filter by involved object kind.
    pub kind: Option<String>,
    /// Filter by involved object name.
    pub name: Option<String>,
    /// Filter by involved object namespace.
    pub namespace: Option<String>,
    /// Filter by reason.
    pub reason: Option<EventReason>,
    /// Filter by minimum count.
    pub min_count: Option<u32>,
    /// Filter by maximum count.
    pub max_count: Option<u32>,
    /// Filter by start time (inclusive).
    pub start_time: Option<DateTime<Utc>>,
    /// Filter by end time (inclusive).
    pub end_time: Option<DateTime<Utc>>,
    /// Filter by reviewed status.
    pub reviewed: Option<bool>,
    /// Filter by source component.
    pub component: Option<String>,
}

impl EventFilter {
    /// Check if an event matches this filter.
    pub fn matches(&self, event: &Event) -> bool {
        if let Some(event_type) = &self.event_type {
            if &event.event_type != event_type {
                return false;
            }
        }
        if let Some(kind) = &self.kind {
            if &event.involved_object.kind != kind {
                return false;
            }
        }
        if let Some(name) = &self.name {
            if &event.involved_object.name != name {
                return false;
            }
        }
        if let Some(namespace) = &self.namespace {
            if event.involved_object.namespace.as_ref() != Some(namespace) {
                return false;
            }
        }
        if let Some(reason) = &self.reason {
            if &event.reason != reason {
                return false;
            }
        }
        if let Some(min_count) = self.min_count {
            if event.count < min_count {
                return false;
            }
        }
        if let Some(max_count) = self.max_count {
            if event.count > max_count {
                return false;
            }
        }
        if let Some(start_time) = &self.start_time {
            if event.last_timestamp < *start_time {
                return false;
            }
        }
        if let Some(end_time) = &self.end_time {
            if event.last_timestamp > *end_time {
                return false;
            }
        }
        if let Some(reviewed) = &self.reviewed {
            if &event.reviewed != reviewed {
                return false;
            }
        }
        if let Some(component) = &self.component {
            if &event.source.component != component {
                return false;
            }
        }
        true
    }
}

/// Aggregated event summary.
#[derive(Debug, Clone)]
pub struct EventAggregation {
    /// The common reason among aggregated events.
    pub reason: EventReason,
    /// The involved object kind.
    pub kind: String,
    /// The involved object name.
    pub name: String,
    /// Namespace (if applicable).
    pub namespace: Option<String>,
    /// Total count of aggregated events.
    pub count: u32,
    /// Number of unique events aggregated.
    pub unique_event_count: usize,
    /// First occurrence.
    pub first_occurrence: DateTime<Utc>,
    /// Last occurrence.
    pub last_occurrence: DateTime<Utc>,
    /// Sample messages from aggregated events.
    pub sample_messages: Vec<String>,
}

/// Event aggregation key for grouping similar events.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventAggregationKey {
    pub reason: EventReason,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
}

/// Event controller for collecting and managing cluster events.
pub struct EventController {
    /// Stored events by ID.
    events: RwLock<HashMap<String, Event>>,
    /// Events organized by aggregation key.
    aggregation_index: RwLock<HashMap<EventAggregationKey, Vec<String>>>,
    /// TTL for events in seconds (default 7 days).
    ttl_seconds: u64,
    /// Maximum number of events to retain.
    max_events: usize,
}

impl EventController {
    /// Create a new event controller.
    pub fn new() -> Self {
        Self {
            events: RwLock::new(HashMap::new()),
            aggregation_index: RwLock::new(HashMap::new()),
            ttl_seconds: 7 * 24 * 60 * 60, // 7 days
            max_events: 100_000,
        }
    }

    /// Create a new event controller with custom settings.
    pub fn with_limits(ttl_seconds: u64, max_events: usize) -> Self {
        Self {
            events: RwLock::new(HashMap::new()),
            aggregation_index: RwLock::new(HashMap::new()),
            ttl_seconds,
            max_events,
        }
    }

    /// Generate a unique event ID.
    fn generate_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}.{:06}", duration.as_secs(), duration.subsec_micros())
    }

    /// Get the aggregation key for an event.
    fn get_aggregation_key(event: &Event) -> EventAggregationKey {
        EventAggregationKey {
            reason: event.reason.clone(),
            kind: event.involved_object.kind.clone(),
            name: event.involved_object.name.clone(),
            namespace: event.involved_object.namespace.clone(),
        }
    }

    /// Record a new event or update an existing one if similar.
    pub async fn record(&self, mut event: Event) {
        // Generate ID if not set
        if event.id.is_empty() {
            event.id = Self::generate_id();
        }

        // Update timestamps
        let now = Utc::now();
        // Use epoch (1970-01-01) as sentinel to detect unset timestamp
        let epoch = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        if event.first_timestamp <= epoch {
            event.first_timestamp = now;
        }
        event.last_timestamp = now;

        // Get aggregation key
        let key = Self::get_aggregation_key(&event);

        // Check for similar existing event to aggregate
        let aggregation_index = self.aggregation_index.read().await;
        if let Some(event_ids) = aggregation_index.get(&key) {
            if let Some(existing_id) = event_ids.first() {
                let existing_id = existing_id.clone(); // Clone to avoid borrow issues
                let events = self.events.read().await;
                if let Some(existing) = events.get(&existing_id) {
                    if existing.message == event.message && existing.reason == event.reason {
                        // Aggregate into existing event
                        drop(events);
                        drop(aggregation_index);
                        self.increment_event(&existing_id, now).await;
                        return;
                    }
                }
            }
        }
        drop(aggregation_index);

        // Insert new event
        let id = event.id.clone();
        let mut events = self.events.write().await;
        events.insert(id.clone(), event);

        // Update aggregation index
        let mut aggregation_index = self.aggregation_index.write().await;
        aggregation_index
            .entry(key)
            .or_insert_with(Vec::new)
            .push(id);

        // Check if we need to evict old events
        if events.len() > self.max_events {
            self.evict_old_events_locked(&mut events, &mut aggregation_index)
                .await;
        }
    }

    /// Increment the count of an existing event.
    async fn increment_event(&self, event_id: &str, timestamp: DateTime<Utc>) {
        let mut events = self.events.write().await;
        if let Some(event) = events.get_mut(event_id) {
            event.count += 1;
            event.last_timestamp = timestamp;
        }
    }

    /// Evict old events when exceeding max capacity.
    async fn evict_old_events_locked(
        &self,
        events: &mut HashMap<String, Event>,
        aggregation_index: &mut HashMap<EventAggregationKey, Vec<String>>,
    ) {
        // Find oldest events - collect IDs to remove
        let mut event_list: Vec<_> = events.iter().collect();
        event_list.sort_by(|a, b| a.1.last_timestamp.cmp(&b.1.last_timestamp));

        // Remove oldest 10%
        let remove_count = self.max_events / 10;
        let ids_to_remove: Vec<String> = event_list
            .into_iter()
            .take(remove_count)
            .map(|(id, _)| id.clone())
            .collect();

        for id in ids_to_remove {
            if let Some(event) = events.get(&id) {
                let key = Self::get_aggregation_key(event);
                if let Some(ids) = aggregation_index.get_mut(&key) {
                    ids.retain(|i| i != &id);
                    if ids.is_empty() {
                        aggregation_index.remove(&key);
                    }
                }
            }
            events.remove(&id);
        }
    }

    /// Delete events older than TTL.
    pub async fn cleanup_expired(&self) {
        let now = Utc::now();
        let cutoff = chrono::Duration::seconds(self.ttl_seconds as i64);
        let cutoff_time = now - cutoff;

        let mut events = self.events.write().await;
        let mut aggregation_index = self.aggregation_index.write().await;

        let to_remove: Vec<_> = events
            .iter()
            .filter(|(_, e)| e.last_timestamp < cutoff_time)
            .map(|(id, _)| id.clone())
            .collect();

        for id in &to_remove {
            if let Some(event) = events.get(id) {
                let key = Self::get_aggregation_key(event);
                if let Some(ids) = aggregation_index.get_mut(&key) {
                    ids.retain(|i| i != id);
                    if ids.is_empty() {
                        aggregation_index.remove(&key);
                    }
                }
            }
            events.remove(id);
        }
    }

    /// Get events matching the filter.
    pub async fn get_events(&self, filter: &EventFilter) -> Vec<Event> {
        let events = self.events.read().await;
        events
            .values()
            .filter(|e| filter.matches(e))
            .cloned()
            .collect()
    }

    /// Get a single event by ID.
    pub async fn get_event(&self, id: &str) -> Option<Event> {
        let events = self.events.read().await;
        events.get(id).cloned()
    }

    /// Get events for a specific object.
    pub async fn get_events_for_object(
        &self,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Vec<Event> {
        let filter = EventFilter {
            kind: Some(kind.to_string()),
            name: Some(name.to_string()),
            namespace: namespace.map(String::from),
            ..Default::default()
        };
        self.get_events(&filter).await
    }

    /// Get events by type.
    pub async fn get_events_by_type(&self, event_type: EventType) -> Vec<Event> {
        let filter = EventFilter {
            event_type: Some(event_type),
            ..Default::default()
        };
        self.get_events(&filter).await
    }

    /// Get warning events.
    pub async fn get_warning_events(&self) -> Vec<Event> {
        self.get_events_by_type(EventType::Warning).await
    }

    /// Get recent events.
    pub async fn get_recent_events(&self, limit: usize) -> Vec<Event> {
        let events = self.events.read().await;
        let mut event_list: Vec<_> = events.values().collect();
        event_list.sort_by(|a, b| b.last_timestamp.cmp(&a.last_timestamp));
        event_list.into_iter().take(limit).cloned().collect()
    }

    /// Aggregate events by reason and involved object.
    pub async fn aggregate_events(&self, filter: &EventFilter) -> Vec<EventAggregation> {
        let events = self.get_events(filter).await;
        let mut aggregated: HashMap<EventAggregationKey, EventAggregation> = HashMap::new();

        for event in events {
            let key = EventAggregationKey {
                reason: event.reason.clone(),
                kind: event.involved_object.kind.clone(),
                name: event.involved_object.name.clone(),
                namespace: event.involved_object.namespace.clone(),
            };

            if let Some(existing) = aggregated.get_mut(&key) {
                existing.count += event.count;
                existing.unique_event_count += 1;
                if event.first_timestamp < existing.first_occurrence {
                    existing.first_occurrence = event.first_timestamp;
                }
                if event.last_timestamp > existing.last_occurrence {
                    existing.last_occurrence = event.last_timestamp;
                }
                if existing.sample_messages.len() < 3
                    && !existing.sample_messages.contains(&event.message)
                {
                    existing.sample_messages.push(event.message);
                }
            } else {
                aggregated.insert(
                    key,
                    EventAggregation {
                        reason: event.reason.clone(),
                        kind: event.involved_object.kind.clone(),
                        name: event.involved_object.name.clone(),
                        namespace: event.involved_object.namespace.clone(),
                        count: event.count,
                        unique_event_count: 1,
                        first_occurrence: event.first_timestamp,
                        last_occurrence: event.last_timestamp,
                        sample_messages: vec![event.message],
                    },
                );
            }
        }

        aggregated.into_values().collect()
    }

    /// Mark an event as reviewed.
    pub async fn mark_reviewed(&self, id: &str) -> bool {
        let mut events = self.events.write().await;
        if let Some(event) = events.get_mut(id) {
            event.reviewed = true;
            true
        } else {
            false
        }
    }

    /// Delete an event by ID.
    pub async fn delete_event(&self, id: &str) -> bool {
        let mut events = self.events.write().await;
        let mut aggregation_index = self.aggregation_index.write().await;

        if let Some(event) = events.remove(id) {
            let key = Self::get_aggregation_key(&event);
            if let Some(ids) = aggregation_index.get_mut(&key) {
                ids.retain(|i| i != id);
                if ids.is_empty() {
                    aggregation_index.remove(&key);
                }
            }
            true
        } else {
            false
        }
    }

    /// Delete events for a specific object.
    pub async fn delete_events_for_object(
        &self,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> usize {
        let events = self.get_events_for_object(kind, name, namespace).await;
        let mut count = 0;
        for event in events {
            if self.delete_event(&event.id).await {
                count += 1;
            }
        }
        count
    }

    /// Get total event count.
    pub async fn event_count(&self) -> usize {
        let events = self.events.read().await;
        events.len()
    }

    /// Get event statistics.
    pub async fn get_stats(&self) -> EventStats {
        let events = self.events.read().await;
        let mut stats = EventStats::default();

        for event in events.values() {
            match event.event_type {
                EventType::Normal => stats.normal_count += 1,
                EventType::Warning => stats.warning_count += 1,
            }
            stats.total_count += 1;
            stats.total_occurrences += event.count;
        }

        stats
    }
}

impl Default for EventController {
    fn default() -> Self {
        Self::new()
    }
}

/// Event statistics.
#[derive(Debug, Clone, Default)]
pub struct EventStats {
    /// Total number of events.
    pub total_count: usize,
    /// Number of normal events.
    pub normal_count: usize,
    /// Number of warning events.
    pub warning_count: usize,
    /// Total number of occurrences (sum of counts).
    pub total_occurrences: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(
        event_type: EventType,
        reason: EventReason,
        kind: &str,
        name: &str,
    ) -> Event {
        Event {
            id: String::new(),
            event_type,
            reason,
            message: format!("Test event for {} {}", kind, name),
            involved_object: InvolvedObject {
                api_version: "a3s.io/v1".to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
                uid: "test-uid".to_string(),
                namespace: None,
            },
            source: EventSource {
                component: "test-controller".to_string(),
                host: Some("test-host".to_string()),
            },
            first_timestamp: Utc::now(),
            last_timestamp: Utc::now(),
            count: 1,
            reviewed: false,
        }
    }

    #[tokio::test]
    async fn test_record_and_get_event() {
        let controller = EventController::new();

        let event = create_test_event(EventType::Normal, EventReason::Created, "Pod", "test-pod");
        controller.record(event.clone()).await;

        let events = controller.get_events(&EventFilter::default()).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].involved_object.name, "test-pod");
    }

    #[tokio::test]
    async fn test_get_events_for_object() {
        let controller = EventController::new();

        controller
            .record(create_test_event(
                EventType::Normal,
                EventReason::Created,
                "Pod",
                "test-pod",
            ))
            .await;
        controller
            .record(create_test_event(
                EventType::Normal,
                EventReason::Started,
                "Pod",
                "test-pod",
            ))
            .await;
        controller
            .record(create_test_event(
                EventType::Normal,
                EventReason::Created,
                "Deployment",
                "test-pod",
            ))
            .await;

        let pod_events = controller
            .get_events_for_object("Pod", "test-pod", None)
            .await;
        assert_eq!(pod_events.len(), 2);

        let deployment_events = controller
            .get_events_for_object("Deployment", "test-pod", None)
            .await;
        assert_eq!(deployment_events.len(), 1);
    }

    #[tokio::test]
    async fn test_filter_by_type() {
        let controller = EventController::new();

        controller
            .record(create_test_event(
                EventType::Normal,
                EventReason::Created,
                "Pod",
                "pod1",
            ))
            .await;
        controller
            .record(create_test_event(
                EventType::Warning,
                EventReason::HealthCheckFailed,
                "Pod",
                "pod2",
            ))
            .await;

        let normal_events = controller.get_events_by_type(EventType::Normal).await;
        assert_eq!(normal_events.len(), 1);

        let warning_events = controller.get_events_by_type(EventType::Warning).await;
        assert_eq!(warning_events.len(), 1);
    }

    #[tokio::test]
    async fn test_aggregation() {
        let controller = EventController::new();

        for _ in 0..3 {
            controller
                .record(create_test_event(
                    EventType::Warning,
                    EventReason::HealthCheckFailed,
                    "Pod",
                    "sick-pod",
                ))
                .await;
        }

        let aggregations = controller.aggregate_events(&EventFilter::default()).await;
        assert_eq!(aggregations.len(), 1);
        assert_eq!(aggregations[0].count, 3);
    }

    #[tokio::test]
    async fn test_event_filter() {
        let controller = EventController::new();

        controller
            .record(create_test_event(
                EventType::Normal,
                EventReason::Created,
                "Pod",
                "pod1",
            ))
            .await;
        controller
            .record(create_test_event(
                EventType::Warning,
                EventReason::HealthCheckFailed,
                "Pod",
                "pod2",
            ))
            .await;

        let filter = EventFilter {
            event_type: Some(EventType::Warning),
            ..Default::default()
        };
        let events = controller.get_events(&filter).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].involved_object.name, "pod2");
    }

    #[tokio::test]
    async fn test_mark_reviewed() {
        let controller = EventController::new();

        let event = create_test_event(EventType::Normal, EventReason::Created, "Pod", "test-pod");
        controller.record(event).await;

        let events = controller.get_events(&EventFilter::default()).await;
        let event_id = events[0].id.clone();
        assert!(!events[0].reviewed);

        controller.mark_reviewed(&event_id).await;

        let events = controller.get_events(&EventFilter::default()).await;
        assert!(events[0].reviewed);
    }

    #[tokio::test]
    async fn test_delete_event() {
        let controller = EventController::new();

        let event = create_test_event(EventType::Normal, EventReason::Created, "Pod", "test-pod");
        controller.record(event).await;

        let events = controller.get_events(&EventFilter::default()).await;
        assert_eq!(events.len(), 1);

        let event_id = events[0].id.clone();
        let deleted = controller.delete_event(&event_id).await;
        assert!(deleted);

        let events = controller.get_events(&EventFilter::default()).await;
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_stats() {
        let controller = EventController::new();

        controller
            .record(create_test_event(
                EventType::Normal,
                EventReason::Created,
                "Pod",
                "pod1",
            ))
            .await;
        controller
            .record(create_test_event(
                EventType::Warning,
                EventReason::HealthCheckFailed,
                "Pod",
                "pod2",
            ))
            .await;

        let stats = controller.get_stats().await;
        assert_eq!(stats.total_count, 2);
        assert_eq!(stats.normal_count, 1);
        assert_eq!(stats.warning_count, 1);
    }

    #[test]
    fn test_event_reason_display() {
        assert_eq!(EventReason::Created.as_str(), "Created");
        assert_eq!(EventReason::Custom("foo".to_string()).as_str(), "foo");
        assert_eq!(EventReason::ScaleUp.as_str(), "ScaleUp");
    }

    #[test]
    fn test_event_filter_matches() {
        let filter = EventFilter {
            event_type: Some(EventType::Warning),
            kind: Some("Pod".to_string()),
            name: Some("test-pod".to_string()),
            ..Default::default()
        };

        let event = create_test_event(
            EventType::Warning,
            EventReason::HealthCheckFailed,
            "Pod",
            "test-pod",
        );
        assert!(filter.matches(&event));

        let wrong_type_event = create_test_event(
            EventType::Normal,
            EventReason::HealthCheckFailed,
            "Pod",
            "test-pod",
        );
        assert!(!filter.matches(&wrong_type_event));

        let wrong_kind_event = create_test_event(
            EventType::Warning,
            EventReason::HealthCheckFailed,
            "Deployment",
            "test-pod",
        );
        assert!(!filter.matches(&wrong_kind_event));
    }
}
