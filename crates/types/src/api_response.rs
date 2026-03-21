//! Standardized API Response Types
//! 
//! Provides consistent response wrappers across all API endpoints
//! addressing Round 24 API governance findings.
//!
//! # Example Usage
//! ```
//! use core_types::{ApiResponse, ResponseMeta, PaginationQuery};
//!
//! // Successful response
//! let response = ApiResponse::success(user_data);
//!
//! // Paginated response
//! let meta = ResponseMeta::paginated(total, page, per_page, request_id);
//! let response = ApiResponse::success_with_meta(users, meta);
//!
//! // Error response
//! let error = ProblemDetails::not_found("User not found");
//! let response = ApiResponse::<User>::error(error);
//! ```

use serde::{Deserialize, Serialize};
use crate::ProblemDetails;

// P1 FIX: ts-rs for TypeScript type generation
#[cfg(feature = "typescript")]
use ts_rs::TS;

/// Standard API response wrapper for single resources
/// 
/// This type provides a consistent response format across all API endpoints,
/// addressing the API governance finding of mixed response formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct ApiResponse<T> {
    /// Whether the request was successful
    pub success: bool,
    /// The response data (only present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error details (only present on error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProblemDetails>,
    /// Response metadata (pagination, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ResponseMeta>,
}

impl<T> ApiResponse<T> {
    /// Create a successful response with data
    /// 
    /// # Example
    /// ```
    /// let user = User { id: "123", name: "John" };
    /// let response = ApiResponse::success(user);
    /// ```
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: None,
        }
    }

    /// Create a successful response with data and metadata
    /// 
    /// Use this for paginated responses or when additional metadata is needed.
    pub fn success_with_meta(data: T, meta: ResponseMeta) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: Some(meta),
        }
    }

    /// Create an error response
    /// 
    /// # Example
    /// ```
    /// let error = ProblemDetails::not_found("User not found");
    /// let response = ApiResponse::<User>::error(error);
    /// ```
    pub fn error(error: ProblemDetails) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            meta: None,
        }
    }
}

/// Response metadata for pagination and filtering
/// 
/// Provides consistent pagination information across all list endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct ResponseMeta {
    /// Total number of items available (for offset pagination)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<i64>,
    /// Number of items returned in this response
    pub returned_count: usize,
    /// Current page number (1-indexed, for offset pagination)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// Items per page (for offset pagination)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u32>,
    /// Cursor for cursor-based pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// API version used for this response
    #[serde(default = "default_api_version")]
    pub api_version: String,
    /// Request ID for tracing and debugging
    pub request_id: String,
}

fn default_api_version() -> String {
    "v1".to_string()
}

impl ResponseMeta {
    /// Create metadata for a paginated response
    /// 
    /// # Arguments
    /// * `total_count` - Total number of items available
    /// * `page` - Current page number (1-indexed)
    /// * `per_page` - Items per page
    /// * `request_id` - Unique request identifier for tracing
    pub fn paginated(
        total_count: i64,
        page: u32,
        per_page: u32,
        request_id: String,
    ) -> Self {
        Self {
            total_count: Some(total_count),
            returned_count: per_page as usize,
            page: Some(page),
            per_page: Some(per_page),
            next_cursor: None,
            api_version: default_api_version(),
            request_id,
        }
    }

    /// Create metadata for cursor-based pagination
    /// 
    /// Cursor-based pagination is preferred for high-volume data
    /// as it provides consistent performance regardless of dataset size.
    pub fn with_cursor(
        returned_count: usize,
        next_cursor: Option<String>,
        request_id: String,
    ) -> Self {
        Self {
            total_count: None,
            returned_count,
            page: None,
            per_page: None,
            next_cursor,
            api_version: default_api_version(),
            request_id,
        }
    }

