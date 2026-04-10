//! LimitRange Controller - Default resource limits per namespace.
//!
//! LimitRange enforces default resource limits within a namespace:
//! - Setting default limits for containers
//! - Enforcing min/max limits on resources
//! - Tracking actual usage against limits

use crate::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// LimitRange type enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LimitType {
    /// Container-level limits.
    Container,
    /// Pod-level limits.
    Pod,
    /// PersistentVolumeClaim limits.
    PersistentVolumeClaim,
}

/// A single limit range item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitRangeItem {
    /// Type of resource this limit applies to.
    #[serde(rename = "type")]
    pub limit_type: LimitType,
    /// Max limits for the resource.
    #[serde(default)]
    pub max: Option<HashMap<String, i64>>,
    /// Min limits for the resource.
    #[serde(default)]
    pub min: Option<HashMap<String, i64>>,
    /// Default limits for the resource.
    #[serde(default)]
    pub default: Option<HashMap<String, i64>>,
    /// Default request limits for the resource.
    #[serde(default)]
    pub default_request: Option<HashMap<String, i64>>,
    /// Max limit to request ratio.
    #[serde(rename = "maxLimitRequestRatio", default)]
    pub max_limit_request_ratio: Option<HashMap<String, i64>>,
}

/// LimitRange spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitRangeSpec {
    /// List of limit range items.
    pub limits: Vec<LimitRangeItem>,
}

/// LimitRange status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitRangeStatus {
    /// Current limits.
    pub limits: Vec<LimitRangeItem>,
}

/// LimitRange desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitRangeDesired {
    /// Name of the limit range.
    pub name: String,
    /// Namespace this limit range belongs to.
    pub namespace: String,
    /// Spec defining the limits.
    pub spec: LimitRangeSpec,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// LimitRangeController enforces limit ranges per namespace.
pub struct LimitRangeController {
    /// LimitRanges by namespace/name.
    limitranges: RwLock<HashMap<String, HashMap<String, LimitRangeDesired>>>,
}

