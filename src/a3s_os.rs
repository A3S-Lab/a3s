//! OS account login helpers for the TUI.

use a3s_code_core::config::OsConfig;
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const PROFILE_TIMEOUT: Duration = Duration::from_secs(5);
const OAUTH_CLIENT_ID: &str = "a3s-code";
const OAUTH_SCOPE: &str = "profile offline_access";
const STORE_FILE: &str = "os-auth.json";

/// Built-in `a3s-os-capabilities` skill that drives OS's progressive API —
/// the platform-wide kernel `capabilities` endpoint (POST /api/v1/kernel/
/// capabilities), spanning all domains, not just security. Materialized under
/// `~/.a3s/os-skills/` only when signed in; `{{BASE_URL}}` is replaced with the
/// configured OS address so the agent calls the right endpoint.
const CAPABILITY_SKILL: &str = include_str!("../skills/a3s-os-capabilities.md");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct StoredOsSession {
    pub address: String,
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    pub login_at_ms: u64,
}

impl StoredOsSession {
    pub(crate) fn display_label(&self) -> String {
        self.account_label
            .clone()
            .unwrap_or_else(|| self.address.clone())
    }
}

/// Result of the post-login SSH-key sync (`sync_ssh_key`). The TUI formats each
/// variant into a transcript line — the OS side is done, this just reports it.
pub(crate) enum SshKeyOutcome {
    /// Uploaded the local public key (carries the short SHA256 fingerprint).
    Registered(String),
    /// The local key was already registered — nothing to do.
    AlreadyRegistered,
    /// No local public key found — the TUI prints a `ssh-keygen` hint.
    NoLocalKey,
    /// Network / OS error (best-effort: login still succeeds).
    Failed(String),
}

/// After OS login, make git-over-SSH "just work": read the machine's SSH public
/// key, and register it with OS (`POST /users/me/developer-config/ssh-keys`)
/// if it isn't already there. Idempotent (deduped by key body + SHA256
/// fingerprint) and best-effort — a failure never blocks login. Public keys are
/// meant to be shared, so uploading one is safe; the private key never leaves.
pub(crate) async fn sync_ssh_key(session: StoredOsSession) -> SshKeyOutcome {
    // 1. Read the local public key (prefer ed25519, the modern default).
    let Some(pubkey_line) = read_local_pubkey() else {
        return SshKeyOutcome::NoLocalKey;
    };
    // 2-3. Dedup + register against the OS.
    register_ssh_key(
        &os_origin(&session.address),
        &session.access_token,
        &pubkey_line,
    )
    .await
}

/// The network half of [`sync_ssh_key`] (dedup via `GET developer-config`, then
/// `POST` if new), split out so it's testable against a mock without touching
/// `$HOME`. `origin` is `scheme://host[:port]`.
async fn register_ssh_key(origin: &str, token: &str, pubkey_line: &str) -> SshKeyOutcome {
    let Some(local_body) = ssh_key_body(pubkey_line) else {
        return SshKeyOutcome::Failed("Could not parse the local public key format".to_string());
    };
    let local_fp = openssh_sha256_fingerprint(pubkey_line);

    let client = match http_client_for_origin(origin) {
        Ok(c) => c,
        Err(e) => return SshKeyOutcome::Failed(e.to_string()),
    };

    // 2. List existing credentials and dedup — by key body (exact, format-
    //    agnostic) or by fingerprint when the list omits the body.
    let list_url = format!("{origin}/api/v1/users/me/developer-config");
    match client.get(&list_url).bearer_auth(token).send().await {
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                let creds = v.get("data").unwrap_or(&v);
                if let Some(arr) = creds.as_array() {
                    let already = arr.iter().any(|c| {
                        let is_key = c.get("type").and_then(|t| t.as_str()) == Some("ssh_key");
                        let body_match = c
                            .get("publicKey")
                            .and_then(|p| p.as_str())
                            .and_then(ssh_key_body)
                            .is_some_and(|b| b == local_body);
                        let fp_match = local_fp.as_deref().is_some_and(|fp| {
                            c.get("fingerprint").and_then(|f| f.as_str()) == Some(fp)
                        });
                        is_key && (body_match || fp_match)
                    });
                    if already {
                        return SshKeyOutcome::AlreadyRegistered;
                    }
                }
            }
        }
        Err(e) => return SshKeyOutcome::Failed(e.to_string()),
    }

    // 3. Register the key. Name it after the pubkey's own comment (usually
    //    user@host) so it's identifiable in the OS credential list.
    let comment = pubkey_line
        .split_whitespace()
        .nth(2)
        .unwrap_or("this machine");
    let name = format!("a3s-code · {comment}");
    let post_url = format!("{origin}/api/v1/users/me/developer-config/ssh-keys");
    match client
        .post(&post_url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": name, "publicKey": pubkey_line }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let short = local_fp
                .as_deref()
                .map(|fp| fp.chars().take(23).collect::<String>())
                .unwrap_or_else(|| "registered".to_string());
            SshKeyOutcome::Registered(short)
        }
        Ok(resp) => {
            let code = resp.status().as_u16();
            let msg = resp
                .text()
                .await
                .ok()
                .and_then(|b| serde_json::from_str::<serde_json::Value>(&b).ok())
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
                .unwrap_or_else(|| "request failed".to_string());
            SshKeyOutcome::Failed(format!("HTTP {code}: {msg}"))
        }
        Err(e) => SshKeyOutcome::Failed(e.to_string()),
    }
}

