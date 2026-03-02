//! TTS playback via piper-rs + cpal.
//!
//! Synthesizes text with Piper (offline neural TTS) and plays audio through
//! the default output device using cpal. Uses a stop flag for cancellation
//! since cpal::Stream is not Send+Sync.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use piper_rs::synth::PiperSpeechSynthesizer;

use super::tts_model::detect_language;

/// Synthesize `text` with the appropriate Piper model and play through cpal.
///
/// Playback runs on the current thread (blocking). Check `stop_flag` to cancel.
pub fn speak_blocking(
    zh_model_dir: &std::path::Path,
    en_model_dir: &std::path::Path,
    text: &str,
    stop_flag: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let lang = detect_language(text);
    let config_path = match lang {
        super::tts_model::TtsLang::Chinese => zh_model_dir.join("model.onnx.json"),
        super::tts_model::TtsLang::English => en_model_dir.join("model.onnx.json"),
    };

    let model = piper_rs::from_config_path(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to load Piper model: {e}"))?;
    let synth = PiperSpeechSynthesizer::new(model)
        .map_err(|e| anyhow::anyhow!("Failed to create synthesizer: {e}"))?;

    // Synthesize all audio samples (22050 Hz mono f32)
    let mut samples: Vec<f32> = Vec::new();
    let audio_iter = synth
        .synthesize_parallel(text.to_string(), None)
        .map_err(|e| anyhow::anyhow!("Synthesis failed: {e}"))?;
    for chunk in audio_iter {
        if stop_flag.load(Ordering::Relaxed) {
            return Ok(());
        }
        let chunk = chunk.map_err(|e| anyhow::anyhow!("Synthesis chunk error: {e}"))?;
        samples.extend_from_slice(chunk.samples.as_slice());
    }

    if samples.is_empty() || stop_flag.load(Ordering::Relaxed) {
        return Ok(());
    }

    play_samples(&samples, &stop_flag)
}

/// Play f32 samples at 22050 Hz mono through the default output device.
/// Blocks until playback completes or stop_flag is set.
fn play_samples(samples: &[f32], stop_flag: &Arc<AtomicBool>) -> anyhow::Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No audio output device found"))?;

    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(22050),
        buffer_size: cpal::BufferSize::Default,
    };

    let samples = samples.to_vec();
    let total = samples.len();
    let pos = Arc::new(std::sync::Mutex::new(0usize));
    let done = Arc::new(AtomicBool::new(false));

    let pos_clone = Arc::clone(&pos);
    let done_clone = Arc::clone(&done);

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut idx = pos_clone.lock().unwrap();
            for sample in data.iter_mut() {
                if *idx < total {
                    *sample = samples[*idx];
                    *idx += 1;
                } else {
                    *sample = 0.0;
                    done_clone.store(true, Ordering::Relaxed);
                }
            }
        },
        |err| {
            tracing::error!("Audio output stream error: {err}");
        },
        None,
    )?;

    stream.play()?;

    // Block until playback finishes or stop is requested
    while !done.load(Ordering::Relaxed) && !stop_flag.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Stream is dropped here, stopping playback
    Ok(())
}
