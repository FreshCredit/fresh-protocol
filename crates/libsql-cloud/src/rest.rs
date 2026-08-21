use super::*;

// TAG: surface=database owner=platform-team rule=DB-001
/// libsql's typed `row.get::<T>()` panics with `unreachable!("invalid value type")`
/// when the SQLite value type does not match `T`. These tolerant helpers inspect
/// the raw `Value` and coerce or default instead of aborting the process.
pub(crate) fn tolerant_string(row: &libsql::Row, idx: i32) -> Option<String> {
    match row.get_value(idx) {
        Ok(libsql::Value::Text(s)) => Some(s),
        Ok(libsql::Value::Integer(i)) => Some(i.to_string()),
        Ok(libsql::Value::Real(f)) => Some(f.to_string()),
        Ok(libsql::Value::Blob(_)) => None,
        Ok(libsql::Value::Null) | Err(_) => None,
    }
}

pub(crate) fn tolerant_i64(row: &libsql::Row, idx: i32) -> Option<i64> {
    match row.get_value(idx) {
        Ok(libsql::Value::Integer(i)) => Some(i),
        Ok(libsql::Value::Text(s)) => s.parse::<i64>().ok(),
        Ok(libsql::Value::Real(f)) => Some(f as i64),
        _ => None,
    }
}

pub(crate) fn tolerant_i32(row: &libsql::Row, idx: i32) -> Option<i32> {
    tolerant_i64(row, idx).and_then(|i| i32::try_from(i).ok())
}

pub(crate) fn tolerant_bool(row: &libsql::Row, idx: i32) -> Option<bool> {
    match row.get_value(idx) {
        Ok(libsql::Value::Integer(i)) => Some(i != 0),
        Ok(libsql::Value::Text(s)) => {
            Some(!s.is_empty() && s != "0" && s.to_lowercase() != "false")
        }
        Ok(libsql::Value::Real(f)) => Some(f != 0.0),
        _ => None,
    }
}

