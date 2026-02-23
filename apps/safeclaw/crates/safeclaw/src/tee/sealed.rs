//! Sealed storage — encrypt/decrypt data bound to the TEE identity.
//!
//! When running inside AMD SEV-SNP, sealed storage derives encryption keys
//! from the VCEK (Versioned Chip Endorsement Key), binding data to the
//! specific CPU and firmware version. Data sealed on one machine cannot
//! be unsealed on another.
//!
//! Outside TEE, sealed storage uses a local file-based key for development.

use crate::error::{Error, Result};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use std::path::{Path, PathBuf};

/// Sealed storage backed by AES-256-GCM.
///
/// In TEE mode, the key is derived from hardware identity.
/// In dev mode, the key is loaded from a local file.
pub struct SealedStorage {
    cipher: Aes256Gcm,
    storage_dir: PathBuf,
}

impl std::fmt::Debug for SealedStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedStorage")
            .field("storage_dir", &self.storage_dir)
            .finish_non_exhaustive()
    }
}

impl SealedStorage {
    /// Initialize sealed storage.
    ///
    /// Attempts to derive key from TEE hardware. Falls back to file-based
    /// key at `~/.safeclaw/sealed.key` for development.
    pub async fn new() -> Result<Self> {
        let key = Self::derive_or_load_key().await?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| Error::Tee(format!("Failed to create cipher: {e}")))?;

        let storage_dir = Self::default_storage_dir()?;
        std::fs::create_dir_all(&storage_dir).map_err(|e| {
            Error::Tee(format!(
                "Failed to create sealed storage dir {}: {e}",
                storage_dir.display()
            ))
        })?;

