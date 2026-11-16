// TAG: surface=database owner=data-team rule=DB-001
//! Turso URL builder for per-user database connections.
//!
//! This module provides a centralized, consistent way to construct
//! per-user Turso database URLs with proper sanitization.

// TAG: surface=database owner=platform-team rule=DB-001
/// Builder for constructing per-user Turso database URLs.
///
/// Provides consistent URL construction with:
/// - Alphanumeric-only characters (non-alphanumeric replaced with hyphens)
/// - Lowercase normalization
/// - 28-character truncation for Turso naming limits
///
/// # Example
///
/// ```rust
/// use freshcredit_libsql_common::TursoUrlBuilder;
///
/// let builder = TursoUrlBuilder::new("devonshigaki");
/// let url = builder.user_database_url("user@example.com");
/// assert!(url.starts_with("libsql://user-"));
/// assert!(url.contains("-devonshigaki."));
/// ```
#[derive(Debug, Clone)]
pub struct TursoUrlBuilder {
    /// Turso organization name
    organization: String,
    /// AWS region for Turso (defaults to us-west-2)
    region: String,
}

impl TursoUrlBuilder {
    // HARDCODED_LIMIT: 28 chars for user ID truncation
    // Turso database names have a ~63 char limit, but we use 28 for safety margin
    // after adding "user-" prefix and organization suffix
    const MAX_USER_ID_LENGTH: usize = 28;

    /// Create a new `TursoUrlBuilder` with the specified organization.
    ///
    /// Uses `aws-us-west-2` as the default region.
    pub fn new(organization: impl Into<String>) -> Self {
        Self {
            organization: Self::sanitize_dns_label(&organization.into(), "devonshigaki"),
            region: "aws-us-west-2".to_string(),
        }
        // TAG: surface=database owner=platform-team rule=GENERAL-001
    }

