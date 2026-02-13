//! Payload encryption for events
//!
//! Provides application-level encrypt/decrypt for event payloads,
//! independent of transport encryption. Supports key rotation via key IDs.

use crate::error::{EventError, Result};
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, AeadCore, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Encrypted payload envelope stored in `event.payload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedPayload {
    /// Identifies which key was used for encryption
    pub key_id: String,

    /// Base64-encoded nonce (96-bit for AES-256-GCM)
    pub nonce: String,

    /// Base64-encoded ciphertext
    pub ciphertext: String,

    /// Marker to identify encrypted payloads
    #[serde(default = "default_encrypted")]
    pub encrypted: bool,
}

fn default_encrypted() -> bool {
    true
}

impl EncryptedPayload {
    /// Check if a JSON value is an encrypted payload
    pub fn is_encrypted(value: &serde_json::Value) -> bool {
        value
            .get("encrypted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

/// Trait for encrypting and decrypting event payloads
pub trait EventEncryptor: Send + Sync {
    /// Encrypt a JSON payload, returning an encrypted envelope as JSON
    fn encrypt(&self, payload: &serde_json::Value) -> Result<serde_json::Value>;

    /// Decrypt an encrypted envelope back to the original JSON payload
    fn decrypt(&self, encrypted: &serde_json::Value) -> Result<serde_json::Value>;

    /// The current active key ID used for encryption
    fn active_key_id(&self) -> &str;
}

/// AES-256-GCM encryptor with key rotation support
///
/// Encrypts with the active key, decrypts with any registered key.
/// Keys are identified by string IDs for rotation tracking.
pub struct Aes256GcmEncryptor {
    /// Active key ID for encryption
    active_key_id: String,

    /// All registered keys (key_id → cipher)
    keys: RwLock<HashMap<String, Aes256Gcm>>,
}

impl Aes256GcmEncryptor {
    /// Create a new encryptor with a single key
    ///
    /// `key` must be exactly 32 bytes (256 bits).
    pub fn new(key_id: impl Into<String>, key: &[u8; 32]) -> Self {
        let key_id = key_id.into();
        let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");
        let mut keys = HashMap::new();
        keys.insert(key_id.clone(), cipher);

        Self {
            active_key_id: key_id,
            keys: RwLock::new(keys),
        }
    }

    /// Add a key for decryption (key rotation)
    ///
    /// Old keys remain available for decrypting messages encrypted before rotation.
    pub fn add_key(&self, key_id: impl Into<String>, key: &[u8; 32]) -> Result<()> {
        let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");
        let mut keys = self.keys.write().map_err(|e| {
            EventError::Config(format!("Failed to acquire key lock: {}", e))
        })?;
        keys.insert(key_id.into(), cipher);
        Ok(())
    }

    /// Rotate to a new active key
    ///
    /// The new key must already be registered via `add_key()`.
    pub fn rotate_to(&mut self, key_id: &str) -> Result<()> {
        let keys = self.keys.read().map_err(|e| {
            EventError::Config(format!("Failed to acquire key lock: {}", e))
        })?;
        if !keys.contains_key(key_id) {
            return Err(EventError::Config(format!(
                "Key '{}' not registered, add it first",
                key_id
            )));
        }
        self.active_key_id = key_id.to_string();
        Ok(())
    }

    /// List all registered key IDs
    pub fn key_ids(&self) -> Vec<String> {
        self.keys
            .read()
            .map(|keys| keys.keys().cloned().collect())
            .unwrap_or_default()
    }
}

impl EventEncryptor for Aes256GcmEncryptor {
    fn encrypt(&self, payload: &serde_json::Value) -> Result<serde_json::Value> {
        let plaintext = serde_json::to_vec(payload)?;

        let keys = self.keys.read().map_err(|e| {
            EventError::Config(format!("Failed to acquire key lock: {}", e))
        })?;
        let cipher = keys.get(&self.active_key_id).ok_or_else(|| {
            EventError::Config(format!("Active key '{}' not found", self.active_key_id))
        })?;

        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref()).map_err(|e| {
            EventError::Config(format!("Encryption failed: {}", e))
        })?;

        let envelope = EncryptedPayload {
            key_id: self.active_key_id.clone(),
            nonce: BASE64.encode(nonce),
            ciphertext: BASE64.encode(ciphertext),
            encrypted: true,
        };

        serde_json::to_value(envelope).map_err(Into::into)
    }

    fn decrypt(&self, encrypted: &serde_json::Value) -> Result<serde_json::Value> {
        let envelope: EncryptedPayload = serde_json::from_value(encrypted.clone())?;

        let keys = self.keys.read().map_err(|e| {
            EventError::Config(format!("Failed to acquire key lock: {}", e))
        })?;
        let cipher = keys.get(&envelope.key_id).ok_or_else(|| {
            EventError::Config(format!(
                "Decryption key '{}' not registered",
                envelope.key_id
            ))
        })?;

        let nonce_bytes = BASE64.decode(&envelope.nonce).map_err(|e| {
            EventError::Config(format!("Invalid nonce encoding: {}", e))
        })?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = BASE64.decode(&envelope.ciphertext).map_err(|e| {
            EventError::Config(format!("Invalid ciphertext encoding: {}", e))
        })?;

        let plaintext = cipher.decrypt(nonce, ciphertext.as_ref()).map_err(|e| {
            EventError::Config(format!("Decryption failed: {}", e))
        })?;

        serde_json::from_slice(&plaintext).map_err(Into::into)
    }

    fn active_key_id(&self) -> &str {
        &self.active_key_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x42; 32]
    }

    fn test_key_2() -> [u8; 32] {
        [0x7A; 32]
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let enc = Aes256GcmEncryptor::new("key-1", &test_key());
        let payload = serde_json::json!({"rate": 7.35, "currency": "USD/CNY"});

        let encrypted = enc.encrypt(&payload).unwrap();
        assert!(EncryptedPayload::is_encrypted(&encrypted));

        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn test_encrypted_payload_marker() {
        let enc = Aes256GcmEncryptor::new("key-1", &test_key());
        let encrypted = enc.encrypt(&serde_json::json!({"data": 1})).unwrap();

        assert_eq!(encrypted["encrypted"], true);
        assert!(encrypted["keyId"].is_string());
        assert!(encrypted["nonce"].is_string());
        assert!(encrypted["ciphertext"].is_string());
    }

    #[test]
    fn test_is_encrypted_false_for_plain() {
        let plain = serde_json::json!({"rate": 7.35});
        assert!(!EncryptedPayload::is_encrypted(&plain));
    }

    #[test]
    fn test_key_rotation() {
        let mut enc = Aes256GcmEncryptor::new("key-1", &test_key());

        // Encrypt with key-1
        let payload = serde_json::json!({"secret": "data"});
        let encrypted_v1 = enc.encrypt(&payload).unwrap();

        // Add and rotate to key-2
        enc.add_key("key-2", &test_key_2()).unwrap();
        enc.rotate_to("key-2").unwrap();
        assert_eq!(enc.active_key_id(), "key-2");

        // Encrypt with key-2
        let encrypted_v2 = enc.encrypt(&payload).unwrap();

        // Both can be decrypted (old key still registered)
        assert_eq!(enc.decrypt(&encrypted_v1).unwrap(), payload);
        assert_eq!(enc.decrypt(&encrypted_v2).unwrap(), payload);

        // Verify different keys were used
        assert_eq!(encrypted_v1["keyId"], "key-1");
        assert_eq!(encrypted_v2["keyId"], "key-2");
    }

    #[test]
    fn test_rotate_to_unknown_key_fails() {
        let mut enc = Aes256GcmEncryptor::new("key-1", &test_key());
        let result = enc.rotate_to("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_with_missing_key_fails() {
        let enc1 = Aes256GcmEncryptor::new("key-1", &test_key());
        let enc2 = Aes256GcmEncryptor::new("key-2", &test_key_2());

        let encrypted = enc1.encrypt(&serde_json::json!({"data": 1})).unwrap();
        let result = enc2.decrypt(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let enc1 = Aes256GcmEncryptor::new("key-1", &test_key());
        let enc2 = Aes256GcmEncryptor::new("key-2", &test_key_2());
        // Register key-1 with wrong bytes
        enc2.add_key("key-1", &[0xFF; 32]).unwrap();

        let encrypted = enc1.encrypt(&serde_json::json!({"data": 1})).unwrap();
        let result = enc2.decrypt(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_ids() {
        let enc = Aes256GcmEncryptor::new("key-1", &test_key());
        enc.add_key("key-2", &test_key_2()).unwrap();

        let mut ids = enc.key_ids();
        ids.sort();
        assert_eq!(ids, vec!["key-1", "key-2"]);
    }

    #[test]
    fn test_encrypt_complex_payload() {
        let enc = Aes256GcmEncryptor::new("key-1", &test_key());
        let payload = serde_json::json!({
            "user": "[email]",
            "action": "login",
            "nested": {"deep": [1, 2, 3]},
            "tags": ["pii", "audit"]
        });

        let encrypted = enc.encrypt(&payload).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn test_each_encryption_unique_nonce() {
        let enc = Aes256GcmEncryptor::new("key-1", &test_key());
        let payload = serde_json::json!({"data": "same"});

        let e1 = enc.encrypt(&payload).unwrap();
        let e2 = enc.encrypt(&payload).unwrap();

        // Same plaintext should produce different ciphertext (random nonce)
        assert_ne!(e1["nonce"], e2["nonce"]);
        assert_ne!(e1["ciphertext"], e2["ciphertext"]);
    }
}
