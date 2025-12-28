//! User preferences database operations
//!
//! Operations for storing and retrieving user preferences:
//! - get_user_preferences: Get user preferences
//! - save_user_preferences: Save user preferences
//!
//! P0g: Added onboarding dismissal fields (§27.3)
//!
//! COMPLIANCE: §27.3 Onboarding Dismissal Patterns

use anyhow::Result;
use tracing::info;

use crate::LocalClient;
use crate::UserPreferences;

impl LocalClient {
    /// Get user preferences
    /// P0g: Added onboarding dismissal fields (§27.3)
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
                    plaid_reminder_dismissed_until
             FROM user_preferences WHERE user_id = ?",
            libsql::params![user_id],
        ).await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(UserPreferences {
                ai_agent_enabled: Some(row.get::<i64>(0)? != 0),
                ai_feedback_enabled: Some(row.get::<i64>(1)? != 0),
                ai_offers_enabled: Some(row.get::<i64>(2)? != 0),
                ai_lenders_enabled: Some(row.get::<i64>(3)? != 0),
                cloud_sync_enabled: Some(row.get::<i64>(4)? != 0),
                blockchain_enabled: Some(row.get::<i64>(5)? != 0),
                email_notifications_enabled: Some(row.get::<i64>(6)? != 0),
                kilt_did_enabled: Some(row.get::<i64>(7)? != 0),
                ai_mode: Some(row.get::<String>(8)?),
                mock_data_enabled: Some(row.get::<i64>(9)? != 0),
                onboarding_completed: Some(row.get::<i64>(10)? != 0),
                onboarding_permanently_dismissed: Some(row.get::<i64>(11)? != 0),
                onboarding_reminder_dismissed_until: row.get::<Option<String>>(12).unwrap_or(None),
                plaid_connection_skipped: Some(row.get::<i64>(13)? != 0),
                plaid_reminder_dismissed_until: row.get::<Option<String>>(14).unwrap_or(None),
            }))
        } else {
            Ok(None)
        }
    }

    /// Save user preferences
    /// P0g: Added onboarding dismissal fields (§27.3)
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
                plaid_connection_skipped, plaid_reminder_dismissed_until, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                updated_at = excluded.updated_at",
                libsql::params![
                    id,
                    user_id,
                    prefs.ai_agent_enabled.unwrap_or(false) as i64,
                    prefs.ai_feedback_enabled.unwrap_or(false) as i64,
                    prefs.ai_offers_enabled.unwrap_or(false) as i64,
                    prefs.ai_lenders_enabled.unwrap_or(false) as i64,
                    prefs.cloud_sync_enabled.unwrap_or(true) as i64,
                    prefs.blockchain_enabled.unwrap_or(true) as i64,
                    prefs.email_notifications_enabled.unwrap_or(true) as i64,
                    prefs.kilt_did_enabled.unwrap_or(false) as i64,
                    prefs.ai_mode.clone().unwrap_or_else(|| "auto".to_string()),
                    prefs.mock_data_enabled.unwrap_or(false) as i64,
                    prefs.onboarding_completed.unwrap_or(false) as i64,
                    prefs.onboarding_permanently_dismissed.unwrap_or(false) as i64,
                    prefs.onboarding_reminder_dismissed_until.clone(),
                    prefs.plaid_connection_skipped.unwrap_or(false) as i64,
                    prefs.plaid_reminder_dismissed_until.clone(),
                    now.clone(),
                    now
                ],
            )
            .await?;

        Ok(())
    }
}
