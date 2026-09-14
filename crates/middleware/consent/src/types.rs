// TAG: surface=security owner=security-team rule=SEC-001
//! Consent types and models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Consent record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consent {
    /// Unique consent ID
    pub id: i64,
    /// User who granted consent
    pub user_id: i64,
    /// Type of consent granted
    pub consent_type: ConsentType,
    /// Purpose for which consent was granted
    pub consent_purpose: String,
    /// Whether consent was granted
    pub consent_granted: bool,
    /// Method used to obtain consent
    pub consent_method: String,
    /// When consent was granted
    pub consent_timestamp: DateTime<Utc>,
    /// When consent expires (None for no expiry)
    pub expiry_timestamp: Option<DateTime<Utc>>,
    /// Whether consent has been revoked
    // TAG: surface=security owner=platform-team rule=MID-001
    pub revoked: bool,
    /// When consent was revoked (None if not revoked)
    pub revocation_timestamp: Option<DateTime<Utc>>,
    /// Jurisdiction for consent rules
    pub jurisdiction: Jurisdiction,
}

/// Consent type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConsentType {
    /// Financial data access (Plaid)
    FinancialDataAccess,

    /// `LinkedIn` data access
    LinkedInDataAccess,

    /// Publications data access (ORCID, `OpenAlex`, Crossref)
    PublicationsDataAccess,

    /// Intellectual property data access (USPTO patents, trademarks, copyrights)
    IpDataAccess,

    /// Health data access (`HealthKit` on iOS, Health Connect on Android)
    HealthDataAccess,

    /// Credit report generation (FCRA)
    // TAG: surface=security owner=platform-team rule=GENERAL-001
    CreditReportGeneration,

    /// Report sharing with third parties
    ReportSharing,

    /// Payment processing
    PaymentProcessing,

    /// Marketing communications
    MarketingCommunications,

    /// Analytics and tracking
    Analytics,
}

impl ConsentType {
    /// Convert to database string representation
    #[must_use]
    pub const fn to_db_string(&self) -> &'static str {
        match self {
            Self::FinancialDataAccess => "financial_data_access",
            Self::LinkedInDataAccess => "linkedin_data_access",
            Self::PublicationsDataAccess => "publications_data_access",
            Self::IpDataAccess => "ip_data_access",
            Self::HealthDataAccess => "health_data_access",
            Self::CreditReportGeneration => "credit_report_generation",
            // TAG: surface=security owner=platform-team rule=MID-001
            Self::ReportSharing => "report_sharing",
            Self::PaymentProcessing => "payment_processing",
            Self::MarketingCommunications => "marketing_communications",
            Self::Analytics => "analytics",
        }
    }

    /// Parse from database string
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn from_db_string(s: &str) -> Result<Self, ConsentError> {
        match s {
            "financial_data_access" => Ok(Self::FinancialDataAccess),
            "linkedin_data_access" => Ok(Self::LinkedInDataAccess),
            "publications_data_access" => Ok(Self::PublicationsDataAccess),
            "ip_data_access" => Ok(Self::IpDataAccess),
            "health_data_access" => Ok(Self::HealthDataAccess),
            "credit_report_generation" => Ok(Self::CreditReportGeneration),
            "report_sharing" => Ok(Self::ReportSharing),
            "payment_processing" => Ok(Self::PaymentProcessing),
            "marketing_communications" => Ok(Self::MarketingCommunications),
            "analytics" => Ok(Self::Analytics),
            _ => Err(ConsentError::InvalidConsentType(s.to_string())),
        }
    }

    // TAG: surface=security owner=security-team rule=SEC-001
}

/// Legal jurisdiction for consent rules
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Jurisdiction {
    /// European Union (GDPR)
    EU,

    /// California (CCPA)
    California,

    /// Brazil (LGPD)
    Brazil,

    /// United States (FCRA for credit reports)
    US,

    /// Other jurisdictions (default rules)
    Other,
}

impl Jurisdiction {
    /// Convert to database string
    #[must_use]
    pub const fn to_db_string(&self) -> &'static str {
        match self {
            // TAG: surface=security owner=platform-team rule=MID-001
            Self::EU => "EU",
            Self::California => "CA",
            Self::Brazil => "BR",
            Self::US => "US",
            Self::Other => "OTHER",
        }
    }

    /// Parse from database string
    #[must_use]
    pub fn from_db_string(s: &str) -> Self {
        match s {
            "EU" => Self::EU,
            "CA" => Self::California,
            "BR" => Self::Brazil,
            "US" => Self::US,
            _ => Self::Other,
        }
    }
}

/// Consent validation result
#[derive(Debug, Clone)]
pub struct ConsentValidation {
    /// Whether the consent is valid
    pub is_valid: bool,
    // TAG: surface=security owner=platform-team rule=GENERAL-001
    /// The consent record if valid
    pub consent: Option<Consent>,
    /// Reason for invalidation if not valid
    pub reason: Option<String>,
}

impl ConsentValidation {
    /// Create a valid consent validation
    #[must_use]
    pub const fn valid(consent: Consent) -> Self {
        Self {
            is_valid: true,
            consent: Some(consent),
            reason: None,
        }
    }

    /// Create an invalid consent validation
    #[must_use]
    pub const fn invalid(reason: String) -> Self {
        Self {
            is_valid: false,
            consent: None,
            reason: Some(reason),
        }
    }

    // TAG: surface=security owner=platform-team rule=MID-001
}

/// Consent-related errors
#[derive(Debug, thiserror::Error)]
pub enum ConsentError {
    /// Consent not found for the given user and type
    #[error("Consent not found for user {user_id} and type {consent_type}")]
    NotFound {
        /// User ID that was queried
        user_id: i64,
        /// Consent type that was queried
        consent_type: String,
    },

