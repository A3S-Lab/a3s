//! Kimi account provider backed by the local Kimi desktop or Kimi Code login.
//!
//! Kimi's print/ACP commands run a complete coding agent, so they are not a
//! safe model-only transport for A3S host tools. This adapter instead uses the
//! Kimi Code model endpoint while leaving login ownership with Kimi. Desktop
//! keys are read from Kimi's protected Daimon state for each request; Kimi Code
//! OAuth tokens are refreshed atomically in place when needed. Neither kind of
//! credential is copied into A3S configuration or logs.

use a3s_code_core::llm::{
    default_http_client,
    structured::{NativeStructuredSupport, StructuredDirective},
    HttpClient, HttpResponse, LlmClient, LlmResponse, Message, OpenAiClient, StreamEvent,
    StreamingHttpResponse, ToolDefinition,
};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

mod local_state;

use local_state::{
    endpoint_from_env, identity_headers, locate_credentials, locate_desktop_account,
    read_desktop_api_key, trim_endpoint, DesktopAccount,
};

const DEFAULT_MODEL: &str = "kimi-for-coding";
const DEFAULT_MODEL_CONTEXT: u32 = 262_144;
const DEFAULT_BASE_URL: &str = "https://api.kimi.com/coding/v1";
const DEFAULT_DESKTOP_BASE_URL: &str = "https://agent-gw.kimi.com/coding/v1";
const DEFAULT_OAUTH_HOST: &str = "https://auth.kimi.com";
const OAUTH_CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
const MAX_CREDENTIAL_BYTES: u64 = 64 * 1024;
const MAX_MODELS_BYTES: usize = 2 * 1024 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_REFRESH_THRESHOLD_SECONDS: f64 = 300.0;
const REFRESH_ATTEMPTS: usize = 3;
const LOCK_ATTEMPTS: usize = 6;

#[derive(Clone, Copy)]
struct KimiModelMetadata {
    context: u32,
    thinking: bool,
}

static MODEL_METADATA: OnceLock<RwLock<HashMap<String, KimiModelMetadata>>> = OnceLock::new();

fn model_metadata() -> &'static RwLock<HashMap<String, KimiModelMetadata>> {
    MODEL_METADATA.get_or_init(|| {
        RwLock::new(HashMap::from([(
            DEFAULT_MODEL.to_string(),
            KimiModelMetadata {
                context: DEFAULT_MODEL_CONTEXT,
                thinking: true,
            },
        )]))
    })
}

#[derive(Clone, Deserialize, Serialize)]
struct KimiCredentials {
    access_token: String,
    refresh_token: String,
    expires_at: f64,
    #[serde(default)]
    scope: String,
    #[serde(default = "default_token_type")]
    token_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_in: Option<f64>,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}

impl KimiCredentials {
    fn has_reusable_login(&self, now: f64) -> bool {
        !self.refresh_token.is_empty() || (!self.access_token.is_empty() && self.expires_at > now)
    }

    fn is_fresh(&self, now: f64) -> bool {
        if self.access_token.is_empty() {
            return false;
        }
        let threshold = self
            .expires_in
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(|value| (value * 0.5).max(MIN_REFRESH_THRESHOLD_SECONDS))
            .unwrap_or(MIN_REFRESH_THRESHOLD_SECONDS);
        self.expires_at - now > threshold
    }
}

#[derive(Deserialize)]
struct OAuthRefreshResponse {
    access_token: String,
    refresh_token: String,
    expires_in: f64,
    #[serde(default)]
    scope: String,
    #[serde(default = "default_token_type")]
    token_type: String,
}

#[derive(Deserialize)]
struct KimiModelsResponse {
    data: Vec<KimiModelResponse>,
}

#[derive(Deserialize)]
struct KimiModelResponse {
    id: String,
    context_length: u64,
    #[serde(default)]
    supports_reasoning: bool,
    #[serde(default)]
    supports_thinking_type: Option<String>,
    #[serde(default)]
    protocol: Option<String>,
}

