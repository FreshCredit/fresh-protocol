//! API response wrappers and error types

use chrono::{
    DateTime,
    Utc,
};
use serde::{
    Deserialize,
    Serialize,
};

#[cfg(feature = "typescript")]
use ts_rs::TS;

/// API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    #[must_use]
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

/// Result type for `FreshCredit` operations
pub type FreshCreditResult<T> = Result<T, FreshCreditError>;

/// `FreshCredit` error types
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum FreshCreditError {
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("External API error: {0}")]
    ExternalApiError(String),
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    #[error("Authorization error: {0}")]
    AuthorizationError(String),
    #[error("Not found: {0}")]
    NotFoundError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Encryption error: {0}")]
    EncryptionError(String),
}

/// RFC 7807 Problem Details for HTTP APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub context: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ProblemDetails {
    #[must_use]
    pub fn new(problem_type: &str, title: &str, status: u16) -> Self {
        Self {
            problem_type: problem_type.to_string(),
            title: title.to_string(),
            status,
            detail: None,
            instance: None,
            context: None,
            request_id: None,
        }
    }

    #[must_use]
    pub fn validation_error(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/validation-error",
            "Validation Error",
            400,
        )
        .with_detail(detail)
    }

    #[must_use]
    pub fn unauthorized(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/unauthorized",
            "Unauthorized",
            401,
        )
        .with_detail(detail)
    }

    #[must_use]
    pub fn forbidden(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/forbidden",
            "Forbidden",
            403,
        )
        .with_detail(detail)
    }

    #[must_use]
    pub fn not_found(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/not-found",
            "Not Found",
            404,
        )
        .with_detail(detail)
    }

    #[must_use]
    pub fn internal_error(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/internal-error",
            "Internal Server Error",
            500,
        )
        .with_detail(detail)
    }

    #[must_use]
    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    #[must_use]
    pub fn with_instance(mut self, instance: &str) -> Self {
        self.instance = Some(instance.to_string());
        self
    }

    #[must_use]
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }

    #[must_use]
    pub fn with_request_id(mut self, request_id: &str) -> Self {
        self.request_id = Some(request_id.to_string());
        self
    }
}

impl From<FreshCreditError> for ProblemDetails {
    fn from(error: FreshCreditError) -> Self {
        match error {
            FreshCreditError::ValidationError(msg) => Self::validation_error(&msg),
            FreshCreditError::DatabaseError(msg) => Self::internal_error(&msg),
            FreshCreditError::ExternalApiError(msg) => Self::internal_error(&msg),
            FreshCreditError::AuthenticationError(msg) => Self::unauthorized(&msg),
            FreshCreditError::AuthorizationError(msg) => Self::forbidden(&msg),
            FreshCreditError::NotFoundError(msg) => Self::not_found(&msg),
            FreshCreditError::InternalError(msg) => Self::internal_error(&msg),
            FreshCreditError::EncryptionError(msg) => Self::internal_error(&msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data".to_string());
        assert!(response.success);
        assert_eq!(response.data, Some("test data".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse::error("Something went wrong".to_string());
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_freshcredit_error_display() {
        let err = FreshCreditError::ValidationError("Invalid input".to_string());
        assert_eq!(err.to_string(), "Validation error: Invalid input");

        let err = FreshCreditError::NotFoundError("User not found".to_string());
        assert_eq!(err.to_string(), "Not found: User not found");
    }

    #[test]
    fn test_problem_details_new() {
        let problem =
            ProblemDetails::new("https://freshcredit.com/problems/test", "Test Error", 400);
        assert_eq!(
            problem.problem_type,
            "https://freshcredit.com/problems/test"
        );
        assert_eq!(problem.title, "Test Error");
        assert_eq!(problem.status, 400);
        assert!(problem.detail.is_none());
    }

    #[test]
    fn test_problem_details_validation_error() {
        let problem = ProblemDetails::validation_error("Email is required");
        assert_eq!(problem.status, 400);
        assert_eq!(problem.title, "Validation Error");
        assert_eq!(problem.detail, Some("Email is required".to_string()));
    }

    #[test]
    fn test_problem_details_unauthorized() {
        let problem = ProblemDetails::unauthorized("Invalid token");
        assert_eq!(problem.status, 401);
        assert_eq!(problem.title, "Unauthorized");
    }

    #[test]
    fn test_problem_details_forbidden() {
        let problem = ProblemDetails::forbidden("Access denied");
        assert_eq!(problem.status, 403);
        assert_eq!(problem.title, "Forbidden");
    }

    #[test]
    fn test_problem_details_not_found() {
        let problem = ProblemDetails::not_found("User not found");
        assert_eq!(problem.status, 404);
        assert_eq!(problem.title, "Not Found");
    }

    #[test]
    fn test_problem_details_internal_error() {
        let problem = ProblemDetails::internal_error("Database connection failed");
        assert_eq!(problem.status, 500);
        assert_eq!(problem.title, "Internal Server Error");
    }

    #[test]
    fn test_problem_details_with_context() {
        let problem = ProblemDetails::validation_error("Invalid field")
            .with_context(serde_json::json!({
                "field": "email",
                "reason": "invalid_format"
            }))
            .with_request_id("req_12345");

        assert!(problem.context.is_some());
        assert_eq!(problem.request_id, Some("req_12345".to_string()));
    }

    #[test]
    fn test_problem_details_serialization() {
        let problem = ProblemDetails::validation_error("Email is required")
            .with_instance("/api/v1/users/123");

        let json = serde_json::to_string(&problem).unwrap();
        assert!(json.contains("\"type\":\"https://freshcredit.com/problems/validation-error\""));
        assert!(json.contains("\"title\":\"Validation Error\""));
        assert!(json.contains("\"status\":400"));
        assert!(json.contains("\"detail\":\"Email is required\""));
        assert!(json.contains("\"instance\":\"/api/v1/users/123\""));
    }

    #[test]
    fn test_problem_details_from_freshcredit_error() {
        let error = FreshCreditError::ValidationError("Invalid input".to_string());
        let problem: ProblemDetails = error.into();
        assert_eq!(problem.status, 400);
        assert_eq!(problem.detail, Some("Invalid input".to_string()));

        let error = FreshCreditError::NotFoundError("User not found".to_string());
        let problem: ProblemDetails = error.into();
        assert_eq!(problem.status, 404);

        let error = FreshCreditError::AuthenticationError("Token expired".to_string());
        let problem: ProblemDetails = error.into();
        assert_eq!(problem.status, 401);
    }
}
