use crate::user_paths::user_home_dir;

pub(crate) fn canonical_model_name(model: &str) -> String {
    let trimmed = model.trim();
    if !trimmed.starts_with("claude") {
        return trimmed.to_string();
    }
    let Some(open_bracket) = trimmed.rfind('[') else {
        return trimmed.to_string();
    };
    if trimmed.ends_with(']') {
        let stripped = trimmed[..open_bracket].trim_end();
        if !stripped.is_empty() {
            return stripped.to_string();
        }
    }
    trimmed.to_string()
}

/// Claude models observed in the local Claude Code configuration and usage
/// cache. Project selections are kept ahead of historical usage entries.
pub(crate) fn models() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(home) = user_home_dir() {
        for path in [
            home.join(".claude.json"),
            home.join(".claude").join("stats-cache.json"),
        ] {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    collect_models(&value, &mut out);
                }
            }
        }
    }
    if out.is_empty() {
        out.push("claude-sonnet-4".to_string());
    }
    out
}

fn collect_models(value: &serde_json::Value, out: &mut Vec<String>) {
    fn push_model(out: &mut Vec<String>, model: &str) {
        let model = canonical_model_name(model);
        if model.starts_with("claude") && !out.iter().any(|item| item == &model) {
            out.push(model);
        }
    }

    fn walk_model_values(
        value: &serde_json::Value,
        parent_key: Option<&str>,
        out: &mut Vec<String>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    if key == "model" {
                        if let Some(model) = child.as_str() {
                            push_model(out, model);
                        }
                    }
                    walk_model_values(child, Some(key), out);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    walk_model_values(child, parent_key, out);
                }
            }
            serde_json::Value::String(model) if parent_key == Some("model") => {
                push_model(out, model);
            }
            _ => {}
        }
    }

    fn walk_usage_maps(
        value: &serde_json::Value,
        parent_key: Option<&str>,
        target_key: &str,
        out: &mut Vec<String>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                if parent_key == Some(target_key) {
                    for key in map.keys() {
                        push_model(out, key);
                    }
                }
                for (key, child) in map {
                    walk_usage_maps(child, Some(key), target_key, out);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    walk_usage_maps(child, parent_key, target_key, out);
                }
            }
            _ => {}
        }
    }

    walk_model_values(value, None, out);
    walk_usage_maps(value, None, "lastModelUsage", out);
    walk_usage_maps(value, None, "tokensByModel", out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_model_name_strips_claude_code_context_suffix() {
        assert_eq!(
            canonical_model_name(" claude-opus-4-8[1m] "),
            "claude-opus-4-8"
        );
        assert_eq!(
            canonical_model_name("claude-opus-4-8 [1m]"),
            "claude-opus-4-8"
        );
        assert_eq!(
            canonical_model_name("claude-sonnet-4-6"),
            "claude-sonnet-4-6"
        );
        assert_eq!(
            canonical_model_name("openai/gpt-5[preview]"),
            "openai/gpt-5[preview]"
        );
    }

    #[test]
    fn collects_models_from_model_values_and_usage_maps() {
        let value = json!({
            "projects": {
                "/workspace": {
                    "model": "claude-sonnet-4-6",
                    "lastModelUsage": {
                        "claude-opus-4-8[1m]": {"inputTokens": 10},
                        "claude-opus-4-8": {"inputTokens": 5},
                        "not-claude": {"inputTokens": 1}
                    }
                }
            },
            "dailyModelTokens": [{
                "tokensByModel": {
                    "claude-haiku-4-5-20251001": 5,
                    "MiniMax-M2.7-highspeed": 5
                }
            }]
        });
        let mut models = Vec::new();

        collect_models(&value, &mut models);

        assert_eq!(
            models,
            vec![
                "claude-sonnet-4-6",
                "claude-opus-4-8",
                "claude-haiku-4-5-20251001"
            ]
        );
    }
}
