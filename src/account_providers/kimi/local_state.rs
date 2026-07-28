//! Discovery and bounded parsing of Kimi-owned local account state.

use super::{
    model_metadata, now_unix_seconds, valid_model_id, KimiCredentials, KimiModelMetadata,
    DEFAULT_DESKTOP_BASE_URL, DEFAULT_MODEL_CONTEXT, MAX_CREDENTIAL_BYTES, MAX_MODELS_BYTES,
};
use crate::user_paths::user_home_dir;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct DesktopKimiKey {
    #[serde(rename = "apiKey")]
    api_key: String,
}

#[derive(Default, Deserialize)]
struct DesktopDaimonConfig {
    #[serde(default)]
    model: DesktopModelConfig,
    #[serde(default, rename = "kimiCode")]
    kimi_code: DesktopKimiCodeConfig,
}

#[derive(Default, Deserialize)]
struct DesktopModelConfig {
    #[serde(default)]
    current: String,
    #[serde(default)]
    providers: HashMap<String, DesktopProviderConfig>,
    #[serde(default)]
    models: HashMap<String, DesktopModelEntry>,
}

#[derive(Deserialize)]
struct DesktopProviderConfig {
    #[serde(rename = "type")]
    provider_type: String,
    #[serde(default, rename = "baseUrl")]
    base_url: String,
}

#[derive(Deserialize)]
struct DesktopModelEntry {
    provider: String,
    model: String,
    #[serde(default, rename = "maxContextSize")]
    max_context_size: u64,
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Default, Deserialize)]
struct DesktopKimiCodeConfig {
    #[serde(default, rename = "kimiRequestHeaders")]
    kimi_request_headers: DesktopRequestHeaders,
}

#[derive(Default, Deserialize)]
struct DesktopRequestHeaders {
    #[serde(default, rename = "User-Agent")]
    user_agent: String,
}

pub(super) struct DesktopAccountModel {
    pub(super) id: String,
    metadata: KimiModelMetadata,
}

pub(super) struct DesktopAccount {
    pub(super) root: PathBuf,
    pub(super) key_path: PathBuf,
    pub(super) base_url: String,
    pub(super) identity_headers: HashMap<String, String>,
    pub(super) models: Vec<DesktopAccountModel>,
}

pub(super) async fn read_desktop_api_key(path: &Path) -> Result<String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("read Kimi desktop credential metadata {}", path.display()))?;
    if metadata.len() > MAX_CREDENTIAL_BYTES {
        bail!("Kimi desktop credential file is unexpectedly large");
    }
    let raw = tokio::fs::read(path)
        .await
        .with_context(|| format!("read Kimi desktop credentials {}", path.display()))?;
    let key: DesktopKimiKey =
        serde_json::from_slice(&raw).context("parse Kimi desktop credentials")?;
    let api_key = key.api_key.trim();
    if api_key.is_empty() || api_key.len() > 4096 {
        bail!("Kimi desktop credential is empty or invalid; reopen Kimi and sign in again");
    }
    Ok(api_key.to_string())
}

pub(super) fn locate_desktop_account() -> Option<DesktopAccount> {
    for root in kimi_desktop_home_candidates() {
        let key_path = root.join("daimon/kimi-code-key.json");
        if read_desktop_api_key_sync(&key_path).is_none() {
            continue;
        }
        let config_path = root.join("daimon/config.json");
        let Some(config) = read_desktop_config(&config_path) else {
            continue;
        };

        let current_provider = config
            .model
            .models
            .get(&config.model.current)
            .map(|model| model.provider.as_str())
            .filter(|provider| {
                config
                    .model
                    .providers
                    .get(*provider)
                    .is_some_and(|entry| entry.provider_type.eq_ignore_ascii_case("kimi"))
            });
        let Some(provider_name) = current_provider.map(str::to_string).or_else(|| {
            let mut names = config
                .model
                .providers
                .iter()
                .filter(|(_, provider)| provider.provider_type.eq_ignore_ascii_case("kimi"))
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>();
            names.sort();
            names.into_iter().next()
        }) else {
            continue;
        };
        let Some(provider) = config.model.providers.get(&provider_name) else {
            continue;
        };
        let configured_base_url = provider.base_url.trim();
        let base_url = endpoint_from_env(
            "A3S_KIMI_BASE_URL",
            "KIMI_CODE_BASE_URL",
            if configured_base_url.is_empty() {
                DEFAULT_DESKTOP_BASE_URL
            } else {
                configured_base_url
            },
        );
        if trim_endpoint(base_url.clone()).is_empty() {
            continue;
        }

        let mut models = config
            .model
            .models
            .values()
            .filter(|model| model.provider == provider_name)
            .filter_map(|model| {
                let id = model.model.trim();
                if !valid_model_id(id) {
                    return None;
                }
                let context = u32::try_from(model.max_context_size)
                    .ok()
                    .filter(|context| *context > 0)
                    .unwrap_or(DEFAULT_MODEL_CONTEXT);
                let thinking = model
                    .capabilities
                    .iter()
                    .any(|capability| capability.eq_ignore_ascii_case("thinking"));
                Some(DesktopAccountModel {
                    id: id.to_string(),
                    metadata: KimiModelMetadata { context, thinking },
                })
            })
            .collect::<Vec<_>>();
        models.sort_by(|left, right| left.id.cmp(&right.id));
        models.dedup_by(|left, right| left.id == right.id);
        if models.is_empty() {
            continue;
        }
        if let Ok(mut metadata) = model_metadata().write() {
            for model in &models {
                metadata.insert(model.id.clone(), model.metadata);
            }
        }

        let mut identity_headers = HashMap::new();
        let user_agent = config.kimi_code.kimi_request_headers.user_agent.trim();
        if !user_agent.is_empty() && user_agent.is_ascii() && user_agent.len() <= 256 {
            identity_headers.insert("User-Agent".to_string(), user_agent.to_string());
        }
        return Some(DesktopAccount {
            root,
            key_path,
            base_url,
            identity_headers,
            models,
        });
    }
    None
}

