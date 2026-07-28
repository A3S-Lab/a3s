use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use transport::{TransportConfig, TransportError, TransportKind, WireClient, WireStream};

struct ErrorWire {
    status: u16,
    body: String,
    calls: AtomicUsize,
}

struct EventWire {
    events: Vec<Value>,
}

struct RotatingAuthWire {
    path: PathBuf,
    replacement: Vec<u8>,
    calls: AtomicUsize,
    authorization: Mutex<Vec<String>>,
}

#[derive(Default)]
struct SessionScopedWire {
    websocket_calls: AtomicUsize,
    http_calls: AtomicUsize,
}

#[async_trait]
impl WireClient for ErrorWire {
    async fn open_websocket(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        Err(TransportError::http(426, None, None))
    }

    async fn open_http_sse(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(TransportError::http(
            self.status,
            Some(self.body.clone()),
            None,
        ))
    }
}

#[async_trait]
impl WireClient for EventWire {
    async fn open_websocket(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        Ok(event_stream(TransportKind::WebSocket, self.events.clone()))
    }

    async fn open_http_sse(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        panic!("unexpected HTTPS fallback")
    }
}

#[async_trait]
impl WireClient for RotatingAuthWire {
    async fn open_websocket(
        &self,
        request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        let authorization = request
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
            .map(|(_, value)| value.clone())
            .unwrap_or_default();
        self.authorization
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(authorization);
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            tokio::fs::write(&self.path, &self.replacement)
                .await
                .unwrap();
            return Err(TransportError::http(401, Some("expired".to_string()), None));
        }
        Ok(event_stream(
            TransportKind::WebSocket,
            vec![json!({
                "type": "response.completed",
                "response": {
                    "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}
                }
            })],
        ))
    }

    async fn open_http_sse(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        panic!("401 recovery must retry the original WebSocket transport")
    }
}

#[async_trait]
impl WireClient for SessionScopedWire {
    async fn open_websocket(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        let call = self.websocket_calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            return Err(TransportError::http(403, None, None));
        }
        Ok(completed_event_stream(TransportKind::WebSocket))
    }

    async fn open_http_sse(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        self.http_calls.fetch_add(1, Ordering::SeqCst);
        Ok(completed_event_stream(TransportKind::HttpSse))
    }
}

struct UnusedWire;

#[async_trait]
impl WireClient for UnusedWire {
    async fn open_websocket(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        panic!("unexpected WebSocket call")
    }

