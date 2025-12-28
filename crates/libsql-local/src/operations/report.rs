//! Financial report database operations
//!
//! Operations for storing and retrieving financial reports:
//! - store_financial_report: Store financial report locally (uses reports table - BlockID)
//! - get_financial_report: Retrieve financial report from local storage
//!
//! COMPLIANCE: §5 Data and Report Handling - user-owned data

use freshcredit_types::{FinancialReport, FreshCreditResult, UserId};
use tracing::info;

use crate::LocalClient;

impl LocalClient {
    /// Store financial report locally (uses reports table - BlockID)
    pub async fn store_financial_report(&self, report: &FinancialReport) -> FreshCreditResult<()> {
        info!("Storing financial report locally for user: {}", report.user_id);

        let data = serde_json::to_string(report)
            .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;

        self.connection.execute(
            "INSERT OR REPLACE INTO reports (id, user_id, report_type, report_status, report_data, raw_report_data, blockchain_hash, created_at, updated_at)
             VALUES (?, ?, 'financial', 'ready', ?, ?, ?, datetime('now'), datetime('now'))",
            libsql::params![
                report.id.to_string(),
                report.user_id.clone(),
                data.clone(),
                data,
                report.blockchain_hash.clone(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Retrieve financial report from local storage using raw SQL
    pub async fn get_financial_report(
        &self,
        user_id: &UserId,
    ) -> FreshCreditResult<Option<FinancialReport>> {
        info!("Retrieving financial report from local storage for user: {}", user_id);

        let mut rows = self.connection.query(
            "SELECT report_data FROM reports WHERE user_id = ? ORDER BY created_at DESC LIMIT 1",
            libsql::params![user_id.clone()],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let data: String = row
                .get(0)
                .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
            let report: FinancialReport = serde_json::from_str(&data)
                .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;
            Ok(Some(report))
        } else {
            Ok(None)
        }
    }
}

