//! Core domain types for FreshCredit

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User identifier
pub type UserId = String;

/// Financial account information
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountType {
    Checking,
    Savings,
    Credit,
    Investment,
    Loan,
}

/// Financial transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Type alias for backward compatibility during migration
#[deprecated(note = "Use FinancialReport instead - CreditReport is being phased out")]
pub type CreditReport = FinancialReport;

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
}
