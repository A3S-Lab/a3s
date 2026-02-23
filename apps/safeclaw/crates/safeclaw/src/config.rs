//! SafeClaw configuration management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Lenient deserializer for the `models` field.
///
/// When parsing HCL via `hcl::from_str`, repeated blocks like `providers`
/// are represented as maps instead of arrays, which causes `CodeConfig`
/// deserialization to fail. This deserializer catches that error and
/// returns a default `CodeConfig` — the real value is populated later
/// by `SafeClawConfig::from_hcl`.
fn deserialize_models_lenient<'de, D>(
    deserializer: D,
) -> Result<a3s_code::config::CodeConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Accept any value; if CodeConfig deserialization fails, return default
    let value: serde_json::Value = serde_json::Value::deserialize(deserializer)
        .unwrap_or(serde_json::Value::Object(Default::default()));
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// Main SafeClaw configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SafeClawConfig {
    /// Gateway configuration
    #[serde(default)]
    pub gateway: ServerConfig,

    /// A3S Gateway integration configuration
    #[serde(default)]
    pub a3s_gateway: A3sGatewayConfig,

    /// Channel configurations
    #[serde(default)]
    pub channels: ChannelsConfig,

    /// TEE configuration
    #[serde(default)]
    pub tee: TeeConfig,

    /// Privacy configuration
    #[serde(default)]
    pub privacy: PrivacyConfig,

    /// Model configuration (a3s-code CodeConfig format).
    /// Deserialized separately via `SafeClawConfig::from_hcl` to handle
    /// repeated HCL blocks (providers, models) correctly.
    #[serde(default, deserialize_with = "deserialize_models_lenient")]
    pub models: a3s_code::config::CodeConfig,

    /// Storage configuration
    #[serde(default)]
    pub storage: StorageConfig,

    /// Audit event pipeline configuration
    #[serde(default)]
    pub audit: AuditConfig,

    /// Skills configuration
    #[serde(default)]
    pub skills: SkillsConfig,

    /// Session lifecycle management configuration
    #[serde(default)]
    pub session_lifecycle: SessionLifecycleConfig,
}

impl SafeClawConfig {
    /// Parse configuration from an HCL string.
    ///
    /// The `models` block is extracted and parsed via `CodeConfig::from_hcl`
    /// which handles repeated HCL blocks (`providers`, `models`) as arrays.
    /// The remaining fields are deserialized via `hcl::from_str` with a
    /// lenient deserializer for the `models` field.
    pub fn from_hcl(content: &str) -> anyhow::Result<Self> {
        // Auto-detect JSON format (legacy safeclaw.local.hcl may be JSON CodeConfig)
        let trimmed = content.trim_start();
        if trimmed.starts_with('{') {
            // Try parsing as CodeConfig JSON first (camelCase fields from old format)
            if let Ok(code_config) = serde_json::from_str::<a3s_code::config::CodeConfig>(content) {
                let mut sc = SafeClawConfig::default();
                sc.models = code_config;
                return Ok(sc);
            }
            // Fall back to SafeClawConfig JSON
            let config: SafeClawConfig = serde_json::from_str(content)
                .map_err(|e| anyhow::anyhow!("Failed to parse JSON config: {}", e))?;
            return Ok(config);
        }

        // Extract the raw `models { ... }` block for CodeConfig parsing.
        // We rebuild the inner body as an HCL string by serializing each structure.
        let body: hcl::Body = hcl::from_str(content)?;
        let models_inner: Option<String> = body
            .blocks()
            .find(|b| b.identifier.as_str() == "models")
            .map(|b| {
                let inner_body = hcl::Body::from(b.body().iter().cloned().collect::<Vec<_>>());
                hcl::to_string(&inner_body).unwrap_or_default()
            });

        // Parse the models block with CodeConfig::from_hcl (handles repeated blocks)
        let code_config = if let Some(ref hcl_str) = models_inner {
            a3s_code::config::CodeConfig::from_hcl(hcl_str)
                .map_err(|e| anyhow::anyhow!("Failed to parse models config: {}", e))?
        } else {
            a3s_code::config::CodeConfig::default()
        };

        // Parse the rest of the config (models field uses lenient deserializer)
        let mut config: SafeClawConfig = hcl::from_str(content)
            .map_err(|e| anyhow::anyhow!("Failed to parse SafeClaw config: {}", e))?;
        config.models = code_config;

        Ok(config)
    }
}

