//! User preferences database operations
//!
//! Operations for storing and retrieving user preferences:
//! - `get_user_preferences`: Get user preferences
//! - `save_user_preferences`: Save user preferences
//!
//! P0g: Added onboarding dismissal fields (§27.3)
//!
//! COMPLIANCE: §27.3 Onboarding Dismissal Patterns

use anyhow::Result;
use tracing::info;

use crate::LocalClient;
use crate::UserPreferences;

// TAG: surface=database owner=platform-team rule=DB-001
impl LocalClient {
    /// Get user preferences
    /// P0g: Added onboarding dismissal fields (§27.3)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn get_bool_pref(row: &libsql::Row, idx: i32) -> Result<Option<bool>> {
        Ok(Some(row.get::<i64>(idx)? != 0))
    }

    fn get_string_pref(row: &libsql::Row, idx: i32) -> Result<Option<String>> {
        Ok(Some(row.get::<String>(idx)?))
    }

    /// Parse a user preferences row into a struct.
    fn row_to_user_preferences(row: &libsql::Row) -> Result<UserPreferences> {
        Ok(UserPreferences {
            ai_agent_enabled: Self::get_bool_pref(row, 0)?,
            ai_feedback_enabled: Self::get_bool_pref(row, 1)?,
            ai_offers_enabled: Self::get_bool_pref(row, 2)?,
            ai_lenders_enabled: Self::get_bool_pref(row, 3)?,
            cloud_sync_enabled: Self::get_bool_pref(row, 4)?,
            blockchain_enabled: Self::get_bool_pref(row, 5)?,
            email_notifications_enabled: Self::get_bool_pref(row, 6)?,
            kilt_did_enabled: Self::get_bool_pref(row, 7)?,
            ai_mode: Self::get_string_pref(row, 8)?,
            mock_data_enabled: Self::get_bool_pref(row, 9)?,
            onboarding_completed: Self::get_bool_pref(row, 10)?,
            onboarding_permanently_dismissed: Self::get_bool_pref(row, 11)?,
            onboarding_reminder_dismissed_until: row.get::<Option<String>>(12).unwrap_or(None),
            plaid_connection_skipped: Self::get_bool_pref(row, 13)?,
            plaid_reminder_dismissed_until: row.get::<Option<String>>(14).unwrap_or(None),
            vault_key_acknowledged: Self::get_bool_pref(row, 15)?,
            backup_sync_chosen: Self::get_bool_pref(row, 16)?,
            assistant_data_consent: Self::get_bool_pref(row, 17)?,
            assistant_model: row.get::<Option<String>>(18).unwrap_or(None),
        })
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Retrieves user preferences from the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get_user_preferences(&self, user_id: &str) -> Result<Option<UserPreferences>> {
        info!("Getting preferences for user: {user_id}");

        let mut rows = self.connection.query(
            "SELECT ai_agent_enabled, ai_feedback_enabled, ai_offers_enabled, ai_lenders_enabled,
                    cloud_sync_enabled, blockchain_enabled, email_notifications_enabled, kilt_did_enabled,
                    COALESCE(ai_mode, 'auto') as ai_mode, COALESCE(mock_data_enabled, 0) as mock_data_enabled,
                    COALESCE(onboarding_completed, 0) as onboarding_completed,
                    COALESCE(onboarding_permanently_dismissed, 0) as onboarding_permanently_dismissed,
                    onboarding_reminder_dismissed_until,
                    COALESCE(plaid_connection_skipped, 0) as plaid_connection_skipped,
                    plaid_reminder_dismissed_until,
                    COALESCE(vault_key_acknowledged, 0) as vault_key_acknowledged,
                    COALESCE(backup_sync_chosen, 0) as backup_sync_chosen,
                    COALESCE(assistant_data_consent, 0) as assistant_data_consent,
                    assistant_model
             FROM user_preferences WHERE user_id = ?",
            libsql::params![user_id],
        ).await?;

        Ok(if let Some(row) = rows.next().await? {
            Some(Self::row_to_user_preferences(&row)?)
        } else {
            None
        })
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Save user preferences
    /// P0g: Added onboarding dismissal fields (§27.3)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn save_user_preferences(
        &self,
        user_id: &str,
        prefs: &UserPreferences,
    ) -> Result<()> {
        info!("Saving preferences for user: {user_id}");

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        self.connection
            .execute(
                "INSERT INTO user_preferences (id, user_id, ai_agent_enabled, ai_feedback_enabled,
                ai_offers_enabled, ai_lenders_enabled, cloud_sync_enabled, blockchain_enabled,
                email_notifications_enabled, kilt_did_enabled, ai_mode, mock_data_enabled,
                onboarding_completed, onboarding_permanently_dismissed, onboarding_reminder_dismissed_until,
                plaid_connection_skipped, plaid_reminder_dismissed_until,
                vault_key_acknowledged, backup_sync_chosen,
                assistant_data_consent, assistant_model, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET
                ai_agent_enabled = excluded.ai_agent_enabled,
                ai_feedback_enabled = excluded.ai_feedback_enabled,
                ai_offers_enabled = excluded.ai_offers_enabled,
                ai_lenders_enabled = excluded.ai_lenders_enabled,
                cloud_sync_enabled = excluded.cloud_sync_enabled,
                blockchain_enabled = excluded.blockchain_enabled,
                email_notifications_enabled = excluded.email_notifications_enabled,
                kilt_did_enabled = excluded.kilt_did_enabled,
                ai_mode = excluded.ai_mode,
                mock_data_enabled = excluded.mock_data_enabled,
                onboarding_completed = excluded.onboarding_completed,
                onboarding_permanently_dismissed = excluded.onboarding_permanently_dismissed,
                onboarding_reminder_dismissed_until = excluded.onboarding_reminder_dismissed_until,
                plaid_connection_skipped = excluded.plaid_connection_skipped,
                plaid_reminder_dismissed_until = excluded.plaid_reminder_dismissed_until,
                vault_key_acknowledged = excluded.vault_key_acknowledged,
                backup_sync_chosen = excluded.backup_sync_chosen,
                assistant_data_consent = excluded.assistant_data_consent,
                assistant_model = excluded.assistant_model,
                updated_at = excluded.updated_at",
                // TASK 4 FIX: AI agent defaults to TRUE (enabled)
                libsql::params![
                    id,
                    user_id,
                    i64::from(prefs.ai_agent_enabled.unwrap_or(true)),  // Default TRUE
                    i64::from(prefs.ai_feedback_enabled.unwrap_or(true)),  // Default TRUE
                    i64::from(prefs.ai_offers_enabled.unwrap_or(true)),  // Default TRUE
                    i64::from(prefs.ai_lenders_enabled.unwrap_or(true)),  // Default TRUE
                    i64::from(prefs.cloud_sync_enabled.unwrap_or(true)),
                    i64::from(prefs.blockchain_enabled.unwrap_or(true)),
                    i64::from(prefs.email_notifications_enabled.unwrap_or(true)),
                    i64::from(prefs.kilt_did_enabled.unwrap_or(false)),
                    prefs.ai_mode.clone().unwrap_or_else(|| "auto".to_string()),
                    i64::from(prefs.mock_data_enabled.unwrap_or(false)),
                    i64::from(prefs.onboarding_completed.unwrap_or(false)),
                    i64::from(prefs.onboarding_permanently_dismissed.unwrap_or(false)),
                    prefs.onboarding_reminder_dismissed_until.clone(),
                    i64::from(prefs.plaid_connection_skipped.unwrap_or(false)),
                    prefs.plaid_reminder_dismissed_until.clone(),
                    i64::from(prefs.vault_key_acknowledged.unwrap_or(false)),
                    i64::from(prefs.backup_sync_chosen.unwrap_or(false)),
                    i64::from(prefs.assistant_data_consent.unwrap_or(false)),
                    prefs.assistant_model.clone(),
                    now.clone(),
                    now
                ],
            )
            .await?;

        Ok(())
    }
}
