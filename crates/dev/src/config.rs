use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use indexmap::IndexMap;
use serde::Deserialize;

use crate::error::{DevError, Result};

#[derive(Debug, Deserialize)]
pub struct DevConfig {
    #[serde(default)]
    pub dev: GlobalSettings,
    #[serde(default)]
    pub service: IndexMap<String, ServiceDef>,
}

#[derive(Debug, Deserialize)]
pub struct GlobalSettings {
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            proxy_port: default_proxy_port(),
            log_level: default_log_level(),
        }
    }
}

fn default_proxy_port() -> u16 {
    7080
}
fn default_log_level() -> String {
    "info".into()
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceDef {
    pub cmd: String,
    #[serde(default)]
    pub dir: Option<PathBuf>,
    pub port: u16,
    #[serde(default)]
    pub subdomain: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub watch: Option<WatchConfig>,
    #[serde(default)]
    pub health: Option<HealthConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WatchConfig {
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default = "default_true")]
    pub restart: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct HealthConfig {
    #[serde(rename = "type")]
    pub kind: HealthKind,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default = "default_interval", with = "duration_serde")]
    pub interval: Duration,
    #[serde(default = "default_timeout", with = "duration_serde")]
    pub timeout: Duration,
    #[serde(default = "default_retries")]
    pub retries: u32,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthKind {
    Http,
    Tcp,
}

fn default_interval() -> Duration {
    Duration::from_secs(2)
}
fn default_timeout() -> Duration {
    Duration::from_secs(1)
}
fn default_retries() -> u32 {
    3
}

mod duration_serde {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let s = String::deserialize(d)?;
        parse_duration(&s).map_err(serde::de::Error::custom)
    }

    fn parse_duration(s: &str) -> Result<Duration, String> {
        if let Some(v) = s.strip_suffix("ms") {
            return v
                .trim()
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|e| e.to_string());
        }
        if let Some(v) = s.strip_suffix('s') {
            return v
                .trim()
                .parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|e| e.to_string());
        }
        Err(format!("unknown duration format: '{s}' (use '2s' or '500ms')"))
    }
}

impl DevConfig {
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| DevError::Config(format!("cannot read {}: {e}", path.display())))?;
        let cfg: DevConfig = hcl::from_str(&src)
            .map_err(|e| DevError::Config(format!("parse error in {}: {e}", path.display())))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<()> {
        // Port conflict check
        let mut seen: HashMap<u16, &str> = HashMap::new();
        for (name, svc) in &self.service {
            if let Some(other) = seen.insert(svc.port, name.as_str()) {
                return Err(DevError::PortConflict {
                    a: other.to_string(),
                    b: name.clone(),
                    port: svc.port,
                });
            }
        }
        // Unknown depends_on references
        for (name, svc) in &self.service {
            for dep in &svc.depends_on {
                if !self.service.contains_key(dep) {
                    return Err(DevError::Config(format!(
                        "service '{name}' depends_on unknown service '{dep}'"
                    )));
                }
            }
        }
        Ok(())
    }
}