/// A3S Gateway integration configuration
///
/// When enabled, SafeClaw runs as a backend service behind a3s-gateway.
/// The gateway handles TLS, routing, rate limiting, and authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct A3sGatewayConfig {
    /// Enable a3s-gateway integration mode
    pub enabled: bool,

    /// Service name registered in a3s-gateway
    pub service_name: String,

    /// A3S Gateway routing rule for SafeClaw API
    pub api_rule: String,

    /// A3S Gateway routing rule for WebSocket
    pub ws_rule: String,

    /// A3S Gateway routing rule for channel webhooks
    pub webhook_rule: String,

    /// Middlewares to apply via a3s-gateway
    pub middlewares: Vec<String>,

    /// Entrypoints to bind in a3s-gateway
    pub entrypoints: Vec<String>,

    /// Enable conversation affinity (sticky sessions)
    pub conversation_affinity: bool,

    /// Sticky session cookie name
    pub affinity_cookie: String,

    /// Enable token metering via a3s-gateway
    pub token_metering: bool,

    /// Max tokens per minute per user (0 = unlimited)
    pub max_tokens_per_minute: u64,
}

impl Default for A3sGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            service_name: "safeclaw".to_string(),
            api_rule: "PathPrefix(`/safeclaw/api`)".to_string(),
            ws_rule: "Path(`/safeclaw/ws`)".to_string(),
            webhook_rule: "PathPrefix(`/safeclaw/webhook`)".to_string(),
            middlewares: vec!["auth-jwt".to_string(), "rate-limit".to_string()],
            entrypoints: vec!["websecure".to_string()],
            conversation_affinity: true,
            affinity_cookie: "safeclaw_session".to_string(),
            token_metering: true,
            max_tokens_per_minute: 10000,
        }
    }
}

/// Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Enable TLS
    pub tls_enabled: bool,

    /// TLS certificate path
    pub tls_cert: Option<PathBuf>,

    /// TLS key path
    pub tls_key: Option<PathBuf>,

    /// WebSocket ping interval in seconds
    pub ws_ping_interval: u64,

    /// Maximum concurrent connections
    pub max_connections: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 18790,
            tls_enabled: false,
            tls_cert: None,
            tls_key: None,
            ws_ping_interval: 30,
            max_connections: 1000,
        }
    }
}

/// Channel configurations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelsConfig {
    /// Telegram channel config
    pub telegram: Option<TelegramConfig>,

    /// Slack channel config
    pub slack: Option<SlackConfig>,

    /// Discord channel config
    pub discord: Option<DiscordConfig>,

    /// WebChat channel config
    pub webchat: Option<WebChatConfig>,

    /// Feishu (Lark) channel config
    pub feishu: Option<FeishuConfig>,

    /// DingTalk channel config
    pub dingtalk: Option<DingTalkConfig>,

    /// WeCom (WeChat Work) channel config
    pub wecom: Option<WeComConfig>,
}

/// Telegram channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Bot token (stored in TEE)
    pub bot_token: String,

    /// Allowed user IDs
    pub allowed_users: Vec<i64>,

    /// DM policy: "pairing" or "open"
    pub dm_policy: String,
}

/// Slack channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    /// Bot token reference (stored in TEE)
    pub bot_token: String,

    /// App token reference (stored in TEE)
    pub app_token: String,

    /// Allowed workspace IDs
    pub allowed_workspaces: Vec<String>,

    /// DM policy
    pub dm_policy: String,
}

/// Discord channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// Bot token reference (stored in TEE)
    pub bot_token: String,

    /// Allowed guild IDs
    pub allowed_guilds: Vec<u64>,

    /// DM policy
    pub dm_policy: String,
}

