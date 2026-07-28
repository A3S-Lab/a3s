use std::collections::HashSet;

use a3s_code_core::config::StorageBackend;
use a3s_code_core::mcp::McpTransportConfig;
use a3s_code_core::CodeConfig;

pub(crate) fn validate_config(config: &CodeConfig) -> Vec<String> {
    let mut issues = Vec::new();
    validate_models(config, &mut issues);
    validate_execution(config, &mut issues);
    validate_storage(config, &mut issues);
    validate_memory(config, &mut issues);
    validate_queue(config, &mut issues);
    validate_integrations(config, &mut issues);
    issues
}

fn validate_models(config: &CodeConfig, issues: &mut Vec<String>) {
    let mut provider_names = HashSet::new();
    for provider in &config.providers {
        let name = provider.name.trim();
        if name.is_empty() {
            issues.push("providers[].name must not be empty".to_string());
            continue;
        }
        if name.contains('/') {
            issues.push(format!("provider `{name}` must not contain `/`"));
        }
        if !provider_names.insert(name.to_string()) {
            issues.push(format!("provider `{name}` is configured more than once"));
        }
        let mut model_ids = HashSet::new();
        for model in &provider.models {
            let id = model.id.trim();
            if id.is_empty() {
                issues.push(format!(
                    "provider `{name}` contains a model with an empty id"
                ));
            } else if !model_ids.insert(id.to_string()) {
                issues.push(format!("model `{name}/{id}` is configured more than once"));
            }
            if !model.cost.input.is_finite()
                || !model.cost.output.is_finite()
                || !model.cost.cache_read.is_finite()
                || !model.cost.cache_write.is_finite()
                || model.cost.input < 0.0
                || model.cost.output < 0.0
                || model.cost.cache_read < 0.0
                || model.cost.cache_write < 0.0
            {
                issues.push(format!(
                    "model `{name}/{id}` costs must be finite non-negative numbers"
                ));
            }
        }
    }

    if let Some(default_model) = config.default_model.as_deref() {
        let Some((provider_name, model_id)) = default_model.split_once('/') else {
            issues.push("defaultModel must use the `provider/model` format".to_string());
            return;
        };
        let exists = config.providers.iter().any(|provider| {
            provider.name == provider_name
                && provider.models.iter().any(|model| model.id == model_id)
        });
        if !exists {
            issues.push(format!(
                "default model `{default_model}` is not present in providers"
            ));
        }
    }

    if config.thinking_budget == Some(0) {
        issues.push("thinkingBudget must be greater than zero when configured".to_string());
    }
    if config.llm_api_timeout_ms.is_some_and(|value| value < 100) {
        issues.push("llmApiTimeoutMs must be at least 100 milliseconds".to_string());
    }
}

fn validate_execution(config: &CodeConfig, issues: &mut Vec<String>) {
    if config.max_tool_rounds.is_some_and(|value| value == 0) {
        issues.push("maxToolRounds must be greater than zero".to_string());
    }
    if config.max_parallel_tasks.is_some_and(|value| value == 0) {
        issues.push("maxParallelTasks must be greater than zero".to_string());
    }
    if !config.auto_delegation.min_confidence.is_finite()
        || !(0.0..=1.0).contains(&config.auto_delegation.min_confidence)
    {
        issues.push("autoDelegation.minConfidence must be between 0 and 1".to_string());
    }
    if config.auto_delegation.max_tasks == 0 {
        issues.push("autoDelegation.maxTasks must be greater than zero".to_string());
    }
}

fn validate_storage(config: &CodeConfig, issues: &mut Vec<String>) {
    if config.storage_backend == StorageBackend::Custom
        && config
            .storage_url
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        issues.push("storageUrl is required when storageBackend is `custom`".to_string());
    }
}

fn validate_memory(config: &CodeConfig, issues: &mut Vec<String>) {
    let Some(memory) = config.memory.as_ref() else {
        return;
    };
    if memory.max_short_term == 0 {
        issues.push("memory.maxShortTerm must be greater than zero".to_string());
    }
    if memory.max_working == 0 {
        issues.push("memory.maxWorking must be greater than zero".to_string());
    }
    if memory.prune_interval_secs == 0 {
        issues.push("memory.pruneIntervalSecs must be greater than zero".to_string());
    }
    if !memory.relevance.decay_days.is_finite() || memory.relevance.decay_days <= 0.0 {
        issues.push("memory.relevance.decayDays must be greater than zero".to_string());
    }
    for (name, value) in [
        ("importanceWeight", memory.relevance.importance_weight),
        ("recencyWeight", memory.relevance.recency_weight),
    ] {
        if !value.is_finite() || value < 0.0 {
            issues.push(format!(
                "memory.relevance.{name} must be a finite non-negative number"
            ));
        }
    }
    if let Some(policy) = memory.prune_policy.as_ref() {
        if !policy.min_importance_to_keep.is_finite()
            || !(0.0..=1.0).contains(&policy.min_importance_to_keep)
        {
            issues
                .push("memory.prunePolicy.minImportanceToKeep must be between 0 and 1".to_string());
        }
    }
}