    async fn open_http_sse(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> std::result::Result<WireStream, TransportError> {
        panic!("unexpected HTTPS call")
    }
}

fn event_stream(kind: TransportKind, events: Vec<Value>) -> WireStream {
    let (tx, rx) = mpsc::channel(events.len().max(1));
    for event in events {
        tx.try_send(Ok(event)).unwrap();
    }
    drop(tx);
    WireStream { kind, events: rx }
}

fn completed_event_stream(kind: TransportKind) -> WireStream {
    event_stream(
        kind,
        vec![json!({
            "type": "response.completed",
            "response": {
                "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}
            }
        })],
    )
}

fn test_transport(wire: Arc<dyn WireClient>) -> TransportController {
    TransportController::with_config(
        wire,
        TransportConfig {
            websocket_retries: 0,
            http_retries: 0,
            retry_base: Duration::ZERO,
        },
    )
}

fn client(use_responses_lite: bool, reasoning_effort: Option<&str>) -> CodexClient {
    CodexClient {
        auth: Arc::new(AuthState::for_test("token", "account")),
        model: if use_responses_lite {
            "gpt-5.6-sol"
        } else {
            "gpt-5.5"
        }
        .to_string(),
        session_id: "session".to_string(),
        use_responses_lite,
        reasoning_effort: reasoning_effort.map(str::to_string),
        forced_tool_choice: None,
        transport: test_transport(Arc::new(UnusedWire)),
    }
}

fn model_with_efforts(default: Option<&str>, supported: &[&str]) -> CodexModel {
    CodexModel {
        slug: "test-model".to_string(),
        context_window: None,
        use_responses_lite: false,
        default_reasoning_effort: default.map(str::to_string),
        supported_reasoning_efforts: supported
            .iter()
            .map(|effort| (*effort).to_string())
            .collect(),
    }
}

fn tool() -> ToolDefinition {
    ToolDefinition {
        name: "read".to_string(),
        description: "Read a file".to_string(),
        parameters: json!({"type": "object"}),
    }
}

fn jwt_with_claims(claims: Value) -> String {
    format!(
        "header.{}.signature",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
    )
}

#[test]
fn account_backend_reports_single_flight_model_generation() {
    assert_eq!(
        client(false, Some("low"))
            .model_generation_concurrency()
            .max_concurrency()
            .get(),
        1
    );
}

#[test]
fn chatgpt_account_id_uses_the_fully_escaped_json_pointer() {
    let token = jwt_with_claims(json!({
        "https://api.openai.com/auth/chatgpt_account_id": "acct_123"
    }));

    assert_eq!(
        auth::account_id_from_id_token(&token).as_deref(),
        Some("acct_123")
    );
}

#[test]
fn parses_model_context_from_cache_shapes() {
    assert_eq!(
        parse_model_context(&json!({"context_length": 200000})),
        Some(200_000)
    );
    assert_eq!(
        parse_model_context(&json!({"model_info": {"max_input_tokens": 1000000}})),
        Some(1_000_000)
    );
    assert_eq!(parse_model_context(&json!({"context_window": 0})), None);
    assert_eq!(parse_model_context(&json!({"slug": "gpt"})), None);
    assert_eq!(
        parse_model_context(&json!({"context_window": u64::MAX})),
        None
    );
}

#[test]
fn parses_all_picker_visible_models_in_priority_order() {
    let models = parse_model_catalog(&json!({
        "models": [
            {
                "slug": "gpt-5.6-terra",
                "visibility": "list",
                "priority": 2,
                "context_window": 372000,
                "use_responses_lite": true
            },
            {
                "slug": "codex-auto-review",
                "visibility": "hide",
                "priority": 0,
                "context_window": 372000
            },
            {
                "slug": "gpt-5.6-sol",
                "visibility": "list",
                "priority": 1,
                "context_window": 372000,
                "use_responses_lite": true,
                "default_reasoning_level": " LOW ",
                "supported_reasoning_levels": [
                    {"effort": "low"},
                    {"effort": "MEDIUM"},
                    {"effort": "low"},
                    {"effort": "high"},
                    {"effort": "xhigh"},
                    {"effort": "max"},
                    {"effort": "ultra"}
                ],
                "supported_in_api": false
            },
            {
                "slug": "gpt-5.5",
                "visibility": "list",
                "priority": 0,
                "context_window": 272000
            },
            {
                "slug": "gpt-5.6-sol",
                "visibility": "list",
                "priority": 99
            }
        ]
    }));

    assert_eq!(
        models
            .iter()
            .map(|model| model.slug.as_str())
            .collect::<Vec<_>>(),
        vec!["gpt-5.5", "gpt-5.6-sol", "gpt-5.6-terra"]
    );
    let sol = models
        .iter()
        .find(|model| model.slug == "gpt-5.6-sol")
        .unwrap();
    assert_eq!(sol.context_window, Some(372_000));
    assert!(sol.use_responses_lite);
    assert_eq!(sol.default_reasoning_effort.as_deref(), Some("low"));
    assert_eq!(
        sol.supported_reasoning_efforts,
        ["low", "medium", "high", "xhigh", "max", "ultra"]
    );
}

#[test]
fn parses_alternate_reasoning_capability_shape() {
    let models = parse_model_catalog(&json!({
        "models": [{
            "slug": "alternate",
            "visibility": "list",
            "default_reasoning_effort": " XHIGH ",
            "supported_reasoning_efforts": [" xhigh ", {"effort": "MAX"}]
        }]
    }));

    assert_eq!(models[0].default_reasoning_effort.as_deref(), Some("xhigh"));
    assert_eq!(models[0].supported_reasoning_efforts, ["xhigh", "max"]);
}

#[test]
fn resolves_a3s_effort_against_model_capabilities() {
    let sol = model_with_efforts(
        Some("low"),
        &["low", "medium", "high", "xhigh", "max", "ultra"],
    );
    let luna = model_with_efforts(Some("medium"), &["low", "medium", "high", "xhigh", "max"]);
    let legacy = model_with_efforts(Some("medium"), &["low", "medium", "high", "xhigh"]);

    assert_eq!(
        sol.resolve_reasoning_effort("high").as_deref(),
        Some("high")
    );
    assert_eq!(
        sol.resolve_reasoning_effort("ultracode").as_deref(),
        Some("max")
    );
    assert_eq!(
        luna.resolve_reasoning_effort("ultracode").as_deref(),
        Some("max")
    );
    assert_eq!(
        legacy.resolve_reasoning_effort("max").as_deref(),
        Some("xhigh")
    );
    assert_eq!(
        legacy.resolve_reasoning_effort("ultracode").as_deref(),
        Some("xhigh")
    );

    let medium_only = model_with_efforts(Some("medium"), &["medium"]);
    assert_eq!(
        medium_only.resolve_reasoning_effort("low").as_deref(),
        Some("medium")
    );
    let below_low = model_with_efforts(Some("minimal"), &["none", "minimal"]);
    assert_eq!(
        below_low.resolve_reasoning_effort("low").as_deref(),
        Some("minimal")
    );
    assert_eq!(
        model_with_efforts(Some("medium"), &[]).resolve_reasoning_effort("high"),
        None
    );
    assert_eq!(sol.resolve_reasoning_effort("unknown"), None);
    assert_eq!(native_reasoning_effort_for_a3s("ultracode"), Some("max"));
    assert_eq!(codex_wire_reasoning_effort("ultra"), Some("max"));
}

#[test]
fn standard_responses_request_keeps_top_level_instructions_and_tools() {
    let client = client(false, Some("xhigh"));
    let body = client.build_body(&[Message::user("hello")], Some("system"), &[tool()], true);

    assert_eq!(body["instructions"], "system");
    assert_eq!(body["tools"][0]["name"], "read");
    assert_eq!(body["input"][0]["role"], "user");
    assert_eq!(body["reasoning"]["effort"], "xhigh");
    assert!(!client
        .request_headers()
        .iter()
        .any(|(name, _)| name == RESPONSES_LITE_HEADER));
}

#[test]
fn provider_session_identity_is_stable_and_bounded() {
    let mut client = client(false, Some("xhigh"));
    let long_session = "research-run:select_evidence_chunks_shard_recovery_1234567890".repeat(2);
    client.session_id = long_session.clone();

    let first = client.build_body(&[], None, &[], true);
    let second = client.build_body(&[], None, &[], true);
    let cache_key = first["prompt_cache_key"].as_str().unwrap();

    assert!(
        cache_key.len() <= 64,
        "cache key was {} bytes",
        cache_key.len()
    );
    assert_eq!(first["prompt_cache_key"], second["prompt_cache_key"]);
    assert_eq!(
        client
            .request_headers()
            .into_iter()
            .find(|(name, _)| name == "session_id")
            .map(|(_, value)| value),
        Some(cache_key.to_string())
    );

    let mut sibling = client;
    sibling.session_id.push_str(":sibling");
    assert_ne!(
        first["prompt_cache_key"],
        sibling.build_body(&[], None, &[], true)["prompt_cache_key"]
    );
}

#[test]
fn responses_lite_moves_instructions_and_tools_into_input() {
    let client = client(true, Some("ultra"));
    let body = client.build_body(&[Message::user("hello")], Some("system"), &[tool()], true);

    assert!(body.get("instructions").is_none());
    assert!(body.get("tools").is_none());
    assert_eq!(body["input"][0]["type"], "additional_tools");
    assert_eq!(body["input"][0]["role"], "developer");
    assert_eq!(body["input"][0]["tools"][0]["name"], "read");
    assert_eq!(body["input"][1]["role"], "developer");
    assert_eq!(body["input"][1]["content"][0]["text"], "system");
    assert_eq!(body["input"][2]["role"], "user");
    assert_eq!(body["reasoning"]["context"], "all_turns");
    assert_eq!(body["reasoning"]["effort"], "max");
    assert!(!body.to_string().contains("ultra"));
    assert!(client
        .request_headers()
        .iter()
        .any(|(name, value)| name == RESPONSES_LITE_HEADER && value == "true"));
}

#[test]
fn unresolved_effort_is_omitted_from_request_bodies() {
    let standard = client(false, None).build_body(&[], None, &[], false);
    let lite = client(true, None).build_body(&[], None, &[], false);
    let invalid = client(false, Some("not-a-wire-effort")).build_body(&[], None, &[], false);

    assert!(standard.get("reasoning").is_none());
    assert_eq!(lite["reasoning"]["context"], "all_turns");
    assert!(lite["reasoning"].get("effort").is_none());
    assert!(invalid.get("reasoning").is_none());
}

#[test]
fn structured_requests_force_the_named_function_for_both_codex_transports() {
    for responses_lite in [false, true] {
        let mut client = client(responses_lite, Some("low"));
        client.forced_tool_choice = Some("emit_research_plan".to_string());
        let body = client.build_body(&[], None, &[tool()], true);

        assert_eq!(body["tool_choice"]["type"], "function");
        assert_eq!(body["tool_choice"]["name"], "emit_research_plan");
        assert_eq!(
            client.native_structured_support(),
            NativeStructuredSupport::ForcedTool
        );
    }
}

#[test]
fn usage_limit_payload_becomes_a_friendly_terminal_error() {
    let body = r#"{
        "error": {
            "type": "usage_limit_reached",
            "message": "The usage limit has been reached",
            "plan_type": "pro",
            "resets_at": 1783656812,
            "resets_in_seconds": 9893
        }
    }"#;