/// WebChat channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebChatConfig {
    /// Enable WebChat
    pub enabled: bool,

    /// Require authentication
    pub require_auth: bool,

    /// Allowed origins for CORS
    pub allowed_origins: Vec<String>,
}

impl Default for WebChatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            require_auth: true,
            allowed_origins: vec!["http://localhost:*".to_string()],
        }
    }
}

/// Feishu (Lark) channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    /// App ID
    pub app_id: String,

    /// App secret reference (stored in TEE)
    pub app_secret: String,

    /// Encrypt key reference for callback verification (stored in TEE)
    pub encrypt_key: String,

    /// Verification token reference (stored in TEE)
    pub verification_token: String,

    /// Allowed user open_ids (empty = all allowed)
    pub allowed_users: Vec<String>,

    /// DM policy: "pairing" or "open"
    pub dm_policy: String,
}

/// DingTalk channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkConfig {
    /// App key reference (stored in TEE)
    pub app_key: String,

    /// App secret reference (stored in TEE)
    pub app_secret: String,

    /// Robot code identifier
    pub robot_code: String,

    /// Allowed user staffIds (empty = all allowed)
    pub allowed_users: Vec<String>,

    /// DM policy: "pairing" or "open"
    pub dm_policy: String,
}

/// WeCom (WeChat Work) channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeComConfig {
    /// Corp ID
    pub corp_id: String,

    /// Agent ID
    pub agent_id: u32,

    /// Corp secret reference (stored in TEE)
    pub secret: String,

    /// Encoding AES key reference for callback decryption (stored in TEE)
    pub encoding_aes_key: String,

    /// Callback token reference (stored in TEE)
    pub token: String,

    /// Allowed user IDs (empty = all allowed)
    pub allowed_users: Vec<String>,

    /// DM policy: "pairing" or "open"
    pub dm_policy: String,
}

/// TEE configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TeeConfig {
    /// Enable TEE mode
    pub enabled: bool,

    /// TEE backend type
    pub backend: TeeBackend,

    /// A3S Box image reference
    pub box_image: String,

    /// Memory allocation for TEE in MB
    pub memory_mb: u32,

    /// CPU cores for TEE
    pub cpu_cores: u32,

    /// Vsock port for TEE secure channel
    pub vsock_port: u32,

    /// Attestation configuration
    pub attestation: AttestationConfig,

    /// Path to a3s-box-shim binary (None = search PATH)
    #[serde(default)]
    pub shim_path: Option<PathBuf>,

    /// Allow simulated TEE reports (development mode only)
    #[serde(default)]
    pub allow_simulated: bool,

    /// Secrets to inject into TEE on boot
    #[serde(default)]
    pub secrets: Vec<SecretRef>,

    /// Workspace directory to mount into VM
    #[serde(default)]
    pub workspace_dir: Option<PathBuf>,

    /// Socket directory for VM communication
    #[serde(default)]
    pub socket_dir: Option<PathBuf>,

    /// Network firewall policy for outbound connections
    #[serde(default)]
    pub network_policy: crate::guard::NetworkPolicy,

    /// Fallback policy when TEE is expected but unavailable.
    /// Controls how the policy engine handles `ProcessInTee` decisions
    /// when the security level is `ProcessOnly`.
    #[serde(default)]
    pub fallback_policy: TeeFallbackPolicy,
}

/// Reference to a secret to inject into the TEE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    /// Secret name (used as key inside TEE)
    pub name: String,

    /// Environment variable to read the secret value from
    pub env_var: String,

    /// Whether to also set as environment variable inside TEE
    #[serde(default = "default_true")]
    pub set_env: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TeeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: TeeBackend::A3sBox,
            box_image: "ghcr.io/a3s-lab/safeclaw-tee:latest".to_string(),
            memory_mb: 2048,
            cpu_cores: 2,
            vsock_port: a3s_common::ports::TEE_CHANNEL,
            attestation: AttestationConfig::default(),
            shim_path: None,
            allow_simulated: false,
            secrets: Vec::new(),
            workspace_dir: None,
            socket_dir: None,
            network_policy: crate::guard::NetworkPolicy::default(),
            fallback_policy: TeeFallbackPolicy::default(),
        }
    }
}

