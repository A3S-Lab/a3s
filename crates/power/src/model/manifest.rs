use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Describes a locally stored model and its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    /// Model name, e.g. "llama3.2:3b"
    pub name: String,

    /// File format of the model weights
    pub format: ModelFormat,

    /// Total size in bytes
    pub size: u64,

    /// SHA-256 hash of the model file for integrity verification
    pub sha256: String,

    /// Model parameters and metadata
    pub parameters: Option<ModelParameters>,

    /// Timestamp when the model was pulled/created locally
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Path to the model blob on disk (content-addressed)
    pub path: PathBuf,
}

/// Supported model file formats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat {
    Gguf,
    SafeTensors,
}

impl std::fmt::Display for ModelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelFormat::Gguf => write!(f, "GGUF"),
            ModelFormat::SafeTensors => write!(f, "SafeTensors"),
        }
    }
}

/// Optional parameter metadata about a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameters {
    /// Maximum context length in tokens
    pub context_length: Option<u32>,

    /// Embedding dimension size
    pub embedding_length: Option<u32>,

    /// Total number of model parameters
    pub parameter_count: Option<u64>,

    /// Quantization level, e.g. "Q4_K_M", "Q8_0"
    pub quantization: Option<String>,
}

impl ModelManifest {
    /// Returns the sanitized filename for the manifest file.
    /// Replaces ':' and '/' with '-' to produce a safe filename.
    pub fn manifest_filename(&self) -> String {
        let safe = self.name.replace([':', '/'], "-");
        format!("{safe}.json")
    }

    /// Returns a human-readable size string (e.g. "4.2 GB").
    pub fn size_display(&self) -> String {
        const GB: u64 = 1_000_000_000;
        const MB: u64 = 1_000_000;
        const KB: u64 = 1_000;

        if self.size >= GB {
            format!("{:.1} GB", self.size as f64 / GB as f64)
        } else if self.size >= MB {
            format!("{:.1} MB", self.size as f64 / MB as f64)
        } else if self.size >= KB {
            format!("{:.1} KB", self.size as f64 / KB as f64)
        } else {
            format!("{} B", self.size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> ModelManifest {
        ModelManifest {
            name: "llama3.2:3b".to_string(),
            format: ModelFormat::Gguf,
            size: 2_000_000_000,
            sha256: "abc123".to_string(),
            parameters: Some(ModelParameters {
                context_length: Some(4096),
                embedding_length: Some(3200),
                parameter_count: Some(3_000_000_000),
                quantization: Some("Q4_K_M".to_string()),
            }),
            created_at: chrono::Utc::now(),
            path: PathBuf::from("/tmp/blobs/sha256-abc123"),
        }
    }

    #[test]
    fn test_manifest_filename() {
        let manifest = sample_manifest();
        assert_eq!(manifest.manifest_filename(), "llama3.2-3b.json");
    }

    #[test]
    fn test_manifest_filename_with_slash() {
        let mut manifest = sample_manifest();
        manifest.name = "library/llama3:latest".to_string();
        assert_eq!(manifest.manifest_filename(), "library-llama3-latest.json");
    }

    #[test]
    fn test_size_display_gb() {
        let manifest = sample_manifest();
        assert_eq!(manifest.size_display(), "2.0 GB");
    }

    #[test]
    fn test_size_display_mb() {
        let mut manifest = sample_manifest();
        manifest.size = 500_000_000;
        assert_eq!(manifest.size_display(), "500.0 MB");
    }

    #[test]
    fn test_size_display_kb() {
        let mut manifest = sample_manifest();
        manifest.size = 1_500;
        assert_eq!(manifest.size_display(), "1.5 KB");
    }

    #[test]
    fn test_size_display_bytes() {
        let mut manifest = sample_manifest();
        manifest.size = 512;
        assert_eq!(manifest.size_display(), "512 B");
    }

    #[test]
    fn test_model_format_display() {
        assert_eq!(ModelFormat::Gguf.to_string(), "GGUF");
        assert_eq!(ModelFormat::SafeTensors.to_string(), "SafeTensors");
    }

    #[test]
    fn test_manifest_serialization_roundtrip() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: ModelManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, manifest.name);
        assert_eq!(deserialized.format, manifest.format);
        assert_eq!(deserialized.size, manifest.size);
        assert_eq!(deserialized.sha256, manifest.sha256);
    }
}
