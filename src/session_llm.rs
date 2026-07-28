use std::sync::Arc;

use a3s_code_core::llm::{create_client_with_config, LlmConfig};
use a3s_code_core::{CodeConfig, LlmClient, SessionOptions};

use crate::model::route::{ModelRoute, ModelSource};

pub(crate) fn resolve_config_llm_client(
    code_config: &CodeConfig,
    options: &SessionOptions,
    session_id: &str,
) -> Result<Arc<dyn LlmClient>, String> {
    let model_ref = options
        .model
        .as_deref()
        .or(code_config.default_model.as_deref())
        .ok_or_else(|| "default_model must be set in 'provider/model' format".to_string())?;
    let route = model_ref
        .parse::<ModelRoute>()
        .map_err(|error| format!("invalid model route `{model_ref}`: {error}"))?;
    match route.source {
        ModelSource::Config => {
            if route.model == model_ref {
                prepare_config_llm_config(code_config, options, session_id)
                    .map(create_client_with_config)
            } else {
                let mut routed = options.clone();
                routed.model = Some(route.model);
                prepare_config_llm_config(code_config, &routed, session_id)
                    .map(create_client_with_config)
            }
        }
        ModelSource::Claude | ModelSource::Codex | ModelSource::Kimi | ModelSource::CodeBuddy => {
            let provider = route
                .source
                .account_provider()
                .ok_or_else(|| format!("{} is not an account provider", route.source.label()))?;
            provider
                .client(&route.model, session_id)
                .map_err(|error| format!("{} account unavailable: {error}", provider.label()))
        }
        ModelSource::OsGateway => {
            Err("A3S OS gateway routes require a signed-in interactive session".to_string())
        }
    }
}

pub(crate) fn resolve_session_llm_client(
    code_config: &CodeConfig,
    options: &SessionOptions,
    session_id: &str,
) -> Result<Arc<dyn LlmClient>, String> {
    match options.llm_client.as_ref() {
        Some(client) => Ok(Arc::clone(client)),
        None => resolve_config_llm_client(code_config, options, session_id),
    }
}

fn prepare_config_llm_config(
    code_config: &CodeConfig,
    options: &SessionOptions,
    session_id: &str,
) -> Result<LlmConfig, String> {
    let model_ref = options
        .model
        .as_deref()
        .or(code_config.default_model.as_deref())
        .ok_or_else(|| "default_model must be set in 'provider/model' format".to_string())?;
    let (provider_name, model_id) = model_ref
        .split_once('/')
        .ok_or_else(|| "model format must be 'provider/model'".to_string())?;
    let mut config = code_config
        .llm_config(provider_name, model_id)
        .ok_or_else(|| {
            format!("provider '{provider_name}' or model '{model_id}' not found in config")
        })?;

    if options.model.is_some() {
        if let Some(temperature) = options.temperature {
            config = config.with_temperature(temperature);
        }
        if let Some(thinking_budget) = options.thinking_budget {
            config = config.with_thinking_budget(thinking_budget);
        }
    }
    if let Some(timeout_ms) = options.llm_api_timeout_ms {
        config = config.with_api_timeout(timeout_ms);
    }
    if let Some(enabled) = options
        .llm_logprobs
        .or_else(|| env_bool("A3S_CODE_LLM_LOGPROBS"))
        .or_else(|| env_bool("A3S_CODE_OPENAI_LOGPROBS"))
    {
        config = config.with_logprobs(enabled);
    }
    if let Some(top_logprobs) = options
        .llm_top_logprobs
        .or_else(|| env_usize("A3S_CODE_LLM_TOP_LOGPROBS"))
        .or_else(|| env_usize("A3S_CODE_OPENAI_TOP_LOGPROBS"))
    {
        config = config.with_top_logprobs(top_logprobs);
    }

    Ok(config.with_session_id(session_id))
}