fn validate_queue(config: &CodeConfig, issues: &mut Vec<String>) {
    let Some(queue) = config.queue.as_ref() else {
        return;
    };
    for (name, value) in [
        ("controlMaxConcurrency", queue.control_max_concurrency),
        ("queryMaxConcurrency", queue.query_max_concurrency),
        ("executeMaxConcurrency", queue.execute_max_concurrency),
        ("generateMaxConcurrency", queue.generate_max_concurrency),
    ] {
        if value == 0 {
            issues.push(format!("queue.{name} must be greater than zero"));
        }
    }
    if queue.default_timeout_ms == Some(0) {
        issues.push("queue.defaultTimeoutMs must be greater than zero".to_string());
    }
    if queue.lane_timeouts.values().any(|timeout| *timeout == 0) {
        issues.push("queue.laneTimeouts values must be greater than zero".to_string());
    }
    if queue
        .lane_handlers
        .values()
        .any(|handler| handler.timeout_ms == 0)
    {
        issues.push("queue.laneHandlers timeoutMs values must be greater than zero".to_string());
    }
    if let Some(policy) = queue.retry_policy.as_ref() {
        if !matches!(policy.strategy.as_str(), "exponential" | "fixed" | "none") {
            issues.push("queue.retryPolicy.strategy is invalid".to_string());
        }
        if policy.strategy == "fixed" && policy.fixed_delay_ms.is_none() {
            issues.push(
                "queue.retryPolicy.fixedDelayMs is required for the `fixed` strategy".to_string(),
            );
        }
    }
    if let Some(rate_limit) = queue.rate_limit.as_ref() {
        if !matches!(
            rate_limit.limit_type.as_str(),
            "per_second" | "per_minute" | "per_hour" | "unlimited"
        ) {
            issues.push("queue.rateLimit.limitType is invalid".to_string());
        }
        if rate_limit.limit_type != "unlimited" && rate_limit.max_operations.is_none() {
            issues.push(
                "queue.rateLimit.maxOperations is required unless the limit is unlimited"
                    .to_string(),
            );
        }
    }
    if let Some(priority) = queue.priority_boost.as_ref() {
        if !matches!(
            priority.strategy.as_str(),
            "standard" | "aggressive" | "disabled"
        ) {
            issues.push("queue.priorityBoost.strategy is invalid".to_string());
        }
    }
}

fn validate_integrations(config: &CodeConfig, issues: &mut Vec<String>) {
    if let Some(os) = config.os.as_ref() {
        validate_http_url("os.address", &os.address, issues);
    }
    if let Some(search) = config.search.as_ref() {
        if search.timeout == 0 {
            issues.push("search.timeout must be greater than zero".to_string());
        }
        if search
            .engines
            .iter()
            .any(|(_, engine)| !engine.weight.is_finite() || engine.weight < 0.0)
        {
            issues.push("search.engine weights must be finite non-negative numbers".to_string());
        }
        if search
            .headless
            .as_ref()
            .is_some_and(|value| value.max_tabs == 0)
        {
            issues.push("search.headless.maxTabs must be greater than zero".to_string());
        }
    }
    if let Some(parser) = config.document_parser.as_ref() {
        if !(1..=1024).contains(&parser.max_file_size_mb) {
            issues.push("documentParser.maxFileSizeMb must be between 1 and 1024".to_string());
        }
    }

    let mut names = HashSet::new();
    for server in &config.mcp_servers {
        let name = server.name.trim();
        if name.is_empty() {
            issues.push("mcpServers[].name must not be empty".to_string());
        } else if !names.insert(name.to_string()) {
            issues.push(format!("MCP server `{name}` is configured more than once"));
        }
        if server.tool_timeout_secs == 0 {
            issues.push(format!(
                "MCP server `{name}` toolTimeoutSecs must be greater than zero"
            ));
        }
        match &server.transport {
            McpTransportConfig::Stdio { command, .. } if command.trim().is_empty() => {
                issues.push(format!("MCP server `{name}` command must not be empty"));
            }
            McpTransportConfig::Http { url, .. }
            | McpTransportConfig::StreamableHttp { url, .. } => {
                validate_http_url(&format!("MCP server `{name}` URL"), url, issues);
            }
            _ => {}
        }
        if let Some(oauth) = server.oauth.as_ref() {
            validate_http_url(
                &format!("MCP server `{name}` OAuth authUrl"),
                &oauth.auth_url,
                issues,
            );
            validate_http_url(
                &format!("MCP server `{name}` OAuth tokenUrl"),
                &oauth.token_url,
                issues,
            );
            if oauth.client_id.trim().is_empty() {
                issues.push(format!(
                    "MCP server `{name}` OAuth clientId must not be empty"
                ));
            }
        }
    }
}

fn validate_http_url(field: &str, value: &str, issues: &mut Vec<String>) {
    let value = value.trim();
    if !(value.starts_with("http://") || value.starts_with("https://")) {
        issues.push(format!("{field} must be an http:// or https:// URL"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_reports_cross_field_model_and_storage_errors() {
        let config = CodeConfig {
            default_model: Some("missing".to_string()),
            storage_backend: StorageBackend::Custom,
            max_parallel_tasks: Some(0),
            ..CodeConfig::default()
        };

        let issues = validate_config(&config).join("\n");
        assert!(issues.contains("provider/model"));
        assert!(issues.contains("storageUrl"));
        assert!(issues.contains("maxParallelTasks"));
    }
}
