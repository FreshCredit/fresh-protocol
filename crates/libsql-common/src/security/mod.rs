//! SQL Injection Prevention Utilities
//!
//! This module provides safe SQL query construction helpers to prevent
//! SQL injection vulnerabilities.

use std::fmt;

/// Validates that an identifier (table/column name) contains only safe characters
/// # Errors
///
/// Returns an error if the operation fails.
pub fn validate_identifier(ident: &str) -> Result<(), SqlSecurityError> {
    if ident.is_empty() {
        return Err(SqlSecurityError::EmptyIdentifier);
    }

    // Only allow alphanumeric and underscore
    if !ident.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(SqlSecurityError::InvalidCharacters(ident.to_string()));
    }

    // Prevent SQL keywords as identifiers
    let keywords = [
        "SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "TABLE", "FROM", "WHERE",
    ];
    if keywords.contains(&ident.to_uppercase().as_str()) {
        return Err(SqlSecurityError::ReservedKeyword(ident.to_string()));
    }

    Ok(())
}

/// Safely formats a table name for use in SQL
/// # Errors
///
/// Returns an error if the operation fails.
pub fn safe_table_name(name: &str) -> Result<String, SqlSecurityError> {
    validate_identifier(name)?;
    Ok(name.to_string())
}

/// Safely formats a column name for use in SQL
/// # Errors
///
/// Returns an error if the operation fails.
pub fn safe_column_name(name: &str) -> Result<String, SqlSecurityError> {
    validate_identifier(name)?;
    Ok(name.to_string())
}

#[derive(Debug, Clone)]
pub enum SqlSecurityError {
    EmptyIdentifier,
    InvalidCharacters(String),
    ReservedKeyword(String),
}

impl fmt::Display for SqlSecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentifier => write!(f, "Identifier cannot be empty"),
            Self::InvalidCharacters(s) => {
                write!(f, "Invalid characters in identifier: {s}")
            }
            Self::ReservedKeyword(s) => {
                write!(f, "Reserved keyword used as identifier: {s}")
            }
        }
    }
}

impl std::error::Error for SqlSecurityError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_identifier_valid() {
        assert!(validate_identifier("users").is_ok());
        assert!(validate_identifier("user_id").is_ok());
        assert!(validate_identifier("table_123").is_ok());
        assert!(validate_identifier("_private").is_ok());
    }

    #[test]
    fn test_validate_identifier_empty() {
        let result = validate_identifier("");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Identifier cannot be empty"
        );
    }

    #[test]
    fn test_validate_identifier_invalid_chars() {
        let result = validate_identifier("user-id");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid characters in identifier: user-id"
        );
    }

    #[test]
    fn test_validate_identifier_reserved_keyword() {
        let result = validate_identifier("SELECT");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Reserved keyword used as identifier: SELECT"
        );
    }

    #[test]
    fn test_validate_identifier_case_insensitive_keyword() {
        let result = validate_identifier("drop");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Reserved keyword used as identifier: drop"
        );
    }

    #[test]
    fn test_safe_table_name() {
        assert_eq!(safe_table_name("accounts").unwrap(), "accounts");
        assert!(safe_table_name("").is_err());
        assert!(safe_table_name("table-name").is_err());
    }

    #[test]
    fn test_safe_column_name() {
        assert_eq!(safe_column_name("email").unwrap(), "email");
        assert!(safe_column_name("").is_err());
        assert!(safe_column_name("column.name").is_err());
    }
}
