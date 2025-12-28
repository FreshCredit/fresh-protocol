//! User profile database operations
//!
//! Operations for storing and retrieving user profile data:
//! - store_user_profile: Store user profile in local database
//! - get_user_profile: Get user profile by platform_user_id
//! - get_user_profile_by_azure_id: Get user profile by Azure AD Object ID
//!
//! P0p: Added is_admin for first provider user admin rule (§27.4)
//! P0g: Added provider_onboarding_complete for nav visibility (§28.1)

use anyhow::Result;
use tracing::info;

use crate::LocalClient;
use crate::UserProfile;

impl LocalClient {
    /// Store user profile in local database (matches production schema)
    /// P0p: Added is_admin for first provider user admin rule (§27.4)
    /// P0g: Added provider_onboarding_complete for nav visibility (§28.1)
    pub async fn store_user_profile(&self, profile: &UserProfile) -> Result<()> {
        info!(
            "Storing user profile locally for user: {}",
            profile.platform_user_id
        );

        self.connection.execute(
            "INSERT OR REPLACE INTO user_profile (
                id, platform_user_id, azure_id, email, display_name, given_name, family_name,
                surname, mobile_phone, job_title, street_address, city, state_province,
                postal_code, country_region, date_of_birth, ssn_last_four, employment_status,
                annual_income, role, is_admin, provider_onboarding_complete, tenant_id, object_id,
                verified_id_credential_id, verified_id_status, verified_id_issued_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
            libsql::params![
                profile.id.clone(),
                profile.platform_user_id.clone(),
                profile.azure_id.clone(),
                profile.email.clone(),
                profile.display_name.clone(),
                profile.given_name.clone().unwrap_or_default(),
                profile.family_name.clone().unwrap_or_default(),
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
                profile.tenant_id.clone(),
                profile.object_id.clone(),
                profile.verified_id_credential_id.clone().unwrap_or_default(),
                profile.verified_id_status.clone(),
                profile.verified_id_issued_at.clone().unwrap_or_default(),
            ],
        ).await?;

        Ok(())
    }

    /// Get user profile from local database (matches production schema)
    pub async fn get_user_profile(&self, platform_user_id: &str) -> Result<Option<UserProfile>> {
        info!(
            "Retrieving user profile locally for user: {}",
            platform_user_id
        );

        let mut rows = self
            .connection
            .query(
                "SELECT * FROM user_profile WHERE platform_user_id = ?",
                libsql::params![platform_user_id],
            )
            .await?;

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
                annual_income: row.get::<Option<i32>>(18).unwrap_or(None),
                role: row.get(19).unwrap_or_else(|_| "consumer".to_string()),
                is_admin: row.get::<i64>(20).unwrap_or(0) != 0,
                provider_onboarding_complete: row.get::<i64>(21).unwrap_or(0) != 0,
                tenant_id: row.get(22)?,
                object_id: row.get(23)?,
                verified_id_credential_id: row.get::<Option<String>>(24).unwrap_or(None),
                verified_id_status: row.get(25).unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get::<Option<String>>(26).unwrap_or(None),
                created_at: row.get(28).unwrap_or_default(),
                updated_at: row.get(29).unwrap_or_default(),
            };

            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }

    /// Get user profile by Azure AD Object ID (used when user_id is the Azure ID)
    pub async fn get_user_profile_by_azure_id(
        &self,
        azure_id: &str,
    ) -> Result<Option<UserProfile>> {
        info!("Retrieving user profile by azure_id: {}", azure_id);

        let mut rows = self
            .connection
            .query(
                "SELECT * FROM user_profile WHERE azure_id = ?",
                libsql::params![azure_id],
            )
            .await?;

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
                annual_income: row.get::<Option<i32>>(18).unwrap_or(None),
                role: row.get(19).unwrap_or_else(|_| "consumer".to_string()),
                is_admin: row.get::<i64>(20).unwrap_or(0) != 0,
                provider_onboarding_complete: row.get::<i64>(21).unwrap_or(0) != 0,
                tenant_id: row.get(22)?,
                object_id: row.get(23)?,
                verified_id_credential_id: row.get::<Option<String>>(24).unwrap_or(None),
                verified_id_status: row.get(25).unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get::<Option<String>>(26).unwrap_or(None),
                created_at: row.get(28).unwrap_or_default(),
                updated_at: row.get(29).unwrap_or_default(),
            };

            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }
}
