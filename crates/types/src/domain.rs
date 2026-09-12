// TAG: surface=api owner=platform-team rule=API-001
//! Core domain types for `FreshCredit`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
    /// Unique identifier
    pub id: String,
    /// User identifier
    pub user_id: UserId,
    /// Account Type
    pub account_type: AccountType,
    /// Balance
    pub balance: Option<f64>,
    /// Current balance (from Plaid `balances.current`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_balance: Option<f64>,
    /// Available balance (from Plaid `balances.available`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_balance: Option<f64>,
    /// Credit/limit (from Plaid `balances.limit`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_limit: Option<f64>,
    /// ISO currency code (from Plaid `balances.iso_currency_code`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iso_currency_code: Option<String>,
    /// Account mask (last digits)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    /// Official account name from the institution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub official_name: Option<String>,
    /// Currency
    // TAG: surface=api owner=platform-team rule=API-001
    pub currency: String,
    /// Institution Name
    pub institution_name: String,
    /// Timestamp when the created was created/updated
    pub created_at: DateTime<Utc>,
}

/// Account types supported by `FreshCredit`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub enum AccountType {
    /// Checking
    Checking,
    /// Savings
    Savings,
    /// Credit
    Credit,
    /// Investment
    Investment,
    /// Loan
    Loan,
}

/// Financial transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
// TAG: surface=api owner=platform-team rule=API-001
#[cfg_attr(feature = "typescript", ts(export))]
pub struct Transaction {
    /// Unique identifier
    pub id: String,
    /// Account identifier
    pub account_id: String,
    /// Amount
    pub amount: f64,
    /// Currency
    pub currency: String,
    /// Description
    pub description: String,
    /// Category
    pub category: Option<String>,
    /// Date
    pub date: DateTime<Utc>,
    /// Merchant Name
    pub merchant_name: Option<String>,
}

/// Financial report data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct FinancialReport {
    /// Unique identifier
    // TAG: surface=api owner=platform-team rule=API-001
    pub id: Uuid,
    /// User identifier
    pub user_id: UserId,
    /// Bureau Score
    pub bureau_score: Option<u16>,
    /// Accounts
    pub accounts: Vec<Account>,
    /// Transactions
    pub transactions: Vec<Transaction>,
    /// Timestamp when the generated was created/updated
    pub generated_at: DateTime<Utc>,
    /// Blockchain Hash
    pub blockchain_hash: Option<String>,
}

/// Payment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    /// Unique identifier
    pub id: String,
    /// User identifier
    pub user_id: UserId,
    /// Amount Cents
    pub amount_cents: i64,
    /// Currency
    pub currency: String,
    /// Status
    // TAG: surface=api owner=platform-team rule=API-001
    pub status: PaymentStatus,
    /// Timestamp when the created was created/updated
    pub created_at: DateTime<Utc>,
}

/// Payment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentStatus {
    /// Pending
    Pending,
    /// Completed
    Completed,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
    /// Refunded
    Refunded,
    /// Requiresverification
    RequiresVerification,
}

/// Payment method types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodType {
    // TAG: surface=api owner=platform-team rule=API-001
    /// Card
    Card,
    /// Bankaccount
    BankAccount,
    /// Cryptowallet
    CryptoWallet,
}

/// Payment processors supported by `FreshCredit`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentProcessor {
    /// Stripe
    Stripe,
    /// Circle
    Circle,
    /// Plaid
    Plaid,
}

/// Card brand types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardBrand {
    /// Visa
    Visa,
    /// Mastercard
    // TAG: surface=api owner=platform-team rule=API-001
    Mastercard,
    /// Amex
    Amex,
    /// Discover
    Discover,
    #[serde(rename = "diners_club")]
    /// Dinersclub
    DinersClub,
    /// Jcb
    Jcb,
    /// Unionpay
    UnionPay,
    /// Unknown
    Unknown,
}

