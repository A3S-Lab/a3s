//! Admission Control - Webhook hooks for validating and mutating resources.
//!
//! Provides admission control webhooks similar to Kubernetes for validating
//! and mutating resources before they are persisted.

use crate::errors::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Admission request.
#[derive(Debug, Clone)]
pub struct AdmissionRequest<T> {
    /// Unique identifier for the request.
    pub uid: String,
    /// The object being admitted.
    pub object: T,
    /// The resource operation (CREATE, UPDATE, DELETE).
    pub operation: AdmissionOperation,
    /// Resource info.
    pub resource: GroupVersionResource,
    /// Subresource info (if applicable).
    pub sub_resource: Option<String>,
    /// User info making the request.
    pub user_info: AdmissionUserInfo,
    /// Options (for scale subresource, etc.).
    pub options: serde_json::Value,
}

/// Admission operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOperation {
    Create,
    Update,
    Delete,
    Connect,
}

/// Group version resource.
#[derive(Debug, Clone)]
pub struct GroupVersionResource {
    /// Group (empty for core API).
    pub group: String,
    /// Version.
    pub version: String,
    /// Resource type.
    pub resource: String,
}

/// User info from the request.
#[derive(Debug, Clone, Default)]
pub struct AdmissionUserInfo {
    /// Username.
    pub username: Option<String>,
    /// UID.
    pub uid: Option<String>,
    /// Groups.
    pub groups: Vec<String>,
    /// Extra info.
    pub extra: HashMap<String, Vec<String>>,
}

/// Admission response.
#[derive(Debug, Clone)]
pub struct AdmissionResponse {
    /// UID matching the request.
    pub uid: String,
    /// Whether the admission is allowed.
    pub allowed: bool,
    /// Result message (if denied).
    pub status: Option<AdmissionStatus>,
    /// Patch operations (for mutations).
    pub patch: Option<Vec<u8>>,
    /// Patch type (JSONPatch, MergePatch, etc.).
    pub patch_type: Option<PatchType>,
}

/// Admission status for denied requests.
#[derive(Debug, Clone)]
pub struct AdmissionStatus {
    /// Status code.
    pub code: i32,
    /// Status message.
    pub message: String,
    /// Status reason.
    pub reason: String,
}

/// Patch type for mutations.
#[derive(Debug, Clone, Copy)]
pub enum PatchType {
    JsonPatch,
    MergePatch,
    StrategicMergePatch,
}

/// Webhook interface for admission controllers.
pub trait AdmissionWebhook<T>: Send + Sync {
    /// Get webhook name.
    fn name(&self) -> &str;

    /// Get webhook path.
    fn path(&self) -> &str;

    /// Check if this webhook should handle the request.
    fn matches(&self, operation: AdmissionOperation, resource: &GroupVersionResource) -> bool;

    /// Admit the request (validate or mutate).
    fn admit(&self, request: &AdmissionRequest<T>) -> AdmissionResponse;
}

/// Validating webhook - only validates, does not mutate.
pub trait ValidatingWebhook<T>: AdmissionWebhook<T> {
    /// Validate the request.
    fn validate(&self, request: &AdmissionRequest<T>) -> Result<AdmissionResponse>;
}

/// Mutating webhook - can both validate and mutate.
pub trait MutatingWebhook<T>: AdmissionWebhook<T> {
    /// Mutate the request.
    fn mutate(&self, request: &AdmissionRequest<T>) -> Result<AdmissionResponse>;
}

/// Webhook registration.
#[derive(Debug, Clone)]
pub struct WebhookRegistration {
    /// Webhook name.
    pub name: String,
    /// Webhook path.
    pub path: String,
    /// Webhook type.
    pub webhook_type: WebhookType,
    /// Operations this webhook handles.
    pub operations: Vec<AdmissionOperation>,
    /// Resources this webhook handles.
    pub resources: Vec<GroupVersionResource>,
    /// Whether to fail closed (deny on error) or fail open (allow on error).
    pub fail_policy: FailPolicy,
}

/// Webhook type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WebhookType {
    Mutating,
    Validating,
}

/// Fail policy for webhooks.
#[derive(Debug, Clone, Copy)]
pub enum FailPolicy {
    /// Deny requests when webhook fails.
    FailClosed,
    /// Allow requests when webhook fails.
    FailOpen,
}

impl Default for FailPolicy {
    fn default() -> Self {
        FailPolicy::FailClosed
    }
}

