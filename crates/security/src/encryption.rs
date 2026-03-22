//! Token encryption utilities using AES-256-GCM
//!
//! Provides authenticated encryption for sensitive tokens (ORCID, Plaid, etc.)
//! Uses AES-256-GCM with a 256-bit key and 96-bit nonce.
//!
//! # Configuration
//! - `FRESHCREDIT_ENCRYPTION_ENABLED`: Set to "false" to disable encryption (testing only)
//! - `FRESHCREDIT_TOKEN_ENCRYPTION_KEY`: 64-character hex key for encryption
//!
//! # Security Notes
//! - Key must be stored securely (GCP Secret Manager, env var, etc.)
//! - Each encryption uses a random nonce
//! - Ciphertext includes authentication tag to detect tampering
//! - Output format: base64(nonce || ciphertext || tag)
//! - NEVER disable encryption in production!

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use std::sync::OnceLock;

/// Nonce size for AES-GCM (96 bits = 12 bytes)
const NONCE_SIZE: usize = 12;

/// Key size for AES-256 (256 bits = 32 bytes)
pub const KEY_SIZE: usize = 32;

/// Global encryption configuration (initialized once)
static ENCRYPTION_CONFIG: OnceLock<EncryptionConfig> = OnceLock::new();

/// Encryption configuration for the application
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Whether encryption is enabled (should be true in production)
    pub enabled: bool,
    /// The encryption key (if encryption is enabled)
    pub encryptor: Option<TokenEncryptor>,
}

impl EncryptionConfig {
    /// Load encryption configuration from environment variables
    ///
    /// Environment variables:
    /// - `FRESHCREDIT_ENCRYPTION_ENABLED`: "true" (default) or "false"
    /// - `FRESHCREDIT_TOKEN_ENCRYPTION_KEY`: 64-character hex key
    pub fn from_env() -> Result<Self> {
        let enabled = std::env::var("FRESHCREDIT_ENCRYPTION_ENABLED")
            .map(|v| v.to_lowercase() != "false")
            .unwrap_or(true); // Enabled by default

        if !enabled {
            tracing::warn!(
                "⚠️ Token encryption is DISABLED. This should only be used for testing!"
            );
            return Ok(Self {
                enabled: false,
                encryptor: None,
            });
        }

        // Try to load encryption key
        let encryptor = match std::env::var("FRESHCREDIT_TOKEN_ENCRYPTION_KEY") {
            Ok(hex_key) => {
                let enc = TokenEncryptor::from_hex_key(&hex_key)?;
                tracing::info!("✅ Token encryption enabled with configured key");
                Some(enc)
            }
            Err(_) => {
                tracing::warn!(
                    "⚠️ FRESHCREDIT_TOKEN_ENCRYPTION_KEY not set. Tokens stored in plaintext."
                );
                None
            }
        };

        Ok(Self { enabled, encryptor })
    }

    /// Check if encryption is actually available (enabled AND key is configured)
    pub fn is_available(&self) -> bool {
        self.enabled && self.encryptor.is_some()
    }

    /// Encrypt a token if encryption is available
    /// 
    /// CRITICAL SECURITY FIX: Never falls back to plaintext. Returns error on encryption failure.
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        match &self.encryptor {
            Some(enc) if self.enabled => enc.encrypt(plaintext).map_err(|e| {
                tracing::error!("Encryption failed: {e}");
                anyhow!("Encryption operation failed")
            }),
            _ => Err(anyhow!("Encryption not configured or disabled")),
        }
    }

    /// Decrypt a token if encryption is available
    ///
    /// Handles both encrypted and plaintext tokens gracefully
    pub fn decrypt(&self, ciphertext: &str) -> String {
        match &self.encryptor {
            Some(enc) if self.enabled => {
                // Try to decrypt; if it fails, assume it's already plaintext
                enc.decrypt(ciphertext).unwrap_or_else(|_| {
                    // This is expected for plaintext tokens or tokens encrypted with different key
                    ciphertext.to_string()
                })
            }
            _ => ciphertext.to_string(),
        }
    }
}

/// Get or initialize the global encryption configuration
pub fn get_encryption_config() -> &'static EncryptionConfig {
    ENCRYPTION_CONFIG.get_or_init(|| {
        EncryptionConfig::from_env().unwrap_or_else(|e| {
            tracing::error!("Failed to load encryption config: {e}");
            EncryptionConfig {
                enabled: false,
                encryptor: None,
            }
        })
    })
}

/// Encrypt a token using the global configuration
///
/// This is the primary API for encrypting tokens throughout the application.
/// CRITICAL SECURITY FIX: Returns Result to prevent silent plaintext storage.
pub fn encrypt_token(plaintext: &str) -> Result<String> {
    get_encryption_config().encrypt(plaintext)
}

/// Decrypt a token using the global configuration
///
/// This is the primary API for decrypting tokens throughout the application.
/// Handles both encrypted and plaintext tokens gracefully.
pub fn decrypt_token(ciphertext: &str) -> String {
    get_encryption_config().decrypt(ciphertext)
}

/// Token encryptor using AES-256-GCM
pub struct TokenEncryptor {
    cipher: Aes256Gcm,
    /// Store key for Clone implementation
    key: [u8; KEY_SIZE],
}

impl std::fmt::Debug for TokenEncryptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenEncryptor")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