fn http_client_for_origin(origin: &str) -> Result<reqwest::Client, reqwest::Error> {
    let mut builder = reqwest::Client::builder().timeout(HTTP_TIMEOUT);
    if is_loopback_origin(origin) {
        builder = builder.no_proxy();
    }
    builder.build()
}

fn is_loopback_origin(origin: &str) -> bool {
    origin.starts_with("http://127.")
        || origin.starts_with("https://127.")
        || origin.starts_with("http://localhost")
        || origin.starts_with("https://localhost")
        || origin.starts_with("http://[::1]")
        || origin.starts_with("https://[::1]")
}

/// Read the first available local SSH public key, preferring modern key types.
/// Returns the trimmed one-line contents (`ssh-ed25519 AAAA… comment`).
fn read_local_pubkey() -> Option<String> {
    let ssh = crate::user_paths::user_home_dir()?.join(".ssh");
    for name in ["id_ed25519.pub", "id_ecdsa.pub", "id_rsa.pub"] {
        if let Ok(s) = std::fs::read_to_string(ssh.join(name)) {
            let line = s.trim();
            if !line.is_empty() {
                return Some(line.to_string());
            }
        }
    }
    None
}

/// The base64 key material (the 2nd whitespace token) — uniquely identifies a
/// key regardless of comment, so it's the exact dedup token.
fn ssh_key_body(pubkey_line: &str) -> Option<&str> {
    pubkey_line.split_whitespace().nth(1)
}

