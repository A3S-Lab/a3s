//! TTS model management — download, storage, and language detection.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::Emitter;

/// Supported TTS languages.
#[derive(Debug, Clone, Copy)]
pub enum TtsLang {
    Chinese,
    English,
}

/// Status of TTS model readiness.
#[derive(Debug, Clone, Serialize)]
pub struct TtsModelStatus {
    pub zh_ready: bool,
    pub en_ready: bool,
}

/// HuggingFace base URL for piper voice models.
const HF_BASE: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

/// Model specs: (language dir, model name, quality).
const ZH_MODEL: (&str, &str, &str) = ("zh_CN", "huayan", "medium");
const EN_MODEL: (&str, &str, &str) = ("en_US", "lessac", "medium");

/// Detect language from text using a simple CJK heuristic.
pub fn detect_language(text: &str) -> TtsLang {
    let cjk_count = text
        .chars()
        .filter(|c| {
            matches!(*c as u32,
                0x4E00..=0x9FFF   // CJK Unified Ideographs
                | 0x3400..=0x4DBF // CJK Extension A
                | 0x3000..=0x303F // CJK Symbols and Punctuation
                | 0xFF00..=0xFFEF // Fullwidth Forms
            )
        })
        .count();
    if cjk_count > 0 {
        TtsLang::Chinese
    } else {
        TtsLang::English
    }
}

/// Root directory for piper models: `<app_data_dir>/piper-models/`.
pub fn models_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("piper-models")
}

/// Directory for a specific model.
fn model_dir(app_data_dir: &Path, lang: &str, name: &str, quality: &str) -> PathBuf {
    models_dir(app_data_dir).join(format!("{lang}-{name}-{quality}"))
}

/// Get the Chinese model directory.
pub fn zh_model_dir(app_data_dir: &Path) -> PathBuf {
    model_dir(app_data_dir, ZH_MODEL.0, ZH_MODEL.1, ZH_MODEL.2)
}

/// Get the English model directory.
pub fn en_model_dir(app_data_dir: &Path) -> PathBuf {
    model_dir(app_data_dir, EN_MODEL.0, EN_MODEL.1, EN_MODEL.2)
}

/// Check if a model is downloaded (both .onnx and .onnx.json exist).
fn is_model_ready(dir: &Path) -> bool {
    dir.join("model.onnx").exists() && dir.join("model.onnx.json").exists()
}

/// Check status of both TTS models.
pub fn check_status(app_data_dir: &Path) -> TtsModelStatus {
    TtsModelStatus {
        zh_ready: is_model_ready(&zh_model_dir(app_data_dir)),
        en_ready: is_model_ready(&en_model_dir(app_data_dir)),
    }
}

/// Download both TTS models, emitting progress events.
pub fn download_models(app_data_dir: &Path, app_handle: &tauri::AppHandle) -> anyhow::Result<()> {
    let models = [ZH_MODEL, EN_MODEL];
    let total_models = models.len();

    for (i, (lang, name, quality)) in models.iter().enumerate() {
        let dir = model_dir(app_data_dir, lang, name, quality);
        if is_model_ready(&dir) {
            emit_progress(app_handle, i + 1, total_models, 100, lang);
            continue;
        }

        std::fs::create_dir_all(&dir)
            .map_err(|e| anyhow::anyhow!("Failed to create model dir {}: {e}", dir.display()))?;

        // Download .onnx file
        let onnx_url = format!("{HF_BASE}/{lang}/{name}/{quality}/{lang}-{name}-{quality}.onnx");
        download_file(
            &onnx_url,
            &dir.join("model.onnx"),
            app_handle,
            i,
            total_models,
            lang,
        )?;

        // Download .onnx.json config
        let json_url =
            format!("{HF_BASE}/{lang}/{name}/{quality}/{lang}-{name}-{quality}.onnx.json");
        download_file(
            &json_url,
            &dir.join("model.onnx.json"),
            app_handle,
            i,
            total_models,
            lang,
        )?;

        emit_progress(app_handle, i + 1, total_models, 100, lang);
    }

    Ok(())
}

/// Download a single file with progress reporting.
fn download_file(
    url: &str,
    dest: &Path,
    app_handle: &tauri::AppHandle,
    model_index: usize,
    total_models: usize,
    lang: &str,
) -> anyhow::Result<()> {
    tracing::info!("Downloading TTS model: {url}");

    let response = reqwest::blocking::Client::new()
        .get(url)
        .send()
        .map_err(|e| anyhow::anyhow!("Download request failed for {url}: {e}"))?;

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut file = std::fs::File::create(dest)
        .map_err(|e| anyhow::anyhow!("Failed to create file {}: {e}", dest.display()))?;

    let mut reader = std::io::BufReader::new(response);
    let mut buf = [0u8; 8192];
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)
            .map_err(|e| anyhow::anyhow!("Read error: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])
            .map_err(|e| anyhow::anyhow!("Write error: {e}"))?;
        downloaded += n as u64;

        if total_size > 0 {
            let pct = (downloaded as f64 / total_size as f64 * 100.0) as u32;
            emit_progress(app_handle, model_index, total_models, pct.min(99), lang);
        }
    }

    Ok(())
}

#[derive(Clone, Serialize)]
struct TtsDownloadProgress {
    model_index: usize,
    total_models: usize,
    percent: u32,
    lang: String,
}

fn emit_progress(
    app_handle: &tauri::AppHandle,
    model_index: usize,
    total_models: usize,
    percent: u32,
    lang: &str,
) {
    let _ = app_handle.emit(
        "voice://tts-download-progress",
        TtsDownloadProgress {
            model_index,
            total_models,
            percent,
            lang: lang.to_string(),
        },
    );
}