    /// Create simple metadata without pagination
    pub fn simple(returned_count: usize, request_id: String) -> Self {
        Self {
            total_count: None,
            returned_count,
            page: None,
            per_page: None,
            next_cursor: None,
            api_version: default_api_version(),
            request_id,
        }
    }
}

/// Standard pagination query parameters
/// 
/// Use this struct as an Axum extractor for consistent pagination
/// across all list endpoints.
/// 
/// # Example
/// ```
/// use axum::extract::Query;
/// 
/// async fn list_users(
///     Query(pagination): Query<PaginationQuery>,
/// ) -> impl IntoResponse {
///     let pagination = pagination.validate();
///     let offset = pagination.offset();
///     let limit = pagination.limit();
///     // ... fetch users
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct PaginationQuery {
    /// Page number (1-indexed, default: 1)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page (default: 20, max: 100)
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    /// Cursor for cursor-based pagination
    /// 
    /// When provided, page/per_page are ignored
    pub cursor: Option<String>,
}

const fn default_page() -> u32 {
    1
}

const fn default_per_page() -> u32 {
    20
}

impl PaginationQuery {
    /// Validate and clamp pagination parameters
    /// 
    /// Ensures page >= 1 and 1 <= per_page <= 100
    pub fn validate(&self) -> Self {
        Self {
            page: self.page.max(1),
            per_page: self.per_page.clamp(1, 100),
            cursor: self.cursor.clone(),
        }
    }

    /// Calculate SQL OFFSET value
    /// 
    /// Returns 0 for page 1, per_page for page 2, etc.
    pub fn offset(&self) -> i64 {
        ((self.page - 1) * self.per_page) as i64
    }

    /// Calculate SQL LIMIT value
    pub fn limit(&self) -> i64 {
        self.per_page as i64
    }

    /// Check if cursor-based pagination is being used
    pub fn is_cursor_based(&self) -> bool {
        self.cursor.is_some()
    }
}

/// API version header constant
/// 
/// Use this header in all API responses for client version tracking.
pub const API_VERSION_HEADER: &str = "X-API-Version";

/// Deprecation header constant (RFC 8594)
/// 
/// Include this header when an endpoint is deprecated.
/// Format: date or URL
/// Example: `Deprecation: Sun, 01 Jun 2025 00:00:00 GMT`
pub const DEPRECATION_HEADER: &str = "Deprecation";

/// Sunset header constant (RFC 8594)
/// 
/// Include this header to indicate when an endpoint will be removed.
/// Format: HTTP date
/// Example: `Sunset: Sun, 01 Sep 2025 00:00:00 GMT`
pub const SUNSET_HEADER: &str = "Sunset";

/// Request ID header constant
/// 
/// Used for request tracing across services.
pub const REQUEST_ID_HEADER: &str = "X-Request-ID";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let data = "test data";
        let response = ApiResponse::success(data);
        
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let error = ProblemDetails::not_found("Test not found");
        let response: ApiResponse<String> = ApiResponse::error(error);
        
        assert!(!response.success);
        assert!(response.data.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_pagination_query_defaults() {
        let query = PaginationQuery {
            page: 1,
            per_page: 20,
            cursor: None,
        };
        
        assert_eq!(query.offset(), 0);
        assert_eq!(query.limit(), 20);
    }

    #[test]
    fn test_pagination_query_validation() {
        let query = PaginationQuery {
            page: 0, // Invalid
            per_page: 200, // Too high
            cursor: None,
        };
        
        let validated = query.validate();
        assert_eq!(validated.page, 1); // Clamped to 1
        assert_eq!(validated.per_page, 100); // Clamped to max
    }

    #[test]
    fn test_response_meta_paginated() {
        let meta = ResponseMeta::paginated(1000, 2, 50, "req-123".to_string());
        
        assert_eq!(meta.total_count, Some(1000));
        assert_eq!(meta.page, Some(2));
        assert_eq!(meta.per_page, Some(50));
        assert_eq!(meta.api_version, "v1");
    }
}
