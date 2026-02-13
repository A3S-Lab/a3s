//! Event schema registry — validate and version event payloads
//!
//! Provides a `SchemaRegistry` trait for registering and validating
//! event schemas. No provider handles this — it's an application-level concern.

use crate::error::{EventError, Result};
use crate::types::Event;
use std::collections::HashMap;
use std::sync::RwLock;

/// Schema definition for an event type at a specific version
#[derive(Debug, Clone)]
pub struct EventSchema {
    /// Event type identifier (e.g., "forex.rate_change")
    pub event_type: String,

    /// Schema version
    pub version: u32,

    /// Required top-level fields in the payload
    pub required_fields: Vec<String>,

    /// Optional description of this schema version
    pub description: String,
}

/// Compatibility mode for schema evolution
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compatibility {
    /// New schema must be readable by old consumers (new fields optional)
    #[default]
    Backward,
    /// Old events must be readable by new consumers (no field removal)
    Forward,
    /// Both backward and forward compatible
    Full,
    /// No compatibility checks
    None,
}

/// Trait for event schema registries
///
/// Implementations store schema definitions and validate events
/// against registered schemas before publishing.
pub trait SchemaRegistry: Send + Sync {
    /// Register a schema for an event type at a specific version
    fn register(&self, schema: EventSchema) -> Result<()>;

    /// Get the schema for an event type at a specific version
    fn get(&self, event_type: &str, version: u32) -> Result<Option<EventSchema>>;

    /// Get the latest schema version for an event type
    fn latest_version(&self, event_type: &str) -> Result<Option<u32>>;

    /// List all registered event types
    fn list_types(&self) -> Result<Vec<String>>;

    /// Validate an event's payload against its registered schema
    ///
    /// Returns Ok(()) if valid or if no schema is registered (untyped events pass).
    fn validate(&self, event: &Event) -> Result<()>;

    /// Check if a new schema version is compatible with the previous version
    fn check_compatibility(
        &self,
        event_type: &str,
        new_version: u32,
        mode: Compatibility,
    ) -> Result<()>;
}

/// In-memory schema registry for development and testing
///
/// Stores schemas in a `HashMap` protected by `RwLock`.
/// Schemas are lost on process restart.
pub struct MemorySchemaRegistry {
    /// (event_type, version) → schema
    schemas: RwLock<HashMap<(String, u32), EventSchema>>,
}

impl MemorySchemaRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            schemas: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemorySchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaRegistry for MemorySchemaRegistry {
    fn register(&self, schema: EventSchema) -> Result<()> {
        if schema.event_type.is_empty() {
            return Err(EventError::Config(
                "Event type cannot be empty".to_string(),
            ));
        }
        if schema.version == 0 {
            return Err(EventError::Config(
                "Schema version must be >= 1".to_string(),
            ));
        }

        let key = (schema.event_type.clone(), schema.version);
        let mut schemas = self.schemas.write().map_err(|e| {
            EventError::Provider(format!("Schema registry lock poisoned: {}", e))
        })?;
        schemas.insert(key, schema);
        Ok(())
    }

    fn get(&self, event_type: &str, version: u32) -> Result<Option<EventSchema>> {
        let schemas = self.schemas.read().map_err(|e| {
            EventError::Provider(format!("Schema registry lock poisoned: {}", e))
        })?;
        Ok(schemas.get(&(event_type.to_string(), version)).cloned())
    }

    fn latest_version(&self, event_type: &str) -> Result<Option<u32>> {
        let schemas = self.schemas.read().map_err(|e| {
            EventError::Provider(format!("Schema registry lock poisoned: {}", e))
        })?;
        let max = schemas
            .keys()
            .filter(|(t, _)| t == event_type)
            .map(|(_, v)| *v)
            .max();
        Ok(max)
    }