/// Policy for handling `ProcessInTee` decisions when TEE is unavailable.
///
/// Controls the security/availability tradeoff when the system detects
/// that TEE hardware is not present but the policy engine would route
/// data to the TEE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeeFallbackPolicy {
    /// Reject processing of sensitive data when TEE is unavailable.
    /// Most secure — no silent degradation.
    Reject,
    /// Allow processing with a warning in the audit log.
    /// Balances security and availability.
    Warn,
    /// Allow processing silently (current behavior, least secure).
    Allow,
}

impl Default for TeeFallbackPolicy {
    fn default() -> Self {
        Self::Warn
    }
}

/// TEE backend type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TeeBackend {
    /// A3S Box MicroVM (default)
    #[default]
    A3sBox,

    /// Intel SGX
    IntelSgx,

    /// AMD SEV
    AmdSev,

    /// ARM TrustZone
    ArmTrustzone,
}

/// Attestation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AttestationConfig {
    /// Enable remote attestation
    pub enabled: bool,

    /// Attestation provider
    pub provider: String,

    /// Expected measurements
    pub expected_measurements: HashMap<String, String>,
}

impl Default for AttestationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "local".to_string(),
            expected_measurements: HashMap::new(),
        }
    }
}

/// Privacy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyConfig {
    /// Enable automatic privacy classification
    pub auto_classify: bool,

    /// Default sensitivity level for unclassified data
    pub default_level: SensitivityLevel,

    /// Classification rules
    pub rules: Vec<ClassificationRule>,

    /// Data retention policy
    pub retention: RetentionConfig,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            auto_classify: true,
            default_level: SensitivityLevel::Normal,
            rules: default_classification_rules(),
            retention: RetentionConfig::default(),
        }
    }
}

// Re-export from shared a3s-common crate (single source of truth)
pub use a3s_common::privacy::default_classification_rules;
pub use a3s_common::privacy::ClassificationRule;
pub use a3s_common::privacy::SensitivityLevel;

/// Data retention configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionConfig {
    /// Retention period for normal data in days
    pub normal_days: u32,

    /// Retention period for sensitive data in days
    pub sensitive_days: u32,

    /// Enable automatic cleanup
    pub auto_cleanup: bool,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            normal_days: 30,
            sensitive_days: 7,
            auto_cleanup: true,
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Base directory for storage
    pub base_dir: PathBuf,

    /// Session storage path
    pub sessions_dir: PathBuf,

    /// Secure storage path (in TEE)
    pub secure_dir: PathBuf,

    /// Enable encryption at rest
    pub encrypt_at_rest: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("safeclaw");

        Self {
            sessions_dir: base.join("sessions"),
            secure_dir: base.join("secure"),
            base_dir: base,
            encrypt_at_rest: true,
        }
    }
}

/// Audit event pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuditConfig {
    /// Broadcast channel / audit log buffer capacity
    pub bus_capacity: usize,

    /// Alert monitor configuration
    pub alert: crate::audit::AlertConfig,

    /// File-based audit persistence configuration
    #[serde(default)]
    pub persistence: crate::audit::PersistenceConfig,

    /// a3s-event bridge configuration (NATS integration)
    #[serde(default)]
    pub event_bridge: EventBridgeConfig,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            bus_capacity: 10_000,
            alert: crate::audit::AlertConfig::default(),
            persistence: crate::audit::PersistenceConfig::default(),
            event_bridge: EventBridgeConfig::default(),
        }
    }
}

/// Configuration for bridging audit events to a3s-event (NATS/memory).
///
/// When enabled, all audit events are forwarded to an `a3s-event::EventBus`
/// under the `audit.<severity>.<vector>` subject hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EventBridgeConfig {
    /// Enable the a3s-event bridge
    pub enabled: bool,

    /// NATS server URL (e.g., "nats://localhost:4222")
    /// When empty, falls back to in-memory provider.
    #[serde(default)]
    pub nats_url: String,
}

