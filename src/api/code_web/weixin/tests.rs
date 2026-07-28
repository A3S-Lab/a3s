use std::collections::{HashMap, VecDeque};
use std::future::pending;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use a3s_boot::{BootApplication, BootError, BootRequest, HttpMethod};
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::Mutex;

use super::credential_store::{CredentialStoreError, WeixinCredentialStore, WeixinCredentials};
use super::module::WeixinModule;
use super::monitor::{AlphaDisabledHandler, MonitorLifecycleState, WeixinMonitorSupervisor};
use super::runtime_store::WeixinRuntimeStore;
use crate::api::code_web::kernel::{ManagedQueueEvidence, ManagedSessionEvidence};
use crate::api::code_web::remote::RemoteAgentReadService;
use crate::system_agents::{
    AgentActivityConfidence, AgentActivityState, AgentVendor, SystemAgentActivity,
    SystemAgentSnapshot,
};
use a3s_boot::ilink::{
    CreateQrResponse, GetConfigResponse, GetUpdatesResponse, IlinkAuth, IlinkError,
    IlinkLoginTransport, IlinkMessagingTransport, NotifyResponse, PollQrResponse, QrCodeStatus,
    SecretValue, SendMessageResponse, SendTypingResponse, ValidatedBaseUrl,
};

struct ApiLoginTransport {
    base_url: ValidatedBaseUrl,
    responses: Mutex<VecDeque<PollQrResponse>>,
    verify_codes: Mutex<Vec<Option<String>>>,
}

