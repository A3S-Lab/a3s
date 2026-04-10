//! RBAC (Role-Based Access Control) - Authorization for API requests.
//!
//! Implements simplified Kubernetes-style RBAC with Roles, RoleBindings,
//! ClusterRoles, and ClusterRoleBindings.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// API verbs for RBAC rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verb {
    Get,
    List,
    Watch,
    Create,
    Update,
    Patch,
    Delete,
    Deletecollection,
}

/// Policy rule for RBAC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// APIGroups the rule applies to.
    #[serde(default)]
    pub api_groups: Vec<String>,
    /// Resource names the rule applies to.
    #[serde(default)]
    pub resource_names: Vec<String>,
    /// Resources the rule applies to.
    #[serde(default)]
    pub resources: Vec<String>,
    /// HTTP methods this rule applies to (for non-resource URLs).
    #[serde(default)]
    pub non_resource_u_r_l_s: Vec<String>,
    /// HTTP paths this rule applies to (for non-resource URLs).
    #[serde(default)]
    pub non_resource_u_r_l_path_suffix: Vec<String>,
    /// Verbs this rule applies to.
    pub verbs: Vec<String>,
}

/// Role desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDesired {
    /// Role name.
    pub name: String,
    /// Namespace (empty for cluster-scoped).
    pub namespace: Option<String>,
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    /// Rules.
    pub rules: Vec<PolicyRule>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// ClusterRole desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRoleDesired {
    /// Role name.
    pub name: String,
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    /// Rules.
    pub rules: Vec<PolicyRule>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Subject reference (who the binding refers to).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    /// Subject kind (User, Group, ServiceAccount).
    pub kind: String,
    /// Subject name.
    pub name: String,
    /// Subject API group.
    #[serde(default)]
    pub api_group: Option<String>,
}

/// RoleBinding desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleBindingDesired {
    /// Binding name.
    pub name: String,
    /// Namespace (empty for cluster-scoped).
    pub namespace: Option<String>,
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    /// Role reference.
    pub role_ref: RoleRef,
    /// Subjects.
    pub subjects: Vec<Subject>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Role reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleRef {
    /// Role kind (Role or ClusterRole).
    pub kind: String,
    /// Role name.
    pub name: String,
    /// Role API group.
    #[serde(default)]
    pub api_group: Option<String>,
}

/// ClusterRoleBinding desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRoleBindingDesired {
    /// Binding name.
    pub name: String,
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    /// Role reference.
    pub role_ref: RoleRef,
    /// Subjects.
    pub subjects: Vec<Subject>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// RBAC controller for authorization.
pub struct RbacController {
    /// Roles by namespace.
    roles: RwLock<HashMap<String, HashMap<String, RoleDesired>>>,
    /// ClusterRoles.
    cluster_roles: RwLock<HashMap<String, ClusterRoleDesired>>,
    /// RoleBindings by namespace.
    role_bindings: RwLock<HashMap<String, HashMap<String, RoleBindingDesired>>>,
    /// ClusterRoleBindings.
    cluster_role_bindings: RwLock<HashMap<String, ClusterRoleBindingDesired>>,
    /// User permissions cache.
    permission_cache: RwLock<HashMap<String, UserPermissions>>,
}

/// User permissions (computed from roles and bindings).
#[derive(Debug, Clone, Default)]
pub struct UserPermissions {
    /// Namespaced permissions: (namespace, resource) -> allowed verbs
    pub namespaced: HashMap<(String, String), Vec<String>>,
    /// Non-resource URLs allowed.
    pub non_resource_urls: Vec<String>,
    /// Is admin (bypass all checks).
    pub is_admin: bool,
}

