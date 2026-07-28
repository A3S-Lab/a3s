use std::collections::HashMap;

use a3s_code_core::mcp::{McpServerConfig, McpTransportConfig};
use a3s_code_core::ProviderConfig;
use serde::Serialize;
use serde_json::{json, Value};

pub(super) const REDACTED_SECRET: &str = "[configured]";

pub(super) fn redact_secret(value: Option<&str>) -> Option<&'static str> {
    value
        .filter(|secret| !secret.trim().is_empty())
        .map(|_| REDACTED_SECRET)
}

pub(super) fn redact_headers(headers: &HashMap<String, String>) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(key, value)| {
            if is_sensitive_key(key) && !value.trim().is_empty() {
                (key.clone(), REDACTED_SECRET.to_string())
            } else {
                (key.clone(), value.clone())
            }
        })
        .collect()
}

pub(super) fn sanitized_json_value<T: Serialize>(value: &T) -> Value {
    let mut value = serde_json::to_value(value).unwrap_or_else(|_| json!({}));
    redact_secrets_in_value(&mut value);
    value
}

pub(super) fn preserve_provider_secrets(
    providers: &mut [ProviderConfig],
    existing: &[ProviderConfig],
) {
    for provider in providers {
        let Some(previous) = existing.iter().find(|item| item.name == provider.name) else {
            continue;
        };
        preserve_secret(&mut provider.api_key, previous.api_key.as_deref());
        preserve_map_placeholders(&mut provider.headers, &previous.headers);
        for model in &mut provider.models {
            let Some(previous_model) = previous.models.iter().find(|item| item.id == model.id)
            else {
                continue;
            };
            preserve_secret(&mut model.api_key, previous_model.api_key.as_deref());
            preserve_map_placeholders(&mut model.headers, &previous_model.headers);
        }
    }
}

pub(super) fn preserve_mcp_secrets(servers: &mut [McpServerConfig], existing: &[McpServerConfig]) {
    for server in servers {
        let Some(previous) = existing.iter().find(|item| item.name == server.name) else {
            continue;
        };
        preserve_map_placeholders(&mut server.env, &previous.env);
        preserve_mcp_transport_secrets(&mut server.transport, &previous.transport);
        if let (Some(oauth), Some(previous_oauth)) =
            (server.oauth.as_mut(), previous.oauth.as_ref())
        {
            preserve_secret(
                &mut oauth.client_secret,
                previous_oauth.client_secret.as_deref(),
            );
            preserve_secret(
                &mut oauth.access_token,
                previous_oauth.access_token.as_deref(),
            );
        }
    }
}

pub(super) fn preserve_secret(value: &mut Option<String>, previous: Option<&str>) {
    if value.as_deref().map(str::trim) == Some(REDACTED_SECRET) {
        *value = previous.map(str::to_string);
    }
}

fn preserve_mcp_transport_secrets(
    transport: &mut McpTransportConfig,
    previous: &McpTransportConfig,
) {
    match (transport, previous) {
        (
            McpTransportConfig::Http { headers, .. },
            McpTransportConfig::Http {
                headers: previous_headers,
                ..
            },
        )
        | (
            McpTransportConfig::StreamableHttp { headers, .. },
            McpTransportConfig::StreamableHttp {
                headers: previous_headers,
                ..
            },
        ) => preserve_map_placeholders(headers, previous_headers),
        _ => {}
    }
}

fn preserve_map_placeholders(
    values: &mut HashMap<String, String>,
    previous: &HashMap<String, String>,
) {
    for (key, value) in values {
        if value.trim() == REDACTED_SECRET {
            if let Some(previous) = previous.get(key) {
                *value = previous.clone();
            }
        }
    }
}

fn redact_secrets_in_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if is_sensitive_key(key)
                    && child.as_str().is_some_and(|text| !text.trim().is_empty())
                {
                    *child = Value::String(REDACTED_SECRET.to_string());
                } else {
                    redact_secrets_in_value(child);
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                redact_secrets_in_value(child);
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
    normalized.contains("apikey")
        || normalized.contains("authorization")
        || normalized.contains("accesstoken")
        || normalized.contains("refreshtoken")
        || normalized.contains("clientsecret")
        || normalized.contains("secretkey")
        || normalized == "token"
        || normalized == "password"
        || normalized.ends_with("token")
        || normalized.ends_with("secret")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_never_exposes_nested_tokens() {
        let value = json!({
            "apiKey": "secret",
            "headers": { "Authorization": "Bearer secret", "X-Tenant": "a3s" },
            "oauth": { "client_secret": "oauth-secret" },
        });

        let masked = sanitized_json_value(&value);
        let serialized = serde_json::to_string(&masked).expect("masked JSON");
        assert!(!serialized.contains("Bearer secret"));
        assert!(!serialized.contains("oauth-secret"));
        assert_eq!(masked["headers"]["X-Tenant"], "a3s");
    }
}
