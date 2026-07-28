//! Codex / ChatGPT-account LLM client.
//!
//! This provider reads the local Codex CLI login (`$CODEX_HOME/auth.json` or
//! `~/.codex/auth.json`) and talks to the ChatGPT Codex Responses backend. It
//! exists because the ChatGPT-account backend uses a different wire format from
//! OpenAI chat completions.

mod auth;
mod stream;
mod tls;
mod transport;

use crate::user_paths::user_home_dir;
use a3s_code_core::llm::{
    structured::{NativeStructuredSupport, StructuredDirective},
    ContentBlock, LlmClient, LlmResponse, Message, ModelGenerationConcurrency,
    NonRetryableLlmError, StreamEvent, ToolDefinition,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use self::auth::AuthState;
use self::transport::{NetworkWireClient, TransportController, TransportError, WireRequest};

const CODEX_BASE: &str = "https://chatgpt.com/backend-api/codex";
const ORIGINATOR: &str = "codex_cli_rs";
const CODEX_MODEL_REFRESH_TIMEOUT: Duration = Duration::from_secs(15);
const RESPONSES_LITE_HEADER: &str = "x-openai-internal-codex-responses-lite";
const MAX_PROMPT_CACHE_KEY_BYTES: usize = 64;
const FALLBACK_CODEX_MODEL: &str = "gpt-5.6-sol";
const CODEX_WIRE_REASONING_EFFORT_ORDER: &[&str] =
    &["none", "minimal", "low", "medium", "high", "xhigh", "max"];

pub(crate) fn codex_home() -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Some(expand_home_path(PathBuf::from(configured)));
    }
    user_home_dir().map(|home| home.join(".codex"))
}

pub(crate) fn codex_auth_path() -> Option<PathBuf> {
    codex_home().map(|home| home.join("auth.json"))
}

fn codex_models_cache_path() -> Option<PathBuf> {
    codex_home().map(|home| home.join("models_cache.json"))
}

fn expand_home_path(path: PathBuf) -> PathBuf {
    let Some(raw) = path.to_str() else {
        return path;
    };
    let Some(rest) = raw.strip_prefix("~/") else {
        return path;
    };
    user_home_dir().map(|home| home.join(rest)).unwrap_or(path)
}

pub(crate) fn native_reasoning_effort_for_a3s(a3s_effort: &str) -> Option<&'static str> {
    match a3s_effort.trim().to_ascii_lowercase().as_str() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "max" => Some("max"),
        // Native Codex treats Ultra as a product/orchestration tier and maps it
        // to the Responses wire effort `max`. A3S supplies the orchestration.
        "ultracode" => Some("max"),
        _ => None,
    }
}

/// Normalize catalog/product effort names to values accepted by the Responses
/// request. In particular, `ultra` is never a wire value; native Codex sends it
/// as `max` and enables its multi-agent behavior separately.
fn codex_wire_reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "none" => Some("none"),
        "minimal" => Some("minimal"),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("xhigh"),
        "max" | "ultra" => Some("max"),
        _ => None,
    }
}

fn codex_usage_limit_error(status: u16, body: &str) -> Option<NonRetryableLlmError> {
    codex_usage_limit_error_at(status, body, chrono::Utc::now().timestamp())
}

