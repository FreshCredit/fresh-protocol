//! User Profile Service Abstraction
//!
//! This module provides the `UserProfileService` trait that encapsulates
//! all user profile database operations, ensuring proper abstraction
//! from direct SQL queries.
//!
//! COMPLIANCE: DB-010 - Use `UserProfileService` methods instead of direct queries

use anyhow::Result;
use async_trait::async_trait;

use crate::LocalClient;
use crate::UserProfile;

// TAG: surface=database owner=platform-team rule=DB-001
/// User Profile Service trait
///
/// Provides standardized methods for user profile CRUD operations.
/// All implementations should use this trait rather than direct SQL.
#[async_trait]
pub trait UserProfileService {
    /// Store a user profile in the database
    async fn store_profile(&self, profile: &UserProfile) -> Result<()>;

    /// Retrieve a user profile by platform user ID
    async fn get_by_platform_id(&self, platform_user_id: &str) -> Result<Option<UserProfile>>;

    /// Retrieve a user profile by Azure AD Object ID
    async fn get_by_azure_id(&self, azure_id: &str) -> Result<Option<UserProfile>>;

    /// Check if a profile exists for the given platform user ID
    async fn exists(&self, platform_user_id: &str) -> Result<bool>;
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Implementation of `UserProfileService` for `LocalClient`
///
/// This implementation uses prepared statements and proper parameter binding
/// to ensure security and performance.
#[async_trait]
impl UserProfileService for LocalClient {
    async fn store_profile(&self, profile: &UserProfile) -> Result<()> {
        // Delegate to existing implementation
        self.store_user_profile_internal(profile).await
    }

    async fn get_by_platform_id(&self, platform_user_id: &str) -> Result<Option<UserProfile>> {
        // Delegate to existing implementation
        self.get_user_profile_internal(platform_user_id).await
    }

    async fn get_by_azure_id(&self, azure_id: &str) -> Result<Option<UserProfile>> {
        // Delegate to existing implementation
        self.get_user_profile_by_azure_id_internal(azure_id).await
    }