/// Webhook manager - registers and invokes admission webhooks.
pub struct WebhookManager<T> {
    /// Registered webhooks.
    webhooks: RwLock<Vec<WebhookRegistration>>,
    /// Webhook implementations (by name).
    handlers: RwLock<HashMap<String, Box<dyn AdmissionWebhook<T>>>>,
}

impl<T: Clone + Send + Sync + 'static> WebhookManager<T> {
    /// Create a new webhook manager.
    pub fn new() -> Self {
        Self {
            webhooks: RwLock::new(Vec::new()),
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a webhook.
    pub async fn register(&self, registration: WebhookRegistration) {
        let mut webhooks = self.webhooks.write().await;
        webhooks.push(registration);
    }

    /// Register a webhook handler.
    pub async fn register_handler(&self, handler: Box<dyn AdmissionWebhook<T>>) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(handler.name().to_string(), handler);
    }

    /// Get all registered webhooks.
    pub async fn get_webhooks(&self) -> Vec<WebhookRegistration> {
        let webhooks = self.webhooks.read().await;
        webhooks.clone()
    }

    /// Find webhooks matching the request.
    pub async fn find_matching_webhooks(
        &self,
        operation: AdmissionOperation,
        resource: &GroupVersionResource,
    ) -> Vec<String> {
        let webhooks = self.webhooks.read().await;
        webhooks
            .iter()
            .filter(|w| w.matches_operation(operation) && w.matches_resource(resource))
            .map(|w| w.name.clone())
            .collect()
    }

    /// Invoke mutating webhooks in order.
    pub async fn invoke_mutating(
        &self,
        request: &AdmissionRequest<T>,
    ) -> Result<AdmissionResponse> {
        let webhooks = self.webhooks.read().await;
        let handlers = self.handlers.read().await;

        let mut response = AdmissionResponse {
            uid: request.uid.clone(),
            allowed: true,
            status: None,
            patch: None,
            patch_type: None,
        };

        for reg in webhooks
            .iter()
            .filter(|w| w.webhook_type == WebhookType::Mutating)
        {
            if !reg.matches_operation(request.operation) || !reg.matches_resource(&request.resource)
            {
                continue;
            }

            if let Some(handler) = handlers.get(&reg.name) {
                let result = handler.admit(request);
                match result {
                    AdmissionResponse {
                        allowed: true,
                        patch: Some(patch),
                        ..
                    } => {
                        // Apply patch
                        response.patch = Some(patch);
                        response.patch_type = Some(PatchType::JsonPatch);
                    }
                    AdmissionResponse { allowed: false, .. } => {
                        return Ok(result);
                    }
                    _ => {}
                }
            }
        }

        Ok(response)
    }

    /// Invoke validating webhooks in order.
    pub async fn invoke_validating(&self, request: &AdmissionRequest<T>) -> Vec<AdmissionResponse> {
        let webhooks = self.webhooks.read().await;
        let handlers = self.handlers.read().await;

        let mut responses = Vec::new();

        for reg in webhooks
            .iter()
            .filter(|w| w.webhook_type == WebhookType::Validating)
        {
            if !reg.matches_operation(request.operation) || !reg.matches_resource(&request.resource)
            {
                continue;
            }

            if let Some(handler) = handlers.get(&reg.name) {
                let response = handler.admit(request);
                if !response.allowed {
                    responses.push(response);
                }
            }
        }

        responses
    }

    /// Admit a request through all mutating then validating webhooks.
    pub async fn admit(&self, request: &AdmissionRequest<T>) -> Result<AdmissionResponse> {
        // First, invoke mutating webhooks
        let response = self.invoke_mutating(request).await?;

        if !response.allowed {
            return Ok(response);
        }

        // Then, invoke validating webhooks
        let validations = self.invoke_validating(request).await;

        if !validations.is_empty() {
            // Return the first denial
            return Ok(validations.into_iter().next().unwrap());
        }

        Ok(response)
    }
}

impl<T: Clone + Send + Sync + 'static> Default for WebhookManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl WebhookRegistration {
    /// Check if this registration matches the operation.
    pub fn matches_operation(&self, operation: AdmissionOperation) -> bool {
        self.operations.is_empty() || self.operations.contains(&operation)
    }

    /// Check if this registration matches the resource.
    pub fn matches_resource(&self, resource: &GroupVersionResource) -> bool {
        self.resources.is_empty()
            || self.resources.iter().any(|r| {
                r.group == resource.group
                    && r.version == resource.version
                    && r.resource == resource.resource
            })
    }
}