impl Default for EventBridgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            nats_url: String::new(),
        }
    }
}

/// Skills configuration for runtime skill management and self-bootstrap
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsConfig {
    /// Directory for storing skill files (.md)
    #[serde(default = "SkillsConfig::default_dir")]
    pub dir: String,

    /// Auto-load skills from directory on startup
    #[serde(default = "SkillsConfig::default_auto_load")]
    pub auto_load: bool,
}

impl SkillsConfig {
    fn default_dir() -> String {
        "./skills".to_string()
    }

    fn default_auto_load() -> bool {
        true
    }
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            dir: Self::default_dir(),
            auto_load: Self::default_auto_load(),
        }
    }
}

/// Session lifecycle management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLifecycleConfig {
    /// Seconds of inactivity before auto-archiving (default: 30 min)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    /// Seconds after archival before purging (default: 7 days)
    #[serde(default = "default_purge_after")]
    pub purge_after_secs: u64,
    /// Maximum number of active sessions (default: 100)
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
    /// Cleanup scan interval in seconds (default: 60)
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_secs: u64,
}

fn default_idle_timeout() -> u64 {
    1800
}
fn default_purge_after() -> u64 {
    604800
}
fn default_max_sessions() -> usize {
    100
}
fn default_cleanup_interval() -> u64 {
    60
}

impl Default for SessionLifecycleConfig {
    fn default() -> Self {
        Self {
            idle_timeout_secs: default_idle_timeout(),
            purge_after_secs: default_purge_after(),
            max_sessions: default_max_sessions(),
            cleanup_interval_secs: default_cleanup_interval(),
        }
    }
}

/// Per-channel agent configuration override.
///
/// Stored as a JSON file in `<config_dir>/safeclaw/channel-agents.json`.
/// Applied at session creation time when a message arrives from a channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelAgentConfig {
    /// Model override (None = use global default)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Permission mode: "default", "strict", or "trust"
    #[serde(default = "default_permission_mode")]
    pub permission_mode: String,

    /// Allowed tools (None = all tools allowed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,

    /// Blocked tools
    #[serde(default)]
    pub blocked_tools: Vec<String>,
}

fn default_permission_mode() -> String {
    "default".to_string()
}

/// Persistent store for per-channel agent configs.
///
/// Reads/writes `<config_dir>/safeclaw/channel-agents.json`.
#[derive(Clone)]
pub struct ChannelAgentConfigStore {
    path: std::path::PathBuf,
    configs: std::sync::Arc<tokio::sync::RwLock<HashMap<String, ChannelAgentConfig>>>,
}

impl ChannelAgentConfigStore {
    /// Load or create the config store.
    pub async fn new(config_dir: std::path::PathBuf) -> Self {
        let path = config_dir.join("channel-agents.json");
        let configs = if path.exists() {
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => HashMap::new(),
            }
        } else {
            HashMap::new()
        };
        Self {
            path,
            configs: std::sync::Arc::new(tokio::sync::RwLock::new(configs)),
        }
    }

    /// Get config for a channel.
    pub async fn get(&self, channel_id: &str) -> Option<ChannelAgentConfig> {
        self.configs.read().await.get(channel_id).cloned()
    }

    /// Get all channel configs.
    pub async fn get_all(&self) -> HashMap<String, ChannelAgentConfig> {
        self.configs.read().await.clone()
    }

    /// Update config for a channel and persist to disk.
    pub async fn set(&self, channel_id: &str, config: ChannelAgentConfig) -> anyhow::Result<()> {
        {
            let mut configs = self.configs.write().await;
            configs.insert(channel_id.to_string(), config);
        }
        self.persist().await
    }

    /// Remove config for a channel and persist.
    pub async fn remove(&self, channel_id: &str) -> anyhow::Result<()> {
        {
            let mut configs = self.configs.write().await;
            configs.remove(channel_id);
        }
        self.persist().await
    }

    /// Write current state to disk.
    async fn persist(&self) -> anyhow::Result<()> {
        let configs = self.configs.read().await;
        let json = serde_json::to_string_pretty(&*configs)?;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&self.path, json).await?;
        Ok(())
    }
}

