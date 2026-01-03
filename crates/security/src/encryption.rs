//! Token encryption utilities using AES-256-GCM
//!
//! Provides authenticated encryption for sensitive tokens (ORCID, Plaid, etc.)
//! Uses AES-256-GCM with a 256-bit key and 96-bit nonce.
//!
//! # Security Notes
//! - Key must be stored securely (GCP Secret Manager, env var, etc.)
//! - Each encryption uses a random nonce
//! - Ciphertext includes authentication tag to detect tampering
//! - Output format: base64(nonce || ciphertext || tag)

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;

/// Nonce size for AES-GCM (96 bits = 12 bytes)
const NONCE_SIZE: usize = 12;

/// Key size for AES-256 (256 bits = 32 bytes)
pub const KEY_SIZE: usize = 32;

/// Token encryptor using AES-256-GCM
pub struct TokenEncryptor {
    cipher: Aes256Gcm,
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

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| anyhow!("Failed to create cipher: {e}"))?;

        Ok(Self { cipher })
    }

    /// Create a TokenEncryptor from a hex-encoded key
    ///
    /// # Arguments
    /// * `hex_key` - 64-character hex string representing a 256-bit key
    pub fn from_hex_key(hex_key: &str) -> Result<Self> {
        let key = hex::decode(hex_key)
            .map_err(|e| anyhow!("Invalid hex key: {e}"))?;
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
        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
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

        String::from_utf8(plaintext)
            .map_err(|e| anyhow!("Decrypted data is not valid UTF-8: {e}"))
    }
}

/// Generate a random 256-bit key for AES-256-GCM
pub fn generate_key() -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    rand::thread_rng().fill_bytes(&mut key);
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
}

