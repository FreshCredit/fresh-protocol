// TAG: surface=security owner=security-team rule=SEC-001
//! Axum consent middleware

use crate::storage::ConsentStorage;
use crate::types::{ConsentType, ConsentValidation, Jurisdiction};
use crate::validator::ConsentValidator;
use axum::http::request::Parts;
use axum::{
    extract::{FromRequestParts, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::{debug, warn};

/// Consent middleware state
#[derive(Clone, Debug)]
pub struct ConsentMiddleware<S: ConsentStorage> {
    validator: Arc<ConsentValidator<S>>,
    jurisdiction: Jurisdiction,
}

impl<S: ConsentStorage> ConsentMiddleware<S> {
    /// Create a new consent middleware
    pub fn new(storage: Arc<S>, jurisdiction: Jurisdiction) -> Self {
        let validator = Arc::new(ConsentValidator::new(storage));
        Self {
            validator,
            jurisdiction,
            // TAG: surface=security owner=platform-team rule=MID-001
        }
    }

    /// Validate consent for a user
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn validate_consent(
        &self,
        user_id: i64,
        consent_type: &ConsentType,
    ) -> Result<ConsentValidation, String> {
        self.validator
            .validate(user_id, consent_type, self.jurisdiction)
            .await
            .map_err(|e| e.to_string())
    }

    /// Validate multiple consent types
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn validate_multiple(
        &self,
        user_id: i64,
        consent_types: &[ConsentType],
    ) -> Result<Vec<ConsentValidation>, String> {
        self.validator
            .validate_multiple(user_id, consent_types, self.jurisdiction)
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            .await
            .map_err(|e| e.to_string())
    }
}

/// Consent requirement for a route
#[derive(Clone, Debug)]
pub struct ConsentRequirement {
    /// Required consent types for this route
    pub consent_types: Vec<ConsentType>,
}

impl ConsentRequirement {
    /// Create a new consent requirement
    #[must_use]
    pub const fn new(consent_types: Vec<ConsentType>) -> Self {
        Self { consent_types }
    }

    /// Create a requirement for a single consent type
    #[must_use]
    pub fn single(consent_type: ConsentType) -> Self {
        Self {
            consent_types: vec![consent_type],
        }
    }
}

/// Consent extractor for Axum handlers
// TAG: surface=security owner=platform-team rule=MID-001
#[derive(Debug)]
pub struct ConsentExtractor {
    /// User ID from the request
    pub user_id: i64,
    /// Consent validation results
    pub validations: Vec<ConsentValidation>,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for ConsentExtractor
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract consent validations from request extensions
        let validations = parts
            .extensions
            .get::<Vec<ConsentValidation>>()
            .cloned()
            .ok_or((
                StatusCode::INTERNAL_SERVER_ERROR,
                "No consent validations found",
            ))?;

        // Extract user ID (should be set by auth middleware)
        let user_id = parts
            .extensions
            .get::<i64>()
            // TAG: surface=security owner=security-team rule=SEC-001
            .copied()
            .ok_or((StatusCode::UNAUTHORIZED, "No user ID found"))?;

        Ok(Self {
            user_id,
            validations,
        })
    }
}

/// Middleware function for consent validation
pub async fn consent_middleware<S: ConsentStorage + 'static>(
    middleware: Arc<ConsentMiddleware<S>>,
    requirement: ConsentRequirement,
    mut request: Request,
    next: Next,
) -> Response {
    // Extract user ID from request extensions (set by auth middleware)
    let Some(user_id) = request.extensions().get::<i64>().copied() else {
        warn!("No user ID found in request extensions");
        return (StatusCode::UNAUTHORIZED, "User not authenticated").into_response();
    };

    // Validate all required consents
    let validations = match middleware
        .validate_multiple(user_id, &requirement.consent_types)
        .await
            // TAG: surface=security owner=platform-team rule=MID-001
    {
        Ok(v) => v,
        Err(e) => {
            warn!("Consent validation error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Consent validation failed",
            )
                .into_response();
        }
    };

    // Check if all consents are valid
    let all_valid = validations.iter().all(|v| v.is_valid);

    if !all_valid {
        // Find first invalid consent
        if let Some(invalid) = validations.iter().find(|v| !v.is_valid) {
            warn!(
                "Consent validation failed for user {}: {}",
                user_id,
                invalid.reason.as_deref().unwrap_or("Unknown reason")
            );
            return (
                StatusCode::FORBIDDEN,
                format!(
                    "Consent required: {}",
                    invalid.reason.as_deref().unwrap_or("Unknown reason")
                ),
                // TAG: surface=security owner=platform-team rule=GENERAL-001
            )
                .into_response();
        }
    }

    debug!("All consents valid for user {}", user_id);

    // Insert validations into request extensions for handlers to access
    request.extensions_mut().insert(validations);

    next.run(request).await
}

#[cfg(test)]
#[allow(unused_variables, unused_imports)]
mod tests {
    use super::*;
    use crate::storage::ConsentStorage;
    use crate::types::{Consent, ConsentError, ConsentType, Jurisdiction};
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockStorage;

    #[async_trait]
    impl ConsentStorage for MockStorage {
        async fn get_consent(
            &self,
            _user_id: i64,
            // TAG: surface=security owner=platform-team rule=MID-001
            _consent_type: &ConsentType,
        ) -> Result<Option<Consent>, ConsentError> {
            Ok(None)
        }

        async fn create_consent(
            &self,
            user_id: i64,
            consent_type: ConsentType,
            consent_purpose: String,
            consent_method: String,
            jurisdiction: Jurisdiction,
            expiry_timestamp: Option<chrono::DateTime<chrono::Utc>>,
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
        }

        async fn revoke_consent(
            // TAG: surface=security owner=security-team rule=SEC-001
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

    #[test]
    fn test_consent_middleware_new() {
        let storage = Arc::new(MockStorage);
        let middleware = ConsentMiddleware::new(storage, Jurisdiction::US);
        assert_eq!(middleware.jurisdiction, Jurisdiction::US);
    }

    #[test]
    fn test_consent_requirement_new() {
        let types = vec![ConsentType::FinancialDataAccess, ConsentType::Analytics];
        let req = ConsentRequirement::new(types);
        assert_eq!(req.consent_types.len(), 2);
    }
    // TAG: surface=security owner=platform-team rule=MID-001

    #[test]
    fn test_consent_requirement_single() {
        let req = ConsentRequirement::single(ConsentType::CreditReportGeneration);
        assert_eq!(req.consent_types.len(), 1);
        assert_eq!(req.consent_types[0], ConsentType::CreditReportGeneration);
    }

    #[tokio::test]
    async fn test_validate_consent() {
        let storage = Arc::new(MockStorage);
        let middleware = ConsentMiddleware::new(storage, Jurisdiction::US);
        let result = middleware
            .validate_consent(1, &ConsentType::FinancialDataAccess)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_multiple() {
        let storage = Arc::new(MockStorage);
        let middleware = ConsentMiddleware::new(storage, Jurisdiction::US);
        let result = middleware
            .validate_multiple(1, &[ConsentType::FinancialDataAccess])
            .await;
        assert!(result.is_ok());
        let validations = result.unwrap();
        assert_eq!(validations.len(), 1);
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