/// Built-in webhook: AlwaysDeny - always denies requests (for testing).
pub struct AlwaysDenyWebhook {
    name: String,
    path: String,
}

impl AlwaysDenyWebhook {
    /// Create a new AlwaysDeny webhook.
    pub fn new() -> Self {
        Self {
            name: "always-deny.example.com".to_string(),
            path: "/admission/always-deny".to_string(),
        }
    }
}

impl Default for AlwaysDenyWebhook {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AdmissionWebhook<T> for AlwaysDenyWebhook {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn matches(&self, _operation: AdmissionOperation, _resource: &GroupVersionResource) -> bool {
        true
    }

    fn admit(&self, request: &AdmissionRequest<T>) -> AdmissionResponse {
        AdmissionResponse {
            uid: request.uid.clone(),
            allowed: false,
            status: Some(AdmissionStatus {
                code: 403,
                message: "This webhook always denies requests".to_string(),
                reason: "AlwaysDeny".to_string(),
            }),
            patch: None,
            patch_type: None,
        }
    }
}

/// Built-in webhook: AlwaysAllow - always allows requests (for testing).
pub struct AlwaysAllowWebhook {
    name: String,
    path: String,
}

impl AlwaysAllowWebhook {
    /// Create a new AlwaysAllow webhook.
    pub fn new() -> Self {
        Self {
            name: "always-allow.example.com".to_string(),
            path: "/admission/always-allow".to_string(),
        }
    }
}

impl Default for AlwaysAllowWebhook {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AdmissionWebhook<T> for AlwaysAllowWebhook {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn matches(&self, _operation: AdmissionOperation, _resource: &GroupVersionResource) -> bool {
        true
    }

    fn admit(&self, request: &AdmissionRequest<T>) -> AdmissionResponse {
        AdmissionResponse {
            uid: request.uid.clone(),
            allowed: true,
            status: None,
            patch: None,
            patch_type: None,
        }
    }
}

/// Built-in webhook: DefaultStorageClass - sets default storage class.
pub struct DefaultStorageClassWebhook {
    name: String,
    path: String,
    default_class: String,
}

impl DefaultStorageClassWebhook {
    /// Create a new DefaultStorageClass webhook.
    pub fn new(default_class: &str) -> Self {
        Self {
            name: "default-storage-class.example.com".to_string(),
            path: "/admission/default-storage-class".to_string(),
            default_class: default_class.to_string(),
        }
    }
}

impl<T> AdmissionWebhook<T> for DefaultStorageClassWebhook {
    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn matches(&self, operation: AdmissionOperation, resource: &GroupVersionResource) -> bool {
        operation == AdmissionOperation::Create
            && resource.group == ""
            && resource.version == "v1"
            && resource.resource == "persistentvolumeclaims"
    }

