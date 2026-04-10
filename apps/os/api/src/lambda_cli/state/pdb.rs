//! Pod Disruption Budget (PDB).
//!
//! Ensures minimum number of pods available during voluntary disruptions
//! like rolling updates or node maintenance.

use crate::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Disruption budget policy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DisruptionPolicy {
    /// Maximum number of disruptions allowed.
    MaxDisruptions,
    /// Minimum number of pods that must remain available.
    MinAvailable,
}

/// Pod Disruption Budget specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodDisruptionBudgetSpec {
    /// Minimum number of pods that must be available.
    /// Can be an absolute number or percentage.
    #[serde(default)]
    pub min_available: Option<String>,
    /// Maximum number of pods that can be disrupted.
    #[serde(default)]
    pub max_disruptions: Option<String>,
}

/// PodDisruptionBudget desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodDisruptionBudgetDesired {
    /// PDB name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// PDB specification.
    pub spec: PodDisruptionBudgetSpec,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// PodDisruptionBudget actual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodDisruptionBudgetActual {
    /// PDB name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Current disruptions allowed.
    pub disruptions_allowed: i32,
    /// Current available pods.
    pub current_available: i32,
    /// Desired minimum available.
    pub desired_available: i32,
    /// Total pods managed by this PDB.
    pub total_pods: i32,
}

/// Disruption check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisruptionDecision {
    /// Disruption is allowed.
    Allow,
    /// Disruption would violate PDB.
    Deny(String),
}

/// PDB engine - evaluates if disruptions are allowed.
pub struct PdbEngine {
    /// PDBs indexed by namespace.
    pdbs: RwLock<HashMap<String, HashMap<String, PodDisruptionBudgetDesired>>>,
    /// Actual PDB states indexed by namespace.
    actuals: RwLock<HashMap<String, HashMap<String, PodDisruptionBudgetActual>>>,
}

impl PdbEngine {
    /// Create a new PDB engine.
    pub fn new() -> Self {
        Self {
            pdbs: RwLock::new(HashMap::new()),
            actuals: RwLock::new(HashMap::new()),
        }
    }

    /// Add or update a PDB.
    pub async fn set_pdb(&self, pdb: PodDisruptionBudgetDesired) -> Result<()> {
        let mut pdbs = self.pdbs.write().await;
        pdbs.entry(pdb.namespace.clone())
            .or_insert_with(HashMap::new)
            .insert(pdb.name.clone(), pdb);
        Ok(())
    }

    /// Remove a PDB.
    pub async fn remove_pdb(&self, namespace: &str, name: &str) -> Result<()> {
        let mut pdbs = self.pdbs.write().await;
        if let Some(ns_pdbs) = pdbs.get_mut(namespace) {
            ns_pdbs.remove(name);
        }
        Ok(())
    }

    /// Get a PDB by name.
    pub async fn get_pdb(&self, namespace: &str, name: &str) -> Option<PodDisruptionBudgetDesired> {
        let pdbs = self.pdbs.read().await;
        pdbs.get(namespace)
            .and_then(|ns_pdbs| ns_pdbs.get(name).cloned())
    }