fn codex_usage_limit_error_at(status: u16, body: &str, now: i64) -> Option<NonRetryableLlmError> {
    if status != 429 {
        return None;
    }

    let payload: Value = serde_json::from_str(body).ok()?;
    let error = payload.get("error")?;
    let is_usage_limit = [error.get("type"), error.get("code")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|kind| kind == "usage_limit_reached");
    if !is_usage_limit {
        return None;
    }

    let plan = codex_plan_label(error.get("plan_type"));
    let plan_suffix = plan
        .as_deref()
        .map(|plan| format!(" ({plan} plan)"))
        .unwrap_or_default();
    let mut message = format!("Codex usage limit reached{plan_suffix}.");

    let reset_in_seconds =
        codex_json_u64(error.get("resets_in_seconds")).filter(|seconds| *seconds > 0);
    let reset_at = codex_json_i64(error.get("resets_at")).and_then(|timestamp| {
        let derived_remaining = u64::try_from(timestamp.checked_sub(now)?).ok()?;
        if derived_remaining == 0 {
            return None;
        }
        let local = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)?
            .with_timezone(&chrono::Local);
        Some((
            local.format("%Y-%m-%d %H:%M:%S %:z").to_string(),
            reset_in_seconds.unwrap_or(derived_remaining),
        ))
    });

    if let Some((local_time, remaining)) = reset_at {
        message.push_str(&format!(
            " It resets at {local_time} local time (in about {}).",
            format_quota_wait(remaining)
        ));
        message.push_str(" Wait for the reset, or use another provider or account.");
    } else if let Some(remaining) = reset_in_seconds {
        message.push_str(&format!(
            " It resets in about {}. Wait for the reset, or use another provider or account.",
            format_quota_wait(remaining)
        ));
    } else {
        message.push_str(" Try again later, or use another provider or account.");
    }

    Some(NonRetryableLlmError::new(message))
}

fn codex_plan_label(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str()?.trim();
    if value.is_empty()
        || value.len() > 32
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }

    let words = value
        .split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();
    (!words.is_empty()).then(|| words.join(" "))
}

fn codex_json_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse().ok())
}

fn codex_json_u64(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse().ok())
}