        Ok(Self {
            cipher,
            storage_dir,
        })
    }

    /// Create sealed storage with a specific key and directory (for testing).
    #[cfg(test)]
    pub fn with_key(key: &[u8; 32], storage_dir: PathBuf) -> Result<Self> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| Error::Tee(format!("Failed to create cipher: {e}")))?;
        std::fs::create_dir_all(&storage_dir).ok();
        Ok(Self {
            cipher,
            storage_dir,
        })
    }

    /// Seal (encrypt) data and store it under the given name.
    pub async fn seal(&self, name: &str, plaintext: &[u8]) -> Result<()> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from(nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| Error::Tee(format!("Seal encryption failed: {e}")))?;

        // Store as: [nonce (12 bytes)][ciphertext]
        let mut sealed = Vec::with_capacity(12 + ciphertext.len());
        sealed.extend_from_slice(&nonce_bytes);
        sealed.extend_from_slice(&ciphertext);

        let path = self.storage_dir.join(Self::safe_filename(name));
        tokio::fs::write(&path, &sealed).await.map_err(|e| {
            Error::Tee(format!(
                "Failed to write sealed data to {}: {e}",
                path.display()
            ))
        })?;

        Ok(())
    }

    /// Unseal (decrypt) data stored under the given name.
    pub async fn unseal(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.storage_dir.join(Self::safe_filename(name));
        let sealed = tokio::fs::read(&path).await.map_err(|e| {
            Error::Tee(format!(
                "Failed to read sealed data from {}: {e}",
                path.display()
            ))
        })?;

        if sealed.len() < 12 {
            return Err(Error::Tee("Sealed data too short".to_string()));
        }

        let (nonce_bytes, ciphertext) = sealed.split_at(12);
        let nonce_arr: [u8; 12] = nonce_bytes
            .try_into()
            .map_err(|_| Error::Tee("Invalid nonce length in sealed data".to_string()))?;
        let nonce = Nonce::from(nonce_arr);

        self.cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| Error::Tee(format!("Unseal decryption failed: {e}")))
    }

    /// Check if sealed data exists.
    pub fn exists(&self, name: &str) -> bool {
        self.storage_dir.join(Self::safe_filename(name)).exists()
    }

    /// Delete sealed data.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let path = self.storage_dir.join(Self::safe_filename(name));
        if path.exists() {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| Error::Tee(format!("Failed to delete sealed data: {e}")))?;
        }
        Ok(())
    }

    // =========================================================================
    // Key derivation (private)
    // =========================================================================

    /// Derive key from TEE hardware or load from file.
    async fn derive_or_load_key() -> Result<[u8; 32]> {
        // Try hardware-derived key first
        if let Ok(key) = Self::derive_from_tee().await {
            tracing::info!("Using TEE hardware-derived sealing key");
            return Ok(key);
        }

        // Fall back to file-based key
        Self::load_or_create_file_key().await
    }

    /// Derive sealing key from AMD SEV-SNP VCEK.
    ///
    /// Uses KDF_SEV_SNP ioctl to derive a key bound to the VM measurement.
    async fn derive_from_tee() -> Result<[u8; 32]> {
        // Check if /dev/sev-guest exists
        if !Path::new("/dev/sev-guest").exists() {
            return Err(Error::Tee("No /dev/sev-guest device".to_string()));
        }

        // Derive key using SNP_GET_DERIVED_KEY ioctl
        // The derived key is bound to: VCEK + measurement + guest policy
        use std::os::unix::io::AsRawFd;

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/sev-guest")
            .map_err(|e| Error::Tee(format!("Failed to open /dev/sev-guest: {e}")))?;

        const SNP_GET_DERIVED_KEY: u64 = 0xC018_0002;

        #[repr(C)]
        struct SnpDerivedKeyReq {
            root_key_select: u32, // 0 = VCEK, 1 = VMRK
            guest_field_select: u64,
            vmpl: u32,
            guest_svn: u32,
            tcb_version: u64,
        }

        #[repr(C)]
        struct SnpDerivedKeyResp {
            status: u32,
            rsvd: [u8; 28],
            key: [u8; 32],
        }

        #[repr(C)]
        struct SnpGuestRequest {
            msg_version: u8,
            req_data: u64,
            resp_data: u64,
            fw_err: u64,
        }

        let mut req = SnpDerivedKeyReq {
            root_key_select: 0,      // VCEK
            guest_field_select: 0x1, // Bind to measurement
            vmpl: 0,
            guest_svn: 0,
            tcb_version: 0,
        };

        let mut resp = SnpDerivedKeyResp {
            status: 0,
            rsvd: [0u8; 28],
            key: [0u8; 32],
        };

        let mut guest_req = SnpGuestRequest {
            msg_version: 1,
            req_data: &mut req as *mut _ as u64,
            resp_data: &mut resp as *mut _ as u64,
            fw_err: 0,
        };

        let ret = unsafe {
            libc::ioctl(
                file.as_raw_fd(),
                SNP_GET_DERIVED_KEY,
                &mut guest_req as *mut _,
            )
        };

        if ret != 0 {
            return Err(Error::Tee(format!(
                "SNP_GET_DERIVED_KEY failed: {}",
                std::io::Error::last_os_error()
            )));
        }

        Ok(resp.key)
    }

    /// Load or create a file-based key for development.
    async fn load_or_create_file_key() -> Result<[u8; 32]> {
        let key_path = Self::default_key_path()?;

        if key_path.exists() {
            let data = tokio::fs::read(&key_path)
                .await
                .map_err(|e| Error::Tee(format!("Failed to read key file: {e}")))?;
            if data.len() != 32 {
                return Err(Error::Tee(format!(
                    "Invalid key file size: expected 32, got {}",
                    data.len()
                )));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&data);
            tracing::info!("Using file-based sealing key from {}", key_path.display());
            Ok(key)
        } else {
            // Generate new key
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);

            if let Some(parent) = key_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            tokio::fs::write(&key_path, &key)
                .await
                .map_err(|e| Error::Tee(format!("Failed to write key file: {e}")))?;

            // Restrict permissions (owner-only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).ok();
            }

            tracing::info!(
                "Generated new file-based sealing key at {}",
                key_path.display()
            );
            Ok(key)
        }
    }

    fn default_key_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Tee("Cannot determine home directory".to_string()))?;
        Ok(home.join(".safeclaw").join("sealed.key"))
    }

    fn default_storage_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Tee("Cannot determine home directory".to_string()))?;
        Ok(home.join(".safeclaw").join("sealed"))
    }

    /// Sanitize a name for use as a filename.
    fn safe_filename(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            + ".sealed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_storage(dir: &Path) -> SealedStorage {
        let key = [42u8; 32];
        SealedStorage::with_key(&key, dir.to_path_buf()).unwrap()
    }

    #[tokio::test]
    async fn test_seal_unseal_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = test_storage(tmp.path());

        let plaintext = b"sensitive API key: sk-ant-12345";
        storage.seal("api_key", plaintext).await.unwrap();

        assert!(storage.exists("api_key"));

        let decrypted = storage.unseal("api_key").await.unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_seal_different_data() {
        let tmp = TempDir::new().unwrap();
        let storage = test_storage(tmp.path());

        storage.seal("key1", b"value1").await.unwrap();
        storage.seal("key2", b"value2").await.unwrap();

        assert_eq!(storage.unseal("key1").await.unwrap(), b"value1");
        assert_eq!(storage.unseal("key2").await.unwrap(), b"value2");
    }

    #[tokio::test]
    async fn test_unseal_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let storage = test_storage(tmp.path());

        let result = storage.unseal("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete() {
        let tmp = TempDir::new().unwrap();
        let storage = test_storage(tmp.path());

        storage.seal("to_delete", b"data").await.unwrap();
        assert!(storage.exists("to_delete"));

        storage.delete("to_delete").await.unwrap();
        assert!(!storage.exists("to_delete"));
    }

    #[tokio::test]
    async fn test_wrong_key_fails() {
        let tmp = TempDir::new().unwrap();

        let storage1 = SealedStorage::with_key(&[1u8; 32], tmp.path().to_path_buf()).unwrap();
        storage1.seal("secret", b"data").await.unwrap();

        let storage2 = SealedStorage::with_key(&[2u8; 32], tmp.path().to_path_buf()).unwrap();
        let result = storage2.unseal("secret").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_filename() {
        assert_eq!(SealedStorage::safe_filename("api_key"), "api_key.sealed");
        assert_eq!(
            SealedStorage::safe_filename("user/secret"),
            "user_secret.sealed"
        );
        assert_eq!(SealedStorage::safe_filename("a.b.c"), "a_b_c.sealed");
    }

    #[tokio::test]
    async fn test_overwrite_sealed_data() {
        let tmp = TempDir::new().unwrap();
        let storage = test_storage(tmp.path());

        storage.seal("key", b"original").await.unwrap();
        storage.seal("key", b"updated").await.unwrap();

        assert_eq!(storage.unseal("key").await.unwrap(), b"updated");
    }
}
