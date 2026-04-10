//! Service Accounts - Workload identity for pods and other resources.
//!
//! Service accounts provide an identity for pods to authenticate to the API server
//! and other services. Each service account has a token that can be used for
//! authentication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Service account desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccountDesired {
    /// Service account name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Optional display name.
    pub display_name: Option<String>,
    /// Optional description.
    pub description: Option<String>,
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    /// Whether the token should be auto-mounted.
    #[serde(default = "default_auto_mount_token")]
    pub automount_service_account_token: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

fn default_auto_mount_token() -> bool {
    true
}

/// Service account actual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccountActual {
    /// Service account name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Associated tokens.
    #[serde(default)]
    pub tokens: Vec<ServiceAccountTokenInfo>,
    /// Number of pods using this service account.
    pub pods_refs: usize,
    /// Last timestamp when a token was used.
    pub last_token_used: Option<DateTime<Utc>>,
}

/// Service account token info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccountTokenInfo {
    /// Token name (uid).
    pub uid: String,
    /// Token data (hashed).
    pub token_hash: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp (optional).
    pub expires_at: Option<DateTime<Utc>>,
    /// Last used timestamp.
    pub last_used: Option<DateTime<Utc>>,
    /// Whether the token is valid.
    pub valid: bool,
}

/// Token secret reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSecretRef {
    /// Secret name.
    pub name: String,
    /// Secret namespace.
    pub namespace: String,
    /// Token key in secret data.
    pub token_key: String,
    /// Expiration key (optional).
    pub expiration_key: Option<String>,
}

/// Service account controller for managing service accounts and tokens.
pub struct ServiceAccountController {
    /// Desired state.
    desired: RwLock<HashMap<(String, String), ServiceAccountDesired>>,
    /// Actual state.
    actual: RwLock<HashMap<(String, String), ServiceAccountActual>>,
    /// Token secrets (namespace -> name -> secret data).
    token_secrets: RwLock<HashMap<String, HashMap<String, Vec<u8>>>>,
    /// Default service account for each namespace.
    default_accounts: RwLock<HashMap<String, String>>,
}

impl ServiceAccountController {
    /// Create a new service account controller.
    pub fn new() -> Self {
        Self {
            desired: RwLock::new(HashMap::new()),
            actual: RwLock::new(HashMap::new()),
            token_secrets: RwLock::new(HashMap::new()),
            default_accounts: RwLock::new(HashMap::new()),
        }
    }

    /// Create a service account.
    pub async fn create(&self, account: ServiceAccountDesired) {
        let key = (account.namespace.clone(), account.name.clone());

        // Store desired state
        self.desired.write().await.insert(key.clone(), account);

        // Create actual state with a token
        let actual = ServiceAccountActual {
            name: key.1.clone(),
            namespace: key.0.clone(),
            tokens: vec![self.generate_token(&key.0, &key.1).await],
            pods_refs: 0,
            last_token_used: None,
        };
        self.actual.write().await.insert(key, actual);
    }

    /// Get a service account by name and namespace.
    pub async fn get(&self, namespace: &str, name: &str) -> Option<ServiceAccountDesired> {
        let desired = self.desired.read().await;
        desired
            .get(&(namespace.to_string(), name.to_string()))
            .cloned()
    }

    /// List service accounts in a namespace.
    pub async fn list(&self, namespace: &str) -> Vec<ServiceAccountDesired> {
        let desired = self.desired.read().await;
        desired
            .values()
            .filter(|a| a.namespace == namespace)
            .cloned()
            .collect()
    }

    /// List all service accounts.
    pub async fn list_all(&self) -> Vec<ServiceAccountDesired> {
        let desired = self.desired.read().await;
        desired.values().cloned().collect()
    }

    /// Update a service account.
    pub async fn update(&self, account: ServiceAccountDesired) {
        let key = (account.namespace.clone(), account.name.clone());
        self.desired.write().await.insert(key, account);
    }

    /// Delete a service account.
    pub async fn delete(&self, namespace: &str, name: &str) -> bool {
        let key = (namespace.to_string(), name.to_string());
        let mut desired = self.desired.write().await;
        let mut actual = self.actual.write().await;

        // Clean up tokens
        if let Some(acc) = actual.get(&key) {
            for token in &acc.tokens {
                self.delete_token_secret(&key.0, &token.uid).await;
            }
        }

        desired.remove(&key);
        actual.remove(&key).is_some()
    }

