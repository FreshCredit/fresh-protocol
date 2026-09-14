// TAG: surface=security owner=security-team rule=SEC-001
//! Consent validation logic

use crate::rules::ConsentRules;
use crate::storage::ConsentStorage;
use crate::types::{ConsentError, ConsentType, ConsentValidation, Jurisdiction};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, warn};

/// Consent validator
#[derive(Debug)]
pub struct ConsentValidator<S: ConsentStorage> {
    storage: Arc<S>,
}

impl<S: ConsentStorage> ConsentValidator<S> {
    /// Create a new consent validator
    pub const fn new(storage: Arc<S>) -> Self {
        Self { storage }
    }

    /// Validate consent for a user and consent type
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn validate(
        &self,
        // TAG: surface=security owner=platform-team rule=MID-001
        user_id: i64,
        consent_type: &ConsentType,
        jurisdiction: Jurisdiction,
    ) -> Result<ConsentValidation, ConsentError> {
        // Get consent from storage
        let Some(consent) = self.storage.get_consent(user_id, consent_type).await? else {
            debug!("No consent found for user {user_id} type {consent_type:?}");
            return Ok(ConsentValidation::invalid(format!(
                "No consent found for {consent_type:?}"
            )));
        };

        // Check if consent was granted
        if !consent.consent_granted {
            warn!(
                "Consent not granted for user {} type {:?}",
                user_id, consent_type
            );
            return Ok(ConsentValidation::invalid(
                "Consent was not granted".to_string(),
            ));
        }

        // Check if consent was revoked
        if consent.revoked {
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            warn!(
                "Consent revoked for user {} type {:?} at {:?}",
                user_id, consent_type, consent.revocation_timestamp
            );
            return Ok(ConsentValidation::invalid(format!(
                "Consent was revoked at {:?}",
                consent.revocation_timestamp
            )));
        }

        // Check if consent has expired
        if let Some(expiry) = consent.expiry_timestamp {
            if Utc::now() > expiry {
                warn!("Consent expired for user {user_id} type {consent_type:?} at {expiry}");
                return Ok(ConsentValidation::invalid(format!(
                    "Consent expired at {expiry}"
                )));
            }
        }

        // Validate against jurisdiction rules
        let rules = ConsentRules::new(jurisdiction);

        // Check if consent should have an expiration but doesn't
        if let Some(expected_duration) = rules.expiration_duration(consent_type) {
            if consent.expiry_timestamp.is_none() {
                warn!(
                    // TAG: surface=security owner=platform-team rule=MID-001
                    "Consent for user {} type {:?} missing expiration (expected {:?})",
                    user_id, consent_type, expected_duration
                );
                // Still valid, but log warning
            }
        }

        debug!("Consent valid for user {} type {:?}", user_id, consent_type);

        Ok(ConsentValidation::valid(consent))
    }