fn format_quota_wait(seconds: u64) -> String {
    let minutes = seconds.saturating_add(59) / 60;
    if minutes >= 24 * 60 {
        let days = minutes / (24 * 60);
        let hours = (minutes % (24 * 60)) / 60;
        if hours == 0 {
            format!("{days}d")
        } else {
            format!("{days}d {hours}h")
        }
    } else if minutes >= 60 {
        let hours = minutes / 60;
        let minutes = minutes % 60;
        if minutes == 0 {
            format!("{hours}h")
        } else {
            format!("{hours}h {minutes}m")
        }
    } else {
        format!("{}m", minutes.max(1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexModel {
    pub(crate) slug: String,
    pub(crate) context_window: Option<u32>,
    use_responses_lite: bool,
    default_reasoning_effort: Option<String>,
    supported_reasoning_efforts: Vec<String>,
}

impl CodexModel {
    fn fallback() -> Self {
        Self {
            slug: FALLBACK_CODEX_MODEL.to_string(),
            context_window: None,
            // GPT-5.6 account models use the Codex Responses Lite contract.
            use_responses_lite: true,
            default_reasoning_effort: Some("low".to_string()),
            supported_reasoning_efforts: ["low", "medium", "high", "xhigh", "max"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }

    /// Resolve an A3S effort profile to the closest native effort supported by
    /// this Codex account model. `ultracode` keeps its A3S orchestration meaning
    /// while using Codex's highest Responses wire effort (`max`, or a lower cap).
    pub(crate) fn resolve_reasoning_effort(&self, a3s_effort: &str) -> Option<String> {
        let target = native_reasoning_effort_for_a3s(a3s_effort)?;
        if self
            .supported_reasoning_efforts
            .iter()
            .any(|effort| effort == target)
        {
            return Some(target.to_string());
        }

        let target_rank = reasoning_effort_rank(target)?;
        let known_supported = self
            .supported_reasoning_efforts
            .iter()
            .filter_map(|effort| reasoning_effort_rank(effort).map(|rank| (rank, effort)));
        if let Some((_, effort)) = known_supported
            .clone()
            .filter(|(rank, _)| *rank <= target_rank)
            .max_by_key(|(rank, _)| *rank)
        {
            return Some(effort.clone());
        }
        if let Some((_, effort)) = known_supported.min_by_key(|(rank, _)| *rank) {
            return Some(effort.clone());
        }

        self.default_reasoning_effort.clone().filter(|default| {
            reasoning_effort_rank(default).is_some()
                && self
                    .supported_reasoning_efforts
                    .iter()
                    .any(|effort| effort == default)
        })
    }
}

fn reasoning_effort_rank(effort: &str) -> Option<usize> {
    CODEX_WIRE_REASONING_EFFORT_ORDER
        .iter()
        .position(|candidate| *candidate == effort)
}

fn cached_catalog_value() -> Option<Value> {
    let path = codex_models_cache_path()?;
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn normalized_effort(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
        .map(str::to_ascii_lowercase)
}

fn parse_supported_reasoning_efforts(model: &Value) -> Vec<String> {
    let Some(levels) = model
        .get("supported_reasoning_levels")
        .or_else(|| model.get("supported_reasoning_efforts"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    levels
        .iter()
        .filter_map(|level| {
            normalized_effort(level).or_else(|| level.get("effort").and_then(normalized_effort))
        })
        .filter(|effort| seen.insert(effort.clone()))
        .collect()
}

fn parse_default_reasoning_effort(model: &Value) -> Option<String> {
    model
        .get("default_reasoning_level")
        .or_else(|| model.get("default_reasoning_effort"))
        .and_then(normalized_effort)
}

fn parse_model_catalog(value: &Value) -> Vec<CodexModel> {
    let Some(models) = value.get("models").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut models = models
        .iter()
        .enumerate()
        .filter_map(|(index, model)| {
            // Match Codex's own picker semantics. Hidden entries such as
            // `codex-auto-review` are internal, not account-selectable models.
            if model.get("visibility").and_then(Value::as_str) != Some("list") {
                return None;
            }
            let slug = model
                .get("slug")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|slug| !slug.is_empty())?;
            let priority = model
                .get("priority")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX);
            Some((
                priority,
                index,
                CodexModel {
                    slug: slug.to_string(),
                    context_window: parse_model_context(model),
                    use_responses_lite: model
                        .get("use_responses_lite")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    default_reasoning_effort: parse_default_reasoning_effort(model),
                    supported_reasoning_efforts: parse_supported_reasoning_efforts(model),
                },
            ))
        })
        .collect::<Vec<_>>();
    models.sort_by_key(|(priority, index, _)| (*priority, *index));

    let mut seen = HashSet::new();
    models
        .into_iter()
        .filter_map(|(_, _, model)| seen.insert(model.slug.clone()).then_some(model))
        .collect()
}

pub(crate) fn cached_codex_models() -> Vec<CodexModel> {
    let models = cached_catalog_value()
        .as_ref()
        .map(parse_model_catalog)
        .unwrap_or_default();
    if models.is_empty() {
        vec![CodexModel::fallback()]
    } else {
        models
    }
}

/// Refresh the picker catalog through the installed Codex CLI. This delegates
/// client-version negotiation, token refresh, account entitlements, ETags, and
/// cache persistence to the owner of the login instead of duplicating them.
pub(crate) async fn refresh_codex_models() -> Result<Vec<CodexModel>> {
    let mut command = tokio::process::Command::new("codex");
    command.args(["debug", "models"]).kill_on_drop(true);
    let output = tokio::time::timeout(CODEX_MODEL_REFRESH_TIMEOUT, command.output())
        .await
        .map_err(|_| {
            anyhow!(
                "`codex debug models` timed out after {} seconds",
                CODEX_MODEL_REFRESH_TIMEOUT.as_secs()
            )
        })?
        .context("run `codex debug models`")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("Codex could not refresh the signed-in account's model catalog");
        return Err(anyhow!(
            "`codex debug models` exited with {}: {}",
            output.status,
            detail.trim()
        ));
    }

    let value: Value =
        serde_json::from_slice(&output.stdout).context("parse `codex debug models` output")?;
    let models = parse_model_catalog(&value);
    if models.is_empty() {
        return Err(anyhow!(
            "the signed-in Codex account returned no picker-visible models"
        ));
    }
    Ok(models)
}

fn cached_codex_model(model: &str) -> Option<CodexModel> {
    cached_catalog_value()
        .as_ref()
        .map(parse_model_catalog)
        .unwrap_or_default()
        .into_iter()
        .find(|candidate| candidate.slug == model)
        .or_else(|| (model == FALLBACK_CODEX_MODEL).then(CodexModel::fallback))
}

pub(crate) fn codex_model_context(model: &str) -> Option<u32> {
    cached_codex_model(model).and_then(|model| model.context_window)
}

/// Return whether the Codex-owned auth cache contains reusable account state.
///
/// The ID token is identity metadata, not the bearer token used by the Codex
/// Responses backend. It can expire before the access token and refresh token,
/// so its expiry must not hide an otherwise usable account. The installed
/// Codex CLI owns token refresh and validates the account when models are
/// refreshed.
pub(crate) fn has_codex_login() -> bool {
    let Some(path) = codex_auth_path() else {
        return false;
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|value| {
            value
                .pointer("/tokens/access_token")
                .and_then(Value::as_str)
                .is_some_and(|token| !token.is_empty())
                || value
                    .get("access_token")
                    .and_then(Value::as_str)
                    .is_some_and(|token| !token.is_empty())
        })
}

fn positive_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok())
}

fn parse_model_context(model: &Value) -> Option<u32> {
    const KEYS: &[&str] = &[
        "context_length",
        "max_context_length",
        "max_context_window",
        "max_model_len",
        "context_window",
        "max_input_tokens",
    ];
    for key in KEYS {
        if let Some(context) = model.get(key).and_then(positive_u32) {
            return Some(context);
        }
    }
    model
        .get("model_info")
        .and_then(|info| info.get("max_input_tokens"))
        .and_then(positive_u32)
}

#[derive(Clone)]
pub(crate) struct CodexClient {
    auth: Arc<AuthState>,
    model: String,
    session_id: String,
    use_responses_lite: bool,
    reasoning_effort: Option<String>,
    forced_tool_choice: Option<String>,
    transport: TransportController,
}

impl CodexClient {
    /// Read Codex's auth cache and bind it to `model`.
    pub(crate) fn from_codex_login(model: &str, session_id: &str) -> Result<Self> {
        let path = codex_auth_path().ok_or_else(|| anyhow!("HOME unset and CODEX_HOME unset"))?;
        let auth = Arc::new(AuthState::load(path)?);
        let wire = Arc::new(NetworkWireClient::new().context("build Codex network client")?);
        Ok(Self {
            auth,
            model: model.to_string(),
            session_id: session_id.to_string(),
            use_responses_lite: cached_codex_model(model)
                .is_some_and(|model| model.use_responses_lite),
            // Preserve the backend default for callers that do not opt into an
            // A3S profile. The TUI always materializes an explicit profile.
            reasoning_effort: None,
            forced_tool_choice: None,
            transport: TransportController::new(wire),
        })
    }

    pub(crate) fn from_codex_login_with_effort(
        model: &str,
        session_id: &str,
        a3s_effort: &str,
    ) -> Result<Self> {
        Ok(Self::from_codex_login(model, session_id)?.with_a3s_effort(a3s_effort))
    }

    /// Clone this authenticated client with the native effort resolved from the
    /// latest signed-in account catalog. Keeping the configured effort immutable
    /// makes `/effort` transactional: a failed session rebuild cannot mutate the
    /// client retained by the old active session.
    pub(crate) fn with_a3s_effort(&self, a3s_effort: &str) -> Self {
        let mut client = self.clone();
        client.reasoning_effort = self.resolve_reasoning_effort(a3s_effort);
        client
    }

    pub(crate) fn resolve_reasoning_effort(&self, a3s_effort: &str) -> Option<String> {
        cached_codex_model(&self.model).and_then(|model| model.resolve_reasoning_effort(a3s_effort))
    }

    #[cfg(test)]
    fn configured_reasoning_effort(&self) -> Option<&str> {
        self.reasoning_effort.as_deref()
    }

    fn build_body(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        stream: bool,
    ) -> Value {
        let mut input = convert_messages(messages);
        let tools = convert_tools(tools);
        let prompt_cache_key = codex_prompt_cache_key(&self.session_id);
        let reasoning_effort = self
            .reasoning_effort
            .as_deref()
            .and_then(codex_wire_reasoning_effort);
        if self.use_responses_lite {
            let mut reasoning = json!({ "context": "all_turns" });
            if let Some(effort) = reasoning_effort {
                reasoning["effort"] = json!(effort);
            }
            let mut prefix = vec![json!({
                "type": "additional_tools",
                "role": "developer",
                "tools": tools,
            })];
            if let Some(instructions) = system.filter(|instructions| !instructions.is_empty()) {
                prefix.push(json!({
                    "type": "message",
                    "role": "developer",
                    "content": [{ "type": "input_text", "text": instructions }],
                }));
            }
            prefix.append(&mut input);
            return json!({
                "model": self.model,
                "input": prefix,
                "tool_choice": self.tool_choice(),
                "parallel_tool_calls": false,
                "reasoning": reasoning,
                "store": false,
                "stream": stream,
                "prompt_cache_key": prompt_cache_key,
            });
        }

        let mut body = json!({
            "model": self.model,
            "instructions": system.unwrap_or(""),
            "input": input,
            "tools": tools,
            "tool_choice": self.tool_choice(),
            "parallel_tool_calls": false,
            "store": false,
            "stream": stream,
            "prompt_cache_key": prompt_cache_key,
        });
        if let Some(effort) = reasoning_effort {
            body["reasoning"] = json!({ "effort": effort });
        }
        body
    }

    fn tool_choice(&self) -> Value {
        self.forced_tool_choice
            .as_ref()
            .map(|name| json!({ "type": "function", "name": name }))
            .unwrap_or_else(|| json!("auto"))
    }

    fn request_headers(&self) -> Vec<(String, String)> {
        let credentials = self.auth.credentials();
        let provider_session_id = codex_prompt_cache_key(&self.session_id);
        let mut headers = vec![
            (
                "Authorization".to_string(),
                format!("Bearer {}", credentials.access_token),
            ),
            ("chatgpt-account-id".to_string(), credentials.account_id),
            (
                "OpenAI-Beta".to_string(),
                "responses=experimental".to_string(),
            ),
            ("originator".to_string(), ORIGINATOR.to_string()),
            ("session_id".to_string(), provider_session_id),
            ("Accept".to_string(), "text/event-stream".to_string()),
            ("User-Agent".to_string(), codex_user_agent()),
        ];
        if self.use_responses_lite {
            headers.push((RESPONSES_LITE_HEADER.to_string(), "true".to_string()));
        }
        headers
    }

    fn map_transport_error(&self, error: TransportError) -> anyhow::Error {
        if let (Some(status), Some(body)) = (error.status, error.body.as_deref()) {
            if let Some(error) = codex_usage_limit_error(status, body) {
                return error.into();
            }
        }
        match error.status {
            Some(401) => NonRetryableLlmError::new(
                "Codex access token expired or is invalid (HTTP 401); run `codex login` to refresh the local account",
            )
            .into(),
            Some(403) => NonRetryableLlmError::new(
                "Codex WebSocket and HTTPS fallback were blocked by ChatGPT network protection (HTTP 403). Check the VPN/proxy route or use the official Codex transport.",
            )
            .into(),
            _ => NonRetryableLlmError::new(error.to_string()).into(),
        }
    }
}

fn codex_prompt_cache_key(session_id: &str) -> String {
    if session_id.len() <= MAX_PROMPT_CACHE_KEY_BYTES {
        return session_id.to_string();
    }
    format!("{:x}", Sha256::digest(session_id.as_bytes()))
}

fn codex_user_agent() -> String {
    format!(
        "codex_cli_rs/{} ({} {}; a3s)",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

#[async_trait]
impl LlmClient for CodexClient {
    fn model_generation_concurrency(&self) -> ModelGenerationConcurrency {
        // The ChatGPT account backend can queue concurrent Responses requests
        // behind account/session admission. Keep that wait local and explicit
        // so callers do not burn active-generation deadlines in the backend.
        ModelGenerationConcurrency::single_flight()
    }

    fn fork_for_session(&self, session_id: &str) -> Option<Arc<dyn LlmClient>> {
        let mut client = self.clone();
        client.session_id = session_id.to_string();
        Some(Arc::new(client))
    }

    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let mut rx = self
            .complete_streaming(messages, system, tools, CancellationToken::new())
            .await?;
        while let Some(event) = rx.recv().await {
            if let StreamEvent::Done(response) = event {
                return Ok(response);
            }
        }
        Err(anyhow!("codex stream closed before response.completed"))
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let body = self.build_body(messages, system, tools, true);
        let url = format!("{CODEX_BASE}/responses");
        let request = WireRequest {
            endpoint: url.clone(),
            headers: self.request_headers(),
            body,
        };
        let rejected_token = self.auth.credentials().access_token;
        let wire = match self.transport.open(&request, cancel_token.clone()).await {
            Ok(wire) => wire,
            Err(error) if error.status == Some(401) => {
                self.auth
                    .refresh_after_unauthorized(&rejected_token)
                    .await
                    .context("Codex login refresh failed after HTTP 401; run `codex login`")?;
                let refreshed_request = WireRequest {
                    endpoint: request.endpoint,
                    headers: self.request_headers(),
                    body: request.body,
                };
                self.transport
                    .open(&refreshed_request, cancel_token)
                    .await
                    .map_err(|error| self.map_transport_error(error))?
            }
            Err(error) => return Err(self.map_transport_error(error)),
        };

        Ok(stream::into_llm_stream(
            wire,
            self.transport.clone(),
            self.model.clone(),
            url,
        ))
    }

    fn native_structured_support(&self) -> NativeStructuredSupport {
        NativeStructuredSupport::ForcedTool
    }

    async fn complete_structured(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: &StructuredDirective,
    ) -> Result<LlmResponse> {
        let mut client = self.clone();
        client.forced_tool_choice = directive.force_tool.clone();
        client.complete(messages, system, tools).await
    }

    async fn complete_streaming_structured(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: &StructuredDirective,
        cancel_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let mut client = self.clone();
        client.forced_tool_choice = directive.force_tool.clone();
        client
            .complete_streaming(messages, system, tools, cancel_token)
            .await
    }
}

fn convert_messages(messages: &[Message]) -> Vec<Value> {
    let mut out = Vec::new();
    for message in messages {
        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    let kind = if message.role == "assistant" {
                        "output_text"
                    } else {
                        "input_text"
                    };
                    out.push(json!({
                        "type": "message",
                        "role": message.role,
                        "content": [{"type": kind, "text": text}],
                    }));
                }
                ContentBlock::ToolUse { id, name, input } => {
                    out.push(json!({
                        "type": "function_call",
                        "name": name,
                        "arguments": input.to_string(),
                        "call_id": id,
                    }));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } => {
                    out.push(json!({
                        "type": "function_call_output",
                        "call_id": tool_use_id,
                        "output": content.as_text(),
                    }));
                }
                ContentBlock::Image { source } => {
                    out.push(json!({
                        "type": "message",
                        "role": message.role,
                        "content": [{
                            "type": "input_image",
                            "image_url": format!(
                                "data:{};base64,{}",
                                source.media_type, source.data
                            ),
                        }],
                    }));
                }
            }
        }
    }
    out
}

fn convert_tools(tools: &[ToolDefinition]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "codex/tests.rs"]
mod tests;