fn read_desktop_config(path: &Path) -> Option<DesktopDaimonConfig> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_MODELS_BYTES as u64 {
        return None;
    }
    serde_json::from_reader(BufReader::new(file)).ok()
}

fn read_desktop_api_key_sync(path: &Path) -> Option<String> {
    let raw = read_bounded_file(path, MAX_CREDENTIAL_BYTES)?;
    let key = serde_json::from_slice::<DesktopKimiKey>(&raw).ok()?;
    let api_key = key.api_key.trim();
    (!api_key.is_empty() && api_key.len() <= 4096).then(|| api_key.to_string())
}

fn read_bounded_file(path: &Path, max_bytes: u64) -> Option<Vec<u8>> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return None;
    }
    std::fs::read(path).ok()
}

pub(super) fn locate_credentials() -> Option<(PathBuf, PathBuf, KimiCredentials)> {
    for home in kimi_home_candidates() {
        let path = home.join("credentials/kimi-code.json");
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        if metadata.len() > MAX_CREDENTIAL_BYTES {
            continue;
        }
        let Ok(raw) = std::fs::read(&path) else {
            continue;
        };
        let Ok(credentials) = serde_json::from_slice::<KimiCredentials>(&raw) else {
            continue;
        };
        if credentials.has_reusable_login(now_unix_seconds()) {
            return Some((home, path, credentials));
        }
    }
    None
}

fn kimi_home_candidates() -> Vec<PathBuf> {
    if let Some(home) = non_empty_env("A3S_KIMI_HOME") {
        return vec![PathBuf::from(home)];
    }
    let mut homes = Vec::new();
    for name in ["KIMI_CODE_HOME", "KIMI_SHARE_DIR"] {
        if let Some(home) = non_empty_env(name) {
            push_unique(&mut homes, PathBuf::from(home));
        }
    }
    if let Some(home) = user_home_dir() {
        push_unique(&mut homes, home.join(".kimi-code"));
        push_unique(&mut homes, home.join(".kimi"));
    }
    homes
}

fn kimi_desktop_home_candidates() -> Vec<PathBuf> {
    if let Some(home) = non_empty_env("A3S_KIMI_DESKTOP_HOME") {
        return vec![PathBuf::from(home)];
    }
    let mut homes = Vec::new();
    if let Some(home) = non_empty_env("KIMI_DESKTOP_HOME") {
        push_unique(&mut homes, PathBuf::from(home));
    }
    if let Some(home) = user_home_dir() {
        #[cfg(target_os = "macos")]
        push_unique(
            &mut homes,
            home.join("Library/Application Support/kimi-desktop/daimon-share"),
        );
        push_unique(&mut homes, home.join(".config/kimi-desktop/daimon-share"));
    }
    if let Some(config_home) = non_empty_env("XDG_CONFIG_HOME") {
        push_unique(
            &mut homes,
            Path::new(&config_home).join("kimi-desktop/daimon-share"),
        );
    }
    if let Some(app_data) = non_empty_env("APPDATA") {
        push_unique(
            &mut homes,
            Path::new(&app_data).join("kimi-desktop/daimon-share"),
        );
    }
    homes
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

fn non_empty_env(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

pub(super) fn endpoint_from_env(a3s_name: &str, kimi_name: &str, default: &str) -> String {
    non_empty_env(a3s_name)
        .or_else(|| non_empty_env(kimi_name))
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| default.to_string())
}

pub(super) fn trim_endpoint(endpoint: String) -> String {
    endpoint.trim().trim_end_matches('/').to_string()
}

pub(super) fn identity_headers(home_dir: &Path) -> HashMap<String, String> {
    let mut headers = HashMap::from([
        ("X-Msh-Platform".to_string(), "kimi_code_cli".to_string()),
        (
            "X-Msh-Version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
    ]);
    if let Ok(device_id) = std::fs::read_to_string(home_dir.join("device_id")) {
        let device_id = device_id.trim();
        if !device_id.is_empty() && device_id.is_ascii() && device_id.len() <= 128 {
            headers.insert("X-Msh-Device-Id".to_string(), device_id.to_string());
        }
    }
    headers
}
