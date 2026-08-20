// TAG: surface=security owner=security-team rule=SEC-001
//! AES-256-GCM encryption for OAuth refresh tokens at rest.
//!
//! The `sessions.refresh_token_encrypted` column historically stored the Entra
//! refresh token in plaintext despite its name. All write paths encrypt with
//! `freshcredit-security` (same key wiring as Plaid/ORCID tokens:
//! `FRESHCREDIT_TOKEN_ENCRYPTION_KEY` env, global config) and all read paths
//! decrypt, with a graceful legacy-plaintext fallback so sessions persisted
//! before encryption was enabled keep working.

use freshcredit_security::EncryptionConfig;
use tracing::debug;

/// Encrypt a refresh token for storage.
///
/// Falls back to plaintext when encryption is not configured (e.g. local dev
/// without `FRESHCREDIT_TOKEN_ENCRYPTION_KEY`) so behavior matches the
/// pre-encryption storage format.
pub(crate) fn encrypt_refresh_token_for_storage(token: &str) -> String {
    encrypt_for_storage_with(freshcredit_security::get_encryption_config(), token)
}

/// Decrypt a stored refresh token.
///
/// Legacy plaintext values (or any value that fails decryption) are returned
/// as-is so pre-encryption sessions keep working.
pub(crate) fn decrypt_refresh_token_from_storage(stored: &str) -> String {
    decrypt_from_storage_with(freshcredit_security::get_encryption_config(), stored)
}

fn encrypt_for_storage_with(config: &EncryptionConfig, token: &str) -> String {
    match config.encrypt(token) {
        Ok(ciphertext) => ciphertext,
        Err(e) => {
            debug!(
                "refresh token encryption unavailable ({}); storing plaintext",
                e
            );
            token.to_string()
        }
    }
}

fn decrypt_from_storage_with(config: &EncryptionConfig, stored: &str) -> String {
    match config.decrypt(stored) {
        Ok(plaintext) => plaintext,
        Err(_) => {
            // Not decryptable with the configured key: treat as legacy
            // plaintext written before at-rest encryption was enabled.
            debug!("stored refresh token is not encrypted; using legacy plaintext");
            stored.to_string()
        }
    }
}

/// Encrypt an optional refresh token (write path helper).
pub(crate) fn encrypt_optional_refresh_token(token: Option<&str>) -> Option<String> {
    token.map(encrypt_refresh_token_for_storage)
}

/// Decrypt an optional stored refresh token (read path helper).
pub(crate) fn decrypt_optional_refresh_token(stored: Option<String>) -> Option<String> {
    stored.map(|t| decrypt_refresh_token_from_storage(&t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use freshcredit_security::{generate_hex_key, TokenEncryptor};

    fn test_config() -> EncryptionConfig {
        EncryptionConfig {
            enabled: true,
            encryptor: Some(TokenEncryptor::from_hex_key(&generate_hex_key()).unwrap()),
        }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip_with_key() {
        let config = test_config();
        let plaintext = "0.ARwA_refresh_token_value";
        let stored = encrypt_for_storage_with(&config, plaintext);
        assert_ne!(stored, plaintext, "ciphertext must differ from plaintext");
        let recovered = decrypt_from_storage_with(&config, &stored);
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn test_decrypt_falls_back_to_legacy_plaintext() {
        let config = test_config();
        // A legacy plaintext token cannot be decrypted -> returned as-is.
        let legacy = "plain-legacy-refresh-token";
        assert_eq!(decrypt_from_storage_with(&config, legacy), legacy);
    }

    #[test]
    fn test_encrypt_passthrough_when_not_configured() {
        let config = EncryptionConfig {
            enabled: false,
            encryptor: None,
        };
        let token = "some-refresh-token";
        assert_eq!(encrypt_for_storage_with(&config, token), token);
        // ...and the read path passes it straight back.
        assert_eq!(decrypt_from_storage_with(&config, token), token);
    }

    #[test]
    fn test_optional_helpers_roundtrip() {
        // Global config may or may not have a key in the test environment;
        // either way the stored value must round-trip to the original.
        let original = "roundtrip-refresh-token";
        let stored = encrypt_refresh_token_for_storage(original);
        assert_eq!(decrypt_refresh_token_from_storage(&stored), original);

        let stored = encrypt_optional_refresh_token(Some(original));
        assert_eq!(
            decrypt_optional_refresh_token(stored),
            Some(original.to_string())
        );
        assert_eq!(decrypt_optional_refresh_token(None), None);
        assert_eq!(encrypt_optional_refresh_token(None), None);
    }

    #[test]
    fn test_wrong_key_falls_back_to_stored_value() {
        let config_a = test_config();
        let config_b = test_config();
        let stored = encrypt_for_storage_with(&config_a, "secret-token");
        // Decrypting with a different key fails -> legacy fallback returns the
        // stored value rather than panicking or erroring.
        assert_eq!(decrypt_from_storage_with(&config_b, &stored), stored);
    }
}
