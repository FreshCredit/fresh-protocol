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
            organization: organization.into(),
            region: "aws-us-west-2".to_string(),
        }
        // TAG: surface=database owner=platform-team rule=GENERAL-001
    }

    /// Create a new `TursoUrlBuilder` with custom organization and region.
    pub fn with_region(organization: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            organization: organization.into(),
            region: region.into(),
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

    /// Sanitize a user ID for use in database naming.
    ///
    /// - Replaces non-alphanumeric characters with hyphens
    /// - Converts to lowercase
    /// - Truncates to `MAX_USER_ID_LENGTH` (28) characters
    fn sanitize_user_id(user_id: &str) -> String {
        // Replace non-alphanumeric runs with a single hyphen and lowercase the result.
        let mut sanitized = String::with_capacity(user_id.len());
        let mut prev_was_dash = true; // treat leading non-alphanumeric as a dash, then trim later
        for c in user_id.chars() {
            if c.is_alphanumeric() {
                sanitized.push(c.to_ascii_lowercase());
                prev_was_dash = false;
            } else if !prev_was_dash {
                sanitized.push('-');
                prev_was_dash = true;
            }
        }

        // Trim trailing dashes introduced by special chars or truncation.
        while sanitized.ends_with('-') {
            sanitized.pop();
        }

        // Truncate to the allowed length, then trim any trailing dash left by the cut.
        if sanitized.len() > Self::MAX_USER_ID_LENGTH {
            let mut truncated = sanitized[..Self::MAX_USER_ID_LENGTH].to_string();
            while truncated.ends_with('-') {
                truncated.pop();
            }
            sanitized = truncated;
        }

        if sanitized.is_empty() {
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
        assert_eq!(
            url,
            "libsql://user-user-example-com-testorg.aws-us-west-2.turso.io"
        );
        // TAG: surface=database owner=platform-team rule=GENERAL-001
    }

    #[test]
    fn test_truncation() {
        let builder = TursoUrlBuilder::new("org");
        let long_id = "a".repeat(50);
        let db_name = builder.user_database_name(&long_id);
        // "user-" (5 chars) + 28 chars = 33 chars max
        assert_eq!(db_name.len(), 33);
        assert!(db_name.starts_with("user-"));
    }

    #[test]
    fn test_lowercase() {
        let builder = TursoUrlBuilder::new("org");
        let db_name = builder.user_database_name("USER@EXAMPLE.COM");
        assert_eq!(db_name, "user-user-example-com");
        // TAG: surface=database owner=data-team rule=DB-001
    }

    #[test]
    fn test_special_chars() {
        let builder = TursoUrlBuilder::new("org");
        let db_name = builder.user_database_name("user+test!@#$%^&*()");
        assert_eq!(db_name, "user-user-test");
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
        assert!(
            !db_name.ends_with('-'),
            "db_name {db_name} ends with dash"
        );
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
        assert!(
            !db_name.ends_with('-'),
            "db_name {db_name} ends with dash"
        );
        assert!(
            !db_name.contains("--"),
            "db_name {db_name} contains double dash"
        );
    }
}
