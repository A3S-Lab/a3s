//! ChatGPT account credentials shared by the Codex transports.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tokio::sync::Mutex;

const REFRESH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const REFRESH_TOKEN_URL_OVERRIDE: &str = "CODEX_REFRESH_TOKEN_URL_OVERRIDE";
const CLIENT_ID_OVERRIDE: &str = "CODEX_APP_SERVER_LOGIN_CLIENT_ID";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AuthCredentials {
    pub(super) access_token: String,
    pub(super) account_id: String,
}

pub(super) struct AuthState {
    path: Option<PathBuf>,
    credentials: RwLock<AuthCredentials>,
    refresh_lock: Mutex<()>,
    refresh_endpoint: String,
    refresh_client: reqwest::Client,
}

impl AuthState {
    pub(super) fn load(path: PathBuf) -> Result<Self> {
        let value = read_auth_value(&path)?;
        let credentials = credentials_from_value(&value, &path)?;
        Ok(Self {
            path: Some(path),
            credentials: RwLock::new(credentials),
            refresh_lock: Mutex::new(()),
            refresh_endpoint: std::env::var(REFRESH_TOKEN_URL_OVERRIDE)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| REFRESH_TOKEN_URL.to_string()),
            refresh_client: build_refresh_client()?,
        })
    }

    #[cfg(test)]
    pub(super) fn for_test(access_token: &str, account_id: &str) -> Self {
        Self {
            path: None,
            credentials: RwLock::new(AuthCredentials {
                access_token: access_token.to_string(),
                account_id: account_id.to_string(),
            }),
            refresh_lock: Mutex::new(()),
            refresh_endpoint: "http://127.0.0.1:1/oauth/token".to_string(),
            refresh_client: reqwest::Client::new(),
        }
    }

    #[cfg(test)]
    fn load_with_endpoint(path: PathBuf, refresh_endpoint: String) -> Result<Self> {
        let mut state = Self::load(path)?;
        state.refresh_endpoint = refresh_endpoint;
        Ok(state)
    }

    pub(super) fn credentials(&self) -> AuthCredentials {
        self.credentials
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Recover once from an unauthorized model request. The auth file is read
    /// before using the refresh token because an official Codex process may
    /// already have rotated the credentials.
    pub(super) async fn refresh_after_unauthorized(&self, rejected_token: &str) -> Result<()> {
        let _guard = self.refresh_lock.lock().await;
        if self.credentials().access_token != rejected_token {
            return Ok(());
        }

        let path = self.path.as_ref().ok_or_else(|| {
            anyhow!("Codex access token is invalid; run `codex login` to sign in again")
        })?;
        let mut value = read_auth_value(path)?;
        let disk_credentials = credentials_from_value(&value, path)?;
        if disk_credentials.access_token != rejected_token {
            *self
                .credentials
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = disk_credentials;
            return Ok(());
        }

        let refresh_token = value
            .pointer("/tokens/refresh_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                anyhow!("Codex refresh token is unavailable; run `codex login` to sign in again")
            })?
            .to_string();
        let response = self
            .refresh_client
            .post(&self.refresh_endpoint)
            .header("Content-Type", "application/json")
            .json(&RefreshRequest {
                client_id: oauth_client_id(),
                grant_type: "refresh_token",
                refresh_token,
            })
            .send()
            .await
            .context("send Codex token refresh request")?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!(refresh_failure_message(status.as_u16(), &body)));
        }
        let refreshed: RefreshResponse =
            serde_json::from_str(&body).context("decode Codex token refresh response")?;
        let access_token = refreshed.access_token.ok_or_else(|| {
            anyhow!("Codex token refresh succeeded without an access token; run `codex login`")
        })?;

        set_json_string(&mut value, "/tokens/access_token", access_token)?;
        if let Some(id_token) = refreshed.id_token {
            set_json_string(&mut value, "/tokens/id_token", id_token)?;
        }
        if let Some(refresh_token) = refreshed.refresh_token {
            set_json_string(&mut value, "/tokens/refresh_token", refresh_token)?;
        }
        if let Some(root) = value.as_object_mut() {
            root.insert(
                "last_refresh".to_string(),
                Value::String(chrono::Utc::now().to_rfc3339()),
            );
        }
        persist_auth_value(path, &value)?;
        let credentials = credentials_from_value(&value, path)?;
        *self
            .credentials
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = credentials;
        Ok(())
    }
}

#[derive(Serialize)]
struct RefreshRequest {
    client_id: String,
    grant_type: &'static str,
    refresh_token: String,
}