    let error =
        codex_usage_limit_error_at(429, body, 1783646919).expect("usage limits should be terminal");
    let message = error.to_string();
    assert!(message.contains("Codex usage limit reached (Pro plan)."));
    assert!(message.contains("It resets at "), "{message}");
    assert!(message.contains(" local time"), "{message}");
    assert!(message.contains("in about 2h 45m"), "{message}");
    assert!(message.contains("another provider or account"), "{message}");
    assert!(!message.contains("usage_limit_reached"), "{message}");
    assert!(!message.contains("resets_in_seconds"), "{message}");
}

#[test]
fn usage_limit_payload_degrades_gracefully_when_reset_is_missing() {
    let error = codex_usage_limit_error_at(
        429,
        r#"{"error":{"code":"usage_limit_reached","plan_type":"plus"}}"#,
        1783646919,
    )
    .expect("the error code shape should also be recognized");

    assert_eq!(
        error.to_string(),
        "Codex usage limit reached (Plus plan). Try again later, or use another provider or account."
    );
}

#[test]
fn usage_limit_prefers_server_relative_reset_over_local_clock_delta() {
    let error = codex_usage_limit_error_at(
        429,
        r#"{"error":{"type":"usage_limit_reached","resets_at":"1783656812","resets_in_seconds":"90"}}"#,
        1783646919,
    )
    .expect("numeric strings should be accepted");
    let message = error.to_string();

    assert!(message.contains("2026-"), "{message}");
    assert!(message.contains(":32 "), "{message}");
    assert!(message.contains("in about 2m"), "{message}");
    assert!(!message.contains("2h 45m"), "{message}");
}

