use std::path::{Path, PathBuf};

use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::config::{
    AutoDelegationConfig, ConfigSection, DocumentParserConfig, OsConfig, SearchConfig,
    StorageBackend,
};
use a3s_code_core::mcp::McpServerConfig;
use a3s_code_core::memory::MemoryConfig;
use a3s_code_core::queue::SessionQueueConfig;
use a3s_code_core::{CodeConfig, ModelConfig, ProviderConfig};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::{json, Map, Value};

use super::secrets::{
    preserve_mcp_secrets, preserve_provider_secrets, preserve_secret, redact_headers,
    redact_secret, sanitized_json_value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsCategory {
    Llm,
    Agent,
    Context,
    Integrations,
}

impl SettingsCategory {
    pub(super) fn parse(value: &str) -> BootResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "llm" | "ai" | "model" | "models" => Ok(Self::Llm),
            "agent" | "execution" => Ok(Self::Agent),
            "context" | "storage" | "memory" => Ok(Self::Context),
            "integration" | "integrations" | "tools" => Ok(Self::Integrations),
            _ => Err(BootError::BadRequest(format!(
                "unknown config category `{value}`; expected llm, agent, context, or integrations"
            ))),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Agent => "agent",
            Self::Context => "context",
            Self::Integrations => "integrations",
        }
    }

    fn effect(self) -> Value {
        match self {
            Self::Llm => json!({
                "scope": "newTasks",
                "label": "Applies to new tasks",
                "description": "Running tasks keep their current model client and limits.",
            }),
            Self::Agent => json!({
                "scope": "restartRequired",
                "label": "Service restart required",
                "description": "Agent tools, directories, delegation, and queue runtimes are created at startup.",
            }),
            Self::Context => json!({
                "scope": "restartRequired",
                "label": "Service restart required",
                "description": "Storage and memory providers are attached when the service or session starts.",
            }),
            Self::Integrations => json!({
                "scope": "restartRequired",
                "label": "Service restart required",
                "description": "Search, document parsing, MCP, and A3S OS integrations are registered at startup.",
            }),
        }
    }
}

pub(super) fn category_settings(
    category: SettingsCategory,
    config: &CodeConfig,
    config_path: &Path,
) -> Value {
    let value = match category {
        SettingsCategory::Llm => llm_settings(config),
        SettingsCategory::Agent => json!({
            "skillDirs": config.skill_dirs,
            "agentDirs": config.agent_dirs,
            "maxToolRounds": config.max_tool_rounds,
            "maxParallelTasks": config.max_parallel_tasks,
            "autoParallel": config.auto_parallel,
            "autoDelegation": config.auto_delegation,
            "queue": config.queue.as_ref().map(sanitized_json_value),
        }),
        SettingsCategory::Context => json!({
            "storageBackend": config.storage_backend,
            "sessionsDir": config.sessions_dir,
            "memoryDir": config.memory_dir,
            "storageUrl": redact_secret(config.storage_url.as_deref()),
            "memory": config.memory.as_ref().map(sanitized_json_value),
        }),
        SettingsCategory::Integrations => json!({
            "os": config.os,
            "search": config.search.as_ref().map(sanitized_json_value),
            "documentParser": config.document_parser.as_ref().map(sanitized_json_value),
            "mcpServers": sanitized_json_value(&config.mcp_servers),
        }),
    };
    with_metadata(value, category, config_path)
}

pub(super) fn llm_settings(config: &CodeConfig) -> Value {
    json!({
        "defaultModel": config.default_model.clone().unwrap_or_default(),
        "providers": config.providers.iter().map(provider_settings).collect::<Vec<_>>(),
        "thinkingBudget": config.thinking_budget,
        "llmApiTimeoutMs": config.llm_api_timeout_ms,
        // Compatibility fields remain readable while their primary editor lives in Agent & Execution.
        "maxToolRounds": config.max_tool_rounds,
        "maxParallelTasks": config.max_parallel_tasks,
        "autoParallel": config.auto_parallel,
    })
}

pub(super) fn apply_category_patch(
    category: SettingsCategory,
    config: &mut CodeConfig,
    patch: Value,
) -> BootResult<Vec<ConfigSection>> {
    let sections = match category {
        SettingsCategory::Llm => apply_llm_patch(config, parse_patch(patch, "llm")?)?,
        SettingsCategory::Agent => apply_agent_patch(config, parse_patch(patch, "agent")?)?,
        SettingsCategory::Context => apply_context_patch(config, parse_patch(patch, "context")?)?,
        SettingsCategory::Integrations => {
            apply_integrations_patch(config, parse_patch(patch, "integrations")?)?
        }
    };
    if sections.is_empty() {
        return Err(BootError::BadRequest(format!(
            "{} settings update did not contain any editable fields",
            category.id()
        )));
    }
    Ok(deduplicate_sections(sections))
}