/// OpenSSH `SHA256:<base64-no-pad>` fingerprint of a public key line (matches
/// `ssh-keygen -lf`), used as a fallback dedup key.
fn openssh_sha256_fingerprint(pubkey_line: &str) -> Option<String> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
    let raw = STANDARD.decode(ssh_key_body(pubkey_line)?).ok()?;
    let digest = Sha256::digest(&raw);
    Some(format!("SHA256:{}", STANDARD_NO_PAD.encode(digest)))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct OsAuthStore {
    #[serde(default)]
    sessions: Vec<StoredOsSession>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenError {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

pub(crate) async fn login_via_browser(config: OsConfig) -> Result<StoredOsSession> {
    validate_address(&config.address)?;
    let state = random_url_token(32);
    let code_verifier = pkce_verifier();
    let code_challenge = pkce_challenge(&code_verifier);
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("bind local OS login callback")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let authorize_url =
        build_authorization_url(&config.address, &redirect_uri, &state, &code_challenge);
    open_browser(&authorize_url).with_context(|| {
        format!("open browser failed; visit this URL manually to continue: {authorize_url}")
    })?;

    let callback = tokio::time::timeout(CALLBACK_TIMEOUT, wait_for_callback(listener, &state))
        .await
        .map_err(|_| anyhow!("timed out waiting for OS login callback"))??;
    if let Some(error) = callback
        .get("error")
        .filter(|value| !value.trim().is_empty())
    {
        let description = callback
            .get("error_description")
            .map(String::as_str)
            .unwrap_or("OAuth2 authorization failed");
        anyhow::bail!("OS OAuth2 authorization failed: {error}: {description}");
    }
    let code = callback
        .get("code")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("OAuth2 callback did not include an authorization code"))?;
    let token =
        exchange_authorization_code(&config.address, code, &redirect_uri, &code_verifier).await?;
    let session = session_from_token_response(&config.address, token);
    finalize_login_session(session).await
}

pub(crate) async fn login_with_token(config: &OsConfig, token: &str) -> Result<StoredOsSession> {
    validate_address(&config.address)?;
    let token = token.trim();
    if token.is_empty() {
        anyhow::bail!("token is empty");
    }
    let session = StoredOsSession {
        address: normalize_address(&config.address),
        access_token: token.to_string(),
        refresh_token: None,
        token_type: Some("Bearer".to_string()),
        expires_at_ms: None,
        account_label: None,
        login_at_ms: now_ms(),
    };
    finalize_login_session(session).await
}

/// Resolve the signed-in OS user's human-readable identity before persisting
/// the new session. Profile lookup is deliberately best-effort: the bearer
/// token is still valid when OS is temporarily unavailable or returns an older
/// response shape, so those failures must not turn a successful login into an
/// authentication failure.
async fn finalize_login_session(mut session: StoredOsSession) -> Result<StoredOsSession> {
    let path = auth_store_path()?;
    finalize_login_session_at(&path, &mut session).await?;
    Ok(session)
}

async fn finalize_login_session_at(path: &Path, session: &mut StoredOsSession) -> Result<()> {
    if let Some(label) = fetch_account_label(&session.address, &session.access_token).await {
        session.account_label = Some(label);
    }
    save_session_at(path, session)
}

/// Fetch the current OS profile using the same bearer token obtained during
/// login. A3S OS normally wraps the user in its unified `{ data: ... }`
/// response envelope, while older deployments return the user object directly;
/// [`account_label_from_profile`] accepts both forms.
async fn fetch_account_label(address: &str, token: &str) -> Option<String> {
    let origin = os_origin(address);
    let url = format!("{origin}/api/v1/users/me");
    let client = http_client_for_origin(&origin).ok()?;
    let response = client
        .get(url)
        .header("accept", "application/json")
        .bearer_auth(token)
        .timeout(PROFILE_TIMEOUT)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload = response.json::<serde_json::Value>().await.ok()?;
    account_label_from_profile(&payload)
}

fn account_label_from_profile(payload: &serde_json::Value) -> Option<String> {
    let profile = payload.get("data").unwrap_or(payload);
    ["displayName", "display_name", "name", "email", "username"]
        .into_iter()
        .find_map(|field| {
            profile
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

pub(crate) fn logout(config: &OsConfig) -> Result<bool> {
    let path = auth_store_path()?;
    remove_session_at(&path, &normalize_address(&config.address))
}

/// Env vars the agent's `bash` inherits so it can call the progressive API
/// without re-reading `~/.a3s/os-auth.json` on every turn (the shell can't keep
/// state between tool calls, so the address/token would otherwise be looked up
/// each time). `spawn_shell` runs `bash` with the cli's process env (no
/// `env_clear`), so setting them here makes `$A3S_OS_BASE_URL` / `$A3S_OS_TOKEN`
/// available to every command.
pub(crate) const OS_ENV_BASE_URL: &str = "A3S_OS_BASE_URL";
pub(crate) const OS_ENV_TOKEN: &str = "A3S_OS_TOKEN";
pub(crate) const OS_ENV_REFRESH_TOKEN: &str = "A3S_OS_REFRESH_TOKEN";

/// Export the signed-in platform endpoint + tokens to the process env so the
/// agent's shell — and the RemoteUI webview helper, which inherits this env —
/// can use them directly. Called on login and on startup restore. The refresh
/// token lets the webview's seeded session survive an edge-expired access token.
pub(crate) fn export_os_env(session: &StoredOsSession) {
    std::env::set_var(OS_ENV_BASE_URL, &session.address);
    std::env::set_var(OS_ENV_TOKEN, &session.access_token);
    match &session.refresh_token {
        Some(rt) => std::env::set_var(OS_ENV_REFRESH_TOKEN, rt),
        None => std::env::remove_var(OS_ENV_REFRESH_TOKEN),
    }
}

/// Clear the exported platform env (called on /logout).
pub(crate) fn clear_os_env() {
    std::env::remove_var(OS_ENV_BASE_URL);
    std::env::remove_var(OS_ENV_TOKEN);
    std::env::remove_var(OS_ENV_REFRESH_TOKEN);
}

/// The stored session for the configured OS address, if the user logged in
/// on a previous run. This is the load-back half of `save_session`: without it a
/// persisted login is never restored, so the user has to `/login` every launch.
/// Best-effort — any read/parse error is treated as "not signed in".
pub(crate) fn current_session(config: &OsConfig) -> Option<StoredOsSession> {
    let path = auth_store_path().ok()?;
    current_session_at(&path, &normalize_address(&config.address))
}

fn current_session_at(path: &Path, address: &str) -> Option<StoredOsSession> {
    read_store(path)
        .ok()?
        .sessions
        .into_iter()
        .find(|s| s.address == address)
}

fn os_skills_root() -> Result<PathBuf> {
    let home = crate::user_paths::user_home_dir()
        .ok_or_else(|| anyhow!("user home is unavailable; cannot install the OS skill"))?;
    Ok(home.join(".a3s").join("os-skills"))
}

/// Materialize the built-in `a3s-os-capabilities` skill (templated with the OS
/// base URL) under `~/.a3s/os-skills/` and return that directory so the caller
/// can add it to the session's skill dirs. Call only when signed in. Best-effort
/// — returns `None` on any I/O error rather than failing the launch.
pub(crate) fn ensure_capability_skill_dir(config: &OsConfig) -> Option<PathBuf> {
    let root = os_skills_root().ok()?;
    ensure_capability_skill_dir_at(&root, config).ok()?;
    Some(root)
}

fn ensure_capability_skill_dir_at(root: &Path, config: &OsConfig) -> Result<()> {
    let skill_dir = root.join("a3s-os-capabilities");
    std::fs::create_dir_all(&skill_dir)?;
    let body = CAPABILITY_SKILL.replace("{{BASE_URL}}", &normalize_address(&config.address));
    std::fs::write(skill_dir.join("SKILL.md"), body)?;
    Ok(())
}

/// Remove the materialized OS skill dir (called on /logout). Best-effort.
pub(crate) fn remove_capability_skill_dir() {
    if let Ok(root) = os_skills_root() {
        let _ = std::fs::remove_dir_all(root);
    }
}

fn session_from_token_response(
    configured_address: &str,
    token: OAuthTokenResponse,
) -> StoredOsSession {
    StoredOsSession {
        address: normalize_address(configured_address),
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        token_type: token.token_type.or_else(|| Some("Bearer".to_string())),
        expires_at_ms: token
            .expires_in
            .map(|seconds| now_ms().saturating_add(seconds.saturating_mul(1000))),
        account_label: None,
        login_at_ms: now_ms(),
    }
}

/// Outcome rendered on the localhost callback page after the OAuth redirect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginOutcome {
    Success,
    NotApproved,
    InvalidState,
}

/// The OS-branded page shown in the browser once the OAuth redirect lands back
/// on the local callback. Returns `(http_status, html_body)`.
fn login_callback_page(outcome: LoginOutcome) -> (&'static str, String) {
    let (status, heading, detail) = match outcome {
        LoginOutcome::Success => (
            "200 OK",
            "OS sign-in successful",
            "You are signed in. You can close this page and return to a3s code.",
        ),
        LoginOutcome::NotApproved => (
            "400 Bad Request",
            "OS sign-in not approved",
            "The authorization was not approved. You can close this page and return to a3s code.",
        ),
        LoginOutcome::InvalidState => (
            "400 Bad Request",
            "Invalid sign-in state",
            "Sign-in state validation failed. Return to a3s code and run /login again.",
        ),
    };
    let body = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>OS sign-in</title></head>\
         <body style=\"margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;\
         font-family:system-ui,-apple-system,'PingFang SC','Microsoft YaHei',sans-serif;\
         background:#0f172a;color:#e2e8f0\">\
         <div style=\"text-align:center;padding:2rem\">\
         <h1 style=\"font-size:1.5rem;margin:0 0 .75rem\">{heading}</h1>\
         <p style=\"margin:0;color:#94a3b8\">{detail}</p></div></body></html>"
    );
    (status, body)
}

async fn wait_for_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<BTreeMap<String, String>> {
    // Browsers don't make exactly one request to the redirect URI: Chrome opens
    // speculative *preconnect* sockets (no bytes sent) and fetches /favicon.ico,
    // and any of those can land before the real ?code=...&state=... redirect.
    // Accepting only once meant a preconnect/favicon consumed the single accept
    // (empty read -> missing-state bail, or a 10s read-timeout error), the
    // listener was dropped, and the real callback then hit a dead port — the
    // "redirects back but can't be reached" bug. So loop: skip non-OAuth
    // requests (replying so the socket closes cleanly) and keep the listener
    // alive until the OAuth params actually arrive. The outer CALLBACK_TIMEOUT
    // bounds the whole wait.
    loop {
        let (mut stream, _) = listener.accept().await?;
        let mut buf = [0_u8; 8192];
        // A preconnect socket sends nothing: a per-connection read timeout (or a
        // 0-byte read) must NOT kill the flow — just move on to the next socket.
        let n = match tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buf)).await {
            Ok(Ok(n)) if n > 0 => n,
            _ => continue,
        };
        let request = String::from_utf8_lossy(&buf[..n]);
        let target = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");
        let query = target.split_once('?').map(|(_, query)| query).unwrap_or("");
        let params = parse_query(query);

        // Not the OAuth redirect (favicon, "/", a bare preconnect that did send a
        // request line): acknowledge it and keep waiting for the real callback.
        if !params.contains_key("state")
            && !params.contains_key("code")
            && !params.contains_key("error")
        {
            let _ = stream
                .write_all(b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n")
                .await;
            continue;
        }

        let state = params.get("state").cloned().unwrap_or_default();
        let error = params.get("error").filter(|value| !value.trim().is_empty());
        let outcome = if state == expected_state && error.is_none() {
            LoginOutcome::Success
        } else if state == expected_state {
            LoginOutcome::NotApproved
        } else {
            LoginOutcome::InvalidState
        };
        let (status, body) = login_callback_page(outcome);
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;

        if state != expected_state {
            anyhow::bail!("login callback state mismatch");
        }
        return Ok(params);
    }
}

async fn exchange_authorization_code(
    address: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<OAuthTokenResponse> {
    let token_url = build_token_url(address);
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .context("build OS OAuth2 HTTP client")?;
    let response = client
        .post(&token_url)
        .header("accept", "application/json")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", OAUTH_CLIENT_ID),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .with_context(|| format!("send OAuth2 token request to {token_url}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("read OAuth2 token response body")?;

    if !status.is_success() {
        if let Ok(error) = serde_json::from_str::<OAuthTokenError>(&body) {
            let description = error.error_description.unwrap_or_default();
            anyhow::bail!(
                "OAuth2 token exchange failed at {token_url} (HTTP {}): {} {}",
                status.as_u16(),
                error.error,
                description
            );
        }
        anyhow::bail!(
            "OAuth2 token exchange failed at {token_url} (HTTP {}): {body}",
            status.as_u16()
        );
    }

    serde_json::from_str::<OAuthTokenResponse>(&body)
        .with_context(|| format!("parse OAuth2 token response from {token_url}"))
}

/// Refresh proactively this many ms before the access token expires, so an
/// in-flight progressive-API call never races an expiry.
const REFRESH_SKEW_MS: u64 = 120_000; // 2 minutes

/// True if the session both *can* be refreshed (has a refresh token + known
/// expiry) and *should* be now (expiry within `REFRESH_SKEW_MS`, or already past).
pub(crate) fn needs_refresh(session: &StoredOsSession) -> bool {
    session.refresh_token.is_some()
        && session
            .expires_at_ms
            .is_some_and(|exp| now_ms().saturating_add(REFRESH_SKEW_MS) >= exp)
}

/// Exchange the stored refresh token for a fresh access token and persist the
/// updated session. Preserves the existing refresh token / account label when the
/// server doesn't re-issue them (refresh-token rotation is optional in OAuth2).
/// Call only when [`needs_refresh`] is true.
pub(crate) async fn refresh_session(session: &StoredOsSession) -> Result<StoredOsSession> {
    let refresh_token = session
        .refresh_token
        .clone()
        .ok_or_else(|| anyhow!("no refresh token to refresh with"))?;
    let token = exchange_refresh_token(&session.address, &refresh_token).await?;
    let mut next = session_from_token_response(&session.address, token);
    if next.refresh_token.is_none() {
        next.refresh_token = Some(refresh_token);
    }
    if next.account_label.is_none() {
        next.account_label = session.account_label.clone();
    }
    save_session(&next)?;
    Ok(next)
}

async fn exchange_refresh_token(address: &str, refresh_token: &str) -> Result<OAuthTokenResponse> {
    let token_url = build_token_url(address);
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .context("build OS OAuth2 HTTP client")?;
    let response = client
        .post(&token_url)
        .header("accept", "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", OAUTH_CLIENT_ID),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .with_context(|| format!("send OAuth2 refresh request to {token_url}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("read OAuth2 refresh response body")?;

    if !status.is_success() {
        if let Ok(error) = serde_json::from_str::<OAuthTokenError>(&body) {
            let description = error.error_description.unwrap_or_default();
            anyhow::bail!(
                "OAuth2 refresh failed at {token_url} (HTTP {}): {} {}",
                status.as_u16(),
                error.error,
                description
            );
        }
        anyhow::bail!(
            "OAuth2 refresh failed at {token_url} (HTTP {}): {body}",
            status.as_u16()
        );
    }

    serde_json::from_str::<OAuthTokenResponse>(&body)
        .with_context(|| format!("parse OAuth2 refresh response from {token_url}"))
}

/// The OS origin (`scheme://host[:port]`). The unified AI gateway's
/// OpenAI-compatible API is host-absolute (`/v1/chat/completions`), so we route
/// to the origin, not the (possibly path-suffixed) configured platform address.
pub(crate) fn os_origin(address: &str) -> String {
    match address.find("://") {
        Some(scheme_end) => {
            let after = &address[scheme_end + 3..];
            let end = after
                .find('/')
                .map_or(address.len(), |j| scheme_end + 3 + j);
            address[..end].to_string()
        }
        None => address.trim_end_matches('/').to_string(),
    }
}

/// One model advertised by the OS unified AI gateway, plus its real context
/// window when the gateway reports it (so the CLI can size auto-compact + the
/// status bar correctly instead of assuming a default).
#[derive(Clone, Debug)]
pub(crate) struct GatewayModel {
    pub id: String,
    pub context: Option<u32>,
}

/// List the OS unified AI gateway's runtime-callable models via
/// `GET {origin}/api/v1/llm/models` (Bearer = the OS token). These are not model
/// assets from the digital asset repository; the gateway is "gateway-managed" (it
/// holds the real provider keys; callers send only the OS token + a model id).
///
/// Returns a precise `Err` on failure so the `/model` picker can say WHY the
/// gateway is unavailable — in particular it distinguishes an HTML/SPA response
/// (the OS origin doesn't proxy `/v1/*` to the gateway) from a genuine empty
/// list, an auth error, or an unreachable host. `Ok(vec![])` means the gateway
/// really has no models configured.
pub(crate) async fn fetch_gateway_models(
    address: &str,
    token: &str,
) -> Result<Vec<GatewayModel>, String> {
    // Go through the OS backend's authenticated proxy (`/api/v1/llm/*`), which
    // validates the OS token then forwards to the internal gateway. `/api` is
    // reverse-proxied, unlike a bare `/v1`.
    let origin = os_origin(address);
    let url = format!("{origin}/api/v1/llm/models");
    let client = http_client_for_origin(&origin).map_err(|e| e.to_string())?;
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("request to {url} failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("{url} returned HTTP {}", status.as_u16()));
    }
    // OpenAI shape: { "data": [ { "id": "...", <optional context field> }, ... ] }.
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        if text.trim_start().starts_with('<') {
            format!(
                "{url} returned HTML, not JSON — the OS build may predate the LLM gateway proxy (/api/v1/llm)"
            )
        } else {
            format!("{url} returned a non-JSON response")
        }
    })?;
    let data = json
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| format!("{url} response had no `data` array"))?;
    Ok(data
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(|i| i.as_str())?.to_string();
            Some(GatewayModel {
                id,
                context: parse_gateway_context(m),
            })
        })
        .collect())
}