#[test]
fn ordinary_rate_limits_and_malformed_payloads_remain_retryable() {
    assert!(codex_usage_limit_error_at(
        429,
        r#"{"error":{"type":"rate_limit_exceeded"}}"#,
        1783646919,
    )
    .is_none());
    assert!(codex_usage_limit_error_at(429, "not json", 1783646919).is_none());
    assert!(codex_usage_limit_error_at(
        500,
        r#"{"error":{"type":"usage_limit_reached"}}"#,
        1783646919,
    )
    .is_none());
}

#[tokio::test]
async fn usage_limit_http_response_returns_non_retryable_marker() {
    let wire = Arc::new(ErrorWire {
        status: 429,
        body:
            r#"{"error":{"type":"usage_limit_reached","plan_type":"pro","resets_in_seconds":90}}"#
                .to_string(),
        calls: AtomicUsize::new(0),
    });
    let mut client = client(true, Some("max"));
    client.transport = test_transport(wire.clone());

    let error = match client
        .complete_streaming(&[], None, &[], CancellationToken::new())
        .await
    {
        Ok(_) => panic!("usage limits must fail before opening a stream"),
        Err(error) => error,
    };

    assert!(error.downcast_ref::<NonRetryableLlmError>().is_some());
    assert_eq!(
        error.to_string(),
        "Codex usage limit reached (Pro plan). It resets in about 2m. Wait for the reset, or use another provider or account."
    );
    assert_eq!(wire.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unauthorized_response_points_to_codex_login_without_leaking_body() {
    let wire = Arc::new(ErrorWire {
        status: 401,
        body: "private backend detail".to_string(),
        calls: AtomicUsize::new(0),
    });
    let mut client = client(true, Some("low"));
    client.transport = test_transport(wire.clone());

    let error = client
        .complete_streaming(&[], None, &[], CancellationToken::new())
        .await
        .expect_err("HTTP 401 must fail before opening a stream");

    assert!(error.to_string().contains("codex login"));
    assert!(!error.to_string().contains("private backend detail"));
    assert_eq!(wire.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unauthorized_reloads_rotated_auth_and_retries_transport_once() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("auth.json");
    let auth_value = |access_token: &str| {
        json!({
            "tokens": {
                "access_token": access_token,
                "refresh_token": "refresh-token",
                "account_id": "account"
            }
        })
    };
    tokio::fs::write(&path, serde_json::to_vec(&auth_value("old-token"))?).await?;
    let wire = Arc::new(RotatingAuthWire {
        path: path.clone(),
        replacement: serde_json::to_vec(&auth_value("new-token"))?,
        calls: AtomicUsize::new(0),
        authorization: Mutex::new(Vec::new()),
    });
    let client = CodexClient {
        auth: Arc::new(AuthState::load(path)?),
        model: "gpt-5.5".to_string(),
        session_id: "session".to_string(),
        use_responses_lite: false,
        reasoning_effort: Some("low".to_string()),
        forced_tool_choice: None,
        transport: test_transport(wire.clone()),
    };

    let mut stream = client
        .complete_streaming(&[], None, &[], CancellationToken::new())
        .await?;
    assert!(matches!(stream.recv().await, Some(StreamEvent::Done(_))));
    assert_eq!(wire.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        *wire
            .authorization
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["Bearer old-token", "Bearer new-token"]
    );
    assert_eq!(client.auth.credentials().access_token, "new-token");
    Ok(())
}

#[tokio::test]
async fn transport_fallback_is_sticky_across_turns_and_child_sessions() -> Result<()> {
    let wire = Arc::new(SessionScopedWire::default());
    let mut client = client(false, Some("low"));
    client.transport = test_transport(wire.clone());

    for _ in 0..2 {
        let scoped = client
            .fork_for_session("session")
            .expect("same-session client");
        let mut stream = scoped
            .complete_streaming(&[], None, &[], CancellationToken::new())
            .await?;
        assert!(matches!(stream.recv().await, Some(StreamEvent::Done(_))));
    }
    assert_eq!(wire.websocket_calls.load(Ordering::SeqCst), 1);
    assert_eq!(wire.http_calls.load(Ordering::SeqCst), 2);

    let child = client
        .fork_for_session("child-session")
        .expect("child-session client");
    let mut stream = child
        .complete_streaming(&[], None, &[], CancellationToken::new())
        .await?;
    assert!(matches!(stream.recv().await, Some(StreamEvent::Done(_))));
    assert_eq!(wire.websocket_calls.load(Ordering::SeqCst), 1);
    assert_eq!(wire.http_calls.load(Ordering::SeqCst), 3);
    Ok(())
}

#[tokio::test]
async fn streaming_tool_argument_deltas_keep_interleaved_call_ids() -> Result<()> {
    let events = vec![
        json!({"type":"response.output_item.added","item":{"type":"function_call","id":"item_a","call_id":"call_a","name":"read"}}),
        json!({"type":"response.output_item.added","item":{"type":"function_call","id":"item_b","call_id":"call_b","name":"search"}}),
        json!({"type":"response.function_call_arguments.delta","item_id":"item_b","delta":"{\"query\":\"beta\"}"}),
        json!({"type":"response.function_call_arguments.delta","item_id":"item_a","delta":"{\"path\":\"alpha\"}"}),
        json!({"type":"response.completed","response":{"usage":{"input_tokens":1,"output_tokens":2,"total_tokens":3}}}),
    ];
    let mut client = client(false, Some("high"));
    client.transport = test_transport(Arc::new(EventWire { events }));
    let mut rx = client
        .complete_streaming(&[], None, &[], CancellationToken::new())
        .await?;

    assert!(matches!(
        rx.recv().await,
        Some(StreamEvent::ToolUseStart { id, name })
            if id == "call_a" && name == "read"
    ));
    assert!(matches!(
        rx.recv().await,
        Some(StreamEvent::ToolUseStart { id, name })
            if id == "call_b" && name == "search"
    ));
    assert!(matches!(
        rx.recv().await,
        Some(StreamEvent::ToolUseInputDelta { id: Some(id), delta })
            if id == "call_b" && delta == r#"{"query":"beta"}"#
    ));
    assert!(matches!(
        rx.recv().await,
        Some(StreamEvent::ToolUseInputDelta { id: Some(id), delta })
            if id == "call_a" && delta == r#"{"path":"alpha"}"#
    ));
    let Some(StreamEvent::Done(response)) = rx.recv().await else {
        panic!("expected done event");
    };
    let calls = response.tool_calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].id, "call_a");
    assert_eq!(calls[0].args, json!({"path": "alpha"}));
    assert_eq!(calls[1].id, "call_b");
    assert_eq!(calls[1].args, json!({"query": "beta"}));
    Ok(())
}

#[tokio::test]
#[ignore = "requires a live Codex login and consumes account quota"]
async fn real_gpt_5_6_sol_native_effort_tool_smoke() -> Result<()> {
    let client =
        CodexClient::from_codex_login_with_effort("gpt-5.6-sol", "a3s-sol-effort-smoke", "high")?;
    assert_eq!(client.configured_reasoning_effort(), Some("high"));
    let echo = ToolDefinition {
        name: "echo".to_string(),
        description: "Echo the supplied text".to_string(),
        parameters: json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
            "additionalProperties": false
        }),
    };

    let response = client
        .complete(
            &[Message::user(
                "Call the echo tool exactly once with the text `sol-ready`. Do not answer in plain text.",
            )],
            Some("Follow the user's tool instruction exactly."),
            &[echo],
        )
        .await?;

    let call_id = response
        .message
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolUse { id, name, input }
                if name == "echo" && input["text"] == "sol-ready" =>
            {
                Some(id.clone())
            }
            _ => None,
        })
        .expect("Sol should request the echo tool");

    let ultracode_client = client.with_a3s_effort("ultracode");
    assert_eq!(client.configured_reasoning_effort(), Some("high"));
    assert_eq!(ultracode_client.configured_reasoning_effort(), Some("max"));
    let final_response = ultracode_client
        .complete(
            &[
                Message::user(
                    "Call the echo tool exactly once with the text `sol-ready`. Do not answer in plain text.",
                ),
                response.message,
                Message::tool_result(&call_id, "sol-ready", false),
            ],
            Some("After the tool result, confirm it briefly."),
            &[tool()],
        )
        .await?;
    assert!(!final_response.message.text().trim().is_empty());
    assert!(!final_response
        .message
        .content
        .iter()
        .any(|block| matches!(block, ContentBlock::ToolUse { .. })));
    Ok(())
}