    fn admit(&self, request: &AdmissionRequest<T>) -> AdmissionResponse {
        // In a real implementation, this would patch the PVC to add the default storage class
        AdmissionResponse {
            uid: request.uid.clone(),
            allowed: true,
            status: None,
            patch: None,
            patch_type: None,
        }
    }
}

/// Validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Field that failed validation.
    pub field: String,
    /// Error message.
    pub message: String,
}

/// Validator helper for building validation responses.
pub struct Validator {
    errors: Vec<ValidationError>,
}

impl Validator {
    /// Create a new validator.
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add an error.
    pub fn error(&mut self, field: &str, message: &str) {
        self.errors.push(ValidationError {
            field: field.to_string(),
            message: message.to_string(),
        });
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Build the error message.
    pub fn error_message(&self) -> String {
        self.errors
            .iter()
            .map(|e| format!("{}: {}", e.field, e.message))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Create a denied response from this validator.
    pub fn deny_response(&self, uid: &str) -> AdmissionResponse {
        AdmissionResponse {
            uid: uid.to_string(),
            allowed: false,
            status: Some(AdmissionStatus {
                code: 400,
                message: self.error_message(),
                reason: "Invalid".to_string(),
            }),
            patch: None,
            patch_type: None,
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator() {
        let mut validator = Validator::new();
        validator.error("metadata.name", "name is required");
        validator.error("spec.replicas", "must be positive");

        assert!(validator.has_errors());
        assert_eq!(
            validator.error_message(),
            "metadata.name: name is required; spec.replicas: must be positive"
        );
    }

    #[test]
    fn test_always_deny_webhook() {
        let webhook = AlwaysDenyWebhook::new();
        let request = AdmissionRequest {
            uid: "test-uid".to_string(),
            object: "test".to_string(),
            operation: AdmissionOperation::Create,
            resource: GroupVersionResource {
                group: "".to_string(),
                version: "v1".to_string(),
                resource: "pods".to_string(),
            },
            sub_resource: None,
            user_info: AdmissionUserInfo::default(),
            options: serde_json::Value::Null,
        };

        let response = webhook.admit(&request);
        assert!(!response.allowed);
        assert_eq!(response.status.as_ref().unwrap().code, 403);
    }

    #[test]
    fn test_always_allow_webhook() {
        let webhook = AlwaysAllowWebhook::new();
        let request = AdmissionRequest {
            uid: "test-uid".to_string(),
            object: "test".to_string(),
            operation: AdmissionOperation::Create,
            resource: GroupVersionResource {
                group: "".to_string(),
                version: "v1".to_string(),
                resource: "pods".to_string(),
            },
            sub_resource: None,
            user_info: AdmissionUserInfo::default(),
            options: serde_json::Value::Null,
        };

        let response = webhook.admit(&request);
        assert!(response.allowed);
    }

    #[tokio::test]
    async fn test_webhook_manager() {
        let manager: WebhookManager<String> = WebhookManager::new();

        // Register AlwaysAllow webhook
        manager
            .register_handler(Box::new(AlwaysAllowWebhook::new()))
            .await;

        let request = AdmissionRequest {
            uid: "test-uid".to_string(),
            object: "test".to_string(),
            operation: AdmissionOperation::Create,
            resource: GroupVersionResource {
                group: "".to_string(),
                version: "v1".to_string(),
                resource: "pods".to_string(),
            },
            sub_resource: None,
            user_info: AdmissionUserInfo::default(),
            options: serde_json::Value::Null,
        };

        let response = manager.admit(&request).await.unwrap();
        assert!(response.allowed);
    }

    #[tokio::test]
    async fn test_webhook_manager_denies() {
        let manager: WebhookManager<String> = WebhookManager::new();

        // Register AlwaysDeny webhook
        let webhook = AlwaysDenyWebhook::new();
        manager
            .register(WebhookRegistration {
                name: "always-deny.example.com".to_string(),
                path: "/admission/always-deny".to_string(),
                webhook_type: WebhookType::Validating,
                operations: vec![AdmissionOperation::Create],
                resources: vec![],
                fail_policy: FailPolicy::FailClosed,
            })
            .await;
        manager.register_handler(Box::new(webhook)).await;

        let request = AdmissionRequest {
            uid: "test-uid".to_string(),
            object: "test".to_string(),
            operation: AdmissionOperation::Create,
            resource: GroupVersionResource {
                group: "".to_string(),
                version: "v1".to_string(),
                resource: "pods".to_string(),
            },
            sub_resource: None,
            user_info: AdmissionUserInfo::default(),
            options: serde_json::Value::Null,
        };

        let response = manager.admit(&request).await.unwrap();
        assert!(!response.allowed);
    }

    #[test]
    fn test_webhook_registration_matches() {
        let registration = WebhookRegistration {
            name: "test-webhook".to_string(),
            path: "/test".to_string(),
            webhook_type: WebhookType::Validating,
            operations: vec![AdmissionOperation::Create, AdmissionOperation::Update],
            resources: vec![GroupVersionResource {
                group: "".to_string(),
                version: "v1".to_string(),
                resource: "pods".to_string(),
            }],
            fail_policy: FailPolicy::FailClosed,
        };

        assert!(registration.matches_operation(AdmissionOperation::Create));
        assert!(registration.matches_operation(AdmissionOperation::Update));
        assert!(!registration.matches_operation(AdmissionOperation::Delete));

        let matching_resource = GroupVersionResource {
            group: "".to_string(),
            version: "v1".to_string(),
            resource: "pods".to_string(),
        };
        assert!(registration.matches_resource(&matching_resource));

        let non_matching_resource = GroupVersionResource {
            group: "".to_string(),
            version: "v1".to_string(),
            resource: "services".to_string(),
        };
        assert!(!registration.matches_resource(&non_matching_resource));
    }
}
