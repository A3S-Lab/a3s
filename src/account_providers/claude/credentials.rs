use crate::user_paths::user_home_dir;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct ClaudeCredentials {
    pub(crate) access_token: String,
    // Read only in the macOS keychain path (`expires_soon`); dead on other OSes.
    #[allow(dead_code)]
    expires_at_ms: Option<u64>,
}

impl ClaudeCredentials {
    pub(crate) fn from_disk() -> Result<Self> {
        if let Some(access_token) =
            env_token("CLAUDE_CODE_OAUTH_TOKEN").or_else(|| env_token("ANTHROPIC_AUTH_TOKEN"))
        {
            return Ok(Self {
                access_token,
                expires_at_ms: None,
            });
        }

        for path in claude_credentials_paths() {
            if let Ok(credentials) = Self::from_file(&path) {
                return Ok(credentials);
            }
        }

        #[cfg(target_os = "macos")]
        if let Ok(credentials) = Self::from_macos_keychain() {
            return Ok(credentials);
        }

        Err(anyhow!(
            "no Claude OAuth access token found; run `claude auth login` or `claude setup-token`"
        ))
    }

    fn from_file(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_json_str(&raw).with_context(|| format!("read {}", path.display()))
    }

    fn from_json_str(raw: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(raw).context("parse Claude credentials")?;
        Self::from_value(&value)
    }

    fn from_value(value: &Value) -> Result<Self> {
        let access_token = credential_string(
            value,
            &[
                "/claudeAiOauth/accessToken",
                "/claudeAiOauth/access_token",
                "/oauth/accessToken",
                "/oauth/access_token",
                "/tokens/accessToken",
                "/tokens/access_token",
                "/accessToken",
                "/access_token",
            ],
        )
        .ok_or_else(|| anyhow!("no Claude OAuth access token found; run `claude /login`"))?;
        Ok(Self {
            access_token,
            expires_at_ms: credential_u64(value, &["/claudeAiOauth/expiresAt", "/oauth/expiresAt"]),
        })
    }

    // Only called from the macOS keychain path; dead code elsewhere.
    #[allow(dead_code)]
    fn expires_soon(&self) -> bool {
        let Some(expires_at_ms) = self.expires_at_ms else {
            return false;
        };
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default();
        expires_at_ms <= now_ms.saturating_add(60_000)
    }

    #[cfg(target_os = "macos")]
    fn from_macos_keychain() -> Result<Self> {
        let credentials = Self::read_macos_keychain()?;
        if !credentials.expires_soon() {
            return Ok(credentials);
        }

        let _ = Command::new("claude")
            .args(["auth", "status", "--json"])
            .output();
        Self::read_macos_keychain().or(Ok(credentials))
    }

    #[cfg(target_os = "macos")]
    fn read_macos_keychain() -> Result<Self> {
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-w",
                "-s",
                "Claude Code-credentials",
            ])
            .output()
            .context("read Claude Code credentials from macOS Keychain")?;
        if !output.status.success() {
            return Err(anyhow!(
                "Claude Code credentials not found in macOS Keychain"
            ));
        }
        let raw = String::from_utf8(output.stdout).context("decode keychain credentials")?;
        Self::from_json_str(raw.trim())
    }
}

pub(crate) fn claude_config_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| user_home_dir().map(|home| home.join(".claude")))
}

pub(crate) fn claude_credentials_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    claude_config_dir()
        .map(|dir| dir.join(".credentials.json"))
        .into_iter()
        .for_each(|path| paths.push(path));
    if let Some(home) = user_home_dir() {
        paths.push(home.join(".claude.json"));
    }
    paths
}

pub(crate) fn has_claude_login() -> bool {
    if env_token("CLAUDE_CODE_OAUTH_TOKEN")
        .or_else(|| env_token("ANTHROPIC_AUTH_TOKEN"))
        .is_some()
    {
        return true;
    }

    if claude_credentials_paths()
        .iter()
        .any(|path| ClaudeCredentials::from_file(path).is_ok())
    {
        return true;
    }

    #[cfg(target_os = "macos")]
    {
        if ClaudeCredentials::from_macos_keychain().is_ok() {
            return true;
        }
    }

    false
}

fn env_token(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn credential_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .filter_map(|pointer| value.pointer(pointer))
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

fn credential_u64(value: &Value, pointers: &[&str]) -> Option<u64> {
    pointers.iter().find_map(|pointer| {
        let value = value.pointer(pointer)?;
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_claude_credentials_access_token() {
        let root = std::env::temp_dir().join(format!(
            "a3s-claude-credentials-test-{}",
            std::process::id()
        ));
        let path = root.join(".credentials.json");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            &path,
            r#"{"claudeAiOauth":{"accessToken":"test-access-token"}}"#,
        )
        .unwrap();

        let credentials = ClaudeCredentials::from_file(&path).unwrap();

        assert_eq!(credentials.access_token, "test-access-token");
        assert_eq!(credentials.expires_at_ms, None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reads_claude_keychain_credential_shape() {
        let credentials = ClaudeCredentials::from_json_str(
            r#"{"claudeAiOauth":{"accessToken":"test-access-token","refreshToken":"refresh","expiresAt":4102444800000}}"#,
        )
        .unwrap();

        assert_eq!(credentials.access_token, "test-access-token");
        assert_eq!(credentials.expires_at_ms, Some(4_102_444_800_000));
        assert!(!credentials.expires_soon());
    }

    #[test]
    fn reads_claude_token_from_env() {
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let old_claude = std::env::var_os("CLAUDE_CODE_OAUTH_TOKEN");
        let old_anthropic = std::env::var_os("ANTHROPIC_AUTH_TOKEN");
        std::env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "env-token");
        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");

        let credentials = ClaudeCredentials::from_disk().unwrap();

        restore_var("CLAUDE_CODE_OAUTH_TOKEN", old_claude);
        restore_var("ANTHROPIC_AUTH_TOKEN", old_anthropic);
        assert_eq!(credentials.access_token, "env-token");
    }

    fn restore_var(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
