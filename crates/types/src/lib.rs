//! Core domain types for FreshCredit
//!
//! P1 FIX: Cross-language type sync with ts-rs
//! Generate TypeScript types: cargo test --features typescript

use chrono::{
    DateTime,
    Utc,
};
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

// P1 FIX: ts-rs for TypeScript type generation
#[cfg(feature = "typescript")]
use ts_rs::TS;

/// User identifier
pub type UserId = String;

/// Financial account information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct Account {
    pub id: String,
    pub user_id: UserId,
    pub account_type: AccountType,
    pub balance: Option<f64>,
    pub currency: String,
    pub institution_name: String,
    pub created_at: DateTime<Utc>,
}

/// Account types supported by FreshCredit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub enum AccountType {
    Checking,
    Savings,
    Credit,
    Investment,
    Loan,
}

/// Financial transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub amount: f64,
    pub currency: String,
    pub description: String,
    pub category: Option<String>,
    pub date: DateTime<Utc>,
    pub merchant_name: Option<String>,
}

/// Financial report data (aggregated financial information from connected accounts)
/// Note: This struct represents user-owned financial data, NOT credit scoring.
/// Any scores displayed are from external credit bureaus, not calculated by FreshCredit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct FinancialReport {
    pub id: Uuid,
    pub user_id: UserId,
    /// Bureau score from external source (e.g., Plaid, Experian) - NOT calculated by FreshCredit
    pub bureau_score: Option<u16>,
    pub accounts: Vec<Account>,
    pub transactions: Vec<Transaction>,
    pub generated_at: DateTime<Utc>,
    pub blockchain_hash: Option<String>,
}

// CODE-001: Deprecated with migration path
// Migration: Use FinancialReport instead. CreditReport is being phased out as part of
// the naming alignment with FCRA compliance (FreshCredit does not calculate credit scores).
/// Payment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    pub id: String,
    pub user_id: UserId,
    pub amount_cents: i64,
    pub currency: String,
    pub status: PaymentStatus,
    pub created_at: DateTime<Utc>,
}

/// Payment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentStatus {
    Pending,
    Completed,
    Failed,
    Cancelled,
    Refunded,
    RequiresVerification,
}

/// Payment method types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodType {
    Card,
    BankAccount,
    CryptoWallet,
}

/// Payment processors supported by FreshCredit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentProcessor {
    Stripe,
    Circle,
    Plaid,
}

/// Card brand types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CardBrand {
    Visa,
    Mastercard,
    Amex,
    Discover,
    #[serde(rename = "diners_club")]
    DinersClub,
    Jcb,
    UnionPay,
    Unknown,
}

/// Payment method information (browser-first compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub id: String,
    pub user_id: UserId,
    /// Opaque reference (e.g., "pm_ref_xxx") - not the actual token
    pub method_ref: String,
    pub processor: PaymentProcessor,
    pub method_type: PaymentMethodType,
    /// Hash anchor for server verification (blake3 hash)
    pub hash_anchor: String,
    /// Display name (e.g., "Visa ending in 4242")
    pub display_name: Option<String>,
    /// Last 4 digits of card/account
    pub last_four: Option<String>,
    /// Card brand (for cards)
    pub brand: Option<CardBrand>,
    /// Expiry month (for cards)
    pub expiry_month: Option<u8>,
    /// Expiry year (for cards)
    pub expiry_year: Option<u16>,
    /// Whether this is the default payment method
    pub is_default: bool,
    /// Whether this method is active
    pub is_active: bool,
    /// Billing details
    pub billing_details: Option<BillingDetails>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Billing details for payment methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingDetails {
    pub name: Option<String>,
    pub email: Option<String>,
    pub address: Option<Address>,
}

/// Request to create a payment method (browser-first)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentMethodRequest {
    pub processor: PaymentProcessor,
    pub method_type: PaymentMethodType,
    /// Payment provider token (Stripe pm_xxx, etc.)
    pub token: String,
    pub set_default: bool,
    pub billing_details: Option<BillingDetails>,
}

/// Response after creating a payment method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodResponse {
    pub id: String,
    pub method_ref: String,
    pub processor: PaymentProcessor,
    pub method_type: PaymentMethodType,
    pub display_name: String,
    pub last_four: Option<String>,
    pub brand: Option<CardBrand>,
    pub expiry_month: Option<u8>,
    pub expiry_year: Option<u16>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

/// Financial institution information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Institution {
    pub id: String,
    pub name: String,
    pub country_codes: Vec<String>,
    pub products: Vec<String>,
    pub routing_numbers: Vec<String>,
}

/// User profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: UserId,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub address: Option<Address>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Address information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country: String,
}

/// API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

/// Result type for FreshCredit operations
pub type FreshCreditResult<T> = Result<T, FreshCreditError>;

