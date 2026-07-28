use std::sync::Arc;

use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::{CodeConfig, ModelConfig};
use serde_json::{json, Value};

use super::categories::{apply_category_patch, category_settings, SettingsCategory};
use super::persistence::persist_config_sections;
use super::validation::validate_config;
use crate::api::code_web::state::CodeWebState;
use crate::config;
use crate::model::catalog::{ModelCatalog, ModelEntry};
use crate::model::route::ModelSource;

pub(in crate::api::code_web) struct ConfigService {
    state: Arc<CodeWebState>,
}

impl ConfigService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) fn system_info(&self) -> Value {
        json!({
            "appName": "书小安",
            "logoUrl": "/logo.png",
            "version": env!("CARGO_PKG_VERSION"),
        })
    }

    pub(in crate::api::code_web) fn assistant_settings(&self) -> Value {
        let code_config = self.state.code_config_snapshot();
        json!({
            "name": "书小安",
            "avatar": "",
            "description": "A3S Code local assistant",
            "model": code_config.default_model,
        })
    }

    pub(in crate::api::code_web) fn app_settings(&self) -> Value {
        let code_config = self.state.code_config_snapshot();
        let workspace = self.state.default_workspace.display().to_string();
        let storage_path = config::memory_dir().display().to_string();
        let config_path = self.state.config_path.as_path();
        json!({
            "general": {
                "appName": "书小安",
                "language": "zh-CN",
                "splashScreen": true,
                "restoreWorkspace": true,
                "workspacePath": workspace,
                "localStoragePath": storage_path,
            },
            "appearance": {
                "theme": "system",
                "sideBarPosition": "left",
                "statusBar": true,
                "activityBar": true,
                "zoomLevel": 1,
            },
            "llm": category_settings(SettingsCategory::Llm, &code_config, config_path),
            "agent": category_settings(SettingsCategory::Agent, &code_config, config_path),
            "context": category_settings(SettingsCategory::Context, &code_config, config_path),
            "integrations": category_settings(
                SettingsCategory::Integrations,
                &code_config,
                config_path,
            ),
        })
    }

    pub(in crate::api::code_web) fn config_category(&self, name: &str) -> BootResult<Value> {
        let category = SettingsCategory::parse(name)?;
        let config = self.state.code_config_snapshot();
        Ok(category_settings(
            category,
            &config,
            &self.state.config_path,
        ))
    }

    pub(in crate::api::code_web) fn update_app_settings(
        &self,
        request: Value,
    ) -> BootResult<Value> {
        let mut config = self
            .state
            .code_config
            .write()
            .map_err(|_| BootError::Internal("code config lock was poisoned".to_string()))?;
        let mut candidate = config.clone();
        let mut sections = Vec::new();

        for (category, keys) in [
            (SettingsCategory::Llm, &["llm", "ai"][..]),
            (SettingsCategory::Agent, &["agent", "execution"][..]),
            (
                SettingsCategory::Context,
                &["context", "storage", "memory"][..],
            ),
            (
                SettingsCategory::Integrations,
                &["integrations", "integration", "tools"][..],
            ),
        ] {
            let Some(patch) = keys.iter().find_map(|key| request.get(*key)).cloned() else {
                continue;
            };
            sections.extend(apply_category_patch(category, &mut candidate, patch)?);
        }

        if sections.is_empty() {
            return Err(BootError::BadRequest(
                "settings update did not contain llm, agent, context, or integrations".to_string(),
            ));
        }

        ensure_valid(&candidate)?;
        let persisted = persist_config_sections(&self.state.config_path, &candidate, &sections)?;
        *config = persisted;
        drop(config);
        Ok(self.app_settings())
    }

    pub(in crate::api::code_web) fn update_config_category(
        &self,
        name: &str,
        patch: Value,
    ) -> BootResult<Value> {
        let category = SettingsCategory::parse(name)?;
        let mut config = self
            .state
            .code_config
            .write()
            .map_err(|_| BootError::Internal("code config lock was poisoned".to_string()))?;
        let persisted =
            update_category_document(&self.state.config_path, &config, category, patch)?;
        *config = persisted;
        Ok(category_settings(
            category,
            &config,
            &self.state.config_path,
        ))
    }

    pub(in crate::api::code_web) fn validate(&self, request: Value) -> BootResult<Value> {
        if let Some(content) = request.get("content").and_then(Value::as_str) {
            return Ok(match CodeConfig::from_acl(content) {
                Ok(config) => validation_response(&config, validate_config(&config)),
                Err(error) => json!({
                    "valid": false,
                    "issues": [error.to_string()],
                    "summary": null,
                }),
            });
        }

        let category_name = request
            .get("category")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                BootError::BadRequest(
                    "validation requires `content` or a `category` and `patch`".to_string(),
                )
            })?;
        let category = SettingsCategory::parse(category_name)?;
        let patch = request
            .get("patch")
            .or_else(|| request.get("settings"))
            .cloned()
            .ok_or_else(|| BootError::BadRequest("validation patch is required".to_string()))?;
        let mut candidate = self.state.code_config_snapshot();
        if let Err(error) = apply_category_patch(category, &mut candidate, patch) {
            return Ok(json!({
                "valid": false,
                "issues": [error.to_string()],
                "summary": config_summary(&candidate),
            }));
        }
        Ok(validation_response(&candidate, validate_config(&candidate)))
    }

    pub(in crate::api::code_web) fn llm_diagnostics(&self) -> Value {
        let code_config = self.state.code_config_snapshot();
        let default_model = code_config.default_model.clone().unwrap_or_default();
        let default_ref = split_model_ref(&default_model);
        let mut issues = Vec::new();

        if code_config.providers.is_empty() {
            issues.push("no providers are configured".to_string());
        }
        if default_model.is_empty() {
            issues.push("defaultModel is not configured".to_string());
        } else if let Some((provider_name, model_id)) = default_ref.as_ref() {
            match code_config
                .providers
                .iter()
                .find(|provider| provider.name == *provider_name)
            {
                Some(provider) if provider.models.iter().any(|model| model.id == *model_id) => {}
                Some(_) => issues.push(format!(
                    "default model `{model_id}` was not found in provider `{provider_name}`"
                )),
                None => issues.push(format!("default provider `{provider_name}` was not found")),
            }
        }

        let providers = code_config
            .providers
            .iter()
            .map(|provider| {
                json!({
                    "name": provider.name,
                    "modelCount": provider.models.len(),
                    "apiKeyConfigured": has_secret(provider.api_key.as_deref()),
                    "baseUrlConfigured": provider.base_url.as_deref().is_some_and(|value| !value.trim().is_empty()),
                    "isDefault": default_ref
                        .as_ref()
                        .is_some_and(|(provider_name, _)| provider_name == &provider.name),
                })
            })
            .collect::<Vec<_>>();

        json!({
            "ok": issues.is_empty(),
            "defaultModel": default_model,
            "providers": providers,
            "issues": issues,
        })
    }

    pub(in crate::api::code_web) fn model_catalog(&self) -> Value {
        let config = self.state.code_config_snapshot();
        let catalog = ModelCatalog::local_with_config(&config);
        model_catalog_from_entries(&config, &catalog.entries, catalog.warnings)
    }

    pub(in crate::api::code_web) async fn refresh_model_catalog(&self) -> Value {
        let config = self.state.code_config_snapshot();
        let catalog = ModelCatalog::discover_with_config(&config, true).await;
        model_catalog_from_entries(&config, &catalog.entries, catalog.warnings)
    }

    pub(in crate::api::code_web) fn fetch_provider_models(
        &self,
        request: Value,
    ) -> BootResult<Value> {
        let provider_name = request
            .get("providerName")
            .or_else(|| request.get("provider_name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| BootError::BadRequest("providerName is required".to_string()))?;

        let code_config = self.state.code_config_snapshot();
        let provider = code_config
            .providers
            .iter()
            .find(|provider| provider.name == provider_name);
        let base_url = request
            .get("baseUrl")
            .or_else(|| request.get("base_url"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| provider.and_then(|provider| provider.base_url.clone()))
            .unwrap_or_default();
        let models = provider
            .map(|provider| {
                provider
                    .models
                    .iter()
                    .map(|model| {
                        json!({
                            "id": model.id,
                            "name": display_model_name(model),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(json!({
            "providerName": provider_name,
            "baseUrl": base_url,
            "models": models,
        }))
    }
}

fn update_category_document(
    path: &std::path::Path,
    current: &CodeConfig,
    category: SettingsCategory,
    patch: Value,
) -> BootResult<CodeConfig> {
    let mut candidate = current.clone();
    let sections = apply_category_patch(category, &mut candidate, patch)?;
    ensure_valid(&candidate)?;
    persist_config_sections(path, &candidate, &sections)
}

fn ensure_valid(config: &CodeConfig) -> BootResult<()> {
    let issues = validate_config(config);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(BootError::BadRequest(format!(
            "configuration is invalid: {}",
            issues.join("; ")
        )))
    }
}

fn validation_response(config: &CodeConfig, issues: Vec<String>) -> Value {
    json!({
        "valid": issues.is_empty(),
        "issues": issues,
        "summary": config_summary(config),
    })
}

fn config_summary(config: &CodeConfig) -> Value {
    json!({
        "defaultModel": config.default_model,
        "providers": config.providers.len(),
        "models": config.providers.iter().map(|provider| provider.models.len()).sum::<usize>(),
        "mcpServers": config.mcp_servers.len(),
    })
}

fn model_catalog_from_entries(
    config: &CodeConfig,
    entries: &[ModelEntry],
    mut warnings: Vec<String>,
) -> Value {
    let items = entries
        .iter()
        .map(|entry| {
            let id = entry.route.to_string();
            let source = match entry.route.source {
                ModelSource::Config => entry
                    .route
                    .model
                    .split_once('/')
                    .map(|(provider, _)| provider)
                    .unwrap_or("config"),
                source => source.label(),
            };
            json!({
                "id": id,
                "name": entry.display_name,
                "source": source,
                "contextWindow": entry.context_window,
                "reasoning": entry.reasoning,
                "toolCall": entry.tool_call,
            })
        })
        .collect::<Vec<_>>();
    let default_model = config.default_model.clone();
    if items.is_empty() {
        warnings
            .push("No models are configured. Add a provider and model in Settings.".to_string());
    }
    if let Some(default_model) = default_model.as_deref() {
        let default_exists = items
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some(default_model));
        if !default_exists {
            warnings.push(format!(
                "The default model `{default_model}` is not present in the configured model catalog."
            ));
        }
    }

    json!({
        "items": items,
        "warnings": warnings,
        "defaultModel": default_model,
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

fn has_secret(value: Option<&str>) -> bool {
    value.is_some_and(|secret| !secret.trim().is_empty())
}

fn split_model_ref(value: &str) -> Option<(String, String)> {
    let (provider, model) = value.split_once('/')?;
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() || model.is_empty() {
        None
    } else {
        Some((provider.to_string(), model.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::model::route::ModelRoute;

    #[test]
    fn model_catalog_uses_qualified_ids_and_provider_sources() {
        let config: CodeConfig = serde_json::from_value(serde_json::json!({
            "defaultModel": "openai/gpt-test",
            "providers": [{
                "name": "openai",
                "models": [{
                    "id": "gpt-test",
                    "name": "GPT Test",
                    "reasoning": true,
                    "toolCall": true,
                    "limit": { "context": 128000 }
                }]
            }]
        }))
        .expect("valid config");

        let entries = ModelCatalog::configured(&config).entries;
        let catalog = model_catalog_from_entries(&config, &entries, Vec::new());
        assert_eq!(catalog["defaultModel"], "openai/gpt-test");
        assert_eq!(catalog["items"][0]["id"], "openai/gpt-test");
        assert_eq!(catalog["items"][0]["source"], "openai");
        assert_eq!(catalog["items"][0]["contextWindow"], 128000);
        assert_eq!(catalog["items"][0]["reasoning"], true);
        assert_eq!(catalog["warnings"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn model_catalog_merges_signed_in_account_models() {
        let config: CodeConfig = serde_json::from_value(serde_json::json!({
            "defaultModel": "openai/gpt-test",
            "providers": [{
                "name": "openai",
                "models": [{ "id": "gpt-test", "name": "GPT Test" }]
            }]
        }))
        .expect("valid config");
        let mut entries = ModelCatalog::configured(&config).entries;
        entries.extend([
            ModelEntry {
                route: ModelRoute::new(ModelSource::Codex, "gpt-5.6-sol").unwrap(),
                display_name: "gpt-5.6-sol".to_string(),
                context_window: Some(200_000),
                reasoning: true,
                tool_call: true,
            },
            ModelEntry {
                route: ModelRoute::new(ModelSource::CodeBuddy, "glm-5.1").unwrap(),
                display_name: "glm-5.1".to_string(),
                context_window: None,
                reasoning: true,
                tool_call: true,
            },
        ]);

        let catalog = model_catalog_from_entries(&config, &entries, Vec::new());

        assert_eq!(catalog["defaultModel"], "openai/gpt-test");
        assert_eq!(catalog["items"].as_array().map(Vec::len), Some(3));
        assert_eq!(catalog["items"][1]["id"], "codex/gpt-5.6-sol");
        assert_eq!(catalog["items"][1]["source"], "Codex");
        assert_eq!(catalog["items"][1]["contextWindow"], 200_000);
        assert_eq!(catalog["items"][1]["reasoning"], true);
        assert_eq!(catalog["items"][1]["toolCall"], true);
        assert_eq!(catalog["items"][2]["id"], "workbuddy/glm-5.1");
        assert_eq!(catalog["items"][2]["source"], "WorkBuddy");
    }

    #[test]
    fn category_update_persists_selected_section_and_preserves_unknown_acl() {
        let directory = temp_directory("category-update");
        let path = directory.join("config.acl");
        let source = r#"# keep this comment
default_model = "openai/model-a"

providers "openai" {
  models "model-a" { name = "Model A" }
  models "model-b" { name = "Model B" }
}

future_feature { enabled = true }
"#;
        fs::write(&path, source).expect("write initial config");
        let current = CodeConfig::from_acl(source).expect("parse initial config");

        let updated = update_category_document(
            &path,
            &current,
            SettingsCategory::Llm,
            json!({ "defaultModel": "openai/model-b" }),
        )
        .expect("persist category");

        let persisted = fs::read_to_string(&path).expect("read persisted config");
        assert_eq!(updated.default_model.as_deref(), Some("openai/model-b"));
        assert!(persisted.contains("# keep this comment"));
        assert!(persisted.contains("future_feature { enabled = true }"));
        assert!(persisted.contains("default_model = \"openai/model-b\""));
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn validation_failure_never_mutates_memory_or_disk() {
        let directory = temp_directory("validation-failure");
        let path = directory.join("config.acl");
        let source = r#"default_model = "openai/model-a"
providers "openai" { models "model-a" { name = "Model A" } }
"#;
        fs::write(&path, source).expect("write initial config");
        let current = CodeConfig::from_acl(source).expect("parse initial config");

        let error = update_category_document(
            &path,
            &current,
            SettingsCategory::Llm,
            json!({ "defaultModel": "missing/model" }),
        )
        .expect_err("invalid model must fail");

        assert!(error.to_string().contains("default model"));
        assert_eq!(current.default_model.as_deref(), Some("openai/model-a"));
        assert_eq!(fs::read_to_string(&path).expect("unchanged config"), source);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn write_failure_never_mutates_memory() {
        let directory = temp_directory("write-failure");
        let path = directory.join("config.acl");
        fs::create_dir(&path).expect("make config path a directory");
        let source = r#"default_model = "openai/model-a"
providers "openai" {
  models "model-a" { name = "Model A" }
  models "model-b" { name = "Model B" }
}
"#;
        let current = CodeConfig::from_acl(source).expect("parse initial config");

        update_category_document(
            &path,
            &current,
            SettingsCategory::Llm,
            json!({ "defaultModel": "openai/model-b" }),
        )
        .expect_err("directory config path must fail");

        assert_eq!(current.default_model.as_deref(), Some("openai/model-a"));
        fs::remove_dir_all(directory).expect("cleanup");
    }

    fn temp_directory(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "a3s-code-web-config-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create temp directory");
        directory
    }
}
