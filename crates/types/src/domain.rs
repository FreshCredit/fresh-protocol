//! Core domain types for `FreshCredit`

use chrono::{
    DateTime,
    Utc,
};
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

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

/// Account types supported by `FreshCredit`
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

/// Financial report data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct FinancialReport {
    pub id: Uuid,
    pub user_id: UserId,
    pub bureau_score: Option<u16>,
    pub accounts: Vec<Account>,
    pub transactions: Vec<Transaction>,
    pub generated_at: DateTime<Utc>,
    pub blockchain_hash: Option<String>,
}

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodType {
    Card,
    BankAccount,
    CryptoWallet,
}

/// Payment processors supported by `FreshCredit`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentProcessor {
    Stripe,
    Circle,
    Plaid,
}

/// Card brand types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Payment method information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub id: String,
    pub user_id: UserId,
    pub method_ref: String,
    pub processor: PaymentProcessor,
    pub method_type: PaymentMethodType,
    pub hash_anchor: String,
    pub display_name: Option<String>,
    pub last_four: Option<String>,
    pub brand: Option<CardBrand>,
    pub expiry_month: Option<u8>,
    pub expiry_year: Option<u16>,
    pub is_default: bool,
    pub is_active: bool,
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

/// Request to create a payment method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentMethodRequest {
    pub processor: PaymentProcessor,
    pub method_type: PaymentMethodType,
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[cfg(feature = "typescript")]
    #[test]
    fn test_typescript_export() {
        use ts_rs::TS;

        let account_ts = Account::name();
        assert_eq!(account_ts, "Account");

        let transaction_ts = Transaction::name();
        assert_eq!(transaction_ts, "Transaction");

        let report_ts = FinancialReport::name();
        assert_eq!(report_ts, "FinancialReport");
    }
}
