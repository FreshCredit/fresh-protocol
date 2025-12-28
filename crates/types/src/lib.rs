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
}
