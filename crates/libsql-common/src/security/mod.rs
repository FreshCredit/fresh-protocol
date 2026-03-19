//! SQL Injection Prevention Utilities
//!
//! This module provides safe SQL query construction helpers to prevent
//! SQL injection vulnerabilities.

use std::fmt;

/// Validates that an identifier (table/column name) contains only safe characters
pub fn validate_identifier(ident: &str) -> Result<(), SqlSecurityError> {
    if ident.is_empty() {
        return Err(SqlSecurityError::EmptyIdentifier);
    }
    
    // Only allow alphanumeric and underscore
    if !ident.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(SqlSecurityError::InvalidCharacters(ident.to_string()));
    }
    
    // Prevent SQL keywords as identifiers
    let keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "TABLE", "FROM", "WHERE"];
    if keywords.contains(&ident.to_uppercase().as_str()) {
        return Err(SqlSecurityError::ReservedKeyword(ident.to_string()));
    }
    
    Ok(())
}

/// Safely formats a table name for use in SQL
pub fn safe_table_name(name: &str) -> Result<String, SqlSecurityError> {
    validate_identifier(name)?;
    Ok(name.to_string())
}

/// Safely formats a column name for use in SQL
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
            SqlSecurityError::EmptyIdentifier => write!(f, "Identifier cannot be empty"),
            SqlSecurityError::InvalidCharacters(s) => write!(f, "Invalid characters in identifier: {}", s),
            SqlSecurityError::ReservedKeyword(s) => write!(f, "Reserved keyword used as identifier: {}", s),
        }
    }
}

impl std::error::Error for SqlSecurityError {}