/// Opportunistically read a model's context window from the several field names
/// OpenAI-compatible gateways use (LiteLLM / one-api / vLLM differ). Absent →
/// `None`, and the caller keeps its default. Never fails.
fn parse_gateway_context(m: &serde_json::Value) -> Option<u32> {
    const KEYS: &[&str] = &[
        "context_length",
        "max_context_length",
        "max_model_len",
        "context_window",
        "max_input_tokens",
    ];
    for key in KEYS {
        if let Some(n) = m.get(key).and_then(|v| v.as_u64()).filter(|n| *n > 0) {
            return Some(n as u32);
        }
    }
    // LiteLLM nests it under `model_info`.
    m.get("model_info")
        .and_then(|mi| mi.get("max_input_tokens"))
        .and_then(|v| v.as_u64())
        .filter(|n| *n > 0)
        .map(|n| n as u32)
}

fn build_authorization_url(
    address: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    let base = normalize_address(address);
    let authorize =
        if base.contains('?') || base.trim_end_matches('/').ends_with("/oauth/authorize") {
            base
        } else {
            format!("{}/oauth/authorize", base.trim_end_matches('/'))
        };
    let sep = if authorize.contains('?') { '&' } else { '?' };
    format!(
        "{authorize}{sep}response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256&scope={}",
        percent_encode(OAUTH_CLIENT_ID),
        percent_encode(redirect_uri),
        percent_encode(state),
        percent_encode(code_challenge),
        percent_encode(OAUTH_SCOPE)
    )
}

