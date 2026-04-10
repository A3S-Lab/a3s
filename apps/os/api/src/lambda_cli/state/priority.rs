//! PriorityClass - Pod priority scheduling.
//!
//! PriorityClass defines a mapping from priority class name to the priority value.

use crate::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// PriorityClass represents a priority class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityClass {
    /// Name.
    pub name: String,
    /// Priority value (higher = more priority).
    pub value: i32,
    /// Global default.
    pub global_default: bool,
    /// Description.
    pub description: String,
    /// Preemption policy.
    pub preemption_policy: PreemptionPolicy,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PreemptionPolicy {
    /// Lower priority pods may be preempted.
    PreemptLowerPriority,
    /// Pod will not be preempted.
    Never,
}

impl Default for PreemptionPolicy {
    fn default() -> Self {
        PreemptionPolicy::PreemptLowerPriority
    }
}

/// PriorityClass controller.
pub struct PriorityClassController {
    /// Priority classes by name.
    classes: RwLock<HashMap<String, PriorityClass>>,
    /// Default priority.
    default_priority: i32,
}

impl PriorityClassController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            classes: RwLock::new(HashMap::new()),
            default_priority: 0,
        }
    }

    /// Set priority class.
    pub async fn set(&self, pc: PriorityClass) -> Result<()> {
        let mut classes = self.classes.write().await;
        classes.insert(pc.name.clone(), pc);
        Ok(())
    }

    /// Get priority class.
    pub async fn get(&self, name: &str) -> Option<PriorityClass> {
        let classes = self.classes.read().await;
        classes.get(name).cloned()
    }

    /// Delete priority class.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let mut classes = self.classes.write().await;
        classes.remove(name);
        Ok(())
    }

    /// List all priority classes.
    pub async fn list(&self) -> Vec<PriorityClass> {
        let classes = self.classes.read().await;
        classes.values().cloned().collect()
    }

    /// Get priority value for a class name.
    pub async fn get_priority(&self, name: &str) -> i32 {
        self.get(name)
            .await
            .map(|pc| pc.value)
            .unwrap_or(self.default_priority)
    }
}

impl Default for PriorityClassController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_priority_class() {
        let controller = PriorityClassController::new();

        let pc = PriorityClass {
            name: "high-priority".to_string(),
            value: 1000,
            global_default: false,
            description: "High priority class".to_string(),
            preemption_policy: PreemptionPolicy::PreemptLowerPriority,
            created_at: Utc::now(),
        };

        controller.set(pc).await.unwrap();

        let found = controller.get("high-priority").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().value, 1000);
    }
}