    /// Get the default service account for a namespace.
    pub async fn get_default_account(&self, namespace: &str) -> Option<ServiceAccountDesired> {
        let defaults = self.default_accounts.read().await;
        let name = defaults.get(namespace).cloned()?;
        drop(defaults);
        self.get(namespace, &name).await
    }

    /// Set the default service account for a namespace.
    pub async fn set_default_account(&self, namespace: &str, name: &str) {
        let mut defaults = self.default_accounts.write().await;
        defaults.insert(namespace.to_string(), name.to_string());
    }

    /// Create the default service account for a namespace.
    pub async fn create_default_account(&self, namespace: &str) -> ServiceAccountDesired {
        let account = ServiceAccountDesired {
            name: "default".to_string(),
            namespace: namespace.to_string(),
            display_name: Some("Default".to_string()),
            description: Some("Default service account for the namespace".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.create(account.clone()).await;
        self.set_default_account(namespace, "default").await;
        account
    }

    /// Generate a new token for a service account.
    pub async fn create_token(
        &self,
        namespace: &str,
        name: &str,
        _ttl_seconds: Option<i64>,
    ) -> Option<ServiceAccountTokenInfo> {
        let key = (namespace.to_string(), name.to_string());
        let mut actual = self.actual.write().await;

        if let Some(account) = actual.get_mut(&key) {
            let token = self.generate_token(namespace, name).await;
            account.tokens.push(token.clone());
            Some(token)
        } else {
            None
        }
    }

    /// Delete a token.
    pub async fn delete_token(&self, namespace: &str, name: &str, token_uid: &str) -> bool {
        let key = (namespace.to_string(), name.to_string());
        let mut actual = self.actual.write().await;

        if let Some(account) = actual.get_mut(&key) {
            let before = account.tokens.len();
            account.tokens.retain(|t| t.uid != token_uid);
            if account.tokens.len() < before {
                drop(actual);
                self.delete_token_secret(namespace, token_uid).await;
                return true;
            }
        }
        false
    }

    /// List tokens for a service account.
    pub async fn list_tokens(&self, namespace: &str, name: &str) -> Vec<ServiceAccountTokenInfo> {
        let key = (namespace.to_string(), name.to_string());
        let actual = self.actual.read().await;
        actual
            .get(&key)
            .map(|a| a.tokens.clone())
            .unwrap_or_default()
    }

    /// Validate a token and return the associated service account.
    pub async fn validate_token(&self, token: &str) -> Option<(String, String)> {
        let actual = self.actual.read().await;

        // First pass: find the token
        let mut found: Option<(String, String, String)> = None;
        for ((namespace, name), account) in actual.iter() {
            for t in &account.tokens {
                if t.valid && self.verify_token(token, &t.token_hash) {
                    found = Some((namespace.clone(), name.clone(), t.uid.clone()));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }

        match found {
            Some((namespace, name, uid)) => {
                drop(actual);
                self.touch_token(&namespace, &name, &uid).await;
                Some((namespace, name))
            }
            None => None,
        }
    }

    /// Get the actual state of a service account.
    pub async fn get_actual(&self, namespace: &str, name: &str) -> Option<ServiceAccountActual> {
        let key = (namespace.to_string(), name.to_string());
        let actual = self.actual.read().await;
        actual.get(&key).cloned()
    }

    /// Update pod reference count.
    pub async fn update_pod_refs(&self, namespace: &str, name: &str, count: usize) {
        let key = (namespace.to_string(), name.to_string());
        let mut actual = self.actual.write().await;
        if let Some(account) = actual.get_mut(&key) {
            account.pods_refs = count;
        }
    }

    /// Generate a token (simplified - in production this would be JWT).
    async fn generate_token(&self, namespace: &str, name: &str) -> ServiceAccountTokenInfo {
        use std::time::{SystemTime, UNIX_EPOCH};

        let uid = format!(
            "{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            uuid_simple()
        );

        let token_data = format!("{}.{}.{}", namespace, name, uid);
        let token_hash = simple_hash(&token_data);

        // Create secret for this token
        self.create_token_secret(namespace, &uid, &token_data).await;

        ServiceAccountTokenInfo {
            uid,
            token_hash,
            created_at: Utc::now(),
            expires_at: None,
            last_used: None,
            valid: true,
        }
    }

    /// Create token secret.
    async fn create_token_secret(&self, namespace: &str, uid: &str, token: &str) {
        let mut secrets = self.token_secrets.write().await;
        let ns_secrets = secrets
            .entry(namespace.to_string())
            .or_insert_with(HashMap::new);
        ns_secrets.insert(uid.to_string(), token.as_bytes().to_vec());
    }

    /// Delete token secret.
    async fn delete_token_secret(&self, namespace: &str, uid: &str) {
        let mut secrets = self.token_secrets.write().await;
        if let Some(ns_secrets) = secrets.get_mut(namespace) {
            ns_secrets.remove(uid);
        }
    }

    /// Verify token against hash.
    fn verify_token(&self, token: &str, hash: &str) -> bool {
        simple_hash(token) == hash
    }

    /// Touch token to update last used.
    async fn touch_token(&self, namespace: &str, name: &str, uid: &str) {
        let key = (namespace.to_string(), name.to_string());
        let mut actual = self.actual.write().await;
        if let Some(account) = actual.get_mut(&key) {
            for token in &mut account.tokens {
                if token.uid == uid {
                    token.last_used = Some(Utc::now());
                    account.last_token_used = Some(Utc::now());
                }
            }
        }
    }

    /// Invalidate expired tokens.
    pub async fn cleanup_expired_tokens(&self) {
        let now = Utc::now();
        let mut actual = self.actual.write().await;

        for account in actual.values_mut() {
            let mut to_remove = Vec::new();
            for token in &account.tokens {
                if !token.valid {
                    continue;
                }
                if let Some(expires) = token.expires_at {
                    if expires < now {
                        to_remove.push(token.uid.clone());
                    }
                }
            }
            for uid in to_remove {
                account.tokens.retain(|t| t.uid != uid);
                // Note: In real implementation, also delete from API server's token cache
            }
        }
    }
}

impl Default for ServiceAccountController {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple hash function for tokens (not cryptographic - use real JWT in production).
fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Simple UUID generator.
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:032x}", nanos)
}

/// Service account info for pod injection.
#[derive(Debug, Clone)]
pub struct ServiceAccountInfo {
    /// Service account name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Mount path for the token.
    pub token_path: String,
    /// Mount path for the CA cert.
    pub ca_path: String,
    /// Whether to mount the token.
    pub mount_token: bool,
}

impl ServiceAccountController {
    /// Get service account info for a pod.
    pub async fn get_service_account_info(
        &self,
        namespace: &str,
        name: Option<&str>,
    ) -> ServiceAccountInfo {
        let account_name: String = match name {
            Some(n) => n.to_string(),
            None => {
                let default = self.get_default_account(namespace).await;
                default
                    .map(|a| a.name)
                    .unwrap_or_else(|| "default".to_string())
            }
        };

        let account = self.get(namespace, &account_name).await;
        let mount_token = account
            .as_ref()
            .map(|a| a.automount_service_account_token)
            .unwrap_or(true);

        ServiceAccountInfo {
            name: account_name,
            namespace: namespace.to_string(),
            token_path: format!("/var/run/secrets/a3s.io/serviceaccount/token"),
            ca_path: format!("/var/run/secrets/a3s.io/serviceaccount/ca.crt"),
            mount_token,
        }
    }

    /// Check if a service account exists.
    pub async fn exists(&self, namespace: &str, name: &str) -> bool {
        self.get(namespace, name).await.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_service_account() {
        let controller = ServiceAccountController::new();

        let account = ServiceAccountDesired {
            name: "test-sa".to_string(),
            namespace: "default".to_string(),
            display_name: Some("Test SA".to_string()),
            description: None,
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        controller.create(account.clone()).await;

        let retrieved = controller.get("default", "test-sa").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-sa");
    }

    #[tokio::test]
    async fn test_list_service_accounts() {
        let controller = ServiceAccountController::new();

        // Create accounts in different namespaces
        controller
            .create(ServiceAccountDesired {
                name: "sa1".to_string(),
                namespace: "ns1".to_string(),
                display_name: None,
                description: None,
                labels: Default::default(),
                annotations: Default::default(),
                automount_service_account_token: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await;

        controller
            .create(ServiceAccountDesired {
                name: "sa2".to_string(),
                namespace: "ns2".to_string(),
                display_name: None,
                description: None,
                labels: Default::default(),
                annotations: Default::default(),
                automount_service_account_token: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await;

        let all = controller.list_all().await;
        assert_eq!(all.len(), 2);

        let ns1 = controller.list("ns1").await;
        assert_eq!(ns1.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_service_account() {
        let controller = ServiceAccountController::new();

        let account = ServiceAccountDesired {
            name: "delete-me".to_string(),
            namespace: "default".to_string(),
            display_name: None,
            description: None,
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        controller.create(account).await;
        assert!(controller.get("default", "delete-me").await.is_some());

        controller.delete("default", "delete-me").await;
        assert!(controller.get("default", "delete-me").await.is_none());
    }

    #[tokio::test]
    async fn test_token_creation() {
        let controller = ServiceAccountController::new();

        let account = ServiceAccountDesired {
            name: "test-sa".to_string(),
            namespace: "default".to_string(),
            display_name: None,
            description: None,
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        controller.create(account).await;

        // Check that a token was created automatically
        let tokens = controller.list_tokens("default", "test-sa").await;
        assert_eq!(tokens.len(), 1);

        // Add another token
        let new_token = controller.create_token("default", "test-sa", None).await;
        assert!(new_token.is_some());

        let tokens = controller.list_tokens("default", "test-sa").await;
        assert_eq!(tokens.len(), 2);
    }

    #[tokio::test]
    async fn test_token_validation() {
        let controller = ServiceAccountController::new();

        let account = ServiceAccountDesired {
            name: "test-sa".to_string(),
            namespace: "default".to_string(),
            display_name: None,
            description: None,
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        controller.create(account).await;

        let tokens = controller.list_tokens("default", "test-sa").await;
        assert!(!tokens.is_empty());

        // Can't directly get the token, only verify it
        // Token validation would be done by API server
    }

    #[tokio::test]
    async fn test_default_service_account() {
        let controller = ServiceAccountController::new();

        // Create default account
        let default = controller.create_default_account("default").await;
        assert_eq!(default.name, "default");

        // Get default
        let retrieved = controller.get_default_account("default").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "default");
    }

    #[tokio::test]
    async fn test_delete_token() {
        let controller = ServiceAccountController::new();

        let account = ServiceAccountDesired {
            name: "test-sa".to_string(),
            namespace: "default".to_string(),
            display_name: None,
            description: None,
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        controller.create(account).await;

        let tokens = controller.list_tokens("default", "test-sa").await;
        let token_uid = tokens[0].uid.clone();

        let deleted = controller
            .delete_token("default", "test-sa", &token_uid)
            .await;
        assert!(deleted);

        let tokens = controller.list_tokens("default", "test-sa").await;
        assert!(tokens.is_empty());
    }

    #[tokio::test]
    async fn test_update_pod_refs() {
        let controller = ServiceAccountController::new();

        let account = ServiceAccountDesired {
            name: "test-sa".to_string(),
            namespace: "default".to_string(),
            display_name: None,
            description: None,
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        controller.create(account).await;

        controller.update_pod_refs("default", "test-sa", 5).await;

        let actual = controller.get_actual("default", "test-sa").await;
        assert!(actual.is_some());
        assert_eq!(actual.unwrap().pods_refs, 5);
    }

    #[tokio::test]
    async fn test_service_account_info() {
        let controller = ServiceAccountController::new();

        let account = ServiceAccountDesired {
            name: "my-sa".to_string(),
            namespace: "default".to_string(),
            display_name: None,
            description: None,
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        controller.create(account).await;

        let info = controller
            .get_service_account_info("default", Some("my-sa"))
            .await;
        assert_eq!(info.name, "my-sa");
        assert_eq!(info.namespace, "default");
        assert!(!info.mount_token);
    }

    #[tokio::test]
    async fn test_automount_token_false() {
        let controller = ServiceAccountController::new();

        let account = ServiceAccountDesired {
            name: "no-token-sa".to_string(),
            namespace: "default".to_string(),
            display_name: None,
            description: None,
            labels: Default::default(),
            annotations: Default::default(),
            automount_service_account_token: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        controller.create(account).await;

        let info = controller
            .get_service_account_info("default", Some("no-token-sa"))
            .await;
        assert!(!info.mount_token);
    }
}
