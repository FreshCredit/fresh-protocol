// TAG: surface=security owner=security-team rule=SEC-001
//! Jurisdiction-specific consent rules
//!
//! This module implements consent expiration and validation rules
//! based on legal requirements for different jurisdictions.

use crate::types::{ConsentType, Jurisdiction};
use chrono::Duration;

/// Consent rules for a specific jurisdiction
#[derive(Debug)]
pub struct ConsentRules {
    jurisdiction: Jurisdiction,
}

impl ConsentRules {
    /// Create consent rules for a jurisdiction
    #[must_use]
    pub const fn new(jurisdiction: Jurisdiction) -> Self {
        Self { jurisdiction }
    }

    /// Get the expiration duration for a consent type
    ///
    /// Returns None if consent does not expire automatically
    #[must_use]
    pub const fn expiration_duration(&self, consent_type: &ConsentType) -> Option<Duration> {
        // TAG: surface=security owner=platform-team rule=MID-001
        match self.jurisdiction {
            Jurisdiction::EU => Self::gdpr_expiration(consent_type),
            Jurisdiction::California => Self::ccpa_expiration(consent_type),
            Jurisdiction::Brazil => Self::lgpd_expiration(consent_type),
            Jurisdiction::US => Self::us_expiration(consent_type),
            Jurisdiction::Other => Self::default_expiration(consent_type),
        }
    }

    /// GDPR (EU) consent expiration rules
    const fn gdpr_expiration(consent_type: &ConsentType) -> Option<Duration> {
        match consent_type {
            // Payment processing: No automatic expiration (per transaction)
            ConsentType::PaymentProcessing => None,
            // GDPR: Consent should be refreshed every 12 months
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            _ => Some(Duration::days(365)),
        }
    }

    /// CCPA (California) consent expiration rules
    const fn ccpa_expiration(consent_type: &ConsentType) -> Option<Duration> {
        match consent_type {
            // Marketing: 12 months (industry best practice)
            ConsentType::MarketingCommunications => Some(Duration::days(365)),
            // CCPA: No automatic expiration, but must be revocable
            _ => None,
        }
    }

    /// FCRA (US) consent expiration rules
    const fn us_expiration(consent_type: &ConsentType) -> Option<Duration> {
        match consent_type {
            // TAG: surface=security owner=platform-team rule=MID-001
            // Financial data, publications, IP: 90 days (Plaid best practice)
            ConsentType::FinancialDataAccess
            | ConsentType::PublicationsDataAccess
            | ConsentType::IpDataAccess => Some(Duration::days(90)),

            // LinkedIn and health data: 30 days
            ConsentType::LinkedInDataAccess | ConsentType::HealthDataAccess => {
                Some(Duration::days(30))
            }

            // FCRA: Credit report consent valid for 2 years
            ConsentType::CreditReportGeneration => Some(Duration::days(730)),

            // Report sharing, marketing, analytics: 1 year
            ConsentType::ReportSharing
            | ConsentType::MarketingCommunications
            | ConsentType::Analytics => Some(Duration::days(365)),

            // Payment: No expiration (per transaction)
            ConsentType::PaymentProcessing => None,
        }
        // TAG: surface=security owner=security-team rule=SEC-001
    }

    /// LGPD (Brazil) consent expiration rules
    const fn lgpd_expiration(consent_type: &ConsentType) -> Option<Duration> {
        match consent_type {
            // Payment processing: No automatic expiration (per transaction)
            ConsentType::PaymentProcessing => None,
            // LGPD Article 40: Data retention should be minimum necessary
            // Using GDPR-aligned 12 months as baseline
            _ => Some(Duration::days(365)),
        }
    }

    /// Default consent expiration rules
    const fn default_expiration(consent_type: &ConsentType) -> Option<Duration> {
        // TAG: surface=security owner=platform-team rule=MID-001
        // Use most conservative rules (US FCRA)
        Self::us_expiration(consent_type)
    }