    /// Get all PDBs in a namespace.
    pub async fn get_pdbs_in_namespace(&self, namespace: &str) -> Vec<PodDisruptionBudgetDesired> {
        let pdbs = self.pdbs.read().await;
        pdbs.get(namespace)
            .map(|ns_pdbs| ns_pdbs.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Update actual state for a PDB.
    pub async fn update_actual(&self, actual: PodDisruptionBudgetActual) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        actuals
            .entry(actual.namespace.clone())
            .or_insert_with(HashMap::new)
            .insert(actual.name.clone(), actual);
        Ok(())
    }

    /// Get actual state for a PDB.
    pub async fn get_actual(
        &self,
        namespace: &str,
        name: &str,
    ) -> Option<PodDisruptionBudgetActual> {
        let actuals = self.actuals.read().await;
        actuals
            .get(namespace)
            .and_then(|ns_actuals| ns_actuals.get(name).cloned())
    }

    /// Check if a disruption is allowed for a deployment's pods.
    pub async fn can_disrupt(
        &self,
        namespace: &str,
        _deployment: &str,
        count: i32,
    ) -> DisruptionDecision {
        let pdbs = self.pdbs.read().await;
        let actuals = self.actuals.read().await;

        // Get all PDBs that might cover this deployment
        if let Some(ns_pdbs) = pdbs.get(namespace) {
            for (_, pdb) in ns_pdbs.iter() {
                // Check if this PDB's selector would match the deployment
                // For now, simplified: match by namespace
                if let Some(actual) = actuals.get(namespace).and_then(|a| a.get(&pdb.name)) {
                    let decision = self.evaluate_pdb(pdb, actual, count);
                    if decision != DisruptionDecision::Allow {
                        return decision;
                    }
                }
            }
        }

        DisruptionDecision::Allow
    }

    /// Evaluate a single PDB.
    fn evaluate_pdb(
        &self,
        pdb: &PodDisruptionBudgetDesired,
        actual: &PodDisruptionBudgetActual,
        requested_disruptions: i32,
    ) -> DisruptionDecision {
        // Calculate disruptions allowed based on min_available
        if let Some(min_available_str) = &pdb.spec.min_available {
            let min_available = self.parse_quantity(min_available_str, actual.total_pods);
            let disruptions_allowed = actual.current_available - min_available;

            if disruptions_allowed < 0 {
                return DisruptionDecision::Deny(format!(
                    "current available ({}) is less than min_available ({})",
                    actual.current_available, min_available
                ));
            }

            if requested_disruptions > disruptions_allowed {
                return DisruptionDecision::Deny(format!(
                    "disruptions requested ({}) exceeds allowed ({})",
                    requested_disruptions, disruptions_allowed
                ));
            }
        }

        // Check max_disruptions
        if let Some(max_disruptions_str) = &pdb.spec.max_disruptions {
            let max_disruptions = self.parse_quantity(max_disruptions_str, actual.total_pods);

            if requested_disruptions > max_disruptions {
                return DisruptionDecision::Deny(format!(
                    "disruptions requested ({}) exceeds max_disruptions ({})",
                    requested_disruptions, max_disruptions
                ));
            }
        }

        DisruptionDecision::Allow
    }

    /// Parse a quantity string like "50%" or "3" to an absolute number.
    fn parse_quantity(&self, s: &str, total: i32) -> i32 {
        let s = s.trim();
        if s.ends_with('%') {
            let pct = s[..s.len() - 1].parse::<f64>().unwrap_or(0.0);
            ((pct / 100.0) * total as f64).ceil() as i32
        } else {
            s.parse::<i32>().unwrap_or(0)
        }
    }

    /// Calculate actual state for a deployment.
    pub async fn calculate_actual(
        &self,
        namespace: &str,
        pdb_name: &str,
        total_pods: i32,
        available_pods: i32,
    ) -> PodDisruptionBudgetActual {
        let pdb = self.get_pdb(namespace, pdb_name).await;

        let desired_available = pdb
            .as_ref()
            .and_then(|p| p.spec.min_available.as_ref())
            .map(|s| self.parse_quantity(s, total_pods))
            .unwrap_or(0);

        let disruptions_allowed = (available_pods - desired_available).max(0);

        PodDisruptionBudgetActual {
            name: pdb_name.to_string(),
            namespace: namespace.to_string(),
            disruptions_allowed,
            current_available: available_pods,
            desired_available,
            total_pods,
        }
    }
}

impl Default for PdbEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// PDB controller - manages PDB lifecycle and integration with reconciler.
pub struct PdbController {
    engine: Arc<PdbEngine>,
}

impl PdbController {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(PdbEngine::new()),
        }
    }

    pub fn engine(&self) -> Arc<PdbEngine> {
        self.engine.clone()
    }

    /// Register a PDB from a manifest.
    pub async fn register_pdb(&self, pdb: PodDisruptionBudgetDesired) -> Result<()> {
        self.engine.set_pdb(pdb).await
    }

    /// Unregister a PDB.
    pub async fn unregister_pdb(&self, namespace: &str, name: &str) -> Result<()> {
        self.engine.remove_pdb(namespace, name).await
    }

    /// Check if disruption is allowed before deleting pods.
    pub async fn check_disruption(
        &self,
        namespace: &str,
        deployment: &str,
        count: i32,
    ) -> DisruptionDecision {
        self.engine.can_disrupt(namespace, deployment, count).await
    }
}

