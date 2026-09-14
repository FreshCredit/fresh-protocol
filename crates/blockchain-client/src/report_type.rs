use crate::freshcredit_runtime;
use serde::{Deserialize, Serialize};

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Report type matching pallet-freshcredit `ReportType` enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReportType {
    /// Plaid financial report (default)
    #[default]
    PlaidFinancial,
    /// Identity verification report
    IdentityVerification,
    /// Transaction history report
    TransactionHistory,
    /// Balance snapshot report
    BalanceSnapshot,
    /// Consumer preferences report
    ConsumerPreferences,
    /// Provider requirements report
    ProviderRequirements,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
impl ReportType {
    /// Parse report type from string
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "identity_verification" | "identityverification" => Self::IdentityVerification,
            "transaction_history" | "transactionhistory" => Self::TransactionHistory,
            "balance_snapshot" | "balancesnapshot" => Self::BalanceSnapshot,
            "consumer_preferences" | "consumerpreferences" => Self::ConsumerPreferences,
            "provider_requirements" | "providerrequirements" => Self::ProviderRequirements,
            _ => Self::PlaidFinancial,
        }
    }

    /// Convert to the runtime enum type
    pub(crate) const fn as_runtime_type(
        self,
    ) -> freshcredit_runtime::runtime_types::pallet_freshcredit::ReportType {
        use freshcredit_runtime::runtime_types::pallet_freshcredit::ReportType as RuntimeReportType;
        match self {
            Self::PlaidFinancial => RuntimeReportType::PlaidFinancial,
            Self::IdentityVerification => RuntimeReportType::IdentityVerification,
            Self::TransactionHistory => RuntimeReportType::TransactionHistory,
            Self::BalanceSnapshot => RuntimeReportType::BalanceSnapshot,
            Self::ConsumerPreferences => RuntimeReportType::ConsumerPreferences,
            Self::ProviderRequirements => RuntimeReportType::ProviderRequirements,
        }
    }
}

impl std::fmt::Display for ReportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlaidFinancial => write!(f, "plaid_financial"),
            Self::IdentityVerification => write!(f, "identity_verification"),
            Self::TransactionHistory => write!(f, "transaction_history"),
            Self::BalanceSnapshot => write!(f, "balance_snapshot"),
            Self::ConsumerPreferences => write!(f, "consumer_preferences"),
            Self::ProviderRequirements => write!(f, "provider_requirements"),
        }
    }
}