fn with_metadata(value: Value, category: SettingsCategory, config_path: &Path) -> Value {
    let mut object = match value {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    object.insert(
        "category".to_string(),
        Value::String(category.id().to_string()),
    );
    object.insert("effect".to_string(), category.effect());
    object.insert(
        "configPath".to_string(),
        Value::String(config_path.display().to_string()),
    );
    Value::Object(object)
}

fn parse_patch<T: DeserializeOwned>(value: Value, category: &str) -> BootResult<T> {
    serde_json::from_value(value)
        .map_err(|error| BootError::BadRequest(format!("invalid {category} settings: {error}")))
}

#[derive(Debug, Default)]
enum Patch<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

impl<'de, T> Deserialize<'de> for Patch<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Self::Value)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct LlmPatch {
    #[serde(alias = "default_model")]
    default_model: Patch<String>,
    providers: Patch<Vec<ProviderConfig>>,
    #[serde(alias = "thinking_budget")]
    thinking_budget: Patch<usize>,
    #[serde(alias = "llm_api_timeout_ms")]
    llm_api_timeout_ms: Patch<u64>,
    #[serde(alias = "max_tool_rounds")]
    max_tool_rounds: Patch<usize>,
    #[serde(alias = "max_parallel_tasks")]
    max_parallel_tasks: Patch<usize>,
    #[serde(alias = "auto_parallel")]
    auto_parallel: Patch<bool>,
}