/// Payment method information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    /// Unique identifier
    pub id: String,
    /// User identifier
    pub user_id: UserId,
    /// Method Ref
    pub method_ref: String,
    /// Processor
    // TAG: surface=api owner=platform-team rule=API-001
    pub processor: PaymentProcessor,
    /// Method Type
    pub method_type: PaymentMethodType,
    /// Hash Anchor
    pub hash_anchor: String,
    /// Display Name
    pub display_name: Option<String>,
    /// Last Four
    pub last_four: Option<String>,
    /// Brand
    pub brand: Option<CardBrand>,
    /// Expiry Month
    pub expiry_month: Option<u8>,
    /// Expiry Year
    pub expiry_year: Option<u16>,
    /// Is Default
    pub is_default: bool,
    /// Is Active
    pub is_active: bool,
    /// Billing Details
    pub billing_details: Option<BillingDetails>,
    /// Timestamp when the created was created/updated
    pub created_at: DateTime<Utc>,
    /// Timestamp when the updated was created/updated
    pub updated_at: DateTime<Utc>,
}

// TAG: surface=api owner=platform-team rule=API-001
/// Billing details for payment methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingDetails {
    /// Name
    pub name: Option<String>,
    /// Email
    pub email: Option<String>,
    /// Address
    pub address: Option<Address>,
}

/// Request to create a payment method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentMethodRequest {
    /// Processor
    pub processor: PaymentProcessor,
    /// Method Type
    pub method_type: PaymentMethodType,
    /// Token
    pub token: String,
    /// Set Default
    pub set_default: bool,
    /// Billing Details
    pub billing_details: Option<BillingDetails>,
}

// TAG: surface=api owner=platform-team rule=API-001
/// Response after creating a payment method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodResponse {
    /// Unique identifier
    pub id: String,
    /// Method Ref
    pub method_ref: String,
    /// Processor
    pub processor: PaymentProcessor,
    /// Method Type
    pub method_type: PaymentMethodType,
    /// Display Name
    pub display_name: String,
    /// Last Four
    pub last_four: Option<String>,
    /// Brand
    pub brand: Option<CardBrand>,
    /// Expiry Month
    pub expiry_month: Option<u8>,
    /// Expiry Year
    pub expiry_year: Option<u16>,
    /// Is Default
    pub is_default: bool,
    /// Timestamp when the created was created/updated
    pub created_at: DateTime<Utc>,
}

// TAG: surface=api owner=platform-team rule=API-001
/// Financial institution information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Institution {
    /// Unique identifier
    pub id: String,
    /// Name
    pub name: String,
    /// Country Codes
    pub country_codes: Vec<String>,
    /// Products
    pub products: Vec<String>,
    /// Routing Numbers
    pub routing_numbers: Vec<String>,
}

/// User profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// User identifier
    pub user_id: UserId,
    /// Email
    pub email: String,
    /// First Name
    pub first_name: String,
    /// Last Name
    pub last_name: String,
    // TAG: surface=api owner=platform-team rule=API-001
    /// Phone
    pub phone: Option<String>,
    /// Address
    pub address: Option<Address>,
    /// Timestamp when the created was created/updated
    pub created_at: DateTime<Utc>,
    /// Timestamp when the updated was created/updated
    pub updated_at: DateTime<Utc>,
}

/// Address information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    /// Street
    pub street: String,
    /// City
    pub city: String,
    /// State
    pub state: String,
    /// Postal Code
    pub postal_code: String,
    /// Country
    pub country: String,
}

#[cfg(test)]
mod tests {
    // TAG: surface=api owner=platform-team rule=API-001
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
            // TAG: surface=api owner=platform-team rule=API-001
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
    // TAG: surface=api owner=platform-team rule=API-001

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
        assert!((parsed.amount - (-50.00)).abs() < f64::EPSILON);
        assert_eq!(parsed.merchant_name, Some("Starbucks".to_string()));
    }

    #[test]
    fn test_account_serialization() {
        let account = Account {
            id: "acct_123".to_string(),
            user_id: "user_456".to_string(),
            account_type: AccountType::Savings,
            // TAG: surface=api owner=platform-team rule=API-001
            balance: Some(1000.50),
            current_balance: None,
            available_balance: None,
            credit_limit: None,
            iso_currency_code: None,
            mask: None,
            official_name: None,
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

        let cfg = ts_rs::Config::default();
        let account_ts = Account::name(&cfg);
        assert_eq!(account_ts, "Account");

        let transaction_ts = Transaction::name(&cfg);
        assert_eq!(transaction_ts, "Transaction");

        let report_ts = FinancialReport::name(&cfg);
        assert_eq!(report_ts, "FinancialReport");
    }
    // TAG: surface=api owner=platform-team rule=API-001
}