/// FreshCredit error types
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
/// <https://datatracker.ietf.org/doc/html/rfc7807>
///
/// This standardized error format provides machine-readable error responses
/// with optional human-readable descriptions.
///
/// HARDCODED_URL: Problem type URIs use <https://freshcredit.com/problems/*> namespace
/// per RFC 7807. Update all helper methods if domain changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct ProblemDetails {
    /// A URI reference that identifies the problem type (RFC 7807 §3.1)
    /// Example: "<https://freshcredit.com/problems/validation-error>"
    #[serde(rename = "type")]
    pub problem_type: String,

    /// A short, human-readable summary of the problem type (RFC 7807 §3.1)
    pub title: String,

    /// The HTTP status code (RFC 7807 §3.1)
    pub status: u16,

    /// A human-readable explanation of this specific problem (RFC 7807 §3.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,

    /// A URI reference that identifies the specific occurrence (RFC 7807 §3.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,

    /// Additional context about the error (extension member)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(skip))]
    pub context: Option<serde_json::Value>,

    /// Request ID for tracing (extension member)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ProblemDetails {
    /// Create a new ProblemDetails with required fields
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

    /// Create a validation error (400 Bad Request)
    pub fn validation_error(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/validation-error",
            "Validation Error",
            400,
        )
        .with_detail(detail)
    }

    /// Create an authentication error (401 Unauthorized)
    pub fn unauthorized(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/unauthorized",
            "Unauthorized",
            401,
        )
        .with_detail(detail)
    }

    /// Create an authorization error (403 Forbidden)
    pub fn forbidden(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/forbidden",
            "Forbidden",
            403,
        )
        .with_detail(detail)
    }

    /// Create a not found error (404 Not Found)
    pub fn not_found(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/not-found",
            "Not Found",
            404,
        )
        .with_detail(detail)
    }

    /// Create an internal server error (500 Internal Server Error)
    pub fn internal_error(detail: &str) -> Self {
        Self::new(
            "https://freshcredit.com/problems/internal-error",
            "Internal Server Error",
            500,
        )
        .with_detail(detail)
    }

    /// Add detail to the problem
    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    /// Add instance URI to the problem
    pub fn with_instance(mut self, instance: &str) -> Self {
        self.instance = Some(instance.to_string());
        self
    }

    /// Add context to the problem
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Add request ID for tracing
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

/// Standardized API response types for consistent API governance
///
/// This module provides standardized response wrappers, pagination,
/// and metadata types to address API governance inconsistencies
/// identified in Round 24 research.
pub mod api_response;

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
    fn test_account_type_serialization() {
        let account_type = AccountType::Checking;
        let json = serde_json::to_string(&account_type).unwrap();
        assert_eq!(json, "\"Checking\"");

        let parsed: AccountType = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AccountType::Checking));
    }

    #[test]
    fn test_payment_status_serialization() {
        let status = PaymentStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Completed\"");

        let parsed: PaymentStatus = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, PaymentStatus::Completed));
    }

    #[test]
    fn test_freshcredit_error_display() {
        let err = FreshCreditError::ValidationError("Invalid input".to_string());
        assert_eq!(err.to_string(), "Validation error: Invalid input");

        let err = FreshCreditError::NotFoundError("User not found".to_string());
        assert_eq!(err.to_string(), "Not found: User not found");
    }

    #[test]
    fn test_address_serialization() {
        let address = Address {
            street: "123 Main St".to_string(),
            city: "San Francisco".to_string(),
            state: "CA".to_string(),
            postal_code: "94102".to_string(),
            country: "US".to_string(),
        };

        let json = serde_json::to_string(&address).unwrap();
        let parsed: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.city, "San Francisco");
        assert_eq!(parsed.state, "CA");
    }

    #[test]
    fn test_institution_serialization() {
        let institution = Institution {
            id: "ins_123".to_string(),
            name: "Test Bank".to_string(),
            country_codes: vec!["US".to_string()],
            products: vec!["transactions".to_string(), "auth".to_string()],
            routing_numbers: vec!["110000000".to_string()],
        };

        let json = serde_json::to_string(&institution).unwrap();
        let parsed: Institution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Test Bank");
        assert_eq!(parsed.products.len(), 2);
    }

    #[test]
    fn test_transaction_serialization() {
        let transaction = Transaction {
            id: "txn_123".to_string(),
            account_id: "acct_456".to_string(),
            amount: -50.00,
            currency: "USD".to_string(),
            description: "Coffee Shop".to_string(),
            category: Some("Food and Drink".to_string()),
            date: Utc::now(),
            merchant_name: Some("Starbucks".to_string()),
        };

        let json = serde_json::to_string(&transaction).unwrap();
        let parsed: Transaction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.amount, -50.00);
        assert_eq!(parsed.merchant_name, Some("Starbucks".to_string()));
    }

    #[test]
    fn test_account_serialization() {
        let account = Account {
            id: "acct_123".to_string(),
            user_id: "user_456".to_string(),
            account_type: AccountType::Savings,
            balance: Some(1000.50),
            currency: "USD".to_string(),
            institution_name: "Test Bank".to_string(),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&account).unwrap();
        let parsed: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.balance, Some(1000.50));
        assert!(matches!(parsed.account_type, AccountType::Savings));
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

    /// P1 FIX: Test TypeScript type generation
    /// Run with: cargo test --features typescript
    #[cfg(feature = "typescript")]
    #[test]
    fn test_typescript_export() {
        use ts_rs::TS;

        // Verify TypeScript types can be generated
        let account_ts = Account::name();
        assert_eq!(account_ts, "Account");

        let transaction_ts = Transaction::name();
        assert_eq!(transaction_ts, "Transaction");

        let report_ts = FinancialReport::name();
        assert_eq!(report_ts, "FinancialReport");
    }
}
