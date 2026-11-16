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

const AAD_V2_PREFIX: &str = "enc:v2:";

/// Encrypt a refresh token bound to its `(session_id, user_id)` AEAD context so
/// ciphertext cannot be transplanted across sessions or users.
///
/// Fails closed in production (`ENVIRONMENT=production`, `PRODUCTION=1|true`, or `K_SERVICE`)
/// if `FRESHCREDIT_TOKEN_ENCRYPTION_KEY` is missing/unavailable.
pub fn try_encrypt_refresh_token_with_context(
    session_id: &str,
    user_id: &str,
    token: &str,
) -> Result<String, &'static str> {
    let aad = format!("sessions:refresh_token_encrypted:{session_id}:{user_id}");
    let config = freshcredit_security::get_encryption_config();
    match config.encrypt_with_aad(token, aad.as_bytes()) {
        Ok(ct) => Ok(format!("{AAD_V2_PREFIX}{ct}")),
        Err(e) => {
            let is_prod = std::env::var("ENVIRONMENT").is_ok_and(|v| v == "production")
                || std::env::var("PRODUCTION").is_ok_and(|v| v == "1" || v == "true")
                || std::env::var("K_SERVICE").is_ok();
            if is_prod {
                tracing::error!(
                    "Refresh token encryption required in production but unavailable: {e}"
                );
                Err("Token encryption key must be configured in production")
            } else {
                debug!("refresh token AAD encryption unavailable ({e}); storing dev plaintext");
                Ok(token.to_string())
            }
        }
    }
}

/// Decrypt an optional refresh token bound to `(session_id, user_id)` AEAD context.
/// Fails closed (`Err`) if the stored token carries `enc:v2:` and AAD/key verification fails;
/// in production, unprefixed tokens are also rejected unless
/// `ALLOW_LEGACY_PLAINTEXT_SESSION_TOKENS=1` is explicitly set for migration.
pub fn decrypt_optional_refresh_token_with_context(
    session_id: &str,
    user_id: &str,
    stored: Option<String>,
) -> Result<Option<String>, &'static str> {
    stored
        .map(|s| decrypt_refresh_token_with_context(session_id, user_id, &s))
        .transpose()
}

/// Decrypt a refresh token bound to `(session_id, user_id)` AEAD context.
/// Fails closed (`Err`) when `enc:v2:` ciphertext fails AEAD verification (never returns raw ciphertext)
/// or when an unprefixed value is encountered in production without explicit migration opt-in.
pub fn decrypt_refresh_token_with_context(
    session_id: &str,
    user_id: &str,
    stored: &str,
) -> Result<String, &'static str> {
    if let Some(b64_ct) = stored.strip_prefix(AAD_V2_PREFIX) {
        let aad = format!("sessions:refresh_token_encrypted:{session_id}:{user_id}");
        let config = freshcredit_security::get_encryption_config();
        return config
            .decrypt_with_aad(b64_ct, aad.as_bytes())
            .map_err(|_| "AEAD tag, key, or session context mismatch");
    }
    let is_prod = std::env::var("ENVIRONMENT").is_ok_and(|v| v == "production")
        || std::env::var("PRODUCTION").is_ok_and(|v| v == "1" || v == "true")
        || std::env::var("K_SERVICE").is_ok();
    let allow_legacy = std::env::var("ALLOW_LEGACY_PLAINTEXT_SESSION_TOKENS")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    if is_prod && !allow_legacy {
        return Err(
            "Unauthenticated or prefix-stripped session refresh token rejected in production",
        );
    }
    Ok(decrypt_from_storage_with(
        freshcredit_security::get_encryption_config(),
        stored,
    ))
}

/// Encrypt a refresh token for storage (legacy un-AAD'd helper, test-only).
#[cfg(test)]
pub fn encrypt_refresh_token_for_storage(token: &str) -> String {
    encrypt_for_storage_with(freshcredit_security::get_encryption_config(), token)
}

/// Decrypt a stored refresh token (legacy un-AAD'd helper, test-only).
#[cfg(test)]
pub fn decrypt_refresh_token_from_storage(stored: &str) -> String {
    decrypt_from_storage_with(freshcredit_security::get_encryption_config(), stored)
}

#[cfg(test)]
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
    config.decrypt(stored).map_or_else(
        |_| {
            // Not decryptable with the configured key: treat as legacy
            // plaintext written before at-rest encryption was enabled.
            debug!("stored refresh token is not encrypted; using legacy plaintext");
            stored.to_string()
        },
        std::convert::identity,
    )
}

/// Encrypt an optional refresh token (legacy un-AAD'd helper, test-only).
#[cfg(test)]
pub fn encrypt_optional_refresh_token(token: Option<&str>) -> Option<String> {
    token.map(encrypt_refresh_token_for_storage)
}

/// Decrypt an optional stored refresh token (legacy un-AAD'd helper, test-only).
#[cfg(test)]
#[allow(clippy::single_option_map)]
pub fn decrypt_optional_refresh_token(stored: Option<String>) -> Option<String> {
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

    #[test]
    fn test_context_aad_roundtrip_and_swap_rejection() {
        let key = generate_hex_key();
        let cfg = EncryptionConfig {
            enabled: true,
            encryptor: Some(TokenEncryptor::from_hex_key(&key).unwrap()),
        };
        let aad_a = b"sessions:refresh_token_encrypted:sess-1:user-a";
        let aad_b = b"sessions:refresh_token_encrypted:sess-2:user-b";
        let ct = cfg.encrypt_with_aad("refresh-xyz", aad_a).unwrap();
        assert_eq!(cfg.decrypt_with_aad(&ct, aad_a).unwrap(), "refresh-xyz");
        assert!(cfg.decrypt_with_aad(&ct, aad_b).is_err());
    }

    #[test]
    fn test_enc_v2_tagged_ciphertext_fails_closed_on_tamper_or_missing_key() {
        // An enc:v2: prefixed payload must NEVER fall back to returning raw ciphertext
        let err = decrypt_refresh_token_with_context("sess-1", "user-a", "enc:v2:tampered-payload");
        assert!(err.is_err());
    }

    #[test]
    fn test_production_rejects_prefix_stripped_or_legacy_plaintext_tokens() {
        // Guard against prefix-strip downgrade attacks when production markers are set
        let prev = std::env::var("ENVIRONMENT").ok();
        std::env::set_var("ENVIRONMENT", "production");
        let err = decrypt_refresh_token_with_context(
            "sess-1",
            "user-a",
            "attacker-stripped-prefix-payload",
        );
        match prev {
            Some(v) => std::env::set_var("ENVIRONMENT", v),
            None => std::env::remove_var("ENVIRONMENT"),
        }
        assert!(err.is_err());
    }
}
