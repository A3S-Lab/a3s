use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::dirs;
use crate::error::Result;

/// User-configurable settings for the Power server and CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerConfig {
    /// Host address for the HTTP server (default: 127.0.0.1)
    #[serde(default = "default_host")]
    pub host: String,

    /// Port for the HTTP server (default: 11435)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Base directory for model storage
    #[serde(default = "dirs::power_home")]
    pub data_dir: PathBuf,

    /// Maximum number of models to keep loaded in memory
    #[serde(default = "default_max_loaded_models")]
    pub max_loaded_models: usize,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    11435
}

fn default_max_loaded_models() -> usize {
    1
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            data_dir: dirs::power_home(),
            max_loaded_models: default_max_loaded_models(),
        }
    }
}

impl PowerConfig {
    /// Load configuration from the default config file path.
    /// Returns default config if the file does not exist.
    pub fn load() -> Result<Self> {
        let path = dirs::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(|e| {
                crate::error::PowerError::Config(format!(
                    "Failed to read config file {}: {}",
                    path.display(),
                    e
                ))
            })?;
            let config: PowerConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save the current configuration to the default config file path.
    pub fn save(&self) -> Result<()> {
        let path = dirs::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Returns the server bind address string (e.g., "127.0.0.1:11435").
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PowerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 11435);
        assert_eq!(config.max_loaded_models, 1);
    }

    #[test]
    fn test_bind_address() {
        let config = PowerConfig::default();
        assert_eq!(config.bind_address(), "127.0.0.1:11435");
    }

    #[test]
    fn test_config_deserialize() {
        let toml_str = r#"
            host = "0.0.0.0"
            port = 8080
            max_loaded_models = 3
        "#;
        let config: PowerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.max_loaded_models, 3);
    }

    #[test]
    fn test_config_serialize() {
        let config = PowerConfig::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        assert!(serialized.contains("host"));
        assert!(serialized.contains("port"));
    }

    #[test]
    fn test_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let config = PowerConfig {
            host: "0.0.0.0".to_string(),
            port: 9999,
            data_dir: dir.path().to_path_buf(),
            max_loaded_models: 5,
        };
        config.save().unwrap();

        let loaded = PowerConfig::load().unwrap();
        assert_eq!(loaded.host, "0.0.0.0");
        assert_eq!(loaded.port, 9999);
        assert_eq!(loaded.max_loaded_models, 5);

        std::env::remove_var("A3S_POWER_HOME");
    }
}
