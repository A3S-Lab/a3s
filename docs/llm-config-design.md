# A3S Code LLM Configuration Design

## Overview

This document defines the LLM configuration format for A3S Code, supporting multiple providers, models, and flexible API key/base URL management.

## Design Goals

1. **Provider Flexibility** - Support multiple LLM providers (Anthropic, OpenAI, DeepSeek, Groq, etc.)
2. **Per-Model Configuration** - Different models can have different API keys and base URLs
3. **Environment Variable Support** - Secure secret management via environment variables
4. **Default Settings** - Sensible defaults with easy overrides
5. **Backward Compatibility** - Works with existing ModelConfig proto

## Configuration File Format

### Location

The LLM configuration can be stored in:
- `~/.a3s/llm-config.json` (global default)
- Project-specific path (passed via SDK or CLI)
- Inline configuration (passed via gRPC Configure RPC)

### JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "defaultProvider": {
      "type": "string",
      "description": "Default provider to use if not specified"
    },
    "defaultModel": {
      "type": "string",
      "description": "Default model to use if not specified (format: provider/model or just model)"
    },
    "providers": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "Provider identifier (e.g., 'anthropic', 'openai', 'deepseek')"
          },
          "type": {
            "type": "string",
            "enum": ["anthropic", "openai", "openai-compatible"],
            "description": "Provider API type"
          },
          "apiKey": {
            "type": "string",
            "description": "Default API key for this provider (supports ${env:VAR_NAME})"
          },
          "baseUrl": {
            "type": "string",
            "description": "Default base URL for this provider"
          },
          "models": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "name": {
                  "type": "string",
                  "description": "Model identifier"
                },
                "displayName": {
                  "type": "string",
                  "description": "Human-readable model name"
                },
                "apiKey": {
                  "type": "string",
                  "description": "Model-specific API key (overrides provider default)"
                },
                "baseUrl": {
                  "type": "string",
                  "description": "Model-specific base URL (overrides provider default)"
                },
                "contextWindow": {
                  "type": "integer",
                  "description": "Maximum context window size in tokens"
                },
                "maxOutputTokens": {
                  "type": "integer",
                  "description": "Maximum output tokens"
                },
                "supportsStreaming": {
                  "type": "boolean",
                  "default": true
                },
                "supportsToolCalling": {
                  "type": "boolean",
                  "default": true
                },
                "supportsStructuredOutput": {
                  "type": "boolean",
                  "default": false
                }
              },
              "required": ["name"]
            }
          }
        },
        "required": ["name", "type"]
      }
    }
  },
  "required": ["providers"]
}
```

## Example Configurations

### Example 1: Basic Configuration

```json
{
  "defaultProvider": "anthropic",
  "defaultModel": "claude-sonnet-4-20250514",
  "providers": [
    {
      "name": "anthropic",
      "type": "anthropic",
      "apiKey": "${env:ANTHROPIC_API_KEY}",
      "baseUrl": "https://api.anthropic.com",
      "models": [
        {
          "name": "claude-sonnet-4-20250514",
          "displayName": "Claude Sonnet 4",
          "contextWindow": 200000,
          "maxOutputTokens": 8192,
          "supportsStreaming": true,
          "supportsToolCalling": true,
          "supportsStructuredOutput": true
        },
        {
          "name": "claude-3-5-sonnet-20241022",
          "displayName": "Claude 3.5 Sonnet",
          "contextWindow": 200000,
          "maxOutputTokens": 8192
        }
      ]
    },
    {
      "name": "openai",
      "type": "openai",
      "apiKey": "${env:OPENAI_API_KEY}",
      "baseUrl": "https://api.openai.com/v1",
      "models": [
        {
          "name": "gpt-4o",
          "displayName": "GPT-4o",
          "contextWindow": 128000,
          "maxOutputTokens": 16384
        },
        {
          "name": "gpt-4-turbo",
          "displayName": "GPT-4 Turbo",
          "contextWindow": 128000,
          "maxOutputTokens": 4096
        }
      ]
    }
  ]
}
```

### Example 2: Multiple Providers with Per-Model API Keys

```json
{
  "defaultProvider": "kimi",
  "defaultModel": "kimi-k2-5",
  "providers": [
    {
      "name": "anthropic",
      "type": "anthropic",
      "apiKey": "${env:ANTHROPIC_API_KEY}",
      "models": [
        {
          "name": "claude-sonnet-4-20250514",
          "displayName": "Claude Sonnet 4"
        }
      ]
    },
    {
      "name": "kimi",
      "type": "openai-compatible",
      "baseUrl": "http://35.220.164.252:3888/v1",
      "models": [
        {
          "name": "kimi-k2-5",
          "displayName": "Kimi K2.5",
          "apiKey": "${env:KIMI_API_KEY}",
          "contextWindow": 128000
        },
        {
          "name": "kimi-k1-5",
          "displayName": "Kimi K1.5",
          "apiKey": "${env:KIMI_API_KEY}",
          "contextWindow": 128000
        }
      ]
    },
    {
      "name": "deepseek",
      "type": "openai-compatible",
      "baseUrl": "https://api.deepseek.com/v1",
      "models": [
        {
          "name": "deepseek-chat",
          "displayName": "DeepSeek Chat",
          "apiKey": "${env:DEEPSEEK_API_KEY}",
          "contextWindow": 64000
        },
        {
          "name": "deepseek-coder",
          "displayName": "DeepSeek Coder",
          "apiKey": "${env:DEEPSEEK_API_KEY}",
          "contextWindow": 64000
        }
      ]
    },
    {
      "name": "groq",
      "type": "openai-compatible",
      "apiKey": "${env:GROQ_API_KEY}",
      "baseUrl": "https://api.groq.com/openai/v1",
      "models": [
        {
          "name": "llama-3.1-70b-versatile",
          "displayName": "Llama 3.1 70B"
        }
      ]
    }
  ]
}
```

### Example 3: Development Setup with Multiple API Keys

```json
{
  "defaultProvider": "anthropic",
  "defaultModel": "claude-sonnet-4-20250514",
  "providers": [
    {
      "name": "anthropic",
      "type": "anthropic",
      "models": [
        {
          "name": "claude-sonnet-4-20250514",
          "displayName": "Claude Sonnet 4 (Personal)",
          "apiKey": "${env:ANTHROPIC_API_KEY_PERSONAL}",
          "baseUrl": "https://api.anthropic.com"
        },
        {
          "name": "claude-sonnet-4-work",
          "displayName": "Claude Sonnet 4 (Work)",
          "apiKey": "${env:ANTHROPIC_API_KEY_WORK}",
          "baseUrl": "https://api.anthropic.com"
        }
      ]
    },
    {
      "name": "ollama",
      "type": "openai-compatible",
      "baseUrl": "http://localhost:11434/v1",
      "apiKey": "ollama",
      "models": [
        {
          "name": "llama3.2",
          "displayName": "Llama 3.2 (Local)"
        }
      ]
    }
  ]
}
```

## Configuration Resolution

### Priority Order

When resolving configuration for a model, the system follows this priority:

1. **Inline Configuration** (via gRPC Configure RPC) - Highest priority
2. **Model-specific settings** in config file
3. **Provider default settings** in config file
4. **System defaults** - Lowest priority

### Environment Variable Substitution

The configuration supports environment variable substitution using the `${env:VAR_NAME}` syntax:

```json
{
  "apiKey": "${env:ANTHROPIC_API_KEY}"
}
```

At runtime, this will be replaced with the value of the `ANTHROPIC_API_KEY` environment variable.

**Security Note**: Environment variables are resolved at configuration load time, not stored in memory.

### Model Reference Formats

Models can be referenced in multiple ways:

1. **Full format**: `provider/model` (e.g., `anthropic/claude-sonnet-4-20250514`)
2. **Short format**: `model` (e.g., `claude-sonnet-4-20250514`) - uses defaultProvider
3. **Alias**: Custom model names defined in config

## Implementation

### Rust Types

```rust
// src/code/src/llm_config.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfigFile {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    Anthropic,
    Openai,
    #[serde(rename = "openai-compatible")]
    OpenaiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    pub name: String,
    pub display_name: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_streaming: Option<bool>,
    pub supports_tool_calling: Option<bool>,
    pub supports_structured_output: Option<bool>,
}

