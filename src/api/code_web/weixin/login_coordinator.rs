use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;

use super::credential_store::{CredentialStoreError, WeixinCredentialStore, WeixinCredentials};
use a3s_boot::ilink::{
    IlinkError, IlinkLoginTransport, PollQrResponse, QrCodeStatus, SecretValue, ValidatedBaseUrl,
};

const DEFAULT_LOGIN_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_VERIFY_SUBMISSIONS: u8 = 3;
const MAX_VERIFY_CODE_DIGITS: usize = 12;

pub(super) struct WeixinLoginCoordinator {
    transport: Arc<dyn IlinkLoginTransport>,
    credential_store: Arc<dyn WeixinCredentialStore>,
    attempt: Mutex<Option<LoginAttempt>>,
    operation_lock: Mutex<()>,
    attempt_ttl: Duration,
}

impl WeixinLoginCoordinator {
    pub(super) fn new(
        transport: Arc<dyn IlinkLoginTransport>,
        credential_store: Arc<dyn WeixinCredentialStore>,
    ) -> Self {
        Self::with_ttl(transport, credential_store, DEFAULT_LOGIN_TTL)
    }

    fn with_ttl(
        transport: Arc<dyn IlinkLoginTransport>,
        credential_store: Arc<dyn WeixinCredentialStore>,
        attempt_ttl: Duration,
    ) -> Self {
        Self {
            transport,
            credential_store,
            attempt: Mutex::new(None),
            operation_lock: Mutex::new(()),
            attempt_ttl,
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(
        transport: Arc<dyn IlinkLoginTransport>,
        credential_store: Arc<dyn WeixinCredentialStore>,
        attempt_ttl: Duration,
    ) -> Self {
        Self::with_ttl(transport, credential_store, attempt_ttl)
    }

    pub(super) async fn start(&self, force: bool) -> Result<WeixinLoginAttempt, WeixinLoginError> {
        let _operation = self.operation_lock.lock().await;
        if self.credential_store.load().await?.is_some() {
            return Err(WeixinLoginError::AlreadyBound);
        }

        let now = Instant::now();
        {
            let mut attempt = self.attempt.lock().await;
            if attempt
                .as_ref()
                .is_some_and(|current| current.expires_at <= now)
            {
                attempt.take();
            }
            if !force {
                if let Some(current) = attempt.as_ref() {
                    return Ok(current.snapshot(now));
                }
            }
        }

        let response = self.transport.create_qr(&[]).await?;
        let started_at = Instant::now();
        let attempt = LoginAttempt {
            id: random_attempt_id(),
            qr_code: response.qrcode,
            qr_content: response.qrcode_img_content,
            poll_base_url: self.transport.qr_base_url(),
            state: WeixinLoginState::WaitingForScan,
            expires_at: started_at + self.attempt_ttl,
            verify_code: None,
            verify_submissions: 0,
        };
        let snapshot = attempt.snapshot(started_at);
        *self.attempt.lock().await = Some(attempt);
        Ok(snapshot)
    }

    pub(super) async fn poll(
        &self,
        attempt_id: &str,
    ) -> Result<WeixinLoginAttempt, WeixinLoginError> {
        let _operation = self.operation_lock.lock().await;
        let now = Instant::now();
        let (base_url, qr_code, verify_code) = {
            let mut active = self.attempt.lock().await;
            let current = active.as_ref().ok_or(WeixinLoginError::AttemptNotFound)?;
            ensure_attempt(current, attempt_id)?;
            if current.expires_at <= now {
                let expired = terminal_snapshot(current, WeixinLoginState::Expired);
                active.take();
                return Ok(expired);
            }
            (
                current.poll_base_url.clone(),
                current.qr_code.clone(),
                current.verify_code.clone(),
            )
        };

        let response = self
            .transport
            .poll_qr(&base_url, &qr_code, verify_code.as_ref())
            .await?;
        self.apply_poll_response(attempt_id, response).await
    }

    pub(super) async fn submit_verify_code(
        &self,
        attempt_id: &str,
        code: &str,
    ) -> Result<WeixinLoginAttempt, WeixinLoginError> {
        let _operation = self.operation_lock.lock().await;
        if code.is_empty()
            || code.len() > MAX_VERIFY_CODE_DIGITS
            || !code.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(WeixinLoginError::InvalidVerifyCode);
        }
        let now = Instant::now();
        let mut active = self.attempt.lock().await;
        let current = active.as_mut().ok_or(WeixinLoginError::AttemptNotFound)?;
        ensure_attempt(current, attempt_id)?;
        if current.expires_at <= now {
            let expired = terminal_snapshot(current, WeixinLoginState::Expired);
            active.take();
            return Ok(expired);
        }
        if current.state != WeixinLoginState::VerificationRequired {
            return Err(WeixinLoginError::VerificationNotRequested);
        }
        if current.verify_submissions >= MAX_VERIFY_SUBMISSIONS {
            return Err(WeixinLoginError::VerifyLimitReached);
        }
        current.verify_code = Some(
            SecretValue::new(code.to_string()).map_err(|_| WeixinLoginError::InvalidVerifyCode)?,
        );
        current.verify_submissions += 1;
        current.state = WeixinLoginState::VerificationSubmitted;
        Ok(current.snapshot(now))
    }

    pub(super) async fn cancel(&self, attempt_id: &str) -> Result<(), WeixinLoginError> {
        let _operation = self.operation_lock.lock().await;
        let mut active = self.attempt.lock().await;
        let current = active.as_ref().ok_or(WeixinLoginError::AttemptNotFound)?;
        ensure_attempt(current, attempt_id)?;
        active.take();
        Ok(())
    }

    #[cfg(test)]
    pub(super) async fn current(
        &self,
        attempt_id: &str,
    ) -> Result<WeixinLoginAttempt, WeixinLoginError> {
        let _operation = self.operation_lock.lock().await;
        let now = Instant::now();
        let mut active = self.attempt.lock().await;
        let current = active.as_ref().ok_or(WeixinLoginError::AttemptNotFound)?;
        ensure_attempt(current, attempt_id)?;
        if current.expires_at <= now {
            let expired = terminal_snapshot(current, WeixinLoginState::Expired);
            active.take();
            return Ok(expired);
        }
        Ok(current.snapshot(now))
    }

    pub(super) async fn disconnect(&self) -> Result<(), WeixinLoginError> {
        let _operation = self.operation_lock.lock().await;
        self.credential_store.delete().await?;
        self.attempt.lock().await.take();
        Ok(())
    }

    async fn apply_poll_response(
        &self,
        attempt_id: &str,
        response: PollQrResponse,
    ) -> Result<WeixinLoginAttempt, WeixinLoginError> {
        match response.status {
            QrCodeStatus::Confirmed => self.confirm(attempt_id, response).await,
            QrCodeStatus::BindedRedirect => self.already_bound(attempt_id).await,
            QrCodeStatus::Expired => self.finish(attempt_id, WeixinLoginState::Expired).await,
            QrCodeStatus::VerifyCodeBlocked => {
                self.finish(attempt_id, WeixinLoginState::VerificationBlocked)
                    .await
            }
            status => {
                let now = Instant::now();
                let mut active = self.attempt.lock().await;
                let current = active.as_mut().ok_or(WeixinLoginError::AttemptSuperseded)?;
                ensure_attempt(current, attempt_id)
                    .map_err(|_| WeixinLoginError::AttemptSuperseded)?;
                match status {
                    QrCodeStatus::Wait => current.state = WeixinLoginState::WaitingForScan,
                    QrCodeStatus::Scaned => {
                        current.verify_code = None;
                        current.state = WeixinLoginState::Scanned;
                    }
                    QrCodeStatus::NeedVerifycode => {
                        current.state = WeixinLoginState::VerificationRequired;
                    }
                    QrCodeStatus::ScanedButRedirect => {
                        let redirect_host = response
                            .redirect_host
                            .as_deref()
                            .ok_or(WeixinLoginError::InvalidProtocolState)?;
                        current.poll_base_url =
                            self.transport.validate_redirect_host(redirect_host)?;
                        current.state = WeixinLoginState::Redirected;
                    }
                    QrCodeStatus::Unknown
                    | QrCodeStatus::Confirmed
                    | QrCodeStatus::Expired
                    | QrCodeStatus::VerifyCodeBlocked
                    | QrCodeStatus::BindedRedirect => {
                        return Err(WeixinLoginError::InvalidProtocolState);
                    }
                }
                Ok(current.snapshot(now))
            }
        }
    }

    async fn confirm(
        &self,
        attempt_id: &str,
        mut response: PollQrResponse,
    ) -> Result<WeixinLoginAttempt, WeixinLoginError> {
        let bot_token = response
            .bot_token
            .take()
            .ok_or(WeixinLoginError::InvalidProtocolState)?;
        let bot_id = response
            .ilink_bot_id
            .take()
            .ok_or(WeixinLoginError::InvalidProtocolState)?;
        let owner_id = response
            .ilink_user_id
            .take()
            .ok_or(WeixinLoginError::InvalidProtocolState)?;
        let base_url = response
            .baseurl
            .take()
            .ok_or(WeixinLoginError::InvalidProtocolState)?;
        self.transport.validate_account_base_url(&base_url)?;
        let credentials = WeixinCredentials::new(bot_token, bot_id, owner_id, base_url, None)?;
        if let Err(error) = self.credential_store.save(&credentials).await {
            self.attempt.lock().await.take();
            return Err(error.into());
        }
        self.finish(attempt_id, WeixinLoginState::Connected).await
    }

    async fn already_bound(
        &self,
        attempt_id: &str,
    ) -> Result<WeixinLoginAttempt, WeixinLoginError> {
        if self.credential_store.load().await?.is_none() {
            self.attempt.lock().await.take();
            return Err(WeixinLoginError::MissingExistingCredential);
        }
        self.finish(attempt_id, WeixinLoginState::AlreadyBound)
            .await
    }

    async fn finish(
        &self,
        attempt_id: &str,
        state: WeixinLoginState,
    ) -> Result<WeixinLoginAttempt, WeixinLoginError> {
        let mut active = self.attempt.lock().await;
        let current = active.as_ref().ok_or(WeixinLoginError::AttemptSuperseded)?;
        ensure_attempt(current, attempt_id).map_err(|_| WeixinLoginError::AttemptSuperseded)?;
        let snapshot = terminal_snapshot(current, state);
        active.take();
        Ok(snapshot)
    }
}

struct LoginAttempt {
    id: String,
    qr_code: SecretValue,
    qr_content: SecretValue,
    poll_base_url: ValidatedBaseUrl,
    state: WeixinLoginState,
    expires_at: Instant,
    verify_code: Option<SecretValue>,
    verify_submissions: u8,
}

impl LoginAttempt {
    fn snapshot(&self, now: Instant) -> WeixinLoginAttempt {
        WeixinLoginAttempt {
            schema_version: 1,
            attempt_id: self.id.clone(),
            state: self.state,
            qr_content: Some(self.qr_content.clone()),
            expires_in_seconds: remaining_seconds(self.expires_at, now),
            verify_submissions: self.verify_submissions,
        }
    }
}

fn terminal_snapshot(current: &LoginAttempt, state: WeixinLoginState) -> WeixinLoginAttempt {
    WeixinLoginAttempt {
        schema_version: 1,
        attempt_id: current.id.clone(),
        state,
        qr_content: None,
        expires_in_seconds: 0,
        verify_submissions: current.verify_submissions,
    }
}

fn ensure_attempt(current: &LoginAttempt, attempt_id: &str) -> Result<(), WeixinLoginError> {
    if attempt_id.is_empty() || current.id != attempt_id {
        return Err(WeixinLoginError::AttemptNotFound);
    }
    Ok(())
}

fn remaining_seconds(expires_at: Instant, now: Instant) -> u64 {
    let remaining = expires_at.saturating_duration_since(now);
    remaining
        .as_secs()
        .saturating_add(u64::from(remaining.subsec_nanos() > 0))
}

fn random_attempt_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum WeixinLoginState {
    WaitingForScan,
    Scanned,
    VerificationRequired,
    VerificationSubmitted,
    Redirected,
    Connected,
    AlreadyBound,
    Expired,
    VerificationBlocked,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WeixinLoginAttempt {
    pub(super) schema_version: u32,
    pub(super) attempt_id: String,
    pub(super) state: WeixinLoginState,
    pub(super) qr_content: Option<SecretValue>,
    pub(super) expires_in_seconds: u64,
    pub(super) verify_submissions: u8,
}

impl fmt::Debug for WeixinLoginAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeixinLoginAttempt")
            .field("schema_version", &self.schema_version)
            .field("attempt_id", &self.attempt_id)
            .field("state", &self.state)
            .field("has_qr_content", &self.qr_content.is_some())
            .field("expires_in_seconds", &self.expires_in_seconds)
            .field("verify_submissions", &self.verify_submissions)
            .finish()
    }
}

#[derive(Debug, Error)]
pub(super) enum WeixinLoginError {
    #[error("a Weixin account is already bound")]
    AlreadyBound,
    #[error("Weixin login attempt was not found")]
    AttemptNotFound,
    #[error("Weixin login attempt was superseded")]
    AttemptSuperseded,
    #[error("Weixin verification code is invalid")]
    InvalidVerifyCode,
    #[error("Weixin verification was not requested")]
    VerificationNotRequested,
    #[error("Weixin verification retry limit was reached")]
    VerifyLimitReached,
    #[error("iLink reported an invalid login state")]
    InvalidProtocolState,
    #[error("iLink reported an existing binding but no local credential exists")]
    MissingExistingCredential,
    #[error("iLink login transport failed")]
    Protocol(#[from] IlinkError),
    #[error("Weixin credential storage failed")]
    Credential(#[from] CredentialStoreError),
}
