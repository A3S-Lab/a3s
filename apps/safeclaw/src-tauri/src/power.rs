use a3s_power::config::PowerConfig;
use a3s_power::server::log_stream::{LogBuffer, LogBufferLayer};
use a3s_power::tee::attestation::{DefaultTeeProvider, TeeProvider};
use anyhow::Result;
use serde::Serialize;
#[cfg(target_os = "linux")]
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const HOST: &str = "127.0.0.1";
const PORT: u16 = 11435;

/// Process-global log buffer shared between the tracing subscriber layer and
/// the embedded Power server's AppState.  Initialized once before the tracing
/// subscriber is set up so that startup logs are also captured.
static LOG_BUFFER: OnceLock<LogBuffer> = OnceLock::new();

/// Return a reference to the global log buffer, initializing it on first call.
pub fn log_buffer() -> &'static LogBuffer {
    LOG_BUFFER.get_or_init(LogBuffer::new)
}

/// Return a new `LogBufferLayer` wired to the global log buffer.
///
/// Call this before `tracing_subscriber::registry().init()` and add it to the
/// subscriber chain so that all relevant log events are captured from startup.
pub fn log_buffer_layer() -> LogBufferLayer {
    LogBufferLayer::new(log_buffer().clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeProfile {
    Minimal,
    Balanced,
    HighMemory,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerRuntimeStatus {
    pub url: String,
    pub host: String,
    pub port: u16,
    pub inference_backend: String,
    pub profile: String,
    pub total_memory_gib: Option<u64>,
    pub tee_type: String,
    pub hardware_tee: bool,
}

impl RuntimeProfile {
    fn detect(total_memory_bytes: Option<u64>) -> Self {
        let gib = total_memory_bytes.unwrap_or(8 * 1024 * 1024 * 1024) / 1024 / 1024 / 1024;
        if gib >= 14 {
            Self::HighMemory
        } else if gib >= 10 {
            Self::Balanced
        } else {
            Self::Minimal
        }
    }

    fn keep_alive(self) -> &'static str {
        match self {
            Self::HighMemory => "10m",
            Self::Balanced => "5m",
            Self::Minimal => "3m",
        }
    }

    fn num_parallel(self) -> usize {
        match self {
            Self::HighMemory => 2,
            Self::Balanced | Self::Minimal => 1,
        }
    }

    fn max_threads(self, cpu_count: usize) -> u32 {
        let threads = match self {
            Self::HighMemory => cpu_count.saturating_sub(1).clamp(2, 8),
            Self::Balanced => cpu_count.saturating_sub(1).clamp(2, 6),
            Self::Minimal => cpu_count.saturating_sub(1).clamp(1, 4),
        };
        threads as u32
    }
}

fn power_data_dir() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("safeclaw").join("power")
    } else {
        PathBuf::from(".safeclaw").join("power")
    }
}

