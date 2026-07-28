use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::credential_store::{CredentialStoreError, WeixinCredentialStore, WeixinCredentials};
use super::login_coordinator::{WeixinLoginCoordinator, WeixinLoginError, WeixinLoginState};
use a3s_boot::ilink::{
    CreateQrResponse, IlinkError, IlinkLoginTransport, PollQrResponse, QrCodeStatus, SecretValue,
    ValidatedBaseUrl,
};

struct FakeLoginTransport {
    initial_base_url: ValidatedBaseUrl,
    redirect_base_url: ValidatedBaseUrl,
    account_base_url: String,
    redirect_host: String,
    responses: Mutex<VecDeque<PollQrResponse>>,
    calls: Mutex<Vec<PollCall>>,
    create_count: AtomicUsize,
}

struct PollCall {
    base_url: ValidatedBaseUrl,
    qr_code: String,
    verify_code: Option<String>,
}

impl FakeLoginTransport {
    fn new(responses: impl IntoIterator<Item = PollQrResponse>) -> Self {
        let account_base_url = "http://127.0.0.1:43123/".to_string();
        let redirect_origin = "http://127.0.0.1:43124/";
        Self {
            initial_base_url: ValidatedBaseUrl::insecure_loopback_for_tests(&account_base_url)
                .unwrap(),
            redirect_base_url: ValidatedBaseUrl::insecure_loopback_for_tests(redirect_origin)
                .unwrap(),
            account_base_url,
            redirect_host: "127.0.0.1:43124".to_string(),
            responses: Mutex::new(responses.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
            create_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl IlinkLoginTransport for FakeLoginTransport {
    fn qr_base_url(&self) -> ValidatedBaseUrl {
        self.initial_base_url.clone()
    }

    fn validate_account_base_url(&self, base_url: &str) -> Result<ValidatedBaseUrl, IlinkError> {
        if base_url == self.account_base_url {
            Ok(self.initial_base_url.clone())
        } else {
            Err(IlinkError::InvalidResponse("account_base_url"))
        }
    }

    fn validate_redirect_host(&self, redirect_host: &str) -> Result<ValidatedBaseUrl, IlinkError> {
        if redirect_host == self.redirect_host {
            Ok(self.redirect_base_url.clone())
        } else {
            Err(IlinkError::InvalidResponse("redirect_host"))
        }
    }

    async fn create_qr(
        &self,
        _local_tokens: &[SecretValue],
    ) -> Result<CreateQrResponse, IlinkError> {
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(CreateQrResponse {
            qrcode: SecretValue::new("qr-code-canary").unwrap(),
            qrcode_img_content: SecretValue::new("weixin://qr-content-canary").unwrap(),
        })
    }

    async fn poll_qr(
        &self,
        base_url: &ValidatedBaseUrl,
        qrcode: &SecretValue,
        verify_code: Option<&SecretValue>,
    ) -> Result<PollQrResponse, IlinkError> {
        self.calls.lock().await.push(PollCall {
            base_url: base_url.clone(),
            qr_code: qrcode.expose().to_string(),
            verify_code: verify_code.map(|code| code.expose().to_string()),
        });
        Ok(self
            .responses
            .lock()
            .await
            .pop_front()
            .unwrap_or_else(|| poll_response(QrCodeStatus::Wait)))
    }
}

#[derive(Default)]
struct MemoryCredentialStore {
    credentials: Mutex<Option<WeixinCredentials>>,
}

#[async_trait]
impl WeixinCredentialStore for MemoryCredentialStore {
    async fn load(&self) -> Result<Option<WeixinCredentials>, CredentialStoreError> {
        Ok(self.credentials.lock().await.clone())
    }

    async fn save(&self, credentials: &WeixinCredentials) -> Result<(), CredentialStoreError> {
        *self.credentials.lock().await = Some(credentials.clone());
        Ok(())
    }

    async fn delete(&self) -> Result<(), CredentialStoreError> {
        *self.credentials.lock().await = None;
        Ok(())
    }
}

#[derive(Default)]
struct FailingSaveCredentialStore;

#[async_trait]
impl WeixinCredentialStore for FailingSaveCredentialStore {
    async fn load(&self) -> Result<Option<WeixinCredentials>, CredentialStoreError> {
        Ok(None)
    }

    async fn save(&self, _credentials: &WeixinCredentials) -> Result<(), CredentialStoreError> {
        Err(CredentialStoreError::InvalidCredential)
    }

    async fn delete(&self) -> Result<(), CredentialStoreError> {
        Ok(())
    }
}

fn poll_response(status: QrCodeStatus) -> PollQrResponse {
    PollQrResponse {
        status,
        bot_token: None,
        ilink_bot_id: None,
        ilink_user_id: None,
        baseurl: None,
        redirect_host: None,
    }
}

fn confirmed_response(base_url: &str) -> PollQrResponse {
    PollQrResponse {
        status: QrCodeStatus::Confirmed,
        bot_token: Some(SecretValue::new("bot-token-canary").unwrap()),
        ilink_bot_id: Some(SecretValue::new("bot-id-canary").unwrap()),
        ilink_user_id: Some(SecretValue::new("owner-id-canary").unwrap()),
        baseurl: Some(base_url.to_string()),
        redirect_host: None,
    }
}

#[tokio::test]
async fn weixin_login_coordinator_reuses_attempt_and_persists_confirmation() {
    let transport = Arc::new(FakeLoginTransport::new([confirmed_response(
        "http://127.0.0.1:43123/",
    )]));
    let credential_store = Arc::new(MemoryCredentialStore::default());
    let coordinator = WeixinLoginCoordinator::for_test(
        transport.clone(),
        credential_store.clone(),
        Duration::from_secs(300),
    );

    let started = coordinator.start(false).await.unwrap();
    assert_eq!(started.state, WeixinLoginState::WaitingForScan);
    assert_eq!(
        started.qr_content.as_ref().map(SecretValue::expose),
        Some("weixin://qr-content-canary")
    );
    let rendered = format!("{started:?}");
    assert!(!rendered.contains("qr-content-canary"));

    let reused = coordinator.start(false).await.unwrap();
    assert_eq!(reused.attempt_id, started.attempt_id);
    assert_eq!(transport.create_count.load(Ordering::Relaxed), 1);

    let connected = coordinator.poll(&started.attempt_id).await.unwrap();
    assert_eq!(connected.state, WeixinLoginState::Connected);
    assert!(connected.qr_content.is_none());
    let saved = credential_store.credentials.lock().await.clone().unwrap();
    assert_eq!(saved.bot_token.expose(), "bot-token-canary");
    assert_eq!(saved.bot_id.expose(), "bot-id-canary");
    assert_eq!(saved.owner_id.expose(), "owner-id-canary");
    assert!(matches!(
        coordinator.start(false).await,
        Err(WeixinLoginError::AlreadyBound)
    ));

    coordinator.disconnect().await.unwrap();
    assert!(credential_store.credentials.lock().await.is_none());
}

#[tokio::test]
async fn weixin_login_coordinator_handles_verification_redirect_and_acceptance() {
    let mut redirected = poll_response(QrCodeStatus::ScanedButRedirect);
    redirected.redirect_host = Some("127.0.0.1:43124".to_string());
    let transport = Arc::new(FakeLoginTransport::new([
        poll_response(QrCodeStatus::NeedVerifycode),
        redirected,
        poll_response(QrCodeStatus::Scaned),
        poll_response(QrCodeStatus::Wait),
    ]));
    let credential_store = Arc::new(MemoryCredentialStore::default());
    let coordinator = WeixinLoginCoordinator::for_test(
        transport.clone(),
        credential_store,
        Duration::from_secs(300),
    );
    let started = coordinator.start(false).await.unwrap();

    let verification = coordinator.poll(&started.attempt_id).await.unwrap();
    assert_eq!(verification.state, WeixinLoginState::VerificationRequired);
    assert!(matches!(
        coordinator
            .submit_verify_code(&started.attempt_id, "12x456")
            .await,
        Err(WeixinLoginError::InvalidVerifyCode)
    ));
    let submitted = coordinator
        .submit_verify_code(&started.attempt_id, "123456")
        .await
        .unwrap();
    assert_eq!(submitted.state, WeixinLoginState::VerificationSubmitted);
    assert_eq!(submitted.verify_submissions, 1);

    let redirected = coordinator.poll(&started.attempt_id).await.unwrap();
    assert_eq!(redirected.state, WeixinLoginState::Redirected);
    let scanned = coordinator.poll(&started.attempt_id).await.unwrap();
    assert_eq!(scanned.state, WeixinLoginState::Scanned);
    let waiting = coordinator.poll(&started.attempt_id).await.unwrap();
    assert_eq!(waiting.state, WeixinLoginState::WaitingForScan);

    let calls = transport.calls.lock().await;
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0].verify_code, None);
    assert_eq!(calls[1].verify_code.as_deref(), Some("123456"));
    assert_eq!(calls[2].verify_code.as_deref(), Some("123456"));
    assert_eq!(calls[3].verify_code, None);
    assert_eq!(calls[0].base_url, transport.initial_base_url);
    assert_eq!(calls[2].base_url, transport.redirect_base_url);
    assert!(calls.iter().all(|call| call.qr_code == "qr-code-canary"));
}

#[tokio::test]
async fn weixin_login_coordinator_expires_cancels_and_rejects_missing_bound_state() {
    let transport = Arc::new(FakeLoginTransport::new([poll_response(
        QrCodeStatus::BindedRedirect,
    )]));
    let credential_store = Arc::new(MemoryCredentialStore::default());
    let coordinator =
        WeixinLoginCoordinator::for_test(transport, credential_store, Duration::from_millis(1));

    let started = coordinator.start(false).await.unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    let expired = coordinator.current(&started.attempt_id).await.unwrap();
    assert_eq!(expired.state, WeixinLoginState::Expired);
    assert!(matches!(
        coordinator.current(&started.attempt_id).await,
        Err(WeixinLoginError::AttemptNotFound)
    ));

    let next = coordinator.start(false).await.unwrap();
    assert!(matches!(
        coordinator.poll(&next.attempt_id).await,
        Err(WeixinLoginError::MissingExistingCredential)
    ));
    let cancelled = coordinator.start(false).await.unwrap();
    coordinator.cancel(&cancelled.attempt_id).await.unwrap();
    assert!(matches!(
        coordinator.current(&cancelled.attempt_id).await,
        Err(WeixinLoginError::AttemptNotFound)
    ));
}

#[tokio::test]
async fn weixin_login_coordinator_serializes_concurrent_starts() {
    let transport = Arc::new(FakeLoginTransport::new([]));
    let coordinator = Arc::new(WeixinLoginCoordinator::for_test(
        transport.clone(),
        Arc::new(MemoryCredentialStore::default()),
        Duration::from_secs(300),
    ));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let coordinator = Arc::clone(&coordinator);
        tasks.spawn(async move { coordinator.start(false).await.unwrap().attempt_id });
    }

    let mut attempt_ids = Vec::new();
    while let Some(result) = tasks.join_next().await {
        attempt_ids.push(result.unwrap());
    }
    assert_eq!(attempt_ids.len(), 16);
    assert!(attempt_ids
        .iter()
        .all(|attempt_id| attempt_id == &attempt_ids[0]));
    assert_eq!(transport.create_count.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn weixin_login_coordinator_drops_qr_material_after_credential_save_failure() {
    let transport = Arc::new(FakeLoginTransport::new([confirmed_response(
        "http://127.0.0.1:43123/",
    )]));
    let coordinator = WeixinLoginCoordinator::for_test(
        transport.clone(),
        Arc::new(FailingSaveCredentialStore),
        Duration::from_secs(300),
    );
    let started = coordinator.start(false).await.unwrap();

    assert!(matches!(
        coordinator.poll(&started.attempt_id).await,
        Err(WeixinLoginError::Credential(
            CredentialStoreError::InvalidCredential
        ))
    ));
    assert!(matches!(
        coordinator.current(&started.attempt_id).await,
        Err(WeixinLoginError::AttemptNotFound)
    ));

    let replacement = coordinator.start(false).await.unwrap();
    assert_ne!(replacement.attempt_id, started.attempt_id);
    assert_eq!(transport.create_count.load(Ordering::Relaxed), 2);
}