struct CredentialFileLock(File);

impl Drop for CredentialFileLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

enum KimiCredentialSource {
    Desktop {
        key_path: PathBuf,
    },
    OAuth {
        credentials_path: PathBuf,
        oauth_host: String,
    },
}

struct KimiAuth {
    credentials: KimiCredentialSource,
    base_url: String,
    http: reqwest::Client,
    refresh_lock: Mutex<()>,
    identity_headers: HashMap<String, String>,
}

impl KimiAuth {
    fn from_local_login() -> Result<Self> {
        if let Some(account) = locate_desktop_account() {
            return Self::new_desktop(account);
        }
        let (home_dir, credentials_path, credentials) = locate_credentials().ok_or_else(|| {
            anyhow!("Kimi account state was not found; open Kimi or Kimi Code and sign in")
        })?;
        if !credentials.has_reusable_login(now_unix_seconds()) {
            bail!("Kimi account state has expired; open Kimi Code and sign in again");
        }
        Self::new_oauth(
            home_dir,
            credentials_path,
            endpoint_from_env("A3S_KIMI_BASE_URL", "KIMI_CODE_BASE_URL", DEFAULT_BASE_URL),
            endpoint_from_env(
                "A3S_KIMI_OAUTH_HOST",
                "KIMI_CODE_OAUTH_HOST",
                DEFAULT_OAUTH_HOST,
            ),
        )
    }

    fn new_desktop(account: DesktopAccount) -> Result<Self> {
        Self::build(
            KimiCredentialSource::Desktop {
                key_path: account.key_path,
            },
            account.base_url,
            account.identity_headers,
        )
    }

    fn new_oauth(
        home_dir: PathBuf,
        credentials_path: PathBuf,
        base_url: String,
        oauth_host: String,
    ) -> Result<Self> {
        Self::build(
            KimiCredentialSource::OAuth {
                credentials_path,
                oauth_host: trim_endpoint(oauth_host),
            },
            base_url,
            identity_headers(&home_dir),
        )
    }