fn detect_total_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sysctl")
            .arg("-n")
            .arg("hw.memsize")
            .output()
        {
            if output.status.success() {
                if let Ok(raw) = String::from_utf8(output.stdout) {
                    if let Ok(value) = raw.trim().parse::<u64>() {
                        return Some(value);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let mut content = String::new();
        if let Ok(mut file) = std::fs::File::open("/proc/meminfo") {
            if file.read_to_string(&mut content).is_ok() {
                for line in content.lines() {
                    if let Some(rest) = line.strip_prefix("MemTotal:") {
                        let kb = rest
                            .split_whitespace()
                            .next()
                            .and_then(|v| v.parse::<u64>().ok())?;
                        return Some(kb * 1024);
                    }
                }
            }
        }
    }

    None
}

fn build_embedded_config() -> PowerConfig {
    let total_memory_bytes = detect_total_memory_bytes();
    let total_memory_gib = total_memory_bytes.map(|v| v / 1024 / 1024 / 1024);
    let profile = RuntimeProfile::detect(total_memory_bytes);
    let cpu_count = num_cpus::get();

    let config = PowerConfig {
        host: HOST.to_string(),
        port: PORT,
        data_dir: power_data_dir(),
        max_loaded_models: 1,
        keep_alive: profile.keep_alive().to_string(),
        num_thread: Some(profile.max_threads(cpu_count)),
        num_parallel: profile.num_parallel(),
        // Privacy baseline for all hosts, even without hardware TEE.
        tee_mode: true,
        redact_logs: true,
        in_memory_decrypt: true,
        streaming_decrypt: true,
        suppress_token_metrics: true,
        ..PowerConfig::default()
    };

    tracing::info!(
        profile = ?profile,
        memory_gib = total_memory_gib,
        cpu_count,
        host = %config.host,
        port = config.port,
        "Selected embedded Power runtime profile"
    );

    config
}

impl RuntimeProfile {
    fn as_name(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Balanced => "balanced",
            Self::HighMemory => "high-memory",
        }
    }
}

pub fn embedded_runtime_status() -> PowerRuntimeStatus {
    let total_memory_bytes = detect_total_memory_bytes();
    let total_memory_gib = total_memory_bytes.map(|v| v / 1024 / 1024 / 1024);
    let profile = RuntimeProfile::detect(total_memory_bytes);
    let tee_type = DefaultTeeProvider::detect().tee_type().to_string();
    let hardware_tee = tee_type == "sev-snp" || tee_type == "tdx";
    let inference_backend = preferred_backend_name();

    PowerRuntimeStatus {
        url: local_power_base_url(),
        host: HOST.to_string(),
        port: PORT,
        inference_backend,
        profile: profile.as_name().to_string(),
        total_memory_gib,
        tee_type,
        hardware_tee,
    }
}

fn preferred_backend_name() -> String {
    let registry = a3s_power::backend::default_backends(Arc::new(PowerConfig::default()));
    let names = registry.list_names();
    names
        .first()
        .map(|v| (*v).to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn is_port_listening(host: &str, port: u16) -> bool {
    let addr = format!("{host}:{port}");
    let mut addrs = match addr.to_socket_addrs() {
        Ok(v) => v,
        Err(_) => return false,
    };

    if let Some(sock_addr) = addrs.next() {
        TcpStream::connect_timeout(&sock_addr, Duration::from_millis(250)).is_ok()
    } else {
        false
    }
}

pub fn local_power_base_url() -> String {
    format!("http://{HOST}:{PORT}/v1")
}

pub async fn start_embedded_power() -> Result<()> {
    if is_port_listening(HOST, PORT) {
        tracing::info!(
            host = HOST,
            port = PORT,
            "Power already running, skip embedded startup"
        );
        return Ok(());
    }

    // Default to HuggingFace when the embedded server starts without an explicit
    // model source override.  The built-in presets use HuggingFace repo paths, so
    // model pull and filename resolution must target the same hub.
    if std::env::var("A3S_POWER_MODEL_SOURCE").is_err() {
        std::env::set_var("A3S_POWER_MODEL_SOURCE", "hf");
    }

    let config = build_embedded_config();
    tracing::info!(host = %config.host, port = config.port, "Starting embedded Power server");
    a3s_power::server::start_with_log_buffer(config, Some(log_buffer().clone())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_selection_matches_memory_budget() {
        assert_eq!(
            RuntimeProfile::detect(Some(16 * 1024 * 1024 * 1024)),
            RuntimeProfile::HighMemory
        );
        assert_eq!(
            RuntimeProfile::detect(Some(12 * 1024 * 1024 * 1024)),
            RuntimeProfile::Balanced
        );
        assert_eq!(
            RuntimeProfile::detect(Some(8 * 1024 * 1024 * 1024)),
            RuntimeProfile::Minimal
        );
    }

    #[test]
    fn profile_thread_caps_stay_bounded() {
        assert_eq!(RuntimeProfile::HighMemory.max_threads(32), 8);
        assert_eq!(RuntimeProfile::Balanced.max_threads(2), 2);
        assert_eq!(RuntimeProfile::Minimal.max_threads(1), 1);
    }

    #[test]
    fn power_url_points_to_embedded_server() {
        assert_eq!(local_power_base_url(), "http://127.0.0.1:11435/v1");
    }

    #[test]
    fn runtime_profile_names_round_trip() {
        assert_eq!(RuntimeProfile::Minimal.as_name(), "minimal");
        assert_eq!(RuntimeProfile::Balanced.as_name(), "balanced");
        assert_eq!(RuntimeProfile::HighMemory.as_name(), "high-memory");
    }

    #[test]
    fn hardware_tee_flag_matches_tee_type() {
        let status = embedded_runtime_status();
        let expected = status.tee_type == "sev-snp" || status.tee_type == "tdx";
        assert_eq!(status.hardware_tee, expected);
    }

    #[test]
    fn embedded_backend_prefers_layer_streaming() {
        assert_eq!(preferred_backend_name(), "picolm");
    }
}