fn apply_llm_patch(config: &mut CodeConfig, patch: LlmPatch) -> BootResult<Vec<ConfigSection>> {
    let mut sections = Vec::new();
    if let Patch::Value(value) = patch.default_model {
        config.default_model = value.and_then(non_empty);
        sections.push(ConfigSection::DefaultModel);
    }
    if let Patch::Value(value) = patch.providers {
        let mut providers = value.unwrap_or_default();
        preserve_provider_secrets(&mut providers, &config.providers);
        config.providers = providers;
        sections.push(ConfigSection::Providers);
    }
    if let Patch::Value(value) = patch.thinking_budget {
        config.thinking_budget = value;
        sections.push(ConfigSection::ModelRuntime);
    }
    if let Patch::Value(value) = patch.llm_api_timeout_ms {
        config.llm_api_timeout_ms = value;
        sections.push(ConfigSection::ModelRuntime);
    }
    if let Patch::Value(value) = patch.max_tool_rounds {
        config.max_tool_rounds = value;
        sections.push(ConfigSection::Execution);
    }
    if let Patch::Value(value) = patch.max_parallel_tasks {
        config.max_parallel_tasks = value;
        sections.push(ConfigSection::Execution);
    }
    if let Patch::Value(value) = patch.auto_parallel {
        config.auto_parallel = value;
        if let Some(value) = value {
            config.auto_delegation.auto_parallel = value;
        }
        sections.push(ConfigSection::Execution);
    }
    Ok(sections)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct AgentPatch {
    #[serde(alias = "skill_dirs")]
    skill_dirs: Patch<Vec<PathBuf>>,
    #[serde(alias = "agent_dirs")]
    agent_dirs: Patch<Vec<PathBuf>>,
    #[serde(alias = "max_tool_rounds")]
    max_tool_rounds: Patch<usize>,
    #[serde(alias = "max_parallel_tasks")]
    max_parallel_tasks: Patch<usize>,
    #[serde(alias = "auto_parallel")]
    auto_parallel: Patch<bool>,
    #[serde(alias = "auto_delegation")]
    auto_delegation: Patch<AutoDelegationConfig>,
    queue: Patch<SessionQueueConfig>,
}

fn apply_agent_patch(config: &mut CodeConfig, patch: AgentPatch) -> BootResult<Vec<ConfigSection>> {
    let mut sections = Vec::new();
    if let Patch::Value(value) = patch.skill_dirs {
        config.skill_dirs = value.unwrap_or_default();
        sections.push(ConfigSection::Execution);
    }
    if let Patch::Value(value) = patch.agent_dirs {
        config.agent_dirs = value.unwrap_or_default();
        sections.push(ConfigSection::Execution);
    }
    if let Patch::Value(value) = patch.max_tool_rounds {
        config.max_tool_rounds = value;
        sections.push(ConfigSection::Execution);
    }
    if let Patch::Value(value) = patch.max_parallel_tasks {
        config.max_parallel_tasks = value;
        sections.push(ConfigSection::Execution);
    }
    if let Patch::Value(value) = patch.auto_parallel {
        config.auto_parallel = value;
        if let Some(value) = value {
            config.auto_delegation.auto_parallel = value;
        }
        sections.push(ConfigSection::Execution);
    }
    if let Patch::Value(value) = patch.auto_delegation {
        config.auto_delegation = required(value, "autoDelegation")?;
        sections.push(ConfigSection::Execution);
    }
    if let Patch::Value(value) = patch.queue {
        config.queue = value;
        sections.push(ConfigSection::Queue);
    }
    Ok(sections)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct ContextPatch {
    #[serde(alias = "storage_backend")]
    storage_backend: Patch<StorageBackend>,
    #[serde(alias = "sessions_dir")]
    sessions_dir: Patch<PathBuf>,
    #[serde(alias = "memory_dir")]
    memory_dir: Patch<PathBuf>,
    #[serde(alias = "storage_url")]
    storage_url: Patch<String>,
    memory: Patch<MemoryConfig>,
}

fn apply_context_patch(
    config: &mut CodeConfig,
    patch: ContextPatch,
) -> BootResult<Vec<ConfigSection>> {
    let mut sections = Vec::new();
    if let Patch::Value(value) = patch.storage_backend {
        config.storage_backend = required(value, "storageBackend")?;
        sections.push(ConfigSection::Storage);
    }
    if let Patch::Value(value) = patch.sessions_dir {
        config.sessions_dir = value;
        sections.push(ConfigSection::Storage);
    }
    if let Patch::Value(value) = patch.memory_dir {
        config.memory_dir = value;
        sections.push(ConfigSection::Storage);
    }
    if let Patch::Value(value) = patch.storage_url {
        let mut value = value.and_then(non_empty);
        preserve_secret(&mut value, config.storage_url.as_deref());
        config.storage_url = value;
        sections.push(ConfigSection::Storage);
    }
    if let Patch::Value(value) = patch.memory {
        config.memory = value;
        sections.push(ConfigSection::Memory);
    }
    Ok(sections)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct IntegrationsPatch {
    os: Patch<OsConfig>,
    search: Patch<SearchConfig>,
    #[serde(alias = "document_parser")]
    document_parser: Patch<DocumentParserConfig>,
    #[serde(alias = "mcp_servers")]
    mcp_servers: Patch<Vec<McpServerConfig>>,
}

fn apply_integrations_patch(
    config: &mut CodeConfig,
    patch: IntegrationsPatch,
) -> BootResult<Vec<ConfigSection>> {
    let mut sections = Vec::new();
    if let Patch::Value(value) = patch.os {
        config.os = value;
        sections.push(ConfigSection::Os);
    }
    if let Patch::Value(value) = patch.search {
        config.search = value;
        sections.push(ConfigSection::Search);
    }
    if let Patch::Value(value) = patch.document_parser {
        config.document_parser = value;
        sections.push(ConfigSection::DocumentParser);
    }
    if let Patch::Value(value) = patch.mcp_servers {
        let mut servers = value.unwrap_or_default();
        preserve_mcp_secrets(&mut servers, &config.mcp_servers);
        config.mcp_servers = servers;
        sections.push(ConfigSection::McpServers);
    }
    Ok(sections)
}

fn provider_settings(provider: &ProviderConfig) -> Value {
    json!({
        "name": provider.name,
        "apiKey": redact_secret(provider.api_key.as_deref()),
        "baseUrl": provider.base_url,
        "headers": redact_headers(&provider.headers),
        "sessionIdHeader": provider.session_id_header,
        "models": provider.models.iter().map(model_settings).collect::<Vec<_>>(),
    })
}

fn model_settings(model: &ModelConfig) -> Value {
    json!({
        "id": model.id,
        "name": display_model_name(model),
        "family": non_empty(model.family.clone()),
        "apiKey": redact_secret(model.api_key.as_deref()),
        "baseUrl": model.base_url,
        "headers": redact_headers(&model.headers),
        "sessionIdHeader": model.session_id_header,
        "attachment": model.attachment,
        "reasoning": model.reasoning,
        "toolCall": model.tool_call,
        "temperature": model.temperature,
        "releaseDate": model.release_date,
        "modalities": model.modalities,
        "cost": model.cost,
        "limit": model.limit,
    })
}

fn display_model_name(model: &ModelConfig) -> &str {
    let name = model.name.trim();
    if name.is_empty() {
        model.id.as_str()
    } else {
        name
    }
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn required<T>(value: Option<T>, field: &str) -> BootResult<T> {
    value.ok_or_else(|| BootError::BadRequest(format!("{field} must not be null")))
}

fn deduplicate_sections(sections: Vec<ConfigSection>) -> Vec<ConfigSection> {
    let mut result = Vec::new();
    for section in sections {
        if !result.contains(&section) {
            result.push(section);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_patch_rejects_unknown_fields() {
        let mut config = CodeConfig::default();
        let error = apply_category_patch(
            SettingsCategory::Agent,
            &mut config,
            json!({ "notASetting": true }),
        )
        .expect_err("unknown field must fail");
        assert!(error.to_string().contains("notASetting"));
    }

    #[test]
    fn nullable_patch_clears_optional_sections() {
        let mut config = CodeConfig {
            memory: Some(MemoryConfig::default()),
            ..CodeConfig::default()
        };

        let sections = apply_category_patch(
            SettingsCategory::Context,
            &mut config,
            json!({ "memory": null }),
        )
        .expect("clear memory");

        assert!(config.memory.is_none());
        assert_eq!(sections, [ConfigSection::Memory]);
    }
}