impl LimitRangeController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            limitranges: RwLock::new(HashMap::new()),
        }
    }

    /// Set a limit range.
    pub async fn set_limitrange(&self, limitrange: LimitRangeDesired) -> Result<()> {
        let mut limitranges = self.limitranges.write().await;
        limitranges
            .entry(limitrange.namespace.clone())
            .or_insert_with(HashMap::new);
        limitranges
            .get_mut(&limitrange.namespace)
            .unwrap()
            .insert(limitrange.name.clone(), limitrange);
        Ok(())
    }

    /// Get a limit range.
    pub async fn get_limitrange(&self, namespace: &str, name: &str) -> Option<LimitRangeDesired> {
        let limitranges = self.limitranges.read().await;
        limitranges
            .get(namespace)
            .and_then(|ns| ns.get(name).cloned())
    }

    /// Delete a limit range.
    pub async fn delete_limitrange(&self, namespace: &str, name: &str) -> Result<()> {
        let mut limitranges = self.limitranges.write().await;
        if let Some(ns_lr) = limitranges.get_mut(namespace) {
            ns_lr.remove(name);
        }
        Ok(())
    }

    /// List limit ranges in a namespace.
    pub async fn list_namespace_limitranges(&self, namespace: &str) -> Vec<LimitRangeDesired> {
        let limitranges = self.limitranges.read().await;
        limitranges
            .get(namespace)
            .map(|ns| ns.values().cloned().collect())
            .unwrap_or_default()
    }

    /// List all limit ranges.
    pub async fn list_limitranges(&self) -> Vec<LimitRangeDesired> {
        let limitranges = self.limitranges.read().await;
        limitranges
            .values()
            .flat_map(|ns| ns.values().cloned())
            .collect()
    }

    /// Get default resources from LimitRange for a container.
    pub async fn get_container_defaults(
        &self,
        namespace: &str,
    ) -> Option<(HashMap<String, i64>, HashMap<String, i64>)> {
        let limitranges = self.limitranges.read().await;

        if let Some(ns_lr) = limitranges.get(namespace) {
            for lr in ns_lr.values() {
                for item in &lr.spec.limits {
                    if item.limit_type == LimitType::Container {
                        let mut default_limits = HashMap::new();
                        let mut default_requests = HashMap::new();

                        if let Some(default) = &item.default {
                            for (k, v) in default {
                                default_limits.insert(k.clone(), *v);
                            }
                        }
                        if let Some(default_req) = &item.default_request {
                            for (k, v) in default_req {
                                default_requests.insert(k.clone(), *v);
                            }
                        }

                        if !default_limits.is_empty() || !default_requests.is_empty() {
                            return Some((default_limits, default_requests));
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for LimitRangeController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get_limitrange() {
        let controller = LimitRangeController::new();

        let lr = LimitRangeDesired {
            name: "test-lr".to_string(),
            namespace: "default".to_string(),
            spec: LimitRangeSpec {
                limits: vec![LimitRangeItem {
                    limit_type: LimitType::Container,
                    max: Some(HashMap::from([
                        ("cpu".to_string(), 2000),
                        ("memory".to_string(), 2 * 1024 * 1024 * 1024),
                    ])),
                    min: Some(HashMap::from([
                        ("cpu".to_string(), 100),
                        ("memory".to_string(), 128 * 1024 * 1024),
                    ])),
                    default: Some(HashMap::from([
                        ("cpu".to_string(), 500),
                        ("memory".to_string(), 512 * 1024 * 1024),
                    ])),
                    default_request: None,
                    max_limit_request_ratio: None,
                }],
            },
            created_at: Utc::now(),
        };

        controller.set_limitrange(lr.clone()).await.unwrap();

        let retrieved = controller.get_limitrange("default", "test-lr").await;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "test-lr");
        assert_eq!(retrieved.namespace, "default");
    }

    #[tokio::test]
    async fn test_delete_limitrange() {
        let controller = LimitRangeController::new();

        let lr = LimitRangeDesired {
            name: "test-lr".to_string(),
            namespace: "default".to_string(),
            spec: LimitRangeSpec { limits: vec![] },
            created_at: Utc::now(),
        };

        controller.set_limitrange(lr).await.unwrap();
        controller
            .delete_limitrange("default", "test-lr")
            .await
            .unwrap();

        let retrieved = controller.get_limitrange("default", "test-lr").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_namespace_limitranges() {
        let controller = LimitRangeController::new();

        for i in 0..3 {
            let lr = LimitRangeDesired {
                name: format!("lr-{}", i),
                namespace: "default".to_string(),
                spec: LimitRangeSpec { limits: vec![] },
                created_at: Utc::now(),
            };
            controller.set_limitrange(lr).await.unwrap();
        }

        let list = controller.list_namespace_limitranges("default").await;
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_get_container_defaults() {
        let controller = LimitRangeController::new();

        let lr = LimitRangeDesired {
            name: "test-lr".to_string(),
            namespace: "default".to_string(),
            spec: LimitRangeSpec {
                limits: vec![LimitRangeItem {
                    limit_type: LimitType::Container,
                    max: None,
                    min: None,
                    default: Some(HashMap::from([
                        ("cpu".to_string(), 500),
                        ("memory".to_string(), 512 * 1024 * 1024),
                    ])),
                    default_request: Some(HashMap::from([
                        ("cpu".to_string(), 100),
                        ("memory".to_string(), 128 * 1024 * 1024),
                    ])),
                    max_limit_request_ratio: None,
                }],
            },
            created_at: Utc::now(),
        };

        controller.set_limitrange(lr).await.unwrap();

        let defaults = controller.get_container_defaults("default").await;
        assert!(defaults.is_some());

        let (limits, requests) = defaults.unwrap();
        assert_eq!(limits.get("cpu"), Some(&500));
        assert_eq!(requests.get("cpu"), Some(&100));
    }
}