fn build_token_url(address: &str) -> String {
    let base = normalize_address(address);
    if base.ends_with("/api/v1") {
        format!("{base}/oauth/token")
    } else {
        format!("{}/api/v1/oauth/token", base.trim_end_matches('/'))
    }
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status();
    #[cfg(target_os = "linux")]
    let status = std::process::Command::new("xdg-open").arg(url).status();
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let status: std::io::Result<std::process::ExitStatus> = Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "unsupported OS",
    ));

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(anyhow!("browser opener exited with {status}")),
        Err(error) => Err(error.into()),
    }
}

fn save_session(session: &StoredOsSession) -> Result<()> {
    let path = auth_store_path()?;
    save_session_at(&path, session)
}

fn save_session_at(path: &Path, session: &StoredOsSession) -> Result<()> {
    let mut store = read_store(path)?;
    store
        .sessions
        .retain(|item| item.address != session.address);
    store.sessions.push(session.clone());
    store.sessions.sort_by(|a, b| a.address.cmp(&b.address));
    write_store(path, &store)
}

fn remove_session_at(path: &Path, address: &str) -> Result<bool> {
    let mut store = read_store(path)?;
    let before = store.sessions.len();
    store.sessions.retain(|item| item.address != address);
    let removed = store.sessions.len() != before;
    if store.sessions.is_empty() {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(removed);
    }
    write_store(path, &store)?;
    Ok(removed)
}

fn read_store(path: &Path) -> Result<OsAuthStore> {
    if !path.exists() {
        return Ok(OsAuthStore::default());
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read OS auth store {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse OS auth store {}", path.display()))
}

fn write_store(path: &Path, store: &OsAuthStore) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(store)?;
    std::fs::write(path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn auth_store_path() -> Result<PathBuf> {
    let home = crate::user_paths::user_home_dir()
        .ok_or_else(|| anyhow!("user home is unavailable; cannot store OS login"))?;
    Ok(home.join(".a3s").join(STORE_FILE))
}

fn validate_address(address: &str) -> Result<()> {
    let address = address.trim();
    if address.starts_with("https://") || address.starts_with("http://") {
        Ok(())
    } else {
        anyhow::bail!("OS address must start with http:// or https://");
    }
}

fn normalize_address(address: &str) -> String {
    address.trim().trim_end_matches('/').to_string()
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'%' => return None,
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn pkce_verifier() -> String {
    random_url_token(32)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn random_url_token(bytes_len: usize) -> String {
    let mut bytes = vec![0_u8; bytes_len];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
