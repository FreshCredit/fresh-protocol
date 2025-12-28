//! Turso URL builder for per-user database connections.
//!
//! This module provides a centralized, consistent way to construct
//! per-user Turso database URLs with proper sanitization.

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

    /// Create a new TursoUrlBuilder with the specified organization.
    ///
    /// Uses `aws-us-west-2` as the default region.
    pub fn new(organization: impl Into<String>) -> Self {
        Self {
            organization: organization.into(),
            region: "aws-us-west-2".to_string(),
        }
    }

    /// Create a new TursoUrlBuilder with custom organization and region.
    pub fn with_region(organization: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            organization: organization.into(),
            region: region.into(),
        }
    }

    /// Create a TursoUrlBuilder from environment variables.
    ///
    /// Uses `TURSO_ORGANIZATION` env var, defaulting to "devonshigaki".
    /// Uses `TURSO_REGION` env var, defaulting to "aws-us-west-2".
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
    pub fn user_database_name(&self, user_id: &str) -> String {
        let sanitized = self.sanitize_user_id(user_id);
        format!("user-{}", sanitized)
    }

    /// Generate the full Turso URL for a user's database.
    ///
    /// Format: `libsql://{db_name}-{organization}.{region}.turso.io`
    pub fn user_database_url(&self, user_id: &str) -> String {
        let db_name = self.user_database_name(user_id);
        format!(
            "libsql://{}-{}.{}.turso.io",
            db_name, self.organization, self.region
        )
    }

    /// Generate the HTTPS pipeline URL for a user's database.
    ///
    /// Format: `https://{db_name}-{organization}.{region}.turso.io/v2/pipeline`
    ///
    /// Used for HTTP-based sync proxy operations.
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
    /// - Truncates to MAX_USER_ID_LENGTH (28) characters
    fn sanitize_user_id(&self, user_id: &str) -> String {
        let sanitized: String = user_id
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();

        if sanitized.len() > Self::MAX_USER_ID_LENGTH {
            sanitized[..Self::MAX_USER_ID_LENGTH].to_string()
        } else {
            sanitized
        }
    }

    /// Get the organization name.
    pub fn organization(&self) -> &str {
        &self.organization
    }

    /// Get the region.
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
    }

    #[test]
    fn test_special_chars() {
        let builder = TursoUrlBuilder::new("org");
        let db_name = builder.user_database_name("user+test!@#$%^&*()");
        assert_eq!(db_name, "user-user-test----------");
    }
}