// Helper module for default directories
mod dirs {
    use std::path::PathBuf;

    pub fn data_local_dir() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join("Library/Application Support"))
        }
        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_DATA_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| PathBuf::from(h).join(".local/share"))
                })
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var("LOCALAPPDATA").ok().map(PathBuf::from)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SafeClawConfig::default();
        assert_eq!(config.gateway.port, 18790);
        assert!(config.tee.enabled);
        assert!(config.privacy.auto_classify);
    }

    #[test]
    fn test_classification_rules() {
        let rules = default_classification_rules();
        assert!(!rules.is_empty());
        assert!(rules.iter().any(|r| r.name == "credit_card"));
    }

    #[test]
    fn test_feishu_config_serialize() {
        let config = FeishuConfig {
            app_id: "cli_test123".to_string(),
            app_secret: "feishu_secret".to_string(),
            encrypt_key: "feishu_encrypt".to_string(),
            verification_token: "feishu_token".to_string(),
            allowed_users: vec!["ou_user1".to_string()],
            dm_policy: "pairing".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: FeishuConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.app_id, "cli_test123");
        assert_eq!(deserialized.allowed_users.len(), 1);
    }

    #[test]
    fn test_dingtalk_config_serialize() {
        let config = DingTalkConfig {
            app_key: "dt_key".to_string(),
            app_secret: "dt_secret".to_string(),
            robot_code: "robot123".to_string(),
            allowed_users: vec!["staff1".to_string(), "staff2".to_string()],
            dm_policy: "open".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DingTalkConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.robot_code, "robot123");
        assert_eq!(deserialized.allowed_users.len(), 2);
    }

    #[test]
    fn test_wecom_config_serialize() {
        let config = WeComConfig {
            corp_id: "ww_corp123".to_string(),
            agent_id: 1000001,
            secret: "wc_secret".to_string(),
            encoding_aes_key: "wc_aes".to_string(),
            token: "wc_token".to_string(),
            allowed_users: vec![],
            dm_policy: "pairing".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: WeComConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.corp_id, "ww_corp123");
        assert_eq!(deserialized.agent_id, 1000001);
        assert!(deserialized.allowed_users.is_empty());
    }

    #[test]
    fn test_channels_config_with_new_channels() {
        let config = ChannelsConfig {
            feishu: Some(FeishuConfig {
                app_id: "cli_test".to_string(),
                app_secret: "secret".to_string(),
                encrypt_key: "encrypt".to_string(),
                verification_token: "token".to_string(),
                allowed_users: vec![],
                dm_policy: "open".to_string(),
            }),
            dingtalk: Some(DingTalkConfig {
                app_key: "key".to_string(),
                app_secret: "secret".to_string(),
                robot_code: "robot".to_string(),
                allowed_users: vec![],
                dm_policy: "open".to_string(),
            }),
            wecom: Some(WeComConfig {
                corp_id: "corp".to_string(),
                agent_id: 100,
                secret: "secret".to_string(),
                encoding_aes_key: "aes".to_string(),
                token: "token".to_string(),
                allowed_users: vec![],
                dm_policy: "open".to_string(),
            }),
            ..Default::default()
        };
        assert!(config.feishu.is_some());
        assert!(config.dingtalk.is_some());
        assert!(config.wecom.is_some());
        assert!(config.telegram.is_none());
    }

    #[test]
    fn test_event_bridge_config_default() {
        let config = EventBridgeConfig::default();
        assert!(!config.enabled);
        assert!(config.nats_url.is_empty());
    }

    #[test]
    fn test_event_bridge_config_serde() {
        let json = serde_json::json!({
            "enabled": true,
            "nats_url": "nats://localhost:4222"
        });
        let config: EventBridgeConfig = serde_json::from_value(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.nats_url, "nats://localhost:4222");
    }

    #[test]
    fn test_audit_config_includes_event_bridge() {
        let config = AuditConfig::default();
        assert!(!config.event_bridge.enabled);
    }
}
