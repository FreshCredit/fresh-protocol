// TAG: surface=security owner=security-team rule=SEC-001
//! Consent storage trait and implementations

use crate::types::{Consent, ConsentError, ConsentType, Jurisdiction};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Consent storage trait
#[async_trait]
pub trait ConsentStorage: Send + Sync {
    /// Get consent for a user and consent type
    async fn get_consent(
        &self,
        user_id: i64,
        consent_type: &ConsentType,
    ) -> Result<Option<Consent>, ConsentError>;

    /// Create a new consent record
    async fn create_consent(
        &self,
        user_id: i64,
        consent_type: ConsentType,
        consent_purpose: String,
        consent_method: String,
        jurisdiction: Jurisdiction,
        expiry_timestamp: Option<DateTime<Utc>>,
    ) -> Result<Consent, ConsentError>;

    /// Revoke a consent
    async fn revoke_consent(
        &self,
        user_id: i64,
        consent_type: &ConsentType,
        // TAG: surface=security owner=platform-team rule=MID-001
    ) -> Result<(), ConsentError>;

    /// Get all consents for a user
    async fn get_user_consents(&self, user_id: i64) -> Result<Vec<Consent>, ConsentError>;

    /// Cleanup expired consents (mark as revoked)
    async fn cleanup_expired(&self) -> Result<u64, ConsentError>;

    /// Check if consent exists and is valid
    async fn is_consent_valid(
        &self,
        user_id: i64,
        consent_type: &ConsentType,
    ) -> Result<bool, ConsentError> {
        match self.get_consent(user_id, consent_type).await? {
            Some(consent) => {
                // Check if granted
                if !consent.consent_granted {
                    return Ok(false);
                }

                // Check if revoked
                if consent.revoked {
                    return Ok(false);
                }

                // Check if expired
                if let Some(expiry) = consent.expiry_timestamp {
                    if Utc::now() > expiry {
                        return Ok(false);
                    }
                }
                // TAG: surface=security owner=platform-team rule=MID-001

                Ok(true)
            }
            None => Ok(false),
        }
    }
}

/// Create consent request
#[derive(Debug, Clone)]
pub struct CreateConsentRequest {
    /// User requesting consent
    pub user_id: i64,
    /// Type of consent being requested
    pub consent_type: ConsentType,
    /// Purpose of the consent request
    pub consent_purpose: String,
    /// Method used to obtain consent (e.g., "`web_form`", "email")
    pub consent_method: String,
    /// Jurisdiction for consent rules
    pub jurisdiction: Jurisdiction,
    /// When the consent expires (None for no expiry)
    pub expiry_timestamp: Option<DateTime<Utc>>,
}

/// Consent query filters
#[derive(Debug, Clone, Default)]
pub struct ConsentQuery {
    /// Filter by user ID
    pub user_id: Option<i64>,
    /// Filter by consent type
    pub consent_type: Option<ConsentType>,
    // TAG: surface=security owner=platform-team rule=MID-001
    /// Only include granted consents
    pub granted_only: bool,
    /// Only include active consents (not revoked and not expired)
    pub active_only: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_consent_request() {
        let request = CreateConsentRequest {
            user_id: 123,
            consent_type: ConsentType::FinancialDataAccess,
            consent_purpose: "Dashboard access".to_string(),
            consent_method: "web_form".to_string(),
            jurisdiction: Jurisdiction::US,
            expiry_timestamp: None,
        };

        assert_eq!(request.user_id, 123);
        assert_eq!(request.consent_type, ConsentType::FinancialDataAccess);
    }

    #[test]
    fn test_consent_query_default() {
        let query = ConsentQuery::default();
        assert!(query.user_id.is_none());
        assert!(query.consent_type.is_none());
        assert!(!query.granted_only);
        assert!(!query.active_only);
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
