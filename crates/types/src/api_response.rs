// TAG: surface=api owner=platform-team rule=API-001
//! Standardized API Response Types
//!
//! Provides consistent response wrappers across all API endpoints
//! addressing Round 24 API governance findings.
//!
//! # Example Usage
//! ```ignore
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

use crate::ProblemDetails;
use serde::{Deserialize, Serialize};

// P1 FIX: ts-rs for TypeScript type generation

// TAG: surface=api owner=platform-team rule=API-001
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
    // TAG: surface=api owner=platform-team rule=API-001
    ///
    /// # Example
    /// ```ignore
    /// let user = User { id: "123", name: "John" };
    /// let response = ApiResponse::success(user);
    /// ```
    pub const fn success(data: T) -> Self {
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
    pub const fn success_with_meta(data: T, meta: ResponseMeta) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: Some(meta),
        }
        // TAG: surface=api owner=platform-team rule=API-001
    }

    /// Create an error response
    ///
    /// # Example
    /// ```ignore
    /// let error = ProblemDetails::not_found("User not found");
    /// let response = ApiResponse::<User>::error(error);
    /// ```
    #[must_use]
    pub const fn error(error: ProblemDetails) -> Self {
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
// TAG: surface=api owner=platform-team rule=API-001
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

// TAG: surface=api owner=platform-team rule=API-001
impl ResponseMeta {
    /// Create metadata for a paginated response
    ///
    /// # Arguments
    /// * `total_count` - Total number of items available
    /// * `page` - Current page number (1-indexed)
    /// * `per_page` - Items per page
    /// * `request_id` - Unique request identifier for tracing
    #[must_use]
    pub fn paginated(total_count: i64, page: u32, per_page: u32, request_id: String) -> Self {
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
    #[must_use]
    // TAG: surface=api owner=platform-team rule=API-001
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
    #[must_use]
    pub fn simple(returned_count: usize, request_id: String) -> Self {
        Self {
            total_count: None,
            returned_count,
            page: None,
            per_page: None,
            next_cursor: None,
            // TAG: surface=api owner=platform-team rule=API-001
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
/// ```ignore
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
// TAG: surface=api owner=platform-team rule=API-001
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
    /// When provided, `page/per_page` are ignored
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
    // TAG: surface=api owner=platform-team rule=API-001
    /// Ensures page >= 1 and 1 <= `per_page` <= 100
    #[must_use]
    pub fn validate(&self) -> Self {
        Self {
            page: self.page.max(1),
            per_page: self.per_page.clamp(1, 100),
            cursor: self.cursor.clone(),
        }
    }

    /// Calculate SQL OFFSET value
    ///
    /// Returns 0 for page 1, `per_page` for page 2, etc.
    #[must_use]
    pub const fn offset(&self) -> i64 {
        ((self.page - 1) * self.per_page) as i64
    }

    /// Calculate SQL LIMIT value
    #[must_use]
    pub const fn limit(&self) -> i64 {
        self.per_page as i64
    }

    /// Check if cursor-based pagination is being used
    #[must_use]
    // TAG: surface=api owner=platform-team rule=API-001
    pub const fn is_cursor_based(&self) -> bool {
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
/// Used for request tracing across services.
// TAG: surface=api owner=platform-team rule=API-001
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
    // TAG: surface=api owner=platform-team rule=API-001

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
            page: 0,       // Invalid
            per_page: 200, // Too high
            cursor: None,
        };

        let validated = query.validate();
        assert_eq!(validated.page, 1); // Clamped to 1
        assert_eq!(validated.per_page, 100); // Clamped to max
    }
    // TAG: surface=api owner=platform-team rule=API-001

    #[test]
    fn test_response_meta_paginated() {
        let meta = ResponseMeta::paginated(1000, 2, 50, "req-123".to_string());

        assert_eq!(meta.total_count, Some(1000));
        assert_eq!(meta.page, Some(2));
        assert_eq!(meta.per_page, Some(50));
        assert_eq!(meta.api_version, "v1");
    }

    #[test]
    fn test_api_response_success_with_meta() {
        let meta = ResponseMeta::simple(1, "req-456".to_string());
        let response = ApiResponse::success_with_meta("data", meta);

        assert!(response.success);
        assert_eq!(response.data, Some("data"));
        assert!(response.meta.is_some());
    }

    #[test]
    fn test_response_meta_with_cursor() {
        let meta =
            ResponseMeta::with_cursor(10, Some("next-cursor".to_string()), "req-789".to_string());

        // TAG: surface=api owner=platform-team rule=API-001
        assert_eq!(meta.returned_count, 10);
        assert_eq!(meta.next_cursor, Some("next-cursor".to_string()));
        assert_eq!(meta.page, None);
        assert_eq!(meta.per_page, None);
    }

    #[test]
    fn test_response_meta_simple() {
        let meta = ResponseMeta::simple(5, "req-abc".to_string());

        assert_eq!(meta.returned_count, 5);
        assert_eq!(meta.total_count, None);
        assert_eq!(meta.next_cursor, None);
    }

    #[test]
    fn test_pagination_query_offset() {
        let query = PaginationQuery {
            page: 3,
            per_page: 25,
            cursor: None,
        };
        assert_eq!(query.offset(), 50);
    }

    // TAG: surface=api owner=platform-team rule=API-001
    #[test]
    fn test_pagination_query_limit() {
        let query = PaginationQuery {
            page: 1,
            per_page: 50,
            cursor: None,
        };
        assert_eq!(query.limit(), 50);
    }

    #[test]
    fn test_pagination_query_is_cursor_based() {
        let cursor_query = PaginationQuery {
            page: 1,
            per_page: 20,
            cursor: Some("abc".to_string()),
        };
        assert!(cursor_query.is_cursor_based());

        let page_query = PaginationQuery {
            page: 1,
            per_page: 20,
            cursor: None,
        };
        assert!(!page_query.is_cursor_based());
    }
    // TAG: surface=api owner=platform-team rule=API-001
}