    fn build(
        credentials: KimiCredentialSource,
        base_url: String,
        identity_headers: HashMap<String, String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent(format!("a3s-code/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .context("build Kimi account HTTP client")?;
        Ok(Self {
            credentials,
            base_url: trim_endpoint(base_url),
            http,
            refresh_lock: Mutex::new(()),
            identity_headers,
        })
    }

    async fn access_token(&self, force: bool) -> Result<String> {
        match &self.credentials {
            KimiCredentialSource::Desktop { key_path } => read_desktop_api_key(key_path).await,
            KimiCredentialSource::OAuth {
                credentials_path,
                oauth_host,
            } => {
                self.oauth_access_token(credentials_path, oauth_host, force)
                    .await
            }
        }
    }

    async fn oauth_access_token(
        &self,
        credentials_path: &Path,
        oauth_host: &str,
        force: bool,
    ) -> Result<String> {
        let observed = self.read_credentials(credentials_path).await?;
        if !force && observed.is_fresh(now_unix_seconds()) {
            return Ok(observed.access_token);
        }

        let _process_guard = self.refresh_lock.lock().await;
        let _file_guard = self.acquire_file_lock(credentials_path).await?;
        let current = self.read_credentials(credentials_path).await?;
        let peer_refreshed = current.access_token != observed.access_token;
        if (!force || peer_refreshed) && current.is_fresh(now_unix_seconds()) {
            return Ok(current.access_token);
        }
        if current.refresh_token.is_empty() {
            bail!("Kimi login cannot be refreshed; open Kimi Code and sign in again");
        }

        let refreshed = self.refresh(oauth_host, &current.refresh_token).await?;
        let access_token = refreshed.access_token.clone();
        self.save_credentials(credentials_path, refreshed).await?;
        Ok(access_token)
    }

    async fn read_credentials(&self, credentials_path: &Path) -> Result<KimiCredentials> {
        let metadata = tokio::fs::metadata(credentials_path)
            .await
            .with_context(|| {
                format!(
                    "read Kimi credential metadata {}",
                    credentials_path.display()
                )
            })?;
        if metadata.len() > MAX_CREDENTIAL_BYTES {
            bail!("Kimi credential file is unexpectedly large");
        }
        let raw = tokio::fs::read(credentials_path)
            .await
            .with_context(|| format!("read Kimi credentials {}", credentials_path.display()))?;
        serde_json::from_slice(&raw).context("parse Kimi credentials")
    }

    async fn acquire_file_lock(&self, credentials_path: &Path) -> Result<CredentialFileLock> {
        let lock_path = credentials_path.with_file_name("kimi-code.lock");
        for attempt in 0..LOCK_ATTEMPTS {
            let path = lock_path.clone();
            let result = tokio::task::spawn_blocking(move || -> std::io::Result<Option<File>> {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .read(true)
                    .write(true)
                    .open(path)?;
                match file.try_lock_exclusive() {
                    Ok(()) => Ok(Some(file)),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                    Err(error) => Err(error),
                }
            })
            .await
            .context("join Kimi credential lock task")?
            .context("open Kimi credential refresh lock")?;
            if let Some(file) = result {
                return Ok(CredentialFileLock(file));
            }
            if attempt + 1 < LOCK_ATTEMPTS {
                tokio::time::sleep(Duration::from_millis(250 + attempt as u64 * 100)).await;
            }
        }
        bail!("Kimi credentials are being refreshed by another process; try again shortly")
    }

    async fn refresh(&self, oauth_host: &str, refresh_token: &str) -> Result<KimiCredentials> {
        let url = format!("{oauth_host}/api/oauth/token");
        let mut last_error = None;
        for attempt in 0..REFRESH_ATTEMPTS {
            let mut request = self.http.post(&url).form(&[
                ("client_id", OAUTH_CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ]);
            for (name, value) in &self.identity_headers {
                request = request.header(name, value);
            }
            let response = match request.send().await {
                Ok(response) => response,
                Err(error) if attempt + 1 < REFRESH_ATTEMPTS => {
                    last_error = Some(error.to_string());
                    tokio::time::sleep(Duration::from_secs(1_u64 << attempt)).await;
                    continue;
                }
                Err(error) => return Err(error).context("refresh Kimi login"),
            };
            let status = response.status();
            let body = response.bytes().await.context("read Kimi OAuth response")?;
            if status.is_success() {
                let payload: OAuthRefreshResponse =
                    serde_json::from_slice(&body).context("parse Kimi OAuth response")?;
                if payload.access_token.is_empty()
                    || payload.refresh_token.is_empty()
                    || !payload.expires_in.is_finite()
                    || payload.expires_in <= 0.0
                {
                    bail!("Kimi OAuth refresh returned incomplete credentials");
                }
                return Ok(KimiCredentials {
                    access_token: payload.access_token,
                    refresh_token: payload.refresh_token,
                    expires_at: now_unix_seconds() + payload.expires_in,
                    scope: payload.scope,
                    token_type: payload.token_type,
                    expires_in: Some(payload.expires_in),
                });
            }
            if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
                || oauth_error_code(&body).as_deref() == Some("invalid_grant")
            {
                bail!("Kimi login has expired; open Kimi Code and sign in again");
            }
            if (status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
                && attempt + 1 < REFRESH_ATTEMPTS
            {
                last_error = Some(provider_error(&body, status.as_u16()));
                tokio::time::sleep(Duration::from_secs(1_u64 << attempt)).await;
                continue;
            }
            bail!("{}", provider_error(&body, status.as_u16()));
        }
        bail!(
            "Kimi login refresh failed after retries{}",
            last_error
                .map(|error| format!(": {error}"))
                .unwrap_or_default()
        )
    }

    async fn save_credentials(
        &self,
        credentials_path: &Path,
        credentials: KimiCredentials,
    ) -> Result<()> {
        let path = credentials_path.to_path_buf();
        let mut body = serde_json::to_vec_pretty(&credentials)
            .context("serialize refreshed Kimi credentials")?;
        body.push(b'\n');
        tokio::task::spawn_blocking(move || save_credentials_atomically(&path, &body))
            .await
            .context("join Kimi credential write task")?
            .with_context(|| {
                format!(
                    "save refreshed Kimi credentials {}",
                    credentials_path.display()
                )
            })
    }

    fn request_with_identity(
        &self,
        mut request: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        for (name, value) in &self.identity_headers {
            request = request.header(name, value);
        }
        request
    }
}

struct KimiHttpClient {
    inner: Arc<dyn HttpClient>,
    auth: Arc<KimiAuth>,
    thinking: bool,
}

impl KimiHttpClient {
    fn request_body(&self, body: &serde_json::Value) -> serde_json::Value {
        let mut body = body.clone();
        if self.thinking && body.get("thinking").is_none() {
            body["thinking"] = serde_json::json!({"type": "enabled"});
        }
        body
    }

    fn headers_with_token(&self, headers: Vec<(&str, &str)>, token: &str) -> Vec<(String, String)> {
        let mut owned = headers
            .into_iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("authorization"))
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<Vec<_>>();
        owned.push(("Authorization".to_string(), format!("Bearer {token}")));
        owned
    }

    async fn post_once(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &serde_json::Value,
        token: &str,
        cancel_token: CancellationToken,
    ) -> Result<HttpResponse> {
        let headers = self.headers_with_token(headers, token);
        let borrowed = headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        self.inner.post(url, borrowed, body, cancel_token).await
    }

    async fn post_streaming_once(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &serde_json::Value,
        token: &str,
        cancel_token: CancellationToken,
    ) -> Result<StreamingHttpResponse> {
        let headers = self.headers_with_token(headers, token);
        let borrowed = headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        self.inner
            .post_streaming(url, borrowed, body, cancel_token)
            .await
    }
}

#[async_trait]
impl HttpClient for KimiHttpClient {
    async fn post(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &serde_json::Value,
        cancel_token: CancellationToken,
    ) -> Result<HttpResponse> {
        let body = self.request_body(body);
        let token = self.auth.access_token(false).await?;
        let response = self
            .post_once(url, headers.clone(), &body, &token, cancel_token.clone())
            .await?;
        if response.status != 401 || cancel_token.is_cancelled() {
            return Ok(response);
        }
        let token = self.auth.access_token(true).await?;
        self.post_once(url, headers, &body, &token, cancel_token)
            .await
    }

    async fn post_streaming(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &serde_json::Value,
        cancel_token: CancellationToken,
    ) -> Result<StreamingHttpResponse> {
        let body = self.request_body(body);
        let token = self.auth.access_token(false).await?;
        let response = self
            .post_streaming_once(url, headers.clone(), &body, &token, cancel_token.clone())
            .await?;
        if response.status != 401 || cancel_token.is_cancelled() {
            return Ok(response);
        }
        let token = self.auth.access_token(true).await?;
        self.post_streaming_once(url, headers, &body, &token, cancel_token)
            .await
    }
}

pub(crate) struct KimiClient {
    inner: OpenAiClient,
    thinking: bool,
}

impl KimiClient {
    pub(crate) fn from_kimi_login(model: &str) -> Result<Self> {
        let model = model.trim();
        if model.is_empty() || model.starts_with('(') {
            bail!("Kimi model id is empty or unavailable");
        }
        let auth = Arc::new(KimiAuth::from_local_login()?);
        Ok(Self::with_auth(model, auth))
    }

    fn with_auth(model: &str, auth: Arc<KimiAuth>) -> Self {
        Self::with_auth_and_http(model, auth, default_http_client())
    }

    fn with_auth_and_http(
        model: &str,
        auth: Arc<KimiAuth>,
        inner_http: Arc<dyn HttpClient>,
    ) -> Self {
        let thinking = model_metadata()
            .read()
            .ok()
            .and_then(|metadata| metadata.get(model).copied())
            .map(|metadata| metadata.thinking)
            .unwrap_or(true);
        let http: Arc<dyn HttpClient> = Arc::new(KimiHttpClient {
            inner: inner_http,
            auth: Arc::clone(&auth),
            thinking,
        });
        let chat_path = if auth.base_url.ends_with("/v1") {
            "/v1/chat/completions"
        } else {
            "/chat/completions"
        };
        let inner = OpenAiClient::new("managed-by-kimi".to_string(), model.to_string())
            .with_base_url(auth.base_url.clone())
            .with_chat_completions_path(chat_path)
            .with_provider_name("Kimi")
            .with_headers(auth.identity_headers.clone())
            .with_http_client(http);
        Self { inner, thinking }
    }
}

#[async_trait]
impl LlmClient for KimiClient {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        self.inner.complete(messages, system, tools).await
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        self.inner
            .complete_streaming(messages, system, tools, cancel_token)
            .await
    }

    fn native_structured_support(&self) -> NativeStructuredSupport {
        if self.thinking {
            // Kimi thinking-only routes reject a specified tool_choice. Report
            // the capability conservatively so structured generation uses its
            // schema-in-prompt fallback and retains local validation/repair.
            NativeStructuredSupport::None
        } else {
            self.inner.native_structured_support()
        }
    }

    async fn complete_structured(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: &StructuredDirective,
    ) -> Result<LlmResponse> {
        self.inner
            .complete_structured(messages, system, tools, directive)
            .await
    }

    async fn complete_streaming_structured(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: &StructuredDirective,
        cancel_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        self.inner
            .complete_streaming_structured(messages, system, tools, directive, cancel_token)
            .await
    }
}

pub(crate) fn has_kimi_login() -> bool {
    locate_desktop_account().is_some() || locate_credentials().is_some()
}

pub(crate) fn fallback_models() -> Vec<String> {
    locate_desktop_account()
        .map(|account| account.models.into_iter().map(|model| model.id).collect())
        .filter(|models: &Vec<String>| !models.is_empty())
        .unwrap_or_else(|| vec![DEFAULT_MODEL.to_string()])
}

pub(crate) fn model_context(model: &str) -> Option<u32> {
    model_metadata()
        .read()
        .ok()
        .and_then(|metadata| metadata.get(model).copied())
        .map(|metadata| metadata.context)
        .or_else(|| (model == DEFAULT_MODEL).then_some(DEFAULT_MODEL_CONTEXT))
}

pub(crate) fn kimi_home() -> Option<PathBuf> {
    locate_desktop_account()
        .map(|account| account.root)
        .or_else(|| locate_credentials().map(|(home, _, _)| home))
}

pub(crate) async fn discover_models() -> Result<Vec<String>> {
    if let Some(account) = locate_desktop_account() {
        return Ok(account.models.into_iter().map(|model| model.id).collect());
    }
    let auth = Arc::new(KimiAuth::from_local_login()?);
    let response = fetch_models_response(&auth).await?;
    let bytes = response.bytes().await.context("read Kimi model catalog")?;
    if bytes.len() > MAX_MODELS_BYTES {
        bail!("Kimi model catalog is unexpectedly large");
    }
    let payload: KimiModelsResponse =
        serde_json::from_slice(&bytes).context("parse Kimi model catalog")?;
    let mut seen = HashSet::new();
    let mut models = Vec::new();
    let mut metadata = model_metadata()
        .write()
        .map_err(|_| anyhow!("Kimi model metadata cache is unavailable"))?;
    for model in payload.data {
        if !valid_model_id(&model.id) || model.context_length == 0 {
            continue;
        }
        // The A3S adapter currently speaks Kimi's OpenAI-compatible wire. The
        // service explicitly marks Anthropic-wire aliases; do not advertise a
        // route that would be sent to the wrong protocol.
        if model.protocol.as_deref() == Some("anthropic") {
            continue;
        }
        let Ok(context) = u32::try_from(model.context_length) else {
            continue;
        };
        let thinking = match model.supports_thinking_type.as_deref() {
            Some("no") => false,
            Some("only" | "both") => true,
            _ => model.supports_reasoning,
        };
        metadata.insert(model.id.clone(), KimiModelMetadata { context, thinking });
        if seen.insert(model.id.clone()) {
            models.push(model.id);
        }
    }
    if models.is_empty() {
        bail!("Kimi returned no OpenAI-compatible account models");
    }
    Ok(models)
}

async fn fetch_models_response(auth: &Arc<KimiAuth>) -> Result<reqwest::Response> {
    for force in [false, true] {
        let token = auth.access_token(force).await?;
        let request = auth
            .request_with_identity(auth.http.get(format!("{}/models", auth.base_url)))
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/json");
        let response = request.send().await.context("list Kimi account models")?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED && !force {
            continue;
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.bytes().await.unwrap_or_default();
            if matches!(status.as_u16(), 401 | 403) {
                bail!("Kimi account is not authorized; open Kimi Code and sign in again");
            }
            bail!("{}", provider_error(&body, status.as_u16()));
        }
        return Ok(response);
    }
    unreachable!("Kimi model discovery attempts are fixed")
}

fn save_credentials_atomically(path: &Path, body: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "credential path has no parent",
        )
    })?;
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    let temporary = parent.join(format!(
        ".kimi-code.json.tmp.{}.{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(body)?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn now_unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn valid_model_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':'))
}

fn oauth_error_code(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()?
        .get("error")?
        .as_str()
        .map(str::to_string)
}

fn provider_error(body: &[u8], status: u16) -> String {
    let fallback = format!("Kimi request failed (HTTP {status})");
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return fallback;
    };
    let message = value
        .get("error_description")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("message").and_then(serde_json::Value::as_str))
        .or_else(|| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| value.get("error").and_then(serde_json::Value::as_str));
    match message.map(str::trim).filter(|message| !message.is_empty()) {
        Some(message) => format!(
            "{fallback}: {}",
            message.chars().take(512).collect::<String>()
        ),
        None => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::{Form, State},
        http::HeaderMap,
        routing::post,
        Json, Router,
    };
    use std::sync::Mutex as StdMutex;

    #[derive(Debug)]
    struct RecordedRequest {
        url: String,
        headers: Vec<(String, String)>,
        body: serde_json::Value,
    }

    #[derive(Default)]
    struct RecordingHttp {
        request: StdMutex<Option<RecordedRequest>>,
    }

    #[async_trait]
    impl HttpClient for RecordingHttp {
        async fn post(
            &self,
            url: &str,
            headers: Vec<(&str, &str)>,
            body: &serde_json::Value,
            _cancel_token: CancellationToken,
        ) -> Result<HttpResponse> {
            *self.request.lock().unwrap() = Some(RecordedRequest {
                url: url.to_string(),
                headers: headers
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
                body: body.clone(),
            });
            Ok(HttpResponse {
                status: 200,
                body: serde_json::json!({
                    "id": "chatcmpl-kimi-test",
                    "object": "chat.completion",
                    "model": "k3-agent-test",
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": "KIMI_OK"},
                        "finish_reason": "stop"
                    }],
                    "usage": {"prompt_tokens": 11, "completion_tokens": 2, "total_tokens": 13}
                })
                .to_string(),
            })
        }

        async fn post_streaming(
            &self,
            _url: &str,
            _headers: Vec<(&str, &str)>,
            _body: &serde_json::Value,
            _cancel_token: CancellationToken,
        ) -> Result<StreamingHttpResponse> {
            unreachable!("this test uses the non-streaming request path")
        }
    }

    #[derive(Clone)]
    struct OAuthServerState {
        requests: tokio::sync::mpsc::UnboundedSender<(HeaderMap, HashMap<String, String>)>,
    }

    async fn oauth_refresh_handler(
        State(state): State<OAuthServerState>,
        headers: HeaderMap,
        Form(form): Form<HashMap<String, String>>,
    ) -> Json<serde_json::Value> {
        let _ = state.requests.send((headers, form));
        Json(serde_json::json!({
            "access_token": "refreshed-access-secret",
            "refresh_token": "rotated-refresh-secret",
            "expires_in": 3600,
            "scope": "kimi-code",
            "token_type": "Bearer"
        }))
    }

    mod account_rounds;

    #[test]
    fn expired_access_token_with_refresh_token_is_reusable_login_state() {
        let credentials = KimiCredentials {
            access_token: "expired".into(),
            refresh_token: "reusable".into(),
            expires_at: 1.0,
            scope: "kimi-code".into(),
            token_type: "Bearer".into(),
            expires_in: None,
        };

        assert!(credentials.has_reusable_login(10_000.0));
        assert!(!credentials.is_fresh(10_000.0));
    }

    #[test]
    fn provider_errors_never_fall_back_to_dumping_response_bodies() {
        let body = br#"{"error":"invalid","access_token":"secret-token"}"#;
        let error = provider_error(body, 400);

        assert!(error.contains("invalid"));
        assert!(!error.contains("secret-token"));
    }

    #[test]
    fn fallback_model_has_the_product_context_window() {
        assert_eq!(model_context(DEFAULT_MODEL), Some(DEFAULT_MODEL_CONTEXT));
    }

    #[tokio::test]
    async fn desktop_chat_uses_the_app_key_openai_path_and_thinking_body() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("kimi-code-key.json");
        std::fs::write(
            &key_path,
            r#"{"userId":"test-user","apiKey":"desktop-api-secret"}"#,
        )
        .unwrap();
        model_metadata().write().unwrap().insert(
            "k3-agent-test".to_string(),
            KimiModelMetadata {
                context: DEFAULT_MODEL_CONTEXT,
                thinking: true,
            },
        );
        let auth = Arc::new(
            KimiAuth::build(
                KimiCredentialSource::Desktop { key_path },
                "http://kimi.test/coding/v1".to_string(),
                HashMap::from([("User-Agent".to_string(), "Desktop Kimi Work".to_string())]),
            )
            .unwrap(),
        );
        let http = Arc::new(RecordingHttp::default());
        let client =
            KimiClient::with_auth_and_http("k3-agent-test", Arc::clone(&auth), http.clone());
        let tool = ToolDefinition {
            name: "echo".to_string(),
            description: "Echo text".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"]
            }),
        };

        let response = client
            .complete(&[Message::user("say hello")], Some("test system"), &[tool])
            .await
            .unwrap();
        assert_eq!(response.text(), "KIMI_OK");
        let request = http.request.lock().unwrap().take().unwrap();
        assert_eq!(request.url, "http://kimi.test/coding/v1/chat/completions");
        assert_eq!(request.body["model"], "k3-agent-test");
        assert_eq!(request.body["thinking"]["type"], "enabled");
        assert_eq!(request.body["tools"][0]["function"]["name"], "echo");
        assert!(!request.body.to_string().contains("desktop-api-secret"));
        assert!(request.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization") && value == "Bearer desktop-api-secret"
        }));
        assert!(request.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("user-agent") && value == "Desktop Kimi Work"
        }));
        assert_eq!(
            client.native_structured_support(),
            NativeStructuredSupport::None
        );

        model_metadata().write().unwrap().insert(
            "non-thinking-test".to_string(),
            KimiModelMetadata {
                context: DEFAULT_MODEL_CONTEXT,
                thinking: false,
            },
        );
        let non_thinking = KimiClient::with_auth_and_http(
            "non-thinking-test",
            auth,
            Arc::new(RecordingHttp::default()),
        );
        assert_eq!(
            non_thinking.native_structured_support(),
            NativeStructuredSupport::JsonSchema
        );
    }
}