fn env_bool(name: &str) -> Option<bool> {
    let value = std::env::var(name).ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use a3s_code_core::llm::ToolDefinition;
    use a3s_code_core::{CodeConfig, LlmClient, LlmResponse, Message, SessionOptions};
    use async_trait::async_trait;

    use super::{prepare_config_llm_config, resolve_session_llm_client};

    struct OverrideClient;

    #[async_trait]
    impl LlmClient for OverrideClient {
        async fn complete(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
        ) -> anyhow::Result<LlmResponse> {
            unreachable!("client identity test does not send requests")
        }

        async fn complete_streaming(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
            _cancel_token: tokio_util::sync::CancellationToken,
        ) -> anyhow::Result<tokio::sync::mpsc::Receiver<a3s_code_core::llm::StreamEvent>> {
            unreachable!("client identity test does not send requests")
        }
    }

    fn test_config() -> CodeConfig {
        CodeConfig::from_acl(
            r#"
                default_model = "openai/default-model"
                llm_api_timeout_ms = 1200

                providers "openai" {
                  apiKey = "sk-test"
                  baseUrl = "https://example.com/v1"
                  sessionIdHeader = "x-session-id"

                  models "default-model" {}
                  models "selected-model" {}
                  models "text-only" { toolCall = false }
                }
            "#,
        )
        .expect("test config")
    }

    #[test]
    fn prepares_selected_model_with_session_overrides() {
        let mut options = SessionOptions::new().with_model("openai/selected-model");
        options.temperature = Some(0.25);
        options.thinking_budget = Some(4096);
        options.llm_api_timeout_ms = Some(2400);
        options.llm_logprobs = Some(true);
        options.llm_top_logprobs = Some(3);

        let resolved = prepare_config_llm_config(&test_config(), &options, "session-42")
            .expect("resolve selected model");

        assert_eq!(resolved.provider, "openai");
        assert_eq!(resolved.model, "selected-model");
        assert_eq!(resolved.session_id.as_deref(), Some("session-42"));
        assert_eq!(resolved.temperature, Some(0.25));
        assert_eq!(resolved.thinking_budget, Some(4096));
        assert_eq!(resolved.api_timeout_ms, Some(2400));
        assert_eq!(resolved.logprobs, Some(true));
        assert_eq!(resolved.top_logprobs, Some(3));
        assert_eq!(resolved.native_structured_support, None);
    }

    #[test]
    fn prepares_default_model_when_session_has_no_override() {
        let resolved =
            prepare_config_llm_config(&test_config(), &SessionOptions::new(), "session-default")
                .expect("resolve default model");

        assert_eq!(resolved.provider, "openai");
        assert_eq!(resolved.model, "default-model");
        assert_eq!(resolved.session_id.as_deref(), Some("session-default"));
        assert_eq!(resolved.api_timeout_ms, Some(1200));
    }

    #[test]
    fn rejects_unknown_model_reference() {
        let options = SessionOptions::new().with_model("openai/missing");

        let error = prepare_config_llm_config(&test_config(), &options, "session-unknown")
            .expect_err("unknown model should fail");

        assert!(error.contains("openai"));
        assert!(error.contains("missing"));
    }

    #[test]
    fn custom_openai_text_only_model_keeps_prompt_structured_fallback() {
        let options = SessionOptions::new().with_model("openai/text-only");
        let resolved = prepare_config_llm_config(&test_config(), &options, "session-text")
            .expect("resolve text-only model");

        assert_eq!(resolved.native_structured_support, None);
    }

    #[test]
    fn session_override_is_retained_by_identity() {
        let override_client: Arc<dyn LlmClient> = Arc::new(OverrideClient);
        let options = SessionOptions::new().with_llm_client(Arc::clone(&override_client));

        let resolved = resolve_session_llm_client(&test_config(), &options, "session-override")
            .expect("resolve override client");

        assert!(Arc::ptr_eq(&override_client, &resolved));
    }
}
