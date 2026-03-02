//! Voice module — TTS commands for Tauri.

pub mod tts;
pub mod tts_model;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::Manager;

/// Shared voice state managed by Tauri.
///
/// Uses an atomic stop flag instead of storing cpal::Stream directly,
/// because cpal::Stream is not Send+Sync on all platforms.
pub struct VoiceState {
    /// Set to true to cancel current playback.
    pub tts_stop_flag: Arc<AtomicBool>,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            tts_stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Check if TTS models are downloaded.
#[tauri::command]
pub fn voice_tts_status(app_handle: tauri::AppHandle) -> Result<tts_model::TtsModelStatus, String> {
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
    Ok(tts_model::check_status(&data_dir))
}

/// Download TTS models (blocking — call from a background thread on the frontend).
#[tauri::command]
pub fn voice_tts_download(app_handle: tauri::AppHandle) -> Result<(), String> {
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
    tts_model::download_models(&data_dir, &app_handle).map_err(|e| format!("{e}"))
}

/// Synthesize text and play audio in background. Stops any current playback first.
#[tauri::command]
pub fn voice_tts_speak(
    text: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, VoiceState>,
) -> Result<(), String> {
    // Signal any current playback to stop, then give it a moment to exit
    state.tts_stop_flag.store(true, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Reset flag for the new playback
    state.tts_stop_flag.store(false, Ordering::SeqCst);
    let stop_flag = Arc::clone(&state.tts_stop_flag);

    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;

    let zh_dir = tts_model::zh_model_dir(&data_dir);
    let en_dir = tts_model::en_model_dir(&data_dir);

    // Spawn playback on a dedicated thread (cpal::Stream must stay on one thread)
    std::thread::spawn(move || {
        if let Err(e) = tts::speak_blocking(&zh_dir, &en_dir, &text, stop_flag) {
            tracing::error!("TTS playback failed: {e}");
        }
    });

    Ok(())
}

/// Stop current TTS playback.
#[tauri::command]
pub fn voice_tts_stop(state: tauri::State<'_, VoiceState>) -> Result<(), String> {
    state.tts_stop_flag.store(true, Ordering::SeqCst);
    Ok(())
}