    fn list_types(&self) -> Result<Vec<String>> {
        let schemas = self.schemas.read().map_err(|e| {
            EventError::Provider(format!("Schema registry lock poisoned: {}", e))
        })?;
        let mut types: Vec<String> = schemas
            .keys()
            .map(|(t, _)| t.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        types.sort();
        Ok(types)
    }

    fn validate(&self, event: &Event) -> Result<()> {
        // Untyped events always pass
        if event.event_type.is_empty() {
            return Ok(());
        }

        let schemas = self.schemas.read().map_err(|e| {
            EventError::Provider(format!("Schema registry lock poisoned: {}", e))
        })?;

        let key = (event.event_type.clone(), event.version);
        let schema = match schemas.get(&key) {
            Some(s) => s,
            None => return Ok(()), // No schema registered — pass through
        };

        // Validate required fields exist in payload
        if let serde_json::Value::Object(ref map) = event.payload {
            for field in &schema.required_fields {
                if !map.contains_key(field) {
                    return Err(EventError::SchemaValidation {
                        event_type: event.event_type.clone(),
                        version: event.version,
                        reason: format!("Missing required field '{}'", field),
                    });
                }
            }
        } else if !schema.required_fields.is_empty() {
            return Err(EventError::SchemaValidation {
                event_type: event.event_type.clone(),
                version: event.version,
                reason: "Payload must be a JSON object when schema has required fields"
                    .to_string(),
            });
        }

        Ok(())
    }

    fn check_compatibility(
        &self,
        event_type: &str,
        new_version: u32,
        mode: Compatibility,
    ) -> Result<()> {
        if mode == Compatibility::None || new_version <= 1 {
            return Ok(());
        }

        let prev_version = new_version - 1;
        let schemas = self.schemas.read().map_err(|e| {
            EventError::Provider(format!("Schema registry lock poisoned: {}", e))
        })?;

        let prev = match schemas.get(&(event_type.to_string(), prev_version)) {
            Some(s) => s,
            None => return Ok(()), // No previous version — compatible by default
        };

        let new = match schemas.get(&(event_type.to_string(), new_version)) {
            Some(s) => s,
            None => return Ok(()), // New version not registered yet
        };

        match mode {
            Compatibility::Backward => {
                // New schema can only ADD optional fields (no new required fields
                // that didn't exist before)
                for field in &new.required_fields {
                    if !prev.required_fields.contains(field) {
                        return Err(EventError::SchemaValidation {
                            event_type: event_type.to_string(),
                            version: new_version,
                            reason: format!(
                                "Backward incompatible: new required field '{}' \
                                 not in v{}",
                                field, prev_version
                            ),
                        });
                    }
                }
            }
            Compatibility::Forward => {
                // Old required fields must still exist in new schema
                for field in &prev.required_fields {
                    if !new.required_fields.contains(field) {
                        return Err(EventError::SchemaValidation {
                            event_type: event_type.to_string(),
                            version: new_version,
                            reason: format!(
                                "Forward incompatible: required field '{}' from v{} \
                                 removed in v{}",
                                field, prev_version, new_version
                            ),
                        });
                    }
                }
            }
            Compatibility::Full => {
                // Both directions: fields must be identical
                if prev.required_fields != new.required_fields {
                    return Err(EventError::SchemaValidation {
                        event_type: event_type.to_string(),
                        version: new_version,
                        reason: format!(
                            "Full incompatible: required fields differ between v{} and v{}",
                            prev_version, new_version
                        ),
                    });
                }
            }
            Compatibility::None => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> MemorySchemaRegistry {
        MemorySchemaRegistry::new()
    }

    #[test]
    fn test_register_and_get() {
        let reg = test_registry();
        reg.register(EventSchema {
            event_type: "forex.rate_change".to_string(),
            version: 1,
            required_fields: vec!["rate".to_string(), "currency".to_string()],
            description: "Forex rate change event".to_string(),
        })
        .unwrap();

        let schema = reg.get("forex.rate_change", 1).unwrap().unwrap();
        assert_eq!(schema.event_type, "forex.rate_change");
        assert_eq!(schema.version, 1);
        assert_eq!(schema.required_fields, vec!["rate", "currency"]);
    }

    #[test]
    fn test_get_nonexistent() {
        let reg = test_registry();
        assert!(reg.get("nonexistent", 1).unwrap().is_none());
    }

    #[test]
    fn test_register_empty_type_fails() {
        let reg = test_registry();
        let result = reg.register(EventSchema {
            event_type: "".to_string(),
            version: 1,
            required_fields: vec![],
            description: String::new(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_register_zero_version_fails() {
        let reg = test_registry();
        let result = reg.register(EventSchema {
            event_type: "test".to_string(),
            version: 0,
            required_fields: vec![],
            description: String::new(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_latest_version() {
        let reg = test_registry();
        for v in 1..=3 {
            reg.register(EventSchema {
                event_type: "test.event".to_string(),
                version: v,
                required_fields: vec![],
                description: String::new(),
            })
            .unwrap();
        }

        assert_eq!(reg.latest_version("test.event").unwrap(), Some(3));
        assert_eq!(reg.latest_version("nonexistent").unwrap(), None);
    }

    #[test]
    fn test_list_types() {
        let reg = test_registry();
        reg.register(EventSchema {
            event_type: "b.event".to_string(),
            version: 1,
            required_fields: vec![],
            description: String::new(),
        })
        .unwrap();
        reg.register(EventSchema {
            event_type: "a.event".to_string(),
            version: 1,
            required_fields: vec![],
            description: String::new(),
        })
        .unwrap();
        reg.register(EventSchema {
            event_type: "a.event".to_string(),
            version: 2,
            required_fields: vec![],
            description: String::new(),
        })
        .unwrap();

        let types = reg.list_types().unwrap();
        assert_eq!(types, vec!["a.event", "b.event"]);
    }

    #[test]
    fn test_validate_untyped_event_passes() {
        let reg = test_registry();
        let event = Event::new(
            "events.test.a",
            "test",
            "Test",
            "test",
            serde_json::json!({}),
        );
        assert!(reg.validate(&event).is_ok());
    }

    #[test]
    fn test_validate_no_schema_registered_passes() {
        let reg = test_registry();
        let event = Event::typed(
            "events.test.a",
            "test",
            "unknown.type",
            1,
            "Test",
            "test",
            serde_json::json!({}),
        );
        assert!(reg.validate(&event).is_ok());
    }

    #[test]
    fn test_validate_valid_event() {
        let reg = test_registry();
        reg.register(EventSchema {
            event_type: "forex.rate_change".to_string(),
            version: 1,
            required_fields: vec!["rate".to_string(), "currency".to_string()],
            description: String::new(),
        })
        .unwrap();

        let event = Event::typed(
            "events.market.forex",
            "market",
            "forex.rate_change",
            1,
            "Rate change",
            "reuters",
            serde_json::json!({"rate": 7.35, "currency": "USD/CNY"}),
        );
        assert!(reg.validate(&event).is_ok());
    }

    #[test]
    fn test_validate_missing_required_field() {
        let reg = test_registry();
        reg.register(EventSchema {
            event_type: "forex.rate_change".to_string(),
            version: 1,
            required_fields: vec!["rate".to_string(), "currency".to_string()],
            description: String::new(),
        })
        .unwrap();

        let event = Event::typed(
            "events.market.forex",
            "market",
            "forex.rate_change",
            1,
            "Rate change",
            "reuters",
            serde_json::json!({"rate": 7.35}), // missing "currency"
        );

        let err = reg.validate(&event).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("currency"), "Error should mention missing field: {}", msg);
    }

    #[test]
    fn test_validate_non_object_payload_with_required_fields() {
        let reg = test_registry();
        reg.register(EventSchema {
            event_type: "test.event".to_string(),
            version: 1,
            required_fields: vec!["field".to_string()],
            description: String::new(),
        })
        .unwrap();

        let event = Event::typed(
            "events.test.a",
            "test",
            "test.event",
            1,
            "Test",
            "test",
            serde_json::json!("not an object"),
        );

        assert!(reg.validate(&event).is_err());
    }

    #[test]
    fn test_backward_compatibility_ok() {
        let reg = test_registry();
        // v1: requires [rate]
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 1,
            required_fields: vec!["rate".to_string()],
            description: String::new(),
        })
        .unwrap();
        // v2: still requires [rate] (no new required fields)
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 2,
            required_fields: vec!["rate".to_string()],
            description: String::new(),
        })
        .unwrap();

        assert!(reg
            .check_compatibility("forex", 2, Compatibility::Backward)
            .is_ok());
    }

    #[test]
    fn test_backward_compatibility_fail() {
        let reg = test_registry();
        // v1: requires [rate]
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 1,
            required_fields: vec!["rate".to_string()],
            description: String::new(),
        })
        .unwrap();
        // v2: requires [rate, currency] — new required field breaks backward compat
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 2,
            required_fields: vec!["rate".to_string(), "currency".to_string()],
            description: String::new(),
        })
        .unwrap();

        let err = reg
            .check_compatibility("forex", 2, Compatibility::Backward)
            .unwrap_err();
        assert!(err.to_string().contains("currency"));
    }

    #[test]
    fn test_forward_compatibility_fail() {
        let reg = test_registry();
        // v1: requires [rate, currency]
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 1,
            required_fields: vec!["rate".to_string(), "currency".to_string()],
            description: String::new(),
        })
        .unwrap();
        // v2: requires [rate] — removed currency breaks forward compat
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 2,
            required_fields: vec!["rate".to_string()],
            description: String::new(),
        })
        .unwrap();

        let err = reg
            .check_compatibility("forex", 2, Compatibility::Forward)
            .unwrap_err();
        assert!(err.to_string().contains("currency"));
    }

    #[test]
    fn test_full_compatibility() {
        let reg = test_registry();
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 1,
            required_fields: vec!["rate".to_string()],
            description: String::new(),
        })
        .unwrap();
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 2,
            required_fields: vec!["rate".to_string()],
            description: String::new(),
        })
        .unwrap();

        assert!(reg
            .check_compatibility("forex", 2, Compatibility::Full)
            .is_ok());
    }

    #[test]
    fn test_no_compatibility_always_passes() {
        let reg = test_registry();
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 1,
            required_fields: vec!["a".to_string()],
            description: String::new(),
        })
        .unwrap();
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 2,
            required_fields: vec!["b".to_string()],
            description: String::new(),
        })
        .unwrap();

        assert!(reg
            .check_compatibility("forex", 2, Compatibility::None)
            .is_ok());
    }

    #[test]
    fn test_compatibility_no_previous_version() {
        let reg = test_registry();
        reg.register(EventSchema {
            event_type: "forex".to_string(),
            version: 1,
            required_fields: vec!["rate".to_string()],
            description: String::new(),
        })
        .unwrap();

        // v1 has no previous — always compatible
        assert!(reg
            .check_compatibility("forex", 1, Compatibility::Full)
            .is_ok());
    }
}
