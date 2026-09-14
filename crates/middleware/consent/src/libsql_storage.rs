// TAG: surface=security owner=security-team rule=SEC-001
//! LibSQL-backed consent storage implementation

use crate::storage::ConsentStorage;
use crate::types::{Consent, ConsentError, ConsentType, Jurisdiction};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use libsql::Connection;
use std::sync::Arc;
use tracing::{debug, info};

/// `LibSQL` consent storage implementation
#[derive(Debug)]
pub struct LibSqlConsentStorage {
    conn: Arc<Connection>,
}

impl LibSqlConsentStorage {
    /// Create a new `LibSQL` consent storage
    #[must_use]
    pub const fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    fn parse_timestamp(s: &str, field_name: &str) -> Result<DateTime<Utc>, ConsentError> {
        DateTime::parse_from_rfc3339(s)
            // TAG: surface=security owner=platform-team rule=MID-001
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| ConsentError::DatabaseError(format!("Invalid {field_name}: {e}")))
    }

    fn parse_optional_timestamp(
        s: Option<String>,
        field_name: &str,
    ) -> Result<Option<DateTime<Utc>>, ConsentError> {
        s.map(|s| Self::parse_timestamp(&s, field_name)).transpose()
    }

    /// Parse consent from database row
    fn parse_consent(row: &libsql::Row) -> Result<Consent, ConsentError> {
        let id: i64 = row.get(0)?;
        let user_id: i64 = row.get(1)?;
        let consent_type_str: String = row.get(2)?;
        let consent_purpose: String = row.get(3)?;
        let consent_granted: bool = row.get(4)?;
        let consent_method: String = row.get(5)?;
        let consent_timestamp_str: String = row.get(6)?;
        let expiry_timestamp_str: Option<String> = row.get(7)?;
        let revoked: bool = row.get(8)?;
        let revocation_timestamp_str: Option<String> = row.get(9)?;

        let consent_timestamp = Self::parse_timestamp(&consent_timestamp_str, "consent_timestamp")?;
        let expiry_timestamp =
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            Self::parse_optional_timestamp(expiry_timestamp_str, "expiry_timestamp")?;
        let revocation_timestamp =
            Self::parse_optional_timestamp(revocation_timestamp_str, "revocation_timestamp")?;

        let consent_type = ConsentType::from_db_string(&consent_type_str)?;
        let jurisdiction = Jurisdiction::US;

        Ok(Consent {
            id,
            user_id,
            consent_type,
            consent_purpose,
            consent_granted,
            consent_method,
            consent_timestamp,
            expiry_timestamp,
            revoked,
            revocation_timestamp,
            jurisdiction,
        })
    }
}

#[async_trait]
impl ConsentStorage for LibSqlConsentStorage {
    async fn get_consent(
        // TAG: surface=security owner=platform-team rule=MID-001
        &self,
        user_id: i64,
        consent_type: &ConsentType,
    ) -> Result<Option<Consent>, ConsentError> {
        let query = r"
            SELECT id, user_id, consent_type, consent_purpose, consent_granted,
                   consent_method, consent_timestamp, expiry_timestamp, revoked,
                   revocation_timestamp
            FROM data_access_consents
            WHERE user_id = ? AND consent_type = ?
            ORDER BY consent_timestamp DESC
            LIMIT 1
        ";

        let mut rows = self
            .conn
            .query(query, libsql::params![user_id, consent_type.to_db_string()])
            .await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(Self::parse_consent(&row)?))
        } else {
            Ok(None)
        }
    }

    // TAG: surface=security owner=security-team rule=SEC-001
    async fn create_consent(
        &self,
        user_id: i64,
        consent_type: ConsentType,
        consent_purpose: String,
        consent_method: String,
        jurisdiction: Jurisdiction,
        expiry_timestamp: Option<DateTime<Utc>>,
    ) -> Result<Consent, ConsentError> {
        let consent_timestamp = Utc::now();
        let consent_timestamp_str = consent_timestamp.to_rfc3339();
        let expiry_timestamp_str = expiry_timestamp.map(|dt| dt.to_rfc3339());

        let query = r"
            INSERT INTO data_access_consents (
                user_id, consent_type, consent_purpose, consent_granted,
                consent_method, consent_timestamp, expiry_timestamp, revoked
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ";

        self.conn
            .execute(
                query,
                libsql::params![
                    // TAG: surface=security owner=platform-team rule=MID-001
                    user_id,
                    consent_type.to_db_string(),
                    consent_purpose.clone(),
                    true, // consent_granted
                    consent_method.clone(),
                    consent_timestamp_str,
                    expiry_timestamp_str,
                    false, // revoked
                ],
            )
            .await?;

        debug!(
            "Created consent for user {} type {:?}",
            user_id, consent_type
        );

        // Return the created consent
        Ok(Consent {
            id: 0, // Will be set by database
            user_id,
            consent_type,
            consent_purpose,
            consent_granted: true,
            consent_method,
            consent_timestamp,
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            expiry_timestamp,
            revoked: false,
            revocation_timestamp: None,
            jurisdiction,
        })
    }

    async fn revoke_consent(
        &self,
        user_id: i64,
        consent_type: &ConsentType,
    ) -> Result<(), ConsentError> {
        let revocation_timestamp = Utc::now().to_rfc3339();

        let query = r"
            UPDATE data_access_consents
            SET revoked = true, revocation_timestamp = ?
            WHERE user_id = ? AND consent_type = ? AND revoked = false
        ";

        self.conn
            .execute(
                query,
                libsql::params![revocation_timestamp, user_id, consent_type.to_db_string(),],
            )
            .await?;
        // TAG: surface=security owner=platform-team rule=MID-001

        debug!(
            "Revoked consent for user {} type {:?}",
            user_id, consent_type
        );
        Ok(())
    }

    async fn get_user_consents(&self, user_id: i64) -> Result<Vec<Consent>, ConsentError> {
        let query = r"
            SELECT id, user_id, consent_type, consent_purpose, consent_granted,
                   consent_method, consent_timestamp, expiry_timestamp, revoked,
                   revocation_timestamp
            FROM data_access_consents
            WHERE user_id = ?
            ORDER BY consent_timestamp DESC
        ";

        let mut rows = self.conn.query(query, libsql::params![user_id]).await?;
        let mut consents = Vec::new();

        while let Some(row) = rows.next().await? {
            consents.push(Self::parse_consent(&row)?);
        }

        Ok(consents)
        // TAG: surface=security owner=security-team rule=SEC-001
    }

    async fn cleanup_expired(&self) -> Result<u64, ConsentError> {
        let now = Utc::now().to_rfc3339();

        let query = r"
            UPDATE data_access_consents
            SET revoked = true, revocation_timestamp = ?
            WHERE expiry_timestamp IS NOT NULL
              AND expiry_timestamp < ?
              AND revoked = false
        ";

        let result = self
            .conn
            .execute(query, libsql::params![now.clone(), now])
            .await?;

        let updated = result;

        if updated > 0 {
            info!("Cleaned up {} expired consents", updated);
        }

        Ok(updated)
        // TAG: surface=security owner=platform-team rule=MID-001
    }
}