pub(crate) fn tolerant_f64(row: &libsql::Row, idx: i32) -> Option<f64> {
    match row.get_value(idx) {
        Ok(libsql::Value::Real(f)) => Some(f),
        Ok(libsql::Value::Integer(i)) => Some(i as f64),
        Ok(libsql::Value::Text(s)) => s.parse::<f64>().ok(),
        _ => None,
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
impl CloudClient {
    /// Get count of accounts from cloud (for sync status)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_account_count(&self) -> FreshCreditResult<u64> {
        info!("Getting account count from cloud");

        let mut rows = self
            .query("SELECT COUNT(*) FROM accounts", cloud_params![])
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        (rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?)
        .map_or(Ok(0), |row| {
            Ok(row.get::<i64>(0).unwrap_or(0).try_into().unwrap_or(0))
        })
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get count of transactions from cloud (for sync status)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_transaction_count(&self) -> FreshCreditResult<u64> {
        info!("Getting transaction count from cloud");

        let mut rows = self
            .query("SELECT COUNT(*) FROM transactions", cloud_params![])
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        (rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?)
        .map_or(Ok(0), |row| {
            Ok(row.get::<i64>(0).unwrap_or(0).try_into().unwrap_or(0))
        })
    }

    /// Check whether a column already exists on a table.
    async fn column_exists(&self, table: &str, column: &str) -> bool {
        let Ok(mut rows) = self
            .query(
                "SELECT 1 FROM pragma_table_info(?) WHERE name = ?",
                cloud_params![table.to_string(), column.to_string()],
            )
            .await
        else {
            return false;
        };
        matches!(rows.next().await, Ok(Some(_)))
    }

    /// Lazily add the onboarding security-step columns to an existing
    /// per-user cloud `user_preferences` table.
    ///
    /// New cloud databases get these columns from the local-crate schema at
    /// provisioning time, but `initialize_schema` only runs at creation — so
    /// databases provisioned before the columns existed are upgraded here, on
    /// first preference access. Idempotent; checks `pragma_table_info` before
    /// each `ALTER TABLE` because libsql does not support `ADD COLUMN IF NOT
    /// EXISTS`.
    async fn ensure_security_preference_columns(&self) {
        if !self
            .column_exists("user_preferences", "vault_key_acknowledged")
            .await
        {
            let _ = self
                .execute(
                    "ALTER TABLE user_preferences ADD COLUMN vault_key_acknowledged BOOLEAN DEFAULT FALSE",
                    cloud_params![],
                )
                .await;
        }
        if !self
            .column_exists("user_preferences", "backup_sync_chosen")
            .await
        {
            let _ = self
                .execute(
                    "ALTER TABLE user_preferences ADD COLUMN backup_sync_chosen BOOLEAN DEFAULT FALSE",
                    cloud_params![],
                )
                .await;
        }
    }

    /// Lazily add the assistant preference columns (widget consent + model
    /// picker) to an existing per-user cloud `user_preferences` table.
    /// Idempotent, same pattern as `ensure_security_preference_columns`.
    async fn ensure_assistant_preference_columns(&self) {
        if !self
            .column_exists("user_preferences", "assistant_data_consent")
            .await
        {
            let _ = self
                .execute(
                    "ALTER TABLE user_preferences ADD COLUMN assistant_data_consent BOOLEAN DEFAULT FALSE",
                    cloud_params![],
                )
                .await;
        }
        if !self
            .column_exists("user_preferences", "assistant_model")
            .await
        {
            let _ = self
                .execute(
                    "ALTER TABLE user_preferences ADD COLUMN assistant_model TEXT",
                    cloud_params![],
                )
                .await;
        }
    }

    /// Get user preferences from cloud
    /// # Errors
    // TAG: surface=database owner=platform-team rule=GENERAL-001
    ///
    /// Returns an error if the operation fails.
    pub async fn get_user_preferences(
        &self,
        user_id: &str,
    ) -> FreshCreditResult<Option<freshcredit_libsql_local::UserPreferences>> {
        info!("Getting user preferences from cloud for: {}", user_id);
        self.ensure_security_preference_columns().await;
        self.ensure_assistant_preference_columns().await;

        let mut rows = self.query(
            "SELECT ai_agent_enabled, ai_feedback_enabled, ai_offers_enabled, ai_lenders_enabled,
                    cloud_sync_enabled, blockchain_enabled, email_notifications_enabled,
                    kilt_did_enabled, ai_mode, COALESCE(mock_data_enabled, 0) as mock_data_enabled,
                    COALESCE(vault_key_acknowledged, 0) as vault_key_acknowledged,
                    COALESCE(backup_sync_chosen, 0) as backup_sync_chosen,
                    COALESCE(assistant_data_consent, 0) as assistant_data_consent,
                    assistant_model
             FROM user_preferences WHERE user_id = ?",
            cloud_params![user_id.to_string()],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        (rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?)
        .map_or(Ok(None), |row| {
            Ok(Some(freshcredit_libsql_local::UserPreferences {
                ai_agent_enabled: tolerant_bool(&row, 0),
                ai_feedback_enabled: tolerant_bool(&row, 1),
                ai_offers_enabled: tolerant_bool(&row, 2),
                // TAG: surface=database owner=platform-team rule=DB-001
                ai_lenders_enabled: tolerant_bool(&row, 3),
                cloud_sync_enabled: tolerant_bool(&row, 4),
                blockchain_enabled: tolerant_bool(&row, 5),
                email_notifications_enabled: tolerant_bool(&row, 6),
                kilt_did_enabled: tolerant_bool(&row, 7),
                ai_mode: row.get::<String>(8).ok(),
                mock_data_enabled: tolerant_bool(&row, 9),
                onboarding_completed: None,
                onboarding_permanently_dismissed: None,
                onboarding_reminder_dismissed_until: None,
                plaid_connection_skipped: None,
                plaid_reminder_dismissed_until: None,
                vault_key_acknowledged: tolerant_bool(&row, 10),
                backup_sync_chosen: tolerant_bool(&row, 11),
                assistant_data_consent: tolerant_bool(&row, 12),
                assistant_model: row.get::<Option<String>>(13).unwrap_or(None),
            }))
        })
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Save user preferences to cloud
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn save_user_preferences(
        &self,
        user_id: &str,
        prefs: &freshcredit_libsql_local::UserPreferences,
    ) -> FreshCreditResult<()> {
        info!("Saving user preferences to cloud for: {}", user_id);
        self.ensure_security_preference_columns().await;
        self.ensure_assistant_preference_columns().await;

        self.execute(
            "INSERT OR REPLACE INTO user_preferences (
                user_id, ai_agent_enabled, ai_feedback_enabled, ai_offers_enabled,
                ai_lenders_enabled, cloud_sync_enabled, blockchain_enabled,
                email_notifications_enabled, kilt_did_enabled, ai_mode, mock_data_enabled,
                vault_key_acknowledged, backup_sync_chosen,
                assistant_data_consent, assistant_model, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            cloud_params![
                user_id.to_string(),
                prefs.ai_agent_enabled.unwrap_or(false),
                prefs.ai_feedback_enabled.unwrap_or(false),
                prefs.ai_offers_enabled.unwrap_or(false),
                prefs.ai_lenders_enabled.unwrap_or(false),
                prefs.cloud_sync_enabled.unwrap_or(true),
                prefs.blockchain_enabled.unwrap_or(true),
                prefs.email_notifications_enabled.unwrap_or(true),
                prefs.kilt_did_enabled.unwrap_or(false),
                prefs.ai_mode.clone().unwrap_or_else(|| "auto".to_string()),
                prefs.mock_data_enabled.unwrap_or(false),
                prefs.vault_key_acknowledged.unwrap_or(false),
                prefs.backup_sync_chosen.unwrap_or(false),
                prefs.assistant_data_consent.unwrap_or(false),
                prefs.assistant_model.clone(),
                chrono::Utc::now().to_rfc3339(),
            ],
        )
        .await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get user profile by Azure ID (`object_id`) from per-user Turso cloud
    ///
    /// ARCHITECTURE: Used by payments/plaid routes to read user data from per-user cloud.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_user_profile_by_azure_id(
        &self,
        azure_id: &str,
    ) -> FreshCreditResult<Option<freshcredit_libsql_local::UserProfile>> {
        info!("Getting user profile from cloud by azure_id: {}", azure_id);

        let mut rows = self
            .query(
                "SELECT id, platform_user_id, azure_id, email, display_name,
                        given_name, family_name, surname, mobile_phone, job_title,
                        street_address, city, state_province, postal_code, country_region,
                        date_of_birth, ssn_last_four, employment_status, annual_income,
                        phone_number, preferred_name, emergency_contact_name,
                        emergency_contact_phone, employer_name, role, is_admin,
                        provider_onboarding_complete, tenant_id, object_id,
                        verified_id_credential_id, verified_id_status,
                        verified_id_issued_at,
                        consumer_verified_id_credential_id, consumer_verified_id_status, consumer_verified_id_issued_at,
                        provider_verified_id_credential_id, provider_verified_id_status, provider_verified_id_issued_at,
                        created_at, updated_at
                 FROM user_profile WHERE azure_id = ? OR object_id = ?",
                cloud_params![azure_id.to_string(), azure_id.to_string()],
            )
// TAG: surface=database owner=platform-team rule=GENERAL-001
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        (rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?)
        .map_or(Ok(None), |row| {
            Ok(Some(freshcredit_libsql_local::UserProfile {
                id: row.get(0).unwrap_or_default(),
                platform_user_id: row.get(1).unwrap_or_default(),
                azure_id: row.get(2).unwrap_or_default(),
                email: row.get(3).unwrap_or_default(),
                display_name: row.get(4).unwrap_or_default(),
                given_name: row.get(5).ok(),
                family_name: row.get(6).ok(),
                surname: row.get(7).ok(),
                mobile_phone: row.get(8).ok(),
                job_title: row.get(9).ok(),
                street_address: row.get(10).ok(),
                city: row.get(11).ok(),
                state_province: row.get(12).ok(),
                postal_code: row.get(13).ok(),
                country_region: row.get(14).ok(),
                date_of_birth: row.get(15).ok(),
                ssn_last_four: row.get(16).ok(),
                employment_status: row.get(17).ok(),
                annual_income: tolerant_i32(&row, 18),
                // New fields added in ARCH-P2-001 (columns 19-23)
                // TAG: surface=database owner=platform-team rule=DB-001
                phone_number: row.get(19).ok(),
                preferred_name: row.get(20).ok(),
                emergency_contact_name: row.get(21).ok(),
                emergency_contact_phone: row.get(22).ok(),
                employer_name: row.get(23).ok(),
                // Existing fields shifted by 5 (columns 24+)
                role: row.get(24).unwrap_or_else(|_| "consumer".to_string()),
                is_admin: tolerant_bool(&row, 25).unwrap_or(false),
                provider_onboarding_complete: tolerant_bool(&row, 26).unwrap_or(false),
                mfa_enabled: false,
                mfa_verified_at: None,
                tenant_id: row.get(27).unwrap_or_default(),
                object_id: row.get(28).unwrap_or_default(),
                verified_id_credential_id: row.get(29).ok(),
                verified_id_status: row.get(30).unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get(31).ok(),
                consumer_verified_id_credential_id: row.get(32).ok(),
                consumer_verified_id_status: row.get(33).unwrap_or_else(|_| "pending".to_string()),
                consumer_verified_id_issued_at: row.get(34).ok(),
                provider_verified_id_credential_id: row.get(35).ok(),
                provider_verified_id_status: row.get(36).unwrap_or_else(|_| "pending".to_string()),
                provider_verified_id_issued_at: row.get(37).ok(),
                created_at: row.get(38).unwrap_or_default(),
                updated_at: row.get(39).unwrap_or_default(),
            }))
        })
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get all accounts for a user from per-user Turso cloud
    ///
    /// ARCHITECTURE: Used by payments/plaid routes to read account data from per-user cloud.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_user_accounts(
        &self,
        user_id: &str,
    ) -> FreshCreditResult<Vec<freshcredit_types::Account>> {
        info!("Getting accounts from cloud for user: {}", user_id);

        let mut rows = self
            .query(
                "SELECT id, user_id, account_type, balance, currency, institution_name, created_at
                 FROM accounts WHERE user_id = ? ORDER BY created_at DESC",
                cloud_params![user_id.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        let mut accounts = Vec::new();
        while let Some(row) = rows
            .next()
            .await
// TAG: surface=database owner=platform-team rule=GENERAL-001
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let account_type_str: String = row.get(2).unwrap_or_default();
            let account_type = match account_type_str.to_lowercase().as_str() {
                "savings" => freshcredit_types::AccountType::Savings,
                "credit" => freshcredit_types::AccountType::Credit,
                "investment" => freshcredit_types::AccountType::Investment,
                "loan" => freshcredit_types::AccountType::Loan,
                _ => freshcredit_types::AccountType::Checking,
            };

            let created_at_str: String = row.get(6).unwrap_or_default();
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map_or_else(|_| chrono::Utc::now(), |dt| dt.with_timezone(&chrono::Utc));

            accounts.push(freshcredit_types::Account {
                id: row.get(0).unwrap_or_default(),
                user_id: row.get(1).unwrap_or_default(),
                account_type,
                balance: tolerant_f64(&row, 3),
                currency: row.get(4).unwrap_or_else(|_| "USD".to_string()),
                institution_name: row.get(5).unwrap_or_default(),
                created_at,
            });
        }

        Ok(accounts)
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get all transactions for a user from per-user Turso cloud
    ///
    /// ARCHITECTURE: Used by payments/plaid routes to read transaction data from per-user cloud.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_user_transactions(
        &self,
        user_id: &str,
    ) -> FreshCreditResult<Vec<freshcredit_types::Transaction>> {
        info!("Getting transactions from cloud for user: {}", user_id);

        let mut rows = self
            .query(
                "SELECT t.id, t.account_id, t.amount, COALESCE(t.iso_currency_code, 'USD'),
                        t.name, t.category, t.date, t.merchant_name
                 FROM transactions t
                 JOIN accounts a ON t.account_id = a.id
                 WHERE a.user_id = ?
                 ORDER BY t.date DESC",
                cloud_params![user_id.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        let mut transactions = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let category: Option<String> = row.get(5).ok();
            let merchant_name: Option<String> = row.get(7).ok();
            let date_str: String = row.get(6).unwrap_or_default();
            let date = chrono::DateTime::parse_from_rfc3339(&date_str)
                .map_or_else(|_| chrono::Utc::now(), |dt| dt.with_timezone(&chrono::Utc));

            // TAG: surface=database owner=platform-team rule=DB-001
            transactions.push(freshcredit_types::Transaction {
                id: row.get(0).unwrap_or_default(),
                account_id: row.get(1).unwrap_or_default(),
                amount: tolerant_f64(&row, 2).unwrap_or(0.0),
                currency: row.get(3).unwrap_or_else(|_| "USD".to_string()),
                description: row.get(4).unwrap_or_default(),
                category,
                date,
                merchant_name,
            });
        }

        Ok(transactions)
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get Plaid access token for a user's account
    ///
    /// ARCHITECTURE: Retrieves the encrypted access token for Plaid API calls.
    /// S2.3: Added for payment flow to retrieve real access tokens.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_plaid_access_token(&self, user_id: &str) -> FreshCreditResult<Option<String>> {
        info!("Getting Plaid access token for user: {}", user_id);

        let mut rows = self
            .query(
                "SELECT plaid_access_token FROM accounts WHERE user_id = ? AND plaid_access_token IS NOT NULL LIMIT 1",
                cloud_params![user_id.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let access_token: Option<String> = row.get(0).ok();
            return Ok(access_token);
        }

        Ok(None)
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get Plaid access token for a specific account ID
    ///
    /// ARCHITECTURE: Retrieves the encrypted access token for a specific account.
    /// S2.3: Added for payment flow to retrieve access token by account.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_plaid_access_token_for_account(
        &self,
        account_id: &str,
    ) -> FreshCreditResult<Option<String>> {
        info!("Getting Plaid access token for account: {}", account_id);

        let mut rows = self
            .query(
                "SELECT plaid_access_token FROM accounts WHERE id = ? AND plaid_access_token IS NOT NULL LIMIT 1",
                cloud_params![account_id.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let access_token: Option<String> = row.get(0).ok();
            return Ok(access_token);
        }

        Ok(None)
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get a specific account by ID
    ///
    /// ARCHITECTURE: Used by payment flow to get account details for a specific account.
    /// S2.3: Added for proper account lookup in payment flow.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_account_by_id(
        &self,
        account_id: &str,
    ) -> FreshCreditResult<Option<freshcredit_types::Account>> {
        info!("Getting account by ID: {}", account_id);

        let mut rows = self
            .query(
                "SELECT id, user_id, account_type, balance, currency, institution_name, created_at
                 FROM accounts WHERE id = ? LIMIT 1",
                cloud_params![account_id.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
// TAG: surface=database owner=platform-team rule=GENERAL-001
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let account_type_str: String = row.get(2).unwrap_or_default();
            let account_type = match account_type_str.to_lowercase().as_str() {
                "savings" => freshcredit_types::AccountType::Savings,
                "credit" => freshcredit_types::AccountType::Credit,
                "investment" => freshcredit_types::AccountType::Investment,
                "loan" => freshcredit_types::AccountType::Loan,
                _ => freshcredit_types::AccountType::Checking,
            };

            let created_at_str: String = row.get(6).unwrap_or_default();
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map_or_else(|_| chrono::Utc::now(), |dt| dt.with_timezone(&chrono::Utc));

            return Ok(Some(freshcredit_types::Account {
                id: row.get(0).unwrap_or_default(),
                user_id: row.get(1).unwrap_or_default(),
                account_type,
                balance: tolerant_f64(&row, 3),
                currency: row.get(4).unwrap_or_else(|_| "USD".to_string()),
                institution_name: row.get(5).unwrap_or_default(),
                created_at,
            }));
        }

        Ok(None)
    }
}