#[derive(serde::Deserialize)]
struct RefreshResponse {
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

fn read_auth_value(path: &Path) -> Result<Value> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read {} (run `codex login`)", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn credentials_from_value(value: &Value, path: &Path) -> Result<AuthCredentials> {
    let access_token = value
        .pointer("/tokens/access_token")
        .or_else(|| value.get("access_token"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("no access_token in {} — run `codex login`", path.display()))?
        .to_string();
    let account_id = value
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .pointer("/tokens/id_token")
                .and_then(Value::as_str)
                .and_then(account_id_from_id_token)
        })
        .ok_or_else(|| {
            anyhow!(
                "no ChatGPT account id in {} — re-run `codex login`",
                path.display()
            )
        })?;
    Ok(AuthCredentials {
        access_token,
        account_id,
    })
}

fn set_json_string(value: &mut Value, pointer: &str, replacement: String) -> Result<()> {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| anyhow!("Codex auth file is missing {pointer}"))?;
    *target = Value::String(replacement);
    Ok(())
}

fn persist_auth_value(path: &Path, value: &Value) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Codex auth path has no parent: {}", path.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary auth file in {}", parent.display()))?;
    let mut encoded = serde_json::to_vec_pretty(value).context("encode refreshed Codex auth")?;
    encoded.push(b'\n');
    temporary
        .write_all(&encoded)
        .context("write refreshed Codex auth")?;
    temporary.flush().context("flush refreshed Codex auth")?;

    #[cfg(unix)]
    if let Ok(metadata) = std::fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .context("preserve Codex auth permissions")?;
    }

    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("replace refreshed Codex auth at {}", path.display()))?;
    Ok(())
}

fn build_refresh_client() -> Result<reqwest::Client> {
    let roots = super::tls::TlsRoots::load()?;
    let builder = roots.add_to_reqwest(reqwest::Client::builder())?;
    builder.build().context("build Codex auth HTTP client")
}

fn oauth_client_id() -> String {
    std::env::var(CLIENT_ID_OVERRIDE)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| CLIENT_ID.to_string())
}

fn refresh_failure_message(status: u16, body: &str) -> String {
    let value = serde_json::from_str::<Value>(body).ok();
    let code = value
        .as_ref()
        .and_then(|value| {
            value
                .pointer("/error/code")
                .or_else(|| value.get("error"))
                .or_else(|| value.get("code"))
        })
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match code {
        "refresh_token_expired" => {
            "Codex refresh token expired; run `codex login` to sign in again".to_string()
        }
        "refresh_token_reused" | "refresh_token_invalidated" => {
            "Codex refresh token is no longer valid; run `codex login` to sign in again".to_string()
        }
        _ => format!(
            "Codex token refresh failed with HTTP {status}; run `codex login` if the problem persists"
        ),
    }
}

pub(super) fn account_id_from_id_token(jwt: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .pointer("/https:~1~1api.openai.com~1auth~1chatgpt_account_id")
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn auth_value(access: &str, refresh: &str) -> Value {
        serde_json::json!({
            "tokens": {
                "access_token": access,
                "refresh_token": refresh,
                "account_id": "account"
            }
        })
    }

    #[tokio::test]
    async fn reloads_token_rotated_by_another_process_before_refreshing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("auth.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&auth_value("old", "refresh")).unwrap(),
        )
        .unwrap();
        let state = AuthState::load_with_endpoint(
            path.clone(),
            "http://127.0.0.1:1/oauth/token".to_string(),
        )
        .unwrap();
        std::fs::write(
            &path,
            serde_json::to_vec(&auth_value("new", "refresh")).unwrap(),
        )
        .unwrap();

        state.refresh_after_unauthorized("old").await.unwrap();

        assert_eq!(state.credentials().access_token, "new");
    }

    #[tokio::test]
    async fn refreshes_and_atomically_persists_rotated_tokens() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = socket.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let body = br#"{"access_token":"new-access","refresh_token":"new-refresh"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.write_all(body).await.unwrap();
        });

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("auth.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&auth_value("old", "refresh")).unwrap(),
        )
        .unwrap();
        let state =
            AuthState::load_with_endpoint(path.clone(), format!("http://{address}/oauth/token"))
                .unwrap();

        state.refresh_after_unauthorized("old").await.unwrap();
        server.await.unwrap();

        assert_eq!(state.credentials().access_token, "new-access");
        let persisted: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted["tokens"]["access_token"], "new-access");
        assert_eq!(persisted["tokens"]["refresh_token"], "new-refresh");
    }

    #[test]
    fn refresh_errors_never_include_backend_body() {
        let message = refresh_failure_message(
            403,
            r#"{"error":{"code":"private_code","message":"private detail"}}"#,
        );
        assert!(message.contains("HTTP 403"));
        assert!(!message.contains("private detail"));
    }
}