#[cfg(test)]
#[allow(unused_variables, unused_imports)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_libsql_storage_new() {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .unwrap();
        let conn = Arc::new(db.connect().unwrap());
        let storage = LibSqlConsentStorage::new(conn);
        // Just verify it creates successfully
    }

    #[tokio::test]
    async fn test_create_consent() {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .unwrap();
        let conn = Arc::new(db.connect().unwrap());
        // TAG: surface=security owner=platform-team rule=GENERAL-001
        let storage = LibSqlConsentStorage::new(conn.clone());

        // Create the table
        conn.execute(
            r"
            CREATE TABLE data_access_consents (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                consent_type TEXT NOT NULL,
                consent_purpose TEXT NOT NULL,
                consent_granted BOOLEAN NOT NULL,
                consent_method TEXT NOT NULL,
                consent_timestamp TEXT NOT NULL,
                expiry_timestamp TEXT,
                revoked BOOLEAN NOT NULL DEFAULT false,
                revocation_timestamp TEXT
            )
            ",
            (),
        )
        .await
        .unwrap();

        let consent = storage
            .create_consent(
                1,
                // TAG: surface=security owner=platform-team rule=MID-001
                ConsentType::FinancialDataAccess,
                "Test purpose".to_string(),
                "explicit".to_string(),
                Jurisdiction::US,
                None,
            )
            .await
            .unwrap();

        assert_eq!(consent.user_id, 1);
        assert_eq!(consent.consent_type, ConsentType::FinancialDataAccess);
        assert_eq!(consent.consent_purpose, "Test purpose");
        assert!(consent.consent_granted);
        assert_eq!(consent.consent_method, "explicit");
        assert!(!consent.revoked);
    }

    #[tokio::test]
    async fn test_get_consent() {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .unwrap();
        let conn = Arc::new(db.connect().unwrap());
        let storage = LibSqlConsentStorage::new(conn.clone());

        // TAG: surface=security owner=security-team rule=SEC-001
        // Create the table
        conn.execute(
            r"
            CREATE TABLE data_access_consents (
                id INTEGER PRIMARY KEY,
                user_id INTEGER NOT NULL,
                consent_type TEXT NOT NULL,
                consent_purpose TEXT NOT NULL,
                consent_granted BOOLEAN NOT NULL,
                consent_method TEXT NOT NULL,
                consent_timestamp TEXT NOT NULL,
                expiry_timestamp TEXT,
                revoked BOOLEAN NOT NULL DEFAULT false,
                revocation_timestamp TEXT
            )
            ",
            (),
        )
        .await
        .unwrap();

        // Initially no consent
        let result = storage
            .get_consent(1, &ConsentType::FinancialDataAccess)
            .await
            // TAG: surface=security owner=platform-team rule=MID-001
            .unwrap();
        assert!(result.is_none());

        // Create a consent
        storage
            .create_consent(
                1,
                ConsentType::FinancialDataAccess,
                "Test".to_string(),
                "explicit".to_string(),
                Jurisdiction::US,
                None,
            )
            .await
            .unwrap();

        // Now it should exist
        let result = storage
            .get_consent(1, &ConsentType::FinancialDataAccess)
            .await
            .unwrap();
        assert!(result.is_some());
        let consent = result.unwrap();
        assert_eq!(consent.user_id, 1);
        assert_eq!(consent.consent_type, ConsentType::FinancialDataAccess);
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
