//! PII redaction helpers for logs.
//!
//! Centralized logs are a data plane: `info!`/`warn!`/`error!` records must
//! not carry raw §10-classified PII. Use these keyed, non-reversible
//! identifiers instead — deterministic so operators can correlate, opaque so
//! the log stream is not a PII plane.

use sha2::{Digest, Sha256};

/// Returns a keyed, non-reversible identifier for an email address, formatted
/// as `email:<hash>` where `<hash>` is the first 16 hex chars of the SHA-256
/// digest of the lowercased, trimmed email.
#[must_use]
pub fn email_log_key(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let hex_hash = hex::encode(Sha256::digest(normalized.as_bytes()));
    format!("email:{}", &hex_hash[..16])
}

#[cfg(test)]
mod tests {
    use super::email_log_key;

    #[test]
    fn same_input_produces_same_hash() {
        assert_eq!(
            email_log_key("user@example.com"),
            email_log_key("user@example.com")
        );
    }

    #[test]
    fn different_emails_produce_different_hashes() {
        assert_ne!(
            email_log_key("a@example.com"),
            email_log_key("b@example.com")
        );
    }

    #[test]
    fn hashing_is_case_and_whitespace_insensitive() {
        assert_eq!(
            email_log_key("  User@Example.COM  "),
            email_log_key("user@example.com")
        );
    }

    #[test]
    fn output_is_email_prefix_plus_16_char_hash() {
        let key = email_log_key("user@example.com");
        assert!(key.starts_with("email:"));
        assert_eq!(key.len(), "email:".len() + 16);
        assert!(key["email:".len()..].chars().all(|c| c.is_ascii_hexdigit()));
    }
}