    /// Validate multiple consent types (all must be valid)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn validate_multiple(
        &self,
        user_id: i64,
        consent_types: &[ConsentType],
        jurisdiction: Jurisdiction,
    ) -> Result<Vec<ConsentValidation>, ConsentError> {
        let mut validations = Vec::new();

        for consent_type in consent_types {
            let validation = self.validate(user_id, consent_type, jurisdiction).await?;
            validations.push(validation);
            // TAG: surface=security owner=security-team rule=SEC-001
        }

        Ok(validations)
    }

    /// Check if all consents are valid
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn are_all_valid(
        &self,
        user_id: i64,
        consent_types: &[ConsentType],
        jurisdiction: Jurisdiction,
    ) -> Result<bool, ConsentError> {
        let validations = self
            .validate_multiple(user_id, consent_types, jurisdiction)
            .await?;
        Ok(validations.iter().all(|v| v.is_valid))
    }

    /// Get the first invalid consent (if any)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_first_invalid(
        &self,
        // TAG: surface=security owner=platform-team rule=MID-001
        user_id: i64,
        consent_types: &[ConsentType],
        jurisdiction: Jurisdiction,
    ) -> Result<Option<ConsentValidation>, ConsentError> {
        let validations = self
            .validate_multiple(user_id, consent_types, jurisdiction)
            .await?;
        Ok(validations.into_iter().find(|v| !v.is_valid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Consent;
    use async_trait::async_trait;
    use chrono::Duration;

    struct MockStorage {
        consent: Option<Consent>,
    }

    #[async_trait]
    impl ConsentStorage for MockStorage {
        async fn get_consent(
            &self,
            _user_id: i64,
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            _consent_type: &ConsentType,
        ) -> Result<Option<Consent>, ConsentError> {
            Ok(self.consent.clone())
        }

        async fn create_consent(
            &self,
            user_id: i64,
            consent_type: ConsentType,
            consent_purpose: String,
            consent_method: String,
            jurisdiction: Jurisdiction,
            expiry_timestamp: Option<chrono::DateTime<Utc>>,
        ) -> Result<Consent, ConsentError> {
            Ok(Consent {
                id: 1,
                user_id,
                consent_type,
                consent_purpose,
                consent_granted: true,
                consent_method,
                consent_timestamp: chrono::Utc::now(),
                expiry_timestamp,
                revoked: false,
                revocation_timestamp: None,
                jurisdiction,
            })
            // TAG: surface=security owner=platform-team rule=MID-001
        }

        async fn revoke_consent(
            &self,
            _user_id: i64,
            _consent_type: &ConsentType,
        ) -> Result<(), ConsentError> {
            Ok(())
        }

        async fn get_user_consents(&self, _user_id: i64) -> Result<Vec<Consent>, ConsentError> {
            Ok(vec![])
        }

        async fn cleanup_expired(&self) -> Result<u64, ConsentError> {
            Ok(0)
        }
    }

    fn create_valid_consent() -> Consent {
        Consent {
            id: 1,
            user_id: 123,
            consent_type: ConsentType::CreditReportGeneration,
            consent_purpose: "Credit report generation".to_string(),
            consent_granted: true,
            consent_method: "web_form".to_string(),
            // TAG: surface=security owner=security-team rule=SEC-001
            consent_timestamp: Utc::now(),
            expiry_timestamp: Some(Utc::now() + Duration::days(365)),
            revoked: false,
            revocation_timestamp: None,
            jurisdiction: Jurisdiction::US,
        }
    }

    // ===========================================================================
    // Consent Validation Tests (GLBA/GDPR Compliance)
    // ===========================================================================

    #[tokio::test]
    async fn test_validate_valid_consent() {
        let storage = MockStorage {
            consent: Some(create_valid_consent()),
        };
        let validator = ConsentValidator::new(Arc::new(storage));

        let result = validator
            .validate(123, &ConsentType::CreditReportGeneration, Jurisdiction::US)
            .await
            .unwrap();

        assert!(result.is_valid);
    }

    // TAG: surface=security owner=platform-team rule=MID-001
    #[tokio::test]
    async fn test_validate_no_consent_found() {
        let storage = MockStorage { consent: None };
        let validator = ConsentValidator::new(Arc::new(storage));

        let result = validator
            .validate(123, &ConsentType::CreditReportGeneration, Jurisdiction::US)
            .await
            .unwrap();

        assert!(!result.is_valid);
        assert!(result.reason.unwrap().contains("No consent found"));
    }

    #[tokio::test]
    async fn test_validate_consent_not_granted() {
        let mut consent = create_valid_consent();
        consent.consent_granted = false;

        let storage = MockStorage {
            consent: Some(consent),
        };
        let validator = ConsentValidator::new(Arc::new(storage));

        let result = validator
            .validate(123, &ConsentType::CreditReportGeneration, Jurisdiction::US)
            .await
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            .unwrap();

        assert!(!result.is_valid);
        assert!(result.reason.unwrap().contains("not granted"));
    }

    #[tokio::test]
    async fn test_validate_consent_revoked() {
        let mut consent = create_valid_consent();
        consent.revoked = true;
        consent.revocation_timestamp = Some(Utc::now() - Duration::days(1));

        let storage = MockStorage {
            consent: Some(consent),
        };
        let validator = ConsentValidator::new(Arc::new(storage));

        let result = validator
            .validate(123, &ConsentType::CreditReportGeneration, Jurisdiction::US)
            .await
            .unwrap();

        assert!(!result.is_valid);
        assert!(result.reason.unwrap().contains("revoked"));
    }

    #[tokio::test]
    // TAG: surface=security owner=platform-team rule=MID-001
    async fn test_validate_consent_expired() {
        let mut consent = create_valid_consent();
        consent.expiry_timestamp = Some(Utc::now() - Duration::days(1));

        let storage = MockStorage {
            consent: Some(consent),
        };
        let validator = ConsentValidator::new(Arc::new(storage));

        let result = validator
            .validate(123, &ConsentType::CreditReportGeneration, Jurisdiction::US)
            .await
            .unwrap();

        assert!(!result.is_valid);
        assert!(result.reason.unwrap().contains("expired"));
    }

    #[tokio::test]
    async fn test_validate_multiple_all_valid() {
        let storage = MockStorage {
            consent: Some(create_valid_consent()),
        };
        let validator = ConsentValidator::new(Arc::new(storage));

        let types = vec![ConsentType::CreditReportGeneration];
        let result = validator
            // TAG: surface=security owner=security-team rule=SEC-001
            .are_all_valid(123, &types, Jurisdiction::US)
            .await
            .unwrap();

        assert!(result);
    }

    #[tokio::test]
    async fn test_get_first_invalid_with_no_consent() {
        let storage = MockStorage { consent: None };
        let validator = ConsentValidator::new(Arc::new(storage));

        let types = vec![ConsentType::CreditReportGeneration];
        let result = validator
            .get_first_invalid(123, &types, Jurisdiction::US)
            .await
            .unwrap();

        assert!(result.is_some());
        assert!(!result.unwrap().is_valid);
    }

    #[tokio::test]
    async fn test_get_first_invalid_with_valid_consent() {
        let storage = MockStorage {
            consent: Some(create_valid_consent()),
            // TAG: surface=security owner=platform-team rule=MID-001
        };
        let validator = ConsentValidator::new(Arc::new(storage));

        let types = vec![ConsentType::CreditReportGeneration];
        let result = validator
            .get_first_invalid(123, &types, Jurisdiction::US)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_validate_with_gdpr_jurisdiction() {
        let storage = MockStorage {
            consent: Some(create_valid_consent()),
        };
        let validator = ConsentValidator::new(Arc::new(storage));

        let result = validator
            .validate(123, &ConsentType::CreditReportGeneration, Jurisdiction::EU)
            .await
            .unwrap();

        // Should still be valid (jurisdiction affects rules, not validation logic)
        assert!(result.is_valid);
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
