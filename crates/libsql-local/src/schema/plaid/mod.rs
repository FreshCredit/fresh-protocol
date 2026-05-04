//! Plaid product schema definitions
//!
//! Contains all Plaid product tables:
//! - auth: Account authentication data (routing/account numbers)
//! - identities: Identity data per account
//! - assets: Asset Reports
//! - balances: Balance history
//! - consumer_reports: Consumer Reports (credit data)
//! - employment: Employment verification
//! - enrich: Transaction enrichment
//! - income: Bank income
//! - income_verification: Payroll/employment verification
//! - investments_holdings, investments_securities, investments_transactions
//! - layer: Plaid Layer data
//! - liabilities: Credit cards, mortgages, student loans
//! - monitor: Plaid Monitor alerts
//! - recurring_transactions: Recurring payment detection
//! - signal_evaluations: Signal score evaluations
//! - statements: Bank statements
//! - transactions_sync: Transaction sync cursors
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

pub mod indexes;
pub mod tables;

use anyhow::Result;
use libsql::Connection;
use tracing::info;

/// Initialize all Plaid tables (convenience function)
pub async fn initialize_all_plaid_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing plaid tables");
    tables::auth::initialize_plaid_auth_tables(conn).await?;
    tables::assets::initialize_plaid_assets_tables(conn).await?;
    tables::reports::initialize_plaid_reports_tables(conn).await?;
    tables::income::initialize_plaid_income_tables(conn).await?;
    tables::investments::initialize_plaid_investments_tables(conn).await?;
    tables::liabilities::initialize_plaid_liabilities_tables(conn).await?;
    tables::monitoring::initialize_plaid_monitoring_tables(conn).await?;
    indexes::initialize_plaid_indexes(conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_plaid_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