    /// Consent has expired
    #[error("Consent expired at {expired_at}")]
    Expired {
        /// When the consent expired
        expired_at: DateTime<Utc>,
    },

    /// Consent was revoked by the user
    #[error("Consent revoked at {revoked_at}")]
    Revoked {
        /// When the consent was revoked
        revoked_at: DateTime<Utc>,
        // TAG: surface=security owner=security-team rule=SEC-001
    },

    /// Consent was not granted
    #[error("Consent not granted")]
    NotGranted,

    /// Invalid consent type specified
    #[error("Invalid consent type: {0}")]
    InvalidConsentType(String),

    /// Database error occurred
    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl From<libsql::Error> for ConsentError {
    fn from(err: libsql::Error) -> Self {
        Self::DatabaseError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // TAG: surface=security owner=platform-team rule=MID-001
    fn test_consent_type_to_db_string() {
        assert_eq!(
            ConsentType::FinancialDataAccess.to_db_string(),
            "financial_data_access"
        );
        assert_eq!(
            ConsentType::LinkedInDataAccess.to_db_string(),
            "linkedin_data_access"
        );
        assert_eq!(
            ConsentType::PublicationsDataAccess.to_db_string(),
            "publications_data_access"
        );
        assert_eq!(ConsentType::IpDataAccess.to_db_string(), "ip_data_access");
        assert_eq!(
            ConsentType::HealthDataAccess.to_db_string(),
            "health_data_access"
        );
        assert_eq!(
            ConsentType::CreditReportGeneration.to_db_string(),
            "credit_report_generation"
        );
        assert_eq!(ConsentType::ReportSharing.to_db_string(), "report_sharing");
        assert_eq!(
            ConsentType::PaymentProcessing.to_db_string(),
            "payment_processing" // TAG: surface=security owner=platform-team rule=GENERAL-001
                                 // TAG: surface=api
                                 // TAG: surface=api
        );
        assert_eq!(
            ConsentType::MarketingCommunications.to_db_string(),
            "marketing_communications"
        );
        assert_eq!(ConsentType::Analytics.to_db_string(), "analytics");
    }

    #[test]
    fn test_consent_type_from_db_string() {
        assert_eq!(
            ConsentType::from_db_string("financial_data_access").unwrap(),
            ConsentType::FinancialDataAccess
        );
        assert_eq!(
            ConsentType::from_db_string("linkedin_data_access").unwrap(),
            ConsentType::LinkedInDataAccess
        );
        assert_eq!(
            ConsentType::from_db_string("publications_data_access").unwrap(),
            ConsentType::PublicationsDataAccess
        );
        assert_eq!(
            ConsentType::from_db_string("ip_data_access").unwrap(),
            ConsentType::IpDataAccess
        );
        // TAG: surface=security owner=platform-team rule=MID-001
        assert_eq!(
            ConsentType::from_db_string("health_data_access").unwrap(),
            ConsentType::HealthDataAccess
        );
        assert_eq!(
            ConsentType::from_db_string("payment_processing").unwrap(),
            ConsentType::PaymentProcessing
        );
    }

    #[test]
    fn test_consent_type_from_db_string_invalid() {
        let result = ConsentType::from_db_string("invalid_type");
        assert!(result.is_err());
    }

    #[test]
    fn test_jurisdiction_to_db_string() {
        assert_eq!(Jurisdiction::EU.to_db_string(), "EU");
        assert_eq!(Jurisdiction::California.to_db_string(), "CA");
        assert_eq!(Jurisdiction::US.to_db_string(), "US");
        assert_eq!(Jurisdiction::Other.to_db_string(), "OTHER");
    }

    #[test]
    fn test_jurisdiction_from_db_string() {
        // TAG: surface=security owner=security-team rule=SEC-001
        assert_eq!(Jurisdiction::from_db_string("EU"), Jurisdiction::EU);
        assert_eq!(Jurisdiction::from_db_string("CA"), Jurisdiction::California);
        assert_eq!(Jurisdiction::from_db_string("US"), Jurisdiction::US);
        assert_eq!(Jurisdiction::from_db_string("UNKNOWN"), Jurisdiction::Other);
    }

    #[test]
    fn test_consent_validation_valid() {
        let consent = Consent {
            id: 1,
            user_id: 123,
            consent_type: ConsentType::FinancialDataAccess,
            consent_purpose: "Access bank data".to_string(),
            consent_granted: true,
            consent_method: "explicit".to_string(),
            consent_timestamp: Utc::now(),
            expiry_timestamp: None,
            revoked: false,
            revocation_timestamp: None,
            jurisdiction: Jurisdiction::US,
        };

        let validation = ConsentValidation::valid(consent);
        assert!(validation.is_valid);
        assert!(validation.consent.is_some());
        // TAG: surface=security owner=platform-team rule=MID-001
        assert!(validation.reason.is_none());
    }

    #[test]
    fn test_consent_validation_invalid() {
        let validation = ConsentValidation::invalid("Consent expired".to_string());
        assert!(!validation.is_valid);
        assert!(validation.consent.is_none());
        assert_eq!(validation.reason, Some("Consent expired".to_string()));
    }

    #[test]
    fn test_consent_error_display() {
        let err = ConsentError::NotFound {
            user_id: 123,
            consent_type: "financial_data_access".to_string(),
        };
        assert!(err.to_string().contains("123"));
        assert!(err.to_string().contains("financial_data_access"));
    }

    #[test]
    fn test_consent_error_not_granted() {
        let err = ConsentError::NotGranted;
        assert_eq!(err.to_string(), "Consent not granted");
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