    /// Check if consent type requires explicit opt-in
    #[must_use]
    pub const fn requires_explicit_opt_in(&self, consent_type: &ConsentType) -> bool {
        match self.jurisdiction {
            // GDPR, CCPA, LGPD and Other: all consent requires explicit opt-in
            Jurisdiction::EU
            | Jurisdiction::California
            | Jurisdiction::Brazil
            | Jurisdiction::Other => true,
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            // FCRA/US: all except analytics require explicit consent
            Jurisdiction::US => !matches!(consent_type, ConsentType::Analytics),
        }
    }

    /// Check if consent can be revoked
    #[must_use]
    pub const fn is_revocable(&self, consent_type: &ConsentType) -> bool {
        match consent_type {
            // TAG: surface=security owner=platform-team rule=MID-001
            // Payment processing consent cannot be revoked after transaction
            ConsentType::PaymentProcessing => false,

            // All other consent types are revocable
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================================================
    // GDPR Expiration Tests
    // ===========================================================================

    #[test]
    fn test_gdpr_expiration() {
        let rules = ConsentRules::new(Jurisdiction::EU);

        // GDPR: 12 months for most consent types
        assert_eq!(
            rules.expiration_duration(&ConsentType::FinancialDataAccess),
            Some(Duration::days(365))
        );

        // TAG: surface=security owner=security-team rule=SEC-001
        // Payment processing: No expiration
        assert_eq!(
            rules.expiration_duration(&ConsentType::PaymentProcessing),
            None
        );
    }

    #[test]
    fn test_gdpr_all_consent_types_expire() {
        let rules = ConsentRules::new(Jurisdiction::EU);

        // GDPR: All data access consent expires at 12 months
        assert_eq!(
            rules.expiration_duration(&ConsentType::LinkedInDataAccess),
            Some(Duration::days(365))
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::CreditReportGeneration),
            Some(Duration::days(365))
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::ReportSharing),
            Some(Duration::days(365))
        );
    }

    // TAG: surface=security owner=platform-team rule=MID-001
    // ===========================================================================
    // FCRA/US Expiration Tests (§604 Permissible Purpose)
    // ===========================================================================

    #[test]
    fn test_fcra_expiration() {
        let rules = ConsentRules::new(Jurisdiction::US);

        // FCRA: 2 years for credit reports
        assert_eq!(
            rules.expiration_duration(&ConsentType::CreditReportGeneration),
            Some(Duration::days(730))
        );

        // Plaid: 90 days
        assert_eq!(
            rules.expiration_duration(&ConsentType::FinancialDataAccess),
            Some(Duration::days(90))
        );
    }

    #[test]
    fn test_fcra_report_sharing_expiration() {
        let rules = ConsentRules::new(Jurisdiction::US);

        // Report sharing: 1 year
        assert_eq!(
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            rules.expiration_duration(&ConsentType::ReportSharing),
            Some(Duration::days(365))
        );
    }

    #[test]
    fn test_fcra_linkedin_short_expiration() {
        let rules = ConsentRules::new(Jurisdiction::US);

        // LinkedIn: 30 days (stricter due to platform ToS)
        assert_eq!(
            rules.expiration_duration(&ConsentType::LinkedInDataAccess),
            Some(Duration::days(30))
        );
    }

    // ===========================================================================
    // CCPA/California Tests
    // ===========================================================================

