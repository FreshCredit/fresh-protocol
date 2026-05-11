//! User profile database operations
//!
//! DEPRECATED: Use `UserProfileService` trait from `user_profile_service.rs` instead.  // CLASS-001: Migration documentation
//! This module is maintained for backwards compatibility.
//!
//! Operations for storing and retrieving user profile data:
//! - `store_user_profile`: Store user profile in local database
//! - `get_user_profile`: Get user profile by `platform_user_id`
//! - `get_user_profile_by_azure_id`: Get user profile by Azure AD Object ID
//!
//! P0p: Added `is_admin` for first provider user admin rule (§27.4)
//! P0g: Added `provider_onboarding_complete` for nav visibility (§28.1)

use anyhow::Result;

use crate::LocalClient;
use crate::UserProfile;

// Re-export the UserProfileService trait
pub use super::user_profile_service::UserProfileService;

impl LocalClient {
    /// Store user profile in local database (matches production schema)
    ///
    /// DEPRECATED: Use `UserProfileService::store_profile()` instead  // CLASS-001: Migration documentation
    /// P0p: Added `is_admin` for first provider user admin rule (§27.4)
    /// P0g: Added `provider_onboarding_complete` for nav visibility (§28.1)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn store_user_profile(&self, profile: &UserProfile) -> Result<()> {
        // Delegate to internal implementation
        self.store_user_profile_internal(profile).await
    }

    /// Get user profile from local database (matches production schema)
    ///
    /// DEPRECATED: Use `UserProfileService::get_by_platform_id()` instead  // CLASS-001: Migration documentation
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_user_profile(&self, platform_user_id: &str) -> Result<Option<UserProfile>> {
        // Delegate to internal implementation
        self.get_user_profile_internal(platform_user_id).await
    }

    /// Get user profile by Azure AD Object ID (used when `user_id` is the Azure ID)
    ///
    /// DEPRECATED: Use `UserProfileService::get_by_azure_id()` instead  // CLASS-001: Migration documentation
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_user_profile_by_azure_id(
        &self,
        azure_id: &str,
    ) -> Result<Option<UserProfile>> {
        // Delegate to internal implementation
        self.get_user_profile_by_azure_id_internal(azure_id).await
    }
}