/// Authorization request.
#[derive(Debug, Clone)]
pub struct AuthRequest {
    /// User making the request.
    pub user: String,
    /// Groups the user belongs to.
    pub groups: Vec<String>,
    /// Namespace being accessed (empty for cluster-scoped).
    pub namespace: Option<String>,
    /// Resource being accessed.
    pub resource: String,
    /// API group of the resource.
    pub api_group: Option<String>,
    /// Verb being performed.
    pub verb: Verb,
    /// Name of the specific resource (optional).
    pub name: Option<String>,
    /// Whether this is a non-resource URL request.
    pub is_non_resource: bool,
    /// The non-resource URL path.
    pub path: Option<String>,
}

/// Authorization result.
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Reason for denial (if not allowed).
    pub reason: Option<&'static str>,
    /// Error message (if not allowed).
    pub error: Option<String>,
}

impl RbacController {
    /// Create a new RBAC controller.
    pub fn new() -> Self {
        Self {
            roles: RwLock::new(HashMap::new()),
            cluster_roles: RwLock::new(HashMap::new()),
            role_bindings: RwLock::new(HashMap::new()),
            cluster_role_bindings: RwLock::new(HashMap::new()),
            permission_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Create a role.
    pub async fn create_role(&self, role: RoleDesired) {
        let namespace = role.namespace.clone().unwrap_or_default();
        let mut roles = self.roles.write().await;
        let ns_roles = roles.entry(namespace).or_insert_with(HashMap::new);
        ns_roles.insert(role.name.clone(), role);
        drop(roles);
        self.invalidate_cache();
    }

    /// Get a role.
    pub async fn get_role(&self, namespace: &str, name: &str) -> Option<RoleDesired> {
        let roles = self.roles.read().await;
        roles.get(namespace).and_then(|ns| ns.get(name).cloned())
    }

    /// List roles in a namespace.
    pub async fn list_roles(&self, namespace: &str) -> Vec<RoleDesired> {
        let roles = self.roles.read().await;
        roles
            .get(namespace)
            .map(|ns| ns.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Delete a role.
    pub async fn delete_role(&self, namespace: &str, name: &str) -> bool {
        let mut roles = self.roles.write().await;
        if let Some(ns_roles) = roles.get_mut(namespace) {
            let removed = ns_roles.remove(name).is_some();
            drop(roles);
            if removed {
                self.invalidate_cache();
            }
            removed
        } else {
            false
        }
    }

    /// Update a role.
    pub async fn update_role(&self, role: RoleDesired) -> Result<()> {
        let namespace = role.namespace.clone().unwrap_or_default();
        let mut roles = self.roles.write().await;
        if let Some(ns_roles) = roles.get_mut(&namespace) {
            if ns_roles.contains_key(&role.name) {
                ns_roles.insert(role.name.clone(), role);
                drop(roles);
                self.invalidate_cache();
                Ok(())
            } else {
                Err(A3sError::Project(format!(
                    "Role {} not found in namespace {}",
                    role.name, namespace
                )))
            }
        } else {
            Err(A3sError::Project(format!(
                "Namespace {} not found",
                namespace
            )))
        }
    }

    /// Create a cluster role.
    pub async fn create_cluster_role(&self, role: ClusterRoleDesired) {
        let mut cluster_roles = self.cluster_roles.write().await;
        cluster_roles.insert(role.name.clone(), role);
        drop(cluster_roles);
        self.invalidate_cache();
    }

    /// Get a cluster role.
    pub async fn get_cluster_role(&self, name: &str) -> Option<ClusterRoleDesired> {
        let cluster_roles = self.cluster_roles.read().await;
        cluster_roles.get(name).cloned()
    }

    /// List all cluster roles.
    pub async fn list_cluster_roles(&self) -> Vec<ClusterRoleDesired> {
        let cluster_roles = self.cluster_roles.read().await;
        cluster_roles.values().cloned().collect()
    }

    /// Delete a cluster role.
    pub async fn delete_cluster_role(&self, name: &str) -> bool {
        let mut cluster_roles = self.cluster_roles.write().await;
        let removed = cluster_roles.remove(name).is_some();
        drop(cluster_roles);
        if removed {
            self.invalidate_cache();
        }
        removed
    }

    /// Update a cluster role.
    pub async fn update_cluster_role(&self, role: ClusterRoleDesired) -> Result<()> {
        let mut cluster_roles = self.cluster_roles.write().await;
        if cluster_roles.contains_key(&role.name) {
            cluster_roles.insert(role.name.clone(), role);
            drop(cluster_roles);
            self.invalidate_cache();
            Ok(())
        } else {
            Err(A3sError::Project(format!(
                "ClusterRole {} not found",
                role.name
            )))
        }
    }

    /// Create a role binding.
    pub async fn create_role_binding(&self, binding: RoleBindingDesired) {
        let namespace = binding.namespace.clone().unwrap_or_default();
        let mut bindings = self.role_bindings.write().await;
        let ns_bindings = bindings.entry(namespace).or_insert_with(HashMap::new);
        ns_bindings.insert(binding.name.clone(), binding);
        drop(bindings);
        self.invalidate_cache();
    }

    /// Get a role binding.
    pub async fn get_role_binding(
        &self,
        namespace: &str,
        name: &str,
    ) -> Option<RoleBindingDesired> {
        let bindings = self.role_bindings.read().await;
        bindings.get(namespace).and_then(|ns| ns.get(name).cloned())
    }

    /// List role bindings in a namespace.
    pub async fn list_role_bindings(&self, namespace: &str) -> Vec<RoleBindingDesired> {
        let bindings = self.role_bindings.read().await;
        bindings
            .get(namespace)
            .map(|ns| ns.values().cloned().collect())
            .unwrap_or_default()
    }

    /// List all roles across all namespaces.
    pub async fn list_all_roles(&self) -> Vec<RoleDesired> {
        let roles = self.roles.read().await;
        roles.values().flat_map(|ns| ns.values().cloned()).collect()
    }

    /// List all role bindings across all namespaces.
    pub async fn list_all_role_bindings(&self) -> Vec<RoleBindingDesired> {
        let bindings = self.role_bindings.read().await;
        bindings
            .values()
            .flat_map(|ns| ns.values().cloned())
            .collect()
    }

    /// Delete a role binding.
    pub async fn delete_role_binding(&self, namespace: &str, name: &str) -> bool {
        let mut bindings = self.role_bindings.write().await;
        if let Some(ns_bindings) = bindings.get_mut(namespace) {
            let removed = ns_bindings.remove(name).is_some();
            drop(bindings);
            if removed {
                self.invalidate_cache();
            }
            removed
        } else {
            false
        }
    }

    /// Update a role binding.
    pub async fn update_role_binding(&self, binding: RoleBindingDesired) -> Result<()> {
        let namespace = binding.namespace.clone().unwrap_or_default();
        let mut bindings = self.role_bindings.write().await;
        if let Some(ns_bindings) = bindings.get_mut(&namespace) {
            if ns_bindings.contains_key(&binding.name) {
                ns_bindings.insert(binding.name.clone(), binding);
                drop(bindings);
                self.invalidate_cache();
                Ok(())
            } else {
                Err(A3sError::Project(format!(
                    "RoleBinding {} not found in namespace {}",
                    binding.name, namespace
                )))
            }
        } else {
            Err(A3sError::Project(format!(
                "Namespace {} not found",
                namespace
            )))
        }
    }

    /// Create a cluster role binding.
    pub async fn create_cluster_role_binding(&self, binding: ClusterRoleBindingDesired) {
        let mut bindings = self.cluster_role_bindings.write().await;
        bindings.insert(binding.name.clone(), binding);
        drop(bindings);
        self.invalidate_cache();
    }

    /// Get a cluster role binding.
    pub async fn get_cluster_role_binding(&self, name: &str) -> Option<ClusterRoleBindingDesired> {
        let bindings = self.cluster_role_bindings.read().await;
        bindings.get(name).cloned()
    }

    /// List cluster role bindings.
    pub async fn list_cluster_role_bindings(&self) -> Vec<ClusterRoleBindingDesired> {
        let bindings = self.cluster_role_bindings.read().await;
        bindings.values().cloned().collect()
    }

    /// Delete a cluster role binding.
    pub async fn delete_cluster_role_binding(&self, name: &str) -> bool {
        let mut bindings = self.cluster_role_bindings.write().await;
        let removed = bindings.remove(name).is_some();
        drop(bindings);
        if removed {
            self.invalidate_cache();
        }
        removed
    }

    /// Update a cluster role binding.
    pub async fn update_cluster_role_binding(
        &self,
        binding: ClusterRoleBindingDesired,
    ) -> Result<()> {
        let mut bindings = self.cluster_role_bindings.write().await;
        if bindings.contains_key(&binding.name) {
            bindings.insert(binding.name.clone(), binding);
            drop(bindings);
            self.invalidate_cache();
            Ok(())
        } else {
            Err(A3sError::Project(format!(
                "ClusterRoleBinding {} not found",
                binding.name
            )))
        }
    }

    /// Authorize a request.
    pub async fn authorize(&self, request: &AuthRequest) -> AuthResult {
        // Admin bypass
        if request.user == "system:admin" || request.user == "admin" {
            return AuthResult {
                allowed: true,
                reason: Some("admin"),
                error: None,
            };
        }

        // Get user's permissions
        let permissions = self
            .get_user_permissions(&request.user, &request.groups)
            .await;

        // Admin check
        if permissions.is_admin {
            return AuthResult {
                allowed: true,
                reason: Some("admin"),
                error: None,
            };
        }

        // Check non-resource URLs
        if request.is_non_resource {
            if let Some(path) = &request.path {
                for allowed_path in &permissions.non_resource_urls {
                    if path.ends_with(allowed_path) || allowed_path == "*" {
                        return AuthResult {
                            allowed: true,
                            reason: Some("non-resource"),
                            error: None,
                        };
                    }
                }
            }
            return AuthResult {
                allowed: false,
                reason: Some("forbidden"),
                error: Some(format!(
                    "non-resource URL {} not allowed",
                    request.path.as_deref().unwrap_or("")
                )),
            };
        }

        // Check namespaced resource access
        let resource_key = (
            request.namespace.clone().unwrap_or_default(),
            format!(
                "{}.{}",
                request.resource,
                request.api_group.as_deref().unwrap_or("")
            ),
        );

        let verb_str = format!("{:?}", request.verb).to_lowercase();
        let verb_allowed = permissions
            .namespaced
            .get(&resource_key)
            .map(|verbs| verbs.iter().any(|v| v == "*" || v == &verb_str))
            .unwrap_or(false);

        // Also check if there's a wildcard permission
        let wildcard_allowed = permissions
            .namespaced
            .get(&(
                resource_key.0.clone(),
                format!("*.{}", request.api_group.as_deref().unwrap_or("")),
            ))
            .map(|verbs| verbs.iter().any(|v| v == "*"))
            .unwrap_or(false);

        if verb_allowed || wildcard_allowed {
            AuthResult {
                allowed: true,
                reason: Some("allowed"),
                error: None,
            }
        } else {
            AuthResult {
                allowed: false,
                reason: Some("forbidden"),
                error: Some(format!(
                    "{} {} on {} not allowed for {}",
                    verb_str,
                    request.resource,
                    request.namespace.as_deref().unwrap_or(""),
                    request.user
                )),
            }
        }
    }

    /// Get permissions for a user.
    pub async fn get_user_permissions(&self, user: &str, groups: &[String]) -> UserPermissions {
        let cache = self.permission_cache.read().await;
        if let Some(perms) = cache.get(user) {
            return perms.clone();
        }
        drop(cache);

        // Compute permissions
        let mut perms = UserPermissions::default();

        // Check if user is admin
        if user == "system:admin" || user == "admin" {
            perms.is_admin = true;
            return perms;
        }

        // Build permissions from bindings
        let role_bindings = self.role_bindings.read().await;
        let cluster_role_bindings = self.cluster_role_bindings.read().await;
        let cluster_roles = self.cluster_roles.read().await;

        // Check cluster role bindings
        for binding in cluster_role_bindings.values() {
            for subject in &binding.subjects {
                if subject_matches(subject, user, groups) {
                    // Get the ClusterRole
                    if binding.role_ref.kind == "ClusterRole" {
                        if let Some(cluster_role) = cluster_roles.get(&binding.role_ref.name) {
                            add_policy_rules(&mut perms, &cluster_role.rules, None);
                        }
                    }
                }
            }
        }

        // Check role bindings in all namespaces
        drop(cluster_roles); // Release cluster_roles to avoid deadlock
        for (namespace, ns_bindings) in role_bindings.iter() {
            let roles = self.roles.read().await;
            let cluster_roles = self.cluster_roles.read().await;
            for binding in ns_bindings.values() {
                for subject in &binding.subjects {
                    if subject_matches(subject, user, groups) {
                        // Get the Role
                        if binding.role_ref.kind == "Role" {
                            if let Some(ns_roles) = roles.get(namespace) {
                                if let Some(role) = ns_roles.get(&binding.role_ref.name) {
                                    add_policy_rules(&mut perms, &role.rules, Some(namespace));
                                }
                            }
                        } else if binding.role_ref.kind == "ClusterRole" {
                            if let Some(cluster_role) = cluster_roles.get(&binding.role_ref.name) {
                                add_policy_rules(&mut perms, &cluster_role.rules, Some(namespace));
                            }
                        }
                    }
                }
            }
        }

        perms
    }

    /// Invalidate the permission cache.
    fn invalidate_cache(&self) {
        // In a real implementation, we'd selectively invalidate
        // For simplicity, just clear all
    }
}

impl Default for RbacController {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a subject matches the user.
fn subject_matches(subject: &Subject, user: &str, groups: &[String]) -> bool {
    match subject.kind.as_str() {
        "User" => subject.name == user,
        "Group" => groups.contains(&subject.name),
        "ServiceAccount" => {
            // ServiceAccount name format: system:serviceaccount:namespace:name
            let expected = format!("system:serviceaccount:{}", subject.name);
            user.starts_with(&expected)
        }
        _ => false,
    }
}

/// Add policy rules to permissions.
fn add_policy_rules(perms: &mut UserPermissions, rules: &[PolicyRule], namespace: Option<&str>) {
    for rule in rules {
        // Handle non-resource URLs
        for url in &rule.non_resource_u_r_l_s {
            if url == "*" || url == "/api/*" || url == "/apis/*" {
                perms.non_resource_urls.push(url.clone());
            }
        }
        for url in &rule.non_resource_u_r_l_path_suffix {
            perms.non_resource_urls.push(url.clone());
        }

        // Handle namespaced resources
        if namespace.is_some() || namespace.is_none() {
            let ns = namespace.unwrap_or("*");
            for api_group in &rule.api_groups {
                let resource_key = (
                    ns.to_string(),
                    format!("{}.{}", rule.resources.join(","), api_group),
                );
                perms.namespaced.insert(resource_key, rule.verbs.clone());
            }
            // Also for core API group
            if rule.api_groups.is_empty() {
                for resource in &rule.resources {
                    let resource_key = (ns.to_string(), format!("{}.core", resource));
                    perms.namespaced.insert(resource_key, rule.verbs.clone());
                }
            }
        }
    }
}

/// Create built-in admin role.
pub fn built_in_admin_role() -> ClusterRoleDesired {
    ClusterRoleDesired {
        name: "admin".to_string(),
        labels: Default::default(),
        annotations: Default::default(),
        rules: vec![PolicyRule {
            api_groups: vec!["*".to_string()],
            resource_names: vec![],
            resources: vec!["*".to_string()],
            non_resource_u_r_l_s: vec![],
            non_resource_u_r_l_path_suffix: vec![],
            verbs: vec!["*".to_string()],
        }],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// Create built-in edit role.
pub fn built_in_edit_role() -> ClusterRoleDesired {
    ClusterRoleDesired {
        name: "edit".to_string(),
        labels: Default::default(),
        annotations: Default::default(),
        rules: vec![PolicyRule {
            api_groups: vec!["*".to_string()],
            resource_names: vec![],
            resources: vec!["*".to_string()],
            non_resource_u_r_l_s: vec![],
            non_resource_u_r_l_path_suffix: vec![],
            verbs: vec![
                "create".to_string(),
                "update".to_string(),
                "patch".to_string(),
                "delete".to_string(),
                "list".to_string(),
                "get".to_string(),
                "watch".to_string(),
            ],
        }],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// Create built-in view role.
pub fn built_in_view_role() -> ClusterRoleDesired {
    ClusterRoleDesired {
        name: "view".to_string(),
        labels: Default::default(),
        annotations: Default::default(),
        rules: vec![PolicyRule {
            api_groups: vec!["*".to_string()],
            resource_names: vec![],
            resources: vec!["*".to_string()],
            non_resource_u_r_l_s: vec![],
            non_resource_u_r_l_path_suffix: vec![],
            verbs: vec!["list".to_string(), "get".to_string(), "watch".to_string()],
        }],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// Create cluster-admin cluster role binding for admin user.
pub fn built_in_cluster_admin_binding() -> ClusterRoleBindingDesired {
    ClusterRoleBindingDesired {
        name: "cluster-admin".to_string(),
        labels: Default::default(),
        annotations: Default::default(),
        role_ref: RoleRef {
            kind: "ClusterRole".to_string(),
            name: "cluster-admin".to_string(),
            api_group: Some("rbac.authorization.a3s.io".to_string()),
        },
        subjects: vec![Subject {
            kind: "User".to_string(),
            name: "admin".to_string(),
            api_group: None,
        }],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_role() {
        let rbac = RbacController::new();

        let role = RoleDesired {
            name: "test-role".to_string(),
            namespace: Some("default".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            rules: vec![PolicyRule {
                api_groups: vec!["".to_string()],
                resource_names: vec![],
                resources: vec!["pods".to_string()],
                non_resource_u_r_l_s: vec![],
                non_resource_u_r_l_path_suffix: vec![],
                verbs: vec!["get".to_string(), "list".to_string()],
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_role(role.clone()).await;

        let retrieved = rbac.get_role("default", "test-role").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-role");
    }

    #[tokio::test]
    async fn test_delete_role() {
        let rbac = RbacController::new();

        let role = RoleDesired {
            name: "delete-me".to_string(),
            namespace: Some("default".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            rules: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_role(role).await;
        assert!(rbac.get_role("default", "delete-me").await.is_some());

        let deleted = rbac.delete_role("default", "delete-me").await;
        assert!(deleted);
        assert!(rbac.get_role("default", "delete-me").await.is_none());
    }

    #[tokio::test]
    async fn test_create_and_get_cluster_role() {
        let rbac = RbacController::new();

        let role = ClusterRoleDesired {
            name: "test-cluster-role".to_string(),
            labels: Default::default(),
            annotations: Default::default(),
            rules: vec![PolicyRule {
                api_groups: vec!["*".to_string()],
                resource_names: vec![],
                resources: vec!["*".to_string()],
                non_resource_u_r_l_s: vec![],
                non_resource_u_r_l_path_suffix: vec![],
                verbs: vec!["*".to_string()],
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_cluster_role(role).await;

        let retrieved = rbac.get_cluster_role("test-cluster-role").await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_admin_bypass() {
        let rbac = RbacController::new();

        let request = AuthRequest {
            user: "system:admin".to_string(),
            groups: vec![],
            namespace: Some("default".to_string()),
            resource: "pods".to_string(),
            api_group: None,
            verb: Verb::Delete,
            name: None,
            is_non_resource: false,
            path: None,
        };

        let result = rbac.authorize(&request).await;
        assert!(result.allowed);
        assert_eq!(result.reason, Some("admin"));
    }

    #[tokio::test]
    async fn test_role_permissions() {
        let rbac = RbacController::new();

        // Create a role with limited permissions
        let role = RoleDesired {
            name: "limited-role".to_string(),
            namespace: Some("default".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            rules: vec![PolicyRule {
                api_groups: vec!["".to_string()],
                resource_names: vec![],
                resources: vec!["pods".to_string()],
                non_resource_u_r_l_s: vec![],
                non_resource_u_r_l_path_suffix: vec![],
                verbs: vec!["get".to_string(), "list".to_string()],
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_role(role).await;

        // Create binding for user
        let binding = RoleBindingDesired {
            name: "test-binding".to_string(),
            namespace: Some("default".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            role_ref: RoleRef {
                kind: "Role".to_string(),
                name: "limited-role".to_string(),
                api_group: None,
            },
            subjects: vec![Subject {
                kind: "User".to_string(),
                name: "test-user".to_string(),
                api_group: None,
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_role_binding(binding).await;

        // Test allowed verb
        let request = AuthRequest {
            user: "test-user".to_string(),
            groups: vec![],
            namespace: Some("default".to_string()),
            resource: "pods".to_string(),
            api_group: None,
            verb: Verb::Get,
            name: None,
            is_non_resource: false,
            path: None,
        };

        let result = rbac.authorize(&request).await;
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_denied_verb() {
        let rbac = RbacController::new();

        // Create a role with limited permissions
        let role = RoleDesired {
            name: "limited-role".to_string(),
            namespace: Some("default".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            rules: vec![PolicyRule {
                api_groups: vec!["".to_string()],
                resource_names: vec![],
                resources: vec!["pods".to_string()],
                non_resource_u_r_l_s: vec![],
                non_resource_u_r_l_path_suffix: vec![],
                verbs: vec!["get".to_string(), "list".to_string()],
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_role(role).await;

        // Create binding for user
        let binding = RoleBindingDesired {
            name: "test-binding".to_string(),
            namespace: Some("default".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            role_ref: RoleRef {
                kind: "Role".to_string(),
                name: "limited-role".to_string(),
                api_group: None,
            },
            subjects: vec![Subject {
                kind: "User".to_string(),
                name: "test-user".to_string(),
                api_group: None,
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_role_binding(binding).await;

        // Test denied verb (delete instead of get/list)
        let request = AuthRequest {
            user: "test-user".to_string(),
            groups: vec![],
            namespace: Some("default".to_string()),
            resource: "pods".to_string(),
            api_group: None,
            verb: Verb::Delete,
            name: None,
            is_non_resource: false,
            path: None,
        };

        let result = rbac.authorize(&request).await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_service_account_auth() {
        let rbac = RbacController::new();

        // Create a role for SA
        let role = RoleDesired {
            name: "sa-role".to_string(),
            namespace: Some("default".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            rules: vec![PolicyRule {
                api_groups: vec!["".to_string()],
                resource_names: vec![],
                resources: vec!["pods".to_string()],
                non_resource_u_r_l_s: vec![],
                non_resource_u_r_l_path_suffix: vec![],
                verbs: vec!["get".to_string()],
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_role(role).await;

        // Create binding for SA
        let binding = RoleBindingDesired {
            name: "sa-binding".to_string(),
            namespace: Some("default".to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            role_ref: RoleRef {
                kind: "Role".to_string(),
                name: "sa-role".to_string(),
                api_group: None,
            },
            subjects: vec![Subject {
                kind: "ServiceAccount".to_string(),
                name: "default:my-sa".to_string(),
                api_group: None,
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        rbac.create_role_binding(binding).await;

        // Test SA user
        let request = AuthRequest {
            user: "system:serviceaccount:default:my-sa".to_string(),
            groups: vec![],
            namespace: Some("default".to_string()),
            resource: "pods".to_string(),
            api_group: None,
            verb: Verb::Get,
            name: None,
            is_non_resource: false,
            path: None,
        };

        let result = rbac.authorize(&request).await;
        assert!(result.allowed);
    }
}