    #[test]
    fn test_ccpa_no_auto_expiration_for_data_access() {
        let rules = ConsentRules::new(Jurisdiction::California);

        // CCPA: No automatic expiration (must be revocable instead)
        assert_eq!(
            // TAG: surface=security owner=platform-team rule=MID-001
            rules.expiration_duration(&ConsentType::FinancialDataAccess),
            None
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::CreditReportGeneration),
            None
        );
    }

    #[test]
    fn test_ccpa_marketing_expires() {
        let rules = ConsentRules::new(Jurisdiction::California);

        // Marketing: Industry best practice 12 months
        assert_eq!(
            rules.expiration_duration(&ConsentType::MarketingCommunications),
            Some(Duration::days(365))
        );
    }

    // ===========================================================================
    // Explicit Opt-In Tests (FCRA §604 Permissible Purpose)
    // ===========================================================================

    #[test]
    fn test_us_credit_report_requires_explicit_opt_in() {
        let rules = ConsentRules::new(Jurisdiction::US);
        // TAG: surface=security owner=security-team rule=SEC-001

        // FCRA: Credit reports require explicit consent
        assert!(rules.requires_explicit_opt_in(&ConsentType::CreditReportGeneration));
    }

    #[test]
    fn test_us_report_sharing_requires_explicit_opt_in() {
        let rules = ConsentRules::new(Jurisdiction::US);

        // FCRA §604: Sharing requires explicit consent
        assert!(rules.requires_explicit_opt_in(&ConsentType::ReportSharing));
    }

    #[test]
    fn test_us_financial_data_requires_explicit_opt_in() {
        let rules = ConsentRules::new(Jurisdiction::US);

        // Financial data access requires explicit consent
        assert!(rules.requires_explicit_opt_in(&ConsentType::FinancialDataAccess));
    }

    #[test]
    fn test_us_analytics_does_not_require_explicit_opt_in() {
        let rules = ConsentRules::new(Jurisdiction::US);

        // Analytics can be implied (not sensitive)
        // TAG: surface=security owner=platform-team rule=MID-001
        assert!(!rules.requires_explicit_opt_in(&ConsentType::Analytics));
    }

    #[test]
    fn test_gdpr_all_require_explicit_opt_in() {
        let rules = ConsentRules::new(Jurisdiction::EU);

        // GDPR: All consent requires explicit opt-in
        assert!(rules.requires_explicit_opt_in(&ConsentType::CreditReportGeneration));
        assert!(rules.requires_explicit_opt_in(&ConsentType::Analytics));
        assert!(rules.requires_explicit_opt_in(&ConsentType::MarketingCommunications));
    }

    // ===========================================================================
    // Revocability Tests (GDPR Art 7, CCPA §1798.120)
    // ===========================================================================

    #[test]
    fn test_payment_processing_not_revocable() {
        let rules = ConsentRules::new(Jurisdiction::US);

        // Payment consent cannot be revoked after transaction
        assert!(!rules.is_revocable(&ConsentType::PaymentProcessing));
    }

    #[test]
    fn test_all_other_consent_revocable() {
        // TAG: surface=security owner=platform-team rule=GENERAL-001
        let rules = ConsentRules::new(Jurisdiction::US);

        // All non-payment consent is revocable
        assert!(rules.is_revocable(&ConsentType::CreditReportGeneration));
        assert!(rules.is_revocable(&ConsentType::ReportSharing));
        assert!(rules.is_revocable(&ConsentType::FinancialDataAccess));
        assert!(rules.is_revocable(&ConsentType::MarketingCommunications));
    }

    // ===========================================================================
    // Default/Other Jurisdiction Tests
    // ===========================================================================

    #[test]
    fn test_other_jurisdiction_uses_conservative_defaults() {
        let rules = ConsentRules::new(Jurisdiction::Other);
        let us_rules = ConsentRules::new(Jurisdiction::US);

        // Default should use US (most conservative)
        assert_eq!(
            rules.expiration_duration(&ConsentType::CreditReportGeneration),
            us_rules.expiration_duration(&ConsentType::CreditReportGeneration)
        );
    }

    // ===========================================================================
    // TAG: surface=security owner=platform-team rule=MID-001
    // LGPD/Brazil Tests
    // ===========================================================================

    #[test]
    fn test_lgpd_expiration() {
        let rules = ConsentRules::new(Jurisdiction::Brazil);

        // LGPD: 12 months for most consent types
        assert_eq!(
            rules.expiration_duration(&ConsentType::FinancialDataAccess),
            Some(Duration::days(365))
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::HealthDataAccess),
            Some(Duration::days(365))
        );

        // Payment processing: No expiration
        assert_eq!(
            rules.expiration_duration(&ConsentType::PaymentProcessing),
            None
        );
    }

    #[test]
    fn test_lgpd_marketing_expires() {
        let rules = ConsentRules::new(Jurisdiction::Brazil);
        // TAG: surface=security owner=security-team rule=SEC-001
        assert_eq!(
            rules.expiration_duration(&ConsentType::MarketingCommunications),
            Some(Duration::days(365))
        );
    }

    // ===========================================================================
    // CCPA Additional Consent Type Tests
    // ===========================================================================

    #[test]
    fn test_ccpa_all_consent_types() {
        let rules = ConsentRules::new(Jurisdiction::California);

        assert_eq!(rules.expiration_duration(&ConsentType::IpDataAccess), None);
        assert_eq!(
            rules.expiration_duration(&ConsentType::HealthDataAccess),
            None
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::PublicationsDataAccess),
            None
        );
        assert_eq!(rules.expiration_duration(&ConsentType::ReportSharing), None);
        assert_eq!(
            rules.expiration_duration(&ConsentType::PaymentProcessing),
            // TAG: surface=security owner=platform-team rule=MID-001
            None
        );
        assert_eq!(rules.expiration_duration(&ConsentType::Analytics), None);
    }

    // ===========================================================================
    // US Additional Consent Type Tests
    // ===========================================================================

    #[test]
    fn test_us_all_consent_types() {
        let rules = ConsentRules::new(Jurisdiction::US);

        assert_eq!(
            rules.expiration_duration(&ConsentType::IpDataAccess),
            Some(Duration::days(90))
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::HealthDataAccess),
            Some(Duration::days(30))
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::PublicationsDataAccess),
            Some(Duration::days(90))
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::MarketingCommunications),
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            Some(Duration::days(365))
        );
        assert_eq!(
            rules.expiration_duration(&ConsentType::Analytics),
            Some(Duration::days(365))
        );
    }

    // ===========================================================================
    // Explicit Opt-In Tests for All Jurisdictions
    // ===========================================================================

    #[test]
    fn test_california_requires_explicit_opt_in() {
        let rules = ConsentRules::new(Jurisdiction::California);
        assert!(rules.requires_explicit_opt_in(&ConsentType::CreditReportGeneration));
        assert!(rules.requires_explicit_opt_in(&ConsentType::Analytics));
    }

    #[test]
    fn test_brazil_requires_explicit_opt_in() {
        let rules = ConsentRules::new(Jurisdiction::Brazil);
        assert!(rules.requires_explicit_opt_in(&ConsentType::HealthDataAccess));
        assert!(rules.requires_explicit_opt_in(&ConsentType::FinancialDataAccess));
        assert!(rules.requires_explicit_opt_in(&ConsentType::MarketingCommunications));
    }
    // TAG: surface=security owner=platform-team rule=MID-001

    #[test]
    fn test_other_requires_explicit_opt_in() {
        let rules = ConsentRules::new(Jurisdiction::Other);
        assert!(rules.requires_explicit_opt_in(&ConsentType::Analytics));
        assert!(rules.requires_explicit_opt_in(&ConsentType::PaymentProcessing));
    }

    // ===========================================================================
    // Revocability Tests for All Types
    // ===========================================================================

    #[test]
    fn test_all_consent_types_revocable_except_payment() {
        let rules = ConsentRules::new(Jurisdiction::US);

        assert!(!rules.is_revocable(&ConsentType::PaymentProcessing));
        assert!(rules.is_revocable(&ConsentType::FinancialDataAccess));
        assert!(rules.is_revocable(&ConsentType::LinkedInDataAccess));
        assert!(rules.is_revocable(&ConsentType::PublicationsDataAccess));
        assert!(rules.is_revocable(&ConsentType::IpDataAccess));
        assert!(rules.is_revocable(&ConsentType::HealthDataAccess));
        assert!(rules.is_revocable(&ConsentType::CreditReportGeneration));
        assert!(rules.is_revocable(&ConsentType::ReportSharing));
        assert!(rules.is_revocable(&ConsentType::MarketingCommunications));
        assert!(rules.is_revocable(&ConsentType::Analytics));
    }
    // TAG: surface=security owner=security-team rule=SEC-001
}