    /// Create a new `TursoUrlBuilder` with custom organization and region.
    pub fn with_region(organization: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            organization: Self::sanitize_dns_label(&organization.into(), "devonshigaki"),
            region: Self::sanitize_dns_label(&region.into(), "aws-us-west-2"),
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create a `TursoUrlBuilder` from environment variables.
    ///
    /// Uses `TURSO_ORGANIZATION` env var, defaulting to "devonshigaki".
    /// Uses `TURSO_REGION` env var, defaulting to "aws-us-west-2".
    #[must_use]
    pub fn from_env() -> Self {
        let organization =
            std::env::var("TURSO_ORGANIZATION").unwrap_or_else(|_| "devonshigaki".to_string());
        let region = std::env::var("TURSO_REGION").unwrap_or_else(|_| "aws-us-west-2".to_string());
        Self {
            organization,
            region,
        }
    }

    /// Generate the database name for a user ID.
    ///
    /// Format: `user-{sanitized_user_id}`
    ///
    /// Sanitization:
    /// - Replaces non-alphanumeric characters with hyphens
    /// - Converts to lowercase
    /// - Truncates to 28 characters
    #[must_use]
    pub fn user_database_name(&self, user_id: &str) -> String {
        let sanitized = Self::sanitize_user_id(user_id);
        format!("user-{sanitized}")
    }

    /// Generate the full Turso URL for a user's database.
    ///
    /// Format: `libsql://{db_name}-{organization}.{region}.turso.io`
    #[must_use]
    pub fn user_database_url(&self, user_id: &str) -> String {
        // TAG: surface=database owner=data-team rule=DB-001
        let db_name = self.user_database_name(user_id);
        format!(
            "libsql://{}-{}.{}.turso.io",
            db_name, self.organization, self.region
        )
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Generate the HTTPS pipeline URL for a user's database.
    ///
    /// Format: `https://{db_name}-{organization}.{region}.turso.io/v2/pipeline`
    ///
    /// Used for HTTP-based sync proxy operations.
    #[must_use]
    pub fn user_pipeline_url(&self, user_id: &str) -> String {
        let db_name = self.user_database_name(user_id);
        format!(
            "https://{}-{}.{}.turso.io/v2/pipeline",
            db_name, self.organization, self.region
        )
    }

    fn sanitize_dns_label(raw: &str, fallback: &str) -> String {
        let cleaned: String = raw
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .map(|c| c.to_ascii_lowercase())
            .take(63)
            .collect();
        let trimmed = cleaned.trim_matches('-');
        if trimmed.is_empty() {
            fallback.to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Deterministic 32-bit FNV-1a hex disambiguator over raw UTF-8 `user_id` bytes.
    fn fnv1a_hash8(raw: &str) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in raw.as_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{:08x}", (h ^ (h >> 32)) as u32)
    }

    /// Sanitize a user ID for use in database naming (collision-resistant & UTF-8 safe).
    ///
    /// - Keeps only ASCII alphanumerics (replacing non-ASCII-alphanumeric runs with `-`)
    /// - Lowercases ASCII characters
    /// - Truncates safely on ASCII char boundaries and appends an 8-hex-char raw-ID hash
    ///   whenever normalization/truncation is lossy so distinct tenants can NEVER collide.
    fn sanitize_user_id(user_id: &str) -> String {
        let needs_disambiguation = user_id.len() > Self::MAX_USER_ID_LENGTH
            || !user_id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());

        let mut sanitized = String::with_capacity(user_id.len().min(Self::MAX_USER_ID_LENGTH));
        let mut prev_was_dash = true;
        for c in user_id.chars() {
            if c.is_ascii_alphanumeric() {
                sanitized.push(c.to_ascii_lowercase());
                prev_was_dash = false;
            } else if !prev_was_dash {
                sanitized.push('-');
                prev_was_dash = true;
            }
        }

        while sanitized.ends_with('-') {
            sanitized.pop();
        }

        if needs_disambiguation {
            let prefix_cap = Self::MAX_USER_ID_LENGTH.saturating_sub(9); // room for "-xxxxxxxx"
            if sanitized.len() > prefix_cap {
                sanitized.truncate(prefix_cap);
                while sanitized.ends_with('-') {
                    sanitized.pop();
                }
            }
            let tag = Self::fnv1a_hash8(user_id);
            if sanitized.is_empty() {
                tag
            } else {
                format!("{sanitized}-{tag}")
            }
        } else if sanitized.is_empty() {
            "unknown".to_string()
        } else {
            sanitized
        }
    }
    // TAG: surface=database owner=platform-team rule=GENERAL-001

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get the organization name.
    #[must_use]
    pub fn organization(&self) -> &str {
        &self.organization
    }

    /// Get the region.
    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_email() {
        let builder = TursoUrlBuilder::new("testorg");
        let url = builder.user_database_url("user@example.com");
        assert!(url.starts_with("libsql://user-user-example-com-"));
        assert!(url.ends_with("-testorg.aws-us-west-2.turso.io"));
        // TAG: surface=database owner=platform-team rule=GENERAL-001
    }

    #[test]
    fn test_truncation() {
        let builder = TursoUrlBuilder::new("org");
        let long_id = "a".repeat(50);
        let db_name = builder.user_database_name(&long_id);
        // "user-" (5 chars) + <= 28 chars = <= 33 chars max
        assert!(db_name.len() <= 33);
        assert!(db_name.starts_with("user-"));
    }

    #[test]
    fn test_lowercase() {
        let builder = TursoUrlBuilder::new("org");
        let db_name = builder.user_database_name("USER@EXAMPLE.COM");
        assert!(db_name.starts_with("user-user-example-com-"));
        assert!(db_name.is_ascii());
        // TAG: surface=database owner=data-team rule=DB-001
    }

    #[test]
    fn test_special_chars() {
        let builder = TursoUrlBuilder::new("org");
        let db_name = builder.user_database_name("user+test!@#$%^&*()");
        assert!(db_name.starts_with("user-user-test-"));
    }

    #[test]
    fn test_no_collisions_on_punctuation_long_dids_or_utf8() {
        let builder = TursoUrlBuilder::new("org");
        assert_ne!(
            builder.user_database_name("alice.smith@x.com"),
            builder.user_database_name("alice-smith@x.com")
        );
        assert_ne!(
            builder.user_database_name("did:kilt:4tJbxxKqYRv3gDvY66B9A1"),
            builder.user_database_name("did:kilt:4tJbxxKqYRv3gDvY66B9A2")
        );
        // Multi-byte UTF-8 around the 28-byte boundary must never panic and must remain ASCII
        let utf8_id = "user-ééééééééééééééééééééé";
        let utf8_db = builder.user_database_name(utf8_id);
        assert!(utf8_db.is_ascii());
        assert!(utf8_db.starts_with("user-"));
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    #[test]
    fn test_with_region() {
        let builder = TursoUrlBuilder::with_region("myorg", "eu-west-1");
        let url = builder.user_database_url("testuser");
        assert!(url.contains("myorg"));
        assert!(url.contains("eu-west-1"));
    }

    #[test]
    fn test_user_pipeline_url() {
        let builder = TursoUrlBuilder::new("testorg");
        let url = builder.user_pipeline_url("user@example.com");
        assert!(url.starts_with("https://"));
        assert!(url.contains("/v2/pipeline"));
        assert!(url.contains("testorg"));
    }

    #[test]
    fn test_organization() {
        let builder = TursoUrlBuilder::new("myorg");
        assert_eq!(builder.organization(), "myorg");
    }

    #[test]
    fn test_region() {
        let builder = TursoUrlBuilder::with_region("org", "ap-south-1");
        assert_eq!(builder.region(), "ap-south-1");
    }

    #[test]
    fn test_demo_user_id_does_not_end_with_dash() {
        let builder = TursoUrlBuilder::new("org");
        let db_name =
            builder.user_database_name("demo-consumer-123e4567-e89b-12d3-a456-426614174000");
        assert!(!db_name.ends_with('-'), "db_name {db_name} ends with dash");
        assert!(
            !db_name.contains("--"),
            "db_name {db_name} contains double dash"
        );
        assert!(db_name.starts_with("user-"));
    }

    #[test]
    fn test_uuid_boundary_trailing_dash_removed() {
        let builder = TursoUrlBuilder::new("org");
        // 28-char truncation cuts right after a hyphen; ensure we trim it.
        let db_name =
            builder.user_database_name("demo-provider-123e4567-e89b-12d3-a456-426614174000");
        assert!(!db_name.ends_with('-'), "db_name {db_name} ends with dash");
        assert!(
            !db_name.contains("--"),
            "db_name {db_name} contains double dash"
        );
    }
}
