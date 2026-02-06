use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::dirs;
use crate::error::{PowerError, Result};
use crate::model::manifest::ModelManifest;

/// Store a model file in the content-addressed blob store.
///
/// Returns the blob path and SHA-256 hash of the stored file.
pub fn store_blob(data: &[u8]) -> Result<(PathBuf, String)> {
    let blob_dir = dirs::blobs_dir();
    std::fs::create_dir_all(&blob_dir)?;

    let hash = compute_sha256(data);
    let blob_name = format!("sha256-{hash}");
    let blob_path = blob_dir.join(&blob_name);

    if !blob_path.exists() {
        std::fs::write(&blob_path, data).map_err(|e| {
            PowerError::Io(std::io::Error::other(format!(
                "Failed to write blob {}: {e}",
                blob_path.display()
            )))
        })?;
    }

    Ok((blob_path, hash))
}

/// Delete the blob file associated with a model manifest.
pub fn delete_blob(manifest: &ModelManifest) -> Result<()> {
    if manifest.path.exists() {
        std::fs::remove_file(&manifest.path).map_err(|e| {
            PowerError::Io(std::io::Error::other(format!(
                "Failed to delete blob {}: {e}",
                manifest.path.display()
            )))
        })?;
    }
    Ok(())
}

/// Verify the integrity of a blob file against its expected SHA-256 hash.
pub fn verify_blob(path: &std::path::Path, expected_sha256: &str) -> Result<bool> {
    let data = std::fs::read(path).map_err(|e| {
        PowerError::Io(std::io::Error::other(format!(
            "Failed to read blob for verification {}: {e}",
            path.display()
        )))
    })?;
    let actual = compute_sha256(&data);
    Ok(actual == expected_sha256)
}

/// Compute SHA-256 hash of the given data, returned as a hex string.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("{result:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sha256() {
        let hash = compute_sha256(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_store_blob() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let data = b"test model data";
        let (path, hash) = store_blob(data).unwrap();

        assert!(path.exists());
        assert!(path.to_string_lossy().contains(&format!("sha256-{hash}")));

        let stored = std::fs::read(&path).unwrap();
        assert_eq!(stored, data);

        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_store_blob_deduplication() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let data = b"identical data";
        let (path1, hash1) = store_blob(data).unwrap();
        let (path2, hash2) = store_blob(data).unwrap();

        assert_eq!(path1, path2);
        assert_eq!(hash1, hash2);

        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_verify_blob() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let data = b"verify me";
        let (path, hash) = store_blob(data).unwrap();

        assert!(verify_blob(&path, &hash).unwrap());
        assert!(!verify_blob(&path, "wrong-hash").unwrap());

        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_delete_blob() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let data = b"to be deleted";
        let (path, _hash) = store_blob(data).unwrap();
        assert!(path.exists());

        let manifest = crate::model::manifest::ModelManifest {
            name: "test".to_string(),
            format: crate::model::manifest::ModelFormat::Gguf,
            size: data.len() as u64,
            sha256: "test".to_string(),
            parameters: None,
            created_at: chrono::Utc::now(),
            path: path.clone(),
        };

        delete_blob(&manifest).unwrap();
        assert!(!path.exists());

        std::env::remove_var("A3S_POWER_HOME");
    }
}