impl Clone for TokenEncryptor {
    fn clone(&self) -> Self {
        Self {
            cipher: Aes256Gcm::new_from_slice(&self.key).expect("Key already validated"),
            key: self.key,
        }
    }
}

impl TokenEncryptor {
    /// Create a new TokenEncryptor with a 256-bit key
    ///
    /// # Arguments
    /// * `key` - 32-byte encryption key
    ///
    /// # Errors
    /// Returns an error if the key is not exactly 32 bytes
    pub fn new(key: &[u8]) -> Result<Self> {
        if key.len() != KEY_SIZE {
            return Err(anyhow!(
                "Key must be exactly {KEY_SIZE} bytes, got {}",
                key.len()
            ));
        }

        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|e| anyhow!("Failed to create cipher: {e}"))?;

        let mut stored_key = [0u8; KEY_SIZE];
        stored_key.copy_from_slice(key);

        Ok(Self {
            cipher,
            key: stored_key,
        })
    }

    /// Create a TokenEncryptor from a hex-encoded key
    ///
    /// # Arguments
    /// * `hex_key` - 64-character hex string representing a 256-bit key
    pub fn from_hex_key(hex_key: &str) -> Result<Self> {
        let key = hex::decode(hex_key).map_err(|e| anyhow!("Invalid hex key: {e}"))?;
        Self::new(&key)
    }

    /// Create a TokenEncryptor from a base64-encoded key
    ///
    /// # Arguments
    /// * `base64_key` - Base64 string representing a 256-bit key
    pub fn from_base64_key(base64_key: &str) -> Result<Self> {
        let key = BASE64
            .decode(base64_key)
            .map_err(|e| anyhow!("Invalid base64 key: {e}"))?;
        Self::new(&key)
    }

    /// Encrypt a plaintext token
    ///
    /// Returns base64-encoded ciphertext (nonce || ciphertext || tag)
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        // Generate random nonce using cryptographically secure RNG
        // SECURITY FIX: Use OsRng instead of thread_rng() for cryptographic operations
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {e}"))?;

        // Combine: nonce || ciphertext (tag is appended by aes-gcm)
        let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(&combined))
    }

    /// Decrypt a ciphertext token
    ///
    /// Input should be base64-encoded (nonce || ciphertext || tag)
    pub fn decrypt(&self, ciphertext: &str) -> Result<String> {
        let combined = BASE64
            .decode(ciphertext)
            .map_err(|e| anyhow!("Invalid base64 ciphertext: {e}"))?;

        if combined.len() < NONCE_SIZE + 16 {
            // At least nonce + auth tag
            return Err(anyhow!("Ciphertext too short"));
        }

        let nonce = Nonce::from_slice(&combined[..NONCE_SIZE]);
        let ciphertext_bytes = &combined[NONCE_SIZE..];

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext_bytes)
            .map_err(|_| anyhow!("Decryption failed - invalid key or corrupted data"))?;

        String::from_utf8(plaintext).map_err(|e| anyhow!("Decrypted data is not valid UTF-8: {e}"))
    }
}

/// Generate a random 256-bit key for AES-256-GCM
/// SECURITY FIX: Uses OsRng for cryptographically secure key generation
pub fn generate_key() -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    rand::rngs::OsRng.fill_bytes(&mut key);
    key
}

/// Generate a random 256-bit key and return as hex string
pub fn generate_hex_key() -> String {
    hex::encode(generate_key())
}

/// Generate a random 256-bit key and return as base64 string
pub fn generate_base64_key() -> String {
    BASE64.encode(generate_key())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let encryptor = TokenEncryptor::new(&key).unwrap();

        let plaintext = "my-secret-access-token-12345";
        let ciphertext = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_hex_key_roundtrip() {
        let hex_key = generate_hex_key();
        assert_eq!(hex_key.len(), 64); // 32 bytes = 64 hex chars

        let encryptor = TokenEncryptor::from_hex_key(&hex_key).unwrap();
        let plaintext = "test-token";
        let encrypted = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encryption_config_encrypt_decrypt() {
        // Test with no encryption (fallback)
        let config = EncryptionConfig {
            enabled: true,
            encryptor: None,
        };
        let plaintext = "test-token";
        let encrypted = config.encrypt(plaintext);
        assert_eq!(plaintext, encrypted); // No encryption = plaintext

        // Test with encryption disabled
        let config = EncryptionConfig {
            enabled: false,
            encryptor: Some(TokenEncryptor::new(&generate_key()).unwrap()),
        };
        let encrypted = config.encrypt(plaintext);
        assert_eq!(plaintext, encrypted); // Disabled = plaintext
    }

    #[test]
    fn test_encryption_config_with_key() {
        let config = EncryptionConfig {
            enabled: true,
            encryptor: Some(TokenEncryptor::new(&generate_key()).unwrap()),
        };

        let plaintext = "my-secret-oauth-token";
        let encrypted = config.encrypt(plaintext);

        // Encrypted should be different from plaintext
        assert_ne!(plaintext, encrypted);

        // Should decrypt back to original
        let decrypted = config.decrypt(&encrypted);
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_decrypt_handles_plaintext_gracefully() {
        let config = EncryptionConfig {
            enabled: true,
            encryptor: Some(TokenEncryptor::new(&generate_key()).unwrap()),
        };

        // Decrypting plaintext (not encrypted) should return as-is
        let plaintext = "already-plaintext-token";
        let decrypted = config.decrypt(plaintext);
        assert_eq!(plaintext, decrypted);
    }
}