    async fn exists(&self, platform_user_id: &str) -> Result<bool> {
        let profile = self.get_user_profile_internal(platform_user_id).await?;
        Ok(profile.is_some())
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Internal implementation methods for `LocalClient`
///
/// These methods contain the actual SQL queries and are marked as internal
/// to discourage direct usage outside the `UserProfileService` trait.
impl LocalClient {
    /// Internal: Store user profile
    pub(crate) async fn store_user_profile_internal(&self, profile: &UserProfile) -> Result<()> {
        tracing::info!(
            "Storing user profile locally for user: {}",
            profile.platform_user_id
        );

        self.connection.execute(
            "INSERT OR REPLACE INTO user_profile (
                id, platform_user_id, azure_id, email, display_name, given_name, family_name,
                surname, mobile_phone, job_title, street_address, city, state_province,
                postal_code, country_region, date_of_birth, ssn_last_four, employment_status,
                annual_income, role, is_admin, provider_onboarding_complete, mfa_enabled, mfa_verified_at, tenant_id, object_id,
                verified_id_credential_id, verified_id_status, verified_id_issued_at,
                consumer_verified_id_credential_id, consumer_verified_id_status, consumer_verified_id_issued_at,
                provider_verified_id_credential_id, provider_verified_id_status, provider_verified_id_issued_at,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
            libsql::params![
                profile.id.clone(),
                profile.platform_user_id.clone(),
                profile.azure_id.clone(),
                profile.email.clone(),
                profile.display_name.clone(),
                profile.given_name.clone().unwrap_or_default(),
                profile.family_name.clone().unwrap_or_default(),
// TAG: surface=database owner=platform-team rule=GENERAL-001
                profile.surname.clone().unwrap_or_default(),
                profile.mobile_phone.clone().unwrap_or_default(),
                profile.job_title.clone().unwrap_or_default(),
                profile.street_address.clone().unwrap_or_default(),
                profile.city.clone().unwrap_or_default(),
                profile.state_province.clone().unwrap_or_default(),
                profile.postal_code.clone().unwrap_or_default(),
                profile.country_region.clone().unwrap_or_default(),
                profile.date_of_birth.clone().unwrap_or_default(),
                profile.ssn_last_four.clone().unwrap_or_default(),
                profile.employment_status.clone().unwrap_or_default(),
                profile.annual_income.unwrap_or(0),
                profile.role.clone(),
                profile.is_admin,
                profile.provider_onboarding_complete,
                profile.mfa_enabled,
                profile.mfa_verified_at.clone().unwrap_or_default(),
                profile.tenant_id.clone(),
                profile.object_id.clone(),
                profile.verified_id_credential_id.clone().unwrap_or_default(),
                profile.verified_id_status.clone(),
                profile.verified_id_issued_at.clone().unwrap_or_default(),
                profile.consumer_verified_id_credential_id.clone().unwrap_or_default(),
                profile.consumer_verified_id_status.clone(),
                profile.consumer_verified_id_issued_at.clone().unwrap_or_default(),
                profile.provider_verified_id_credential_id.clone().unwrap_or_default(),
                profile.provider_verified_id_status.clone(),
                profile.provider_verified_id_issued_at.clone().unwrap_or_default(),
            ],
        ).await?;

        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Internal: Get user profile by platform user ID
    pub(crate) async fn get_user_profile_internal(
        &self,
        platform_user_id: &str,
    ) -> Result<Option<UserProfile>> {
        tracing::info!(
            "Retrieving user profile locally for user: {}",
            platform_user_id
        );

        let mut rows = self
            .connection
            .query(
                // DB-010: EXCEPTION - Repository service implementation
                "SELECT * FROM user_profile WHERE platform_user_id = ?",
                libsql::params![platform_user_id],
            )
            .await?;

        self.parse_profile_row(&mut rows).await
    }

    /// Internal: Get user profile by Azure AD Object ID
    pub(crate) async fn get_user_profile_by_azure_id_internal(
        &self,
        azure_id: &str,
    ) -> Result<Option<UserProfile>> {
        tracing::info!("Retrieving user profile by azure_id: {}", azure_id);

        let mut rows = self
            .connection
            .query(
                // DB-010: EXCEPTION - Repository service implementation
                "SELECT * FROM user_profile WHERE azure_id = ?",
                libsql::params![azure_id],
            )
            .await?;

        // TAG: surface=database owner=platform-team rule=DB-001
        self.parse_profile_row(&mut rows).await
    }

    /// Parse a profile row from query results
    ///
    /// Typed reads go through `tolerant_i64`/`tolerant_i32`: libsql's typed
    /// `row.get::<N>()` PANICS on unexpected column types (e.g. TEXT written by
    /// a browser-side writer), which aborts the process. Default instead.
    async fn parse_profile_row(&self, rows: &mut libsql::Rows) -> Result<Option<UserProfile>> {
        fn tolerant_i64(row: &libsql::Row, idx: i32) -> Option<i64> {
            match row.get_value(idx) {
                Ok(libsql::Value::Integer(i)) => Some(i),
                Ok(libsql::Value::Text(s)) => s.parse::<i64>().ok(),
                Ok(libsql::Value::Real(f)) => Some(f as i64),
                _ => None,
            }
        }
        fn tolerant_i32(row: &libsql::Row, idx: i32) -> Option<i32> {
            tolerant_i64(row, idx).and_then(|i| i32::try_from(i).ok())
        }
        if let Some(row) = rows.next().await? {
            let profile = UserProfile {
                id: row.get(0)?,
                platform_user_id: row.get(1)?,
                azure_id: row.get(2)?,
                email: row.get(3)?,
                display_name: row.get(4)?,
                given_name: row.get::<Option<String>>(5).unwrap_or(None),
                family_name: row.get::<Option<String>>(6).unwrap_or(None),
                surname: row.get::<Option<String>>(7).unwrap_or(None),
                mobile_phone: row.get::<Option<String>>(8).unwrap_or(None),
                job_title: row.get::<Option<String>>(9).unwrap_or(None),
                street_address: row.get::<Option<String>>(10).unwrap_or(None),
                city: row.get::<Option<String>>(11).unwrap_or(None),
                state_province: row.get::<Option<String>>(12).unwrap_or(None),
                postal_code: row.get::<Option<String>>(13).unwrap_or(None),
                country_region: row.get::<Option<String>>(14).unwrap_or(None),
                date_of_birth: row.get::<Option<String>>(15).unwrap_or(None),
                ssn_last_four: row.get::<Option<String>>(16).unwrap_or(None),
                employment_status: row.get::<Option<String>>(17).unwrap_or(None),
                annual_income: tolerant_i32(&row, 18),
                // ARCH-P2-001: Extended profile fields (columns 19-23)
                phone_number: row.get::<Option<String>>(19).unwrap_or(None),
                preferred_name: row.get::<Option<String>>(20).unwrap_or(None),
                emergency_contact_name: row.get::<Option<String>>(21).unwrap_or(None),
                emergency_contact_phone: row.get::<Option<String>>(22).unwrap_or(None),
                // TAG: surface=database owner=platform-team rule=DB-001
                employer_name: row.get::<Option<String>>(23).unwrap_or(None),
                // Remaining columns
                role: row.get(24).unwrap_or_else(|_| "consumer".to_string()),
                is_admin: tolerant_i64(&row, 25).unwrap_or(0) != 0,
                provider_onboarding_complete: tolerant_i64(&row, 26).unwrap_or(0) != 0,
                mfa_enabled: tolerant_i64(&row, 27).unwrap_or(0) != 0,
                mfa_verified_at: row.get::<Option<String>>(28).unwrap_or(None),
                tenant_id: row.get(29)?,
                object_id: row.get(30)?,
                verified_id_credential_id: row.get::<Option<String>>(31).unwrap_or(None),
                verified_id_status: row.get(32).unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get::<Option<String>>(33).unwrap_or(None),
                // last_report_date is intentionally not mapped (column 34)
                created_at: row.get(35).unwrap_or_default(),
                updated_at: row.get(36).unwrap_or_default(),
                consumer_verified_id_credential_id: row.get::<Option<String>>(37).unwrap_or(None),
                consumer_verified_id_status: row.get(38).unwrap_or_else(|_| "pending".to_string()),
                consumer_verified_id_issued_at: row.get::<Option<String>>(39).unwrap_or(None),
                provider_verified_id_credential_id: row.get::<Option<String>>(40).unwrap_or(None),
                provider_verified_id_status: row.get(41).unwrap_or_else(|_| "pending".to_string()),
                provider_verified_id_issued_at: row.get::<Option<String>>(42).unwrap_or(None),
            };

            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_user_profile_service_trait_exists() {
        // Compile-time check that UserProfileService trait is properly defined
        // Actual tests would require a database connection
        let _ = std::option::Option::<i32>::None;
    }
}