impl Default for PdbController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pdb_creation() {
        let engine = PdbEngine::new();

        let pdb = PodDisruptionBudgetDesired {
            name: "web-pdb".to_string(),
            namespace: "default".to_string(),
            spec: PodDisruptionBudgetSpec {
                min_available: Some("50%".to_string()),
                max_disruptions: None,
            },
            created_at: Utc::now(),
        };

        engine.set_pdb(pdb.clone()).await.unwrap();

        let retrieved = engine.get_pdb("default", "web-pdb").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "web-pdb");
    }

    #[tokio::test]
    async fn test_pdb_allows_disruption() {
        let engine = PdbEngine::new();

        let pdb = PodDisruptionBudgetDesired {
            name: "web-pdb".to_string(),
            namespace: "default".to_string(),
            spec: PodDisruptionBudgetSpec {
                min_available: Some("2".to_string()),
                max_disruptions: None,
            },
            created_at: Utc::now(),
        };

        engine.set_pdb(pdb.clone()).await.unwrap();

        // Update actual state: 5 total pods, 5 available
        let actual = PodDisruptionBudgetActual {
            name: "web-pdb".to_string(),
            namespace: "default".to_string(),
            disruptions_allowed: 3,
            current_available: 5,
            desired_available: 2,
            total_pods: 5,
        };
        engine.update_actual(actual).await.unwrap();

        // Request to disrupt 2 pods - should be allowed (5 - 2 = 3 available >= min 2)
        let decision = engine.can_disrupt("default", "web", 2).await;
        assert_eq!(decision, DisruptionDecision::Allow);
    }

    #[tokio::test]
    async fn test_pdb_denies_excessive_disruption() {
        let engine = PdbEngine::new();

        let pdb = PodDisruptionBudgetDesired {
            name: "web-pdb".to_string(),
            namespace: "default".to_string(),
            spec: PodDisruptionBudgetSpec {
                min_available: Some("3".to_string()),
                max_disruptions: None,
            },
            created_at: Utc::now(),
        };

        engine.set_pdb(pdb.clone()).await.unwrap();

        // Update actual state: 5 total pods, 4 available
        let actual = PodDisruptionBudgetActual {
            name: "web-pdb".to_string(),
            namespace: "default".to_string(),
            disruptions_allowed: 1,
            current_available: 4,
            desired_available: 3,
            total_pods: 5,
        };
        engine.update_actual(actual).await.unwrap();

        // Request to disrupt 2 pods - should be denied (only 1 allowed)
        let decision = engine.can_disrupt("default", "web", 2).await;
        assert!(
            matches!(decision, DisruptionDecision::Deny(_)),
            "Should deny excessive disruption"
        );
    }

    #[tokio::test]
    async fn test_parse_percentage() {
        let engine = PdbEngine::new();
        assert_eq!(engine.parse_quantity("50%", 10), 5);
        assert_eq!(engine.parse_quantity("100%", 5), 5);
        assert_eq!(engine.parse_quantity("25%", 10), 3); // ceil
        assert_eq!(engine.parse_quantity("3", 10), 3);
    }
}
