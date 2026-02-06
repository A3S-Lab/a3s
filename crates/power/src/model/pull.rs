use crate::error::{PowerError, Result};
use crate::model::manifest::{ModelFormat, ModelManifest};
use crate::model::storage;

/// Progress callback type for download reporting.
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Download a model from a direct URL.
///
/// Returns the resulting `ModelManifest` after download and storage.
pub async fn pull_model(
    name: &str,
    url: &str,
    progress: Option<ProgressCallback>,
) -> Result<ModelManifest> {
    tracing::info!(name, url, "Pulling model");

    let response = reqwest::get(url)
        .await
        .map_err(|e| PowerError::DownloadFailed {
            model: name.to_string(),
            source: e,
        })?;

    let total_size = response.content_length().unwrap_or(0);
    let bytes = download_with_progress(response, total_size, progress).await?;

    let (blob_path, sha256) = storage::store_blob(&bytes)?;
    let format = detect_format(name, &blob_path);

    let manifest = ModelManifest {
        name: name.to_string(),
        format,
        size: bytes.len() as u64,
        sha256,
        parameters: None,
        created_at: chrono::Utc::now(),
        path: blob_path,
    };

    Ok(manifest)
}

async fn download_with_progress(
    response: reqwest::Response,
    total_size: u64,
    progress: Option<ProgressCallback>,
) -> Result<Vec<u8>> {
    use futures::StreamExt;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut data = Vec::with_capacity(total_size as usize);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| PowerError::DownloadFailed {
            model: "unknown".to_string(),
            source: e,
        })?;
        data.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;

        if let Some(ref cb) = progress {
            cb(downloaded, total_size);
        }
    }

    Ok(data)
}

/// Detect model format from filename extension.
fn detect_format(name: &str, path: &std::path::Path) -> ModelFormat {
    let name_lower = name.to_lowercase();
    let path_str = path.to_string_lossy().to_lowercase();

    if name_lower.contains("gguf") || path_str.ends_with(".gguf") {
        ModelFormat::Gguf
    } else if name_lower.contains("safetensors") || path_str.ends_with(".safetensors") {
        ModelFormat::SafeTensors
    } else {
        // Default to GGUF as it's the most common format for local inference
        ModelFormat::Gguf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_format_gguf() {
        let path = PathBuf::from("/tmp/model.gguf");
        assert_eq!(detect_format("model", &path), ModelFormat::Gguf);
    }

    #[test]
    fn test_detect_format_safetensors() {
        let path = PathBuf::from("/tmp/model.safetensors");
        assert_eq!(detect_format("model", &path), ModelFormat::SafeTensors);
    }

    #[test]
    fn test_detect_format_from_name() {
        let path = PathBuf::from("/tmp/sha256-abc123");
        assert_eq!(detect_format("model-gguf", &path), ModelFormat::Gguf);
        assert_eq!(
            detect_format("model-safetensors", &path),
            ModelFormat::SafeTensors
        );
    }

    #[test]
    fn test_detect_format_default() {
        let path = PathBuf::from("/tmp/sha256-abc123");
        assert_eq!(detect_format("some-model", &path), ModelFormat::Gguf);
    }
}