impl ApiLoginTransport {
    fn new(responses: impl IntoIterator<Item = PollQrResponse>) -> Self {
        Self {
            base_url: ValidatedBaseUrl::insecure_loopback_for_tests("http://127.0.0.1:43125/")
                .unwrap(),
            responses: Mutex::new(responses.into_iter().collect()),
            verify_codes: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl IlinkLoginTransport for ApiLoginTransport {
    fn qr_base_url(&self) -> ValidatedBaseUrl {
        self.base_url.clone()
    }

    fn validate_account_base_url(&self, base_url: &str) -> Result<ValidatedBaseUrl, IlinkError> {
        if base_url == "http://127.0.0.1:43125/" {
            Ok(self.base_url.clone())
        } else {
            Err(IlinkError::InvalidResponse("account_base_url"))
        }
    }

    fn validate_redirect_host(&self, _redirect_host: &str) -> Result<ValidatedBaseUrl, IlinkError> {
        Err(IlinkError::InvalidResponse("redirect_host"))
    }

    async fn create_qr(
        &self,
        _local_tokens: &[SecretValue],
    ) -> Result<CreateQrResponse, IlinkError> {
        Ok(CreateQrResponse {
            qrcode: SecretValue::new("qr-code-api-canary").unwrap(),
            qrcode_img_content: SecretValue::new("weixin://qr-api-canary").unwrap(),
        })
    }

    async fn poll_qr(
        &self,
        _base_url: &ValidatedBaseUrl,
        _qrcode: &SecretValue,
        verify_code: Option<&SecretValue>,
    ) -> Result<PollQrResponse, IlinkError> {
        self.verify_codes
            .lock()
            .await
            .push(verify_code.map(|code| code.expose().to_string()));
        self.responses
            .lock()
            .await
            .pop_front()
            .ok_or(IlinkError::InvalidResponse("test_poll_sequence"))
    }
}

#[derive(Default)]
struct ApiCredentialStore {
    credentials: Mutex<Option<WeixinCredentials>>,
}

#[async_trait]
impl WeixinCredentialStore for ApiCredentialStore {
    async fn load(&self) -> Result<Option<WeixinCredentials>, CredentialStoreError> {
        Ok(self.credentials.lock().await.clone())
    }

    async fn save(&self, credentials: &WeixinCredentials) -> Result<(), CredentialStoreError> {
        *self.credentials.lock().await = Some(credentials.clone());
        Ok(())
    }

    async fn delete(&self) -> Result<(), CredentialStoreError> {
        self.credentials.lock().await.take();
        Ok(())
    }
}

struct ApiMessagingTransport {
    base_url: ValidatedBaseUrl,
    notify_start_calls: AtomicUsize,
    notify_stop_calls: AtomicUsize,
}

impl ApiMessagingTransport {
    fn new() -> Self {
        Self {
            base_url: ValidatedBaseUrl::insecure_loopback_for_tests("http://127.0.0.1:43125/")
                .unwrap(),
            notify_start_calls: AtomicUsize::new(0),
            notify_stop_calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl IlinkMessagingTransport for ApiMessagingTransport {
    fn validate_account_base_url(&self, base_url: &str) -> Result<ValidatedBaseUrl, IlinkError> {
        if base_url == "http://127.0.0.1:43125/" {
            Ok(self.base_url.clone())
        } else {
            Err(IlinkError::InvalidResponse("monitor_base_url"))
        }
    }

    async fn get_updates(
        &self,
        _auth: &IlinkAuth,
        _update_cursor: &str,
        _long_poll_timeout: Duration,
    ) -> Result<GetUpdatesResponse, IlinkError> {
        pending().await
    }

    async fn send_text(
        &self,
        _auth: &IlinkAuth,
        _recipient: &SecretValue,
        _context_token: Option<&SecretValue>,
        _client_id: &str,
        _run_id: Option<&str>,
        _text: &str,
    ) -> Result<SendMessageResponse, IlinkError> {
        Ok(SendMessageResponse::default())
    }

    async fn get_config(
        &self,
        _auth: &IlinkAuth,
        _owner_id: Option<&SecretValue>,
        _context_token: Option<&SecretValue>,
    ) -> Result<GetConfigResponse, IlinkError> {
        Ok(GetConfigResponse::default())
    }

    async fn send_typing(
        &self,
        _auth: &IlinkAuth,
        _owner_id: &SecretValue,
        _typing_ticket: &SecretValue,
        _status: i32,
    ) -> Result<SendTypingResponse, IlinkError> {
        Ok(SendTypingResponse::default())
    }

    async fn notify_start(&self, _auth: &IlinkAuth) -> Result<NotifyResponse, IlinkError> {
        self.notify_start_calls.fetch_add(1, Ordering::Relaxed);
        Ok(NotifyResponse::default())
    }

    async fn notify_stop(&self, _auth: &IlinkAuth) -> Result<NotifyResponse, IlinkError> {
        self.notify_stop_calls.fetch_add(1, Ordering::Relaxed);
        Ok(NotifyResponse::default())
    }
}

fn verification_required() -> PollQrResponse {
    PollQrResponse {
        status: QrCodeStatus::NeedVerifycode,
        bot_token: None,
        ilink_bot_id: None,
        ilink_user_id: None,
        baseurl: None,
        redirect_host: None,
    }
}

fn confirmed() -> PollQrResponse {
    PollQrResponse {
        status: QrCodeStatus::Confirmed,
        bot_token: Some(SecretValue::new("api-bot-token-canary").unwrap()),
        ilink_bot_id: Some(SecretValue::new("api-bot-id-canary").unwrap()),
        ilink_user_id: Some(SecretValue::new("api-owner-id-canary").unwrap()),
        baseurl: Some("http://127.0.0.1:43125/".to_string()),
        redirect_host: None,
    }
}

fn json_request(
    method: HttpMethod,
    path: impl Into<String>,
    value: serde_json::Value,
) -> BootRequest {
    BootRequest::new(method, path)
        .with_content_type("application/json")
        .with_body(serde_json::to_vec(&value).unwrap())
}

#[tokio::test]
async fn weixin_capability_route_reports_an_explicitly_disabled_runtime() {
    let app = BootApplication::builder()
        .global_prefix("/api")
        .import(WeixinModule::disabled_isolated())
        .build()
        .expect("build Weixin test application");

    let response = app
        .call(BootRequest::new(
            HttpMethod::Get,
            "/api/v1/weixin/capability",
        ))
        .await
        .expect("read Weixin capability");

    assert_eq!(response.status(), 200);
    let body = response
        .body_json::<serde_json::Value>()
        .expect("Weixin capability JSON");
    assert_eq!(
        body,
        json!({
            "schemaVersion": 2,
            "state": "unavailable",
            "protocolMode": "disabled",
            "supportedScopes": [],
            "releaseBlockers": [{
                "code": "ilink_channel_unavailable",
                "message": "The Weixin iLink channel is not enabled in this runtime."
            }]
        })
    );
    let serialized = serde_json::to_string(&body).expect("serialize capability response");
    for forbidden in [
        "bot_token",
        "context_token",
        "ilink_user_id",
        "get_updates_buf",
        "baseurl",
    ] {
        assert!(!serialized.contains(forbidden));
    }

    let error = app
        .call(BootRequest::new(HttpMethod::Get, "/api/v1/weixin/account"))
        .await
        .expect_err("disabled runtime must reject account access");
    assert!(matches!(error, BootError::ServiceUnavailable(_)));

    let error = app
        .call(json_request(
            HttpMethod::Post,
            "/api/v1/weixin/login-attempts",
            json!({}),
        ))
        .await
        .expect_err("disabled runtime must reject QR creation");
    assert!(matches!(error, BootError::ServiceUnavailable(_)));
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_production_runtime_exposes_qr_binding() {
    let temporary = tempfile::tempdir().expect("create production Weixin fixture");
    let root = std::fs::canonicalize(temporary.path()).expect("canonicalize fixture root");
    let remote = Arc::new(RemoteAgentReadService::for_test(
        Vec::new(),
        HashMap::new(),
        SystemAgentSnapshot {
            activities: Vec::new(),
            warnings: Vec::new(),
        },
    ));
    let app = BootApplication::builder()
        .global_prefix("/api")
        .import(WeixinModule::production_for_test(
            Arc::new(ApiLoginTransport::new([])),
            Arc::new(ApiMessagingTransport::new()),
            Arc::new(ApiCredentialStore::default()),
            remote,
            root.join("runtime"),
        ))
        .build()
        .expect("build production Weixin application");

    app.bootstrap()
        .await
        .expect("bootstrap production Weixin application");
    let capability = app
        .call(BootRequest::new(
            HttpMethod::Get,
            "/api/v1/weixin/capability",
        ))
        .await
        .expect("read production capability")
        .body_json::<serde_json::Value>()
        .expect("decode production capability");
    assert_eq!(capability["state"], "unbound");
    assert_eq!(capability["protocolMode"], "tencent");
    assert_eq!(capability["schemaVersion"], 2);
    assert_eq!(
        capability["supportedScopes"],
        json!(["agents.read", "sessions.read"])
    );
    assert_eq!(capability["releaseBlockers"], json!([]));

    let attempt = app
        .call(json_request(
            HttpMethod::Post,
            "/api/v1/weixin/login-attempts",
            json!({}),
        ))
        .await
        .expect("create production QR attempt")
        .body_json::<serde_json::Value>()
        .expect("decode production QR attempt");
    assert_eq!(attempt["state"], "waitingForScan");
    assert_eq!(attempt["qrContent"], "weixin://qr-api-canary");

    app.shutdown()
        .await
        .expect("shutdown production Weixin application");
}

#[tokio::test]
async fn weixin_mock_targets_api_exposes_only_sanitized_read_only_inventory() {
    let remote = Arc::new(RemoteAgentReadService::for_test(
        vec![ManagedSessionEvidence {
            source_id: "managed-session-secret".to_string(),
            title: Some("Build remote panel".to_string()),
            workspace: "/Users/alice/private-workspace".to_string(),
            created_at_ms: 1,
            updated_at_ms: 2,
            goal: None,
            queue: ManagedQueueEvidence {
                pending_turns: 1,
                active: false,
                paused: false,
            },
            children: Vec::new(),
        }],
        HashMap::new(),
        SystemAgentSnapshot {
            activities: vec![
                SystemAgentActivity {
                    id: "exact-presence-secret".to_string(),
                    parent_id: None,
                    agent: "a3s-code".to_string(),
                    workspace: Some("/Users/alice/cooperative".to_string()),
                    task: Some("Review the remote target API".to_string()),
                    reason: None,
                    state: AgentActivityState::Working,
                    confidence: AgentActivityConfidence::Exact,
                    vendor: AgentVendor::A3s,
                    started_at_ms: Some(1),
                    finished_at_ms: None,
                    expires_at_ms: u64::MAX,
                    actions: Vec::new(),
                    local: false,
                },
                SystemAgentActivity {
                    id: "process:4242".to_string(),
                    parent_id: None,
                    agent: "codex".to_string(),
                    workspace: Some("/Users/alice/observed".to_string()),
                    task: Some("inferred process evidence".to_string()),
                    reason: None,
                    state: AgentActivityState::Unknown,
                    confidence: AgentActivityConfidence::Process,
                    vendor: AgentVendor::OpenAi,
                    started_at_ms: Some(1),
                    finished_at_ms: None,
                    expires_at_ms: u64::MAX,
                    actions: Vec::new(),
                    local: false,
                },
            ],
            warnings: Vec::new(),
        },
    ));
    let app = BootApplication::builder()
        .global_prefix("/api")
        .import(WeixinModule::mock_with_remote(
            Arc::new(ApiLoginTransport::new([])),
            Arc::new(ApiCredentialStore::default()),
            remote,
        ))
        .build()
        .expect("build mock Weixin remote application");

    let capability = app
        .call(BootRequest::new(
            HttpMethod::Get,
            "/api/v1/weixin/capability",
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(
        capability["supportedScopes"],
        json!(["agents.read", "sessions.read"])
    );

    let response = app
        .call(BootRequest::new(HttpMethod::Get, "/api/v1/weixin/targets"))
        .await
        .expect("read sanitized remote targets");
    assert_eq!(response.status(), 200);
    let body = response.body_json::<serde_json::Value>().unwrap();
    assert_eq!(body["schemaVersion"], 1);
    assert_eq!(
        body["totals"],
        json!({ "managed": 1, "cooperative": 1, "observed": 1 })
    );
    let items = body["items"].as_array().expect("remote target items");
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|target| target["id"]
        .as_str()
        .is_some_and(|id| id.starts_with("rt") && id.len() == 28)));
    let observed = items
        .iter()
        .find(|target| target["kind"] == "observedProcess")
        .expect("observed target");
    assert_eq!(observed["capabilities"], json!(["readStatus"]));
    assert_eq!(observed["state"], "detected");

    let serialized = serde_json::to_string(&body).unwrap();
    for forbidden in [
        "managed-session-secret",
        "exact-presence-secret",
        "4242",
        "/Users/alice",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }
}

#[tokio::test]
async fn weixin_mock_login_api_binds_and_disconnects_without_exposing_credentials() {
    let transport = Arc::new(ApiLoginTransport::new([
        verification_required(),
        confirmed(),
    ]));
    let credential_store = Arc::new(ApiCredentialStore::default());
    let app = BootApplication::builder()
        .global_prefix("/api")
        .import(WeixinModule::mock(
            transport.clone(),
            credential_store.clone(),
        ))
        .build()
        .expect("build mock Weixin application");

    let capability = app
        .call(BootRequest::new(
            HttpMethod::Get,
            "/api/v1/weixin/capability",
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(capability["state"], "unbound");
    assert_eq!(capability["protocolMode"], "mock");
    assert_eq!(capability["schemaVersion"], 2);

    let started = app
        .call(json_request(
            HttpMethod::Post,
            "/api/v1/weixin/login-attempts",
            json!({}),
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(started["state"], "waitingForScan");
    assert_eq!(started["qrContent"], "weixin://qr-api-canary");
    let attempt_id = started["attemptId"].as_str().unwrap();

    let verification = app
        .call(BootRequest::new(
            HttpMethod::Get,
            format!("/api/v1/weixin/login-attempts/{attempt_id}"),
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(verification["state"], "verificationRequired");

    let submitted = app
        .call(json_request(
            HttpMethod::Post,
            format!("/api/v1/weixin/login-attempts/{attempt_id}/verification"),
            json!({ "code": "123456" }),
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(submitted["state"], "verificationSubmitted");

    let connected = app
        .call(BootRequest::new(
            HttpMethod::Get,
            format!("/api/v1/weixin/login-attempts/{attempt_id}"),
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(connected["state"], "connected");
    assert!(connected["qrContent"].is_null());

    let account = app
        .call(BootRequest::new(HttpMethod::Get, "/api/v1/weixin/account"))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(account["state"], "paused");
    assert_eq!(account["protocolMode"], "mock");
    assert_eq!(account["bound"], true);
    assert_eq!(account["monitorState"], "paused");
    assert!(account["ownerLabel"]
        .as_str()
        .is_some_and(|label| label.starts_with("WeChat owner • ")));
    assert_eq!(
        transport.verify_codes.lock().await.as_slice(),
        &[None, Some("123456".to_string())]
    );

    let serialized = serde_json::to_string(&account).unwrap();
    for forbidden in [
        "api-bot-token-canary",
        "api-bot-id-canary",
        "api-owner-id-canary",
        "127.0.0.1",
        "qr-code-api-canary",
    ] {
        assert!(!serialized.contains(forbidden));
    }

    let disconnected = app
        .call(BootRequest::new(
            HttpMethod::Delete,
            "/api/v1/weixin/account",
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(disconnected["state"], "unbound");
    assert_eq!(disconnected["bound"], false);
    assert_eq!(disconnected["monitorState"], "stopped");
    assert!(credential_store.credentials.lock().await.is_none());
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_mock_monitor_api_bootstraps_pauses_resumes_and_disconnects() {
    let credential_store = Arc::new(ApiCredentialStore {
        credentials: Mutex::new(Some(
            WeixinCredentials::new(
                SecretValue::new("monitor-api-token-canary").unwrap(),
                SecretValue::new("monitor-api-bot-canary").unwrap(),
                SecretValue::new("monitor-api-owner-canary").unwrap(),
                "http://127.0.0.1:43125/",
                None,
            )
            .unwrap(),
        )),
    });
    let messaging_transport = Arc::new(ApiMessagingTransport::new());
    let temporary = tempfile::tempdir().unwrap();
    let temporary_root = std::fs::canonicalize(temporary.path()).unwrap();
    let runtime_store = WeixinRuntimeStore::open(temporary_root.join("runtime"))
        .await
        .unwrap();
    let monitor = Arc::new(WeixinMonitorSupervisor::for_test(
        messaging_transport.clone(),
        credential_store.clone(),
        runtime_store.clone(),
        Arc::new(AlphaDisabledHandler),
        Duration::from_millis(1),
        Duration::from_millis(5),
    ));
    let app = BootApplication::builder()
        .global_prefix("/api")
        .import(WeixinModule::mock_with_monitor(
            Arc::new(ApiLoginTransport::new([])),
            credential_store.clone(),
            monitor.clone(),
            runtime_store.clone(),
        ))
        .build()
        .expect("build monitored Weixin application");

    app.bootstrap().await.unwrap();
    wait_until(|| monitor.health().state == MonitorLifecycleState::Running).await;
    assert_eq!(
        messaging_transport
            .notify_start_calls
            .load(Ordering::Relaxed),
        1
    );

    let running = app
        .call(BootRequest::new(HttpMethod::Get, "/api/v1/weixin/account"))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(running["state"], "active");
    assert_eq!(running["monitorState"], "running");
    assert_eq!(running["mutationsEnabled"], false);

    let paused = app
        .call(json_request(
            HttpMethod::Post,
            "/api/v1/weixin/account/pause",
            json!({}),
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(paused["state"], "paused");
    assert_eq!(paused["monitorState"], "paused");
    assert_eq!(
        messaging_transport
            .notify_stop_calls
            .load(Ordering::Relaxed),
        1
    );

    let resumed = app
        .call(json_request(
            HttpMethod::Post,
            "/api/v1/weixin/account/resume",
            json!({}),
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(resumed["monitorState"], "starting");
    wait_until(|| monitor.health().state == MonitorLifecycleState::Running).await;
    assert_eq!(
        messaging_transport
            .notify_start_calls
            .load(Ordering::Relaxed),
        2
    );

    let disconnected = app
        .call(BootRequest::new(
            HttpMethod::Delete,
            "/api/v1/weixin/account",
        ))
        .await
        .unwrap()
        .body_json::<serde_json::Value>()
        .unwrap();
    assert_eq!(disconnected["state"], "unbound");
    assert_eq!(disconnected["bound"], false);
    assert_eq!(disconnected["monitorState"], "stopped");
    assert!(credential_store.credentials.lock().await.is_none());
    let checkpoint = runtime_store.checkpoint().await;
    assert!(checkpoint.inbox.is_empty());
    assert!(checkpoint.outbox.is_empty());

    let serialized = serde_json::to_string(&running).unwrap();
    for forbidden in [
        "monitor-api-token-canary",
        "monitor-api-bot-canary",
        "monitor-api-owner-canary",
        "127.0.0.1",
    ] {
        assert!(!serialized.contains(forbidden));
    }
    app.shutdown().await.unwrap();
}

async fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !predicate() {
        assert!(Instant::now() < deadline, "condition did not become true");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