impl LlmConfigFile {
    /// Load configuration from file
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Self = serde_json::from_str(&content)?;
        config.resolve_env_vars()?;
        Ok(config)
    }

    /// Resolve environment variables in configuration
    fn resolve_env_vars(&mut self) -> Result<()> {
        for provider in &mut self.providers {
            if let Some(ref mut api_key) = provider.api_key {
                *api_key = resolve_env_var(api_key)?;
            }
            for model in &mut provider.models {
                if let Some(ref mut api_key) = model.api_key {
                    *api_key = resolve_env_var(api_key)?;
                }
            }
        }
        Ok(())
    }

    /// Get model configuration by reference
    pub fn get_model(&self, model_ref: &str) -> Result<ResolvedModelConfig> {
        let (provider_name, model_name) = parse_model_ref(model_ref, &self.default_provider)?;

        let provider = self.providers.iter()
            .find(|p| p.name == provider_name)
            .ok_or_else(|| anyhow!("Provider '{}' not found", provider_name))?;

        let model = provider.models.iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| anyhow!("Model '{}' not found in provider '{}'", model_name, provider_name))?;

        Ok(ResolvedModelConfig {
            provider: provider_name,
            model: model_name,
            api_key: model.api_key.clone()
                .or_else(|| provider.api_key.clone())
                .ok_or_else(|| anyhow!("No API key configured for model '{}'", model_ref))?,
            base_url: model.base_url.clone()
                .or_else(|| provider.base_url.clone()),
            provider_type: provider.provider_type.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedModelConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub provider_type: ProviderType,
}

/// Resolve environment variable reference
fn resolve_env_var(value: &str) -> Result<String> {
    if value.starts_with("${env:") && value.ends_with("}") {
        let var_name = &value[6..value.len()-1];
        std::env::var(var_name)
            .map_err(|_| anyhow!("Environment variable '{}' not found", var_name))
    } else {
        Ok(value.to_string())
    }
}

/// Parse model reference (e.g., "anthropic/claude-sonnet-4" or "claude-sonnet-4")
fn parse_model_ref(model_ref: &str, default_provider: &Option<String>) -> Result<(String, String)> {
    if let Some((provider, model)) = model_ref.split_once('/') {
        Ok((provider.to_string(), model.to_string()))
    } else {
        let provider = default_provider.as_ref()
            .ok_or_else(|| anyhow!("No default provider configured"))?;
        Ok((provider.clone(), model_ref.to_string()))
    }
}
```

### Integration with Existing Code

```rust
// src/code/src/service.rs

use crate::llm_config::{LlmConfigFile, ResolvedModelConfig};

impl AgentService {
    pub async fn configure(&self, request: ConfigureRequest) -> Result<ConfigureResponse> {
        let session_id = request.session_id;

        if let Some(model_config) = request.model {
            // Option 1: Direct ModelConfig from proto (existing behavior)
            let llm_config = LlmConfig {
                provider: model_config.provider,
                model: model_config.name,
                api_key: model_config.api_key.unwrap_or_default(),
                base_url: model_config.base_url,
            };

            // Update session with new LLM client
            self.session_manager.configure_model(session_id, llm_config).await?;
        }

        // Option 2: Load from config file (new feature)
        // This would be triggered by a new field in ConfigureRequest
        // e.g., model_ref: "anthropic/claude-sonnet-4"

        Ok(ConfigureResponse {})
    }
}
```

## SDK Integration

### TypeScript SDK

```typescript
// SDK usage with config file
const client = new A3sClient({
  llmConfigPath: './llm-config.json'
});

const sessionId = await client.createSession({
  system: "You are a helpful assistant.",
  model: "anthropic/claude-sonnet-4-20250514" // Uses config file
});

// Or inline configuration (existing behavior)
await client.configure(sessionId, {
  model: {
    provider: "anthropic",
    name: "claude-sonnet-4-20250514",
    apiKey: process.env.ANTHROPIC_API_KEY,
    baseUrl: "https://api.anthropic.com"
  }
});
```

### Python SDK

```python
# SDK usage with config file
client = A3sClient(llm_config_path="./llm-config.json")

session_id = await client.create_session(
    system="You are a helpful assistant.",
    model="anthropic/claude-sonnet-4-20250514"  # Uses config file
)

# Or inline configuration (existing behavior)
await client.configure(session_id, model={
    "provider": "anthropic",
    "name": "claude-sonnet-4-20250514",
    "api_key": os.getenv("ANTHROPIC_API_KEY"),
    "base_url": "https://api.anthropic.com"
})
```

## Migration Path

### Phase 1: Add Configuration File Support (Current)
- Implement `LlmConfigFile` types
- Add configuration file loading
- Support environment variable substitution
- Maintain backward compatibility with existing ModelConfig proto

### Phase 2: SDK Integration
- Add `llmConfigPath` option to SDK clients
- Add model reference support (e.g., "anthropic/claude-sonnet-4")
- Update examples and documentation

### Phase 3: Enhanced Features
- Model aliasing
- Configuration validation
- Hot reload support
- Configuration UI/CLI tools

## Security Considerations

1. **API Key Storage**: Never store API keys in plain text in config files. Always use environment variables.
2. **File Permissions**: Config files should have restricted permissions (0600)
3. **Environment Variables**: Use secure environment variable management (e.g., dotenv, secrets managers)
4. **Logging**: Never log API keys or sensitive configuration

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let config = LlmConfigFile::from_file("tests/fixtures/llm-config.json").unwrap();
        assert_eq!(config.default_provider, Some("anthropic".to_string()));
        assert_eq!(config.providers.len(), 2);
    }

    #[test]
    fn test_resolve_model() {
        let config = LlmConfigFile::from_file("tests/fixtures/llm-config.json").unwrap();
        let resolved = config.get_model("claude-sonnet-4-20250514").unwrap();
        assert_eq!(resolved.provider, "anthropic");
        assert_eq!(resolved.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_env_var_substitution() {
        std::env::set_var("TEST_API_KEY", "sk-test-123");
        let value = resolve_env_var("${env:TEST_API_KEY}").unwrap();
        assert_eq!(value, "sk-test-123");
    }
}
```

## References

- Current implementation: `src/code/src/llm.rs`
- Proto definition: `src/proto/code_agent.proto`
- Similar systems: Vercel AI SDK, LangChain model configuration

---

**Status**: 📋 Design Document
**Last Updated**: 2026-02-03
