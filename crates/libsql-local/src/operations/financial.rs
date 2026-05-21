//! Financial data database operations
//!
//! Operations for storing and retrieving account and transaction data:
//! - `store_account`: Store account data in local database
//! - `store_transaction`: Store transaction data in local database
//! - `get_user_accounts`: Get all accounts for a user
//! - `get_user_transactions`: Get all transactions for a user
//!
//! COMPLIANCE: §5 Data and Report Handling - user-owned data

use anyhow::Result;
use tracing::info;

use crate::LocalClient;

impl LocalClient {
    /// Store account data in local database
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn store_account(&self, account: &freshcredit_types::Account) -> Result<()> {
        info!("Storing account locally: {}", account.id);

        self.connection.execute(
            "INSERT OR REPLACE INTO accounts (id, user_id, account_type, balance, currency, institution_name, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                account.id.clone(),
                account.user_id.clone(),
                format!("{:?}", account.account_type), // Convert enum to string
                account.balance,
                account.currency.clone(),
                account.institution_name.clone(),
                account.created_at.to_rfc3339(),
            ],
        ).await?;

        Ok(())
    }

    /// Store transaction data in local database
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn store_transaction(
        &self,
        transaction: &freshcredit_types::Transaction,
    ) -> Result<()> {
        info!("Storing transaction locally: {}", transaction.id);

        self.connection.execute(
            "INSERT OR REPLACE INTO transactions (id, account_id, amount, currency, description, category, date, merchant_name)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                transaction.id.clone(),
                transaction.account_id.clone(),
                transaction.amount,
                transaction.currency.clone(),
                transaction.description.clone(),
                transaction.category.clone().unwrap_or_default(),
                transaction.date.to_rfc3339(),
                transaction.merchant_name.clone().unwrap_or_default(),
            ],
        ).await?;

        Ok(())
    }

    /// Get all accounts for a user
    /// P0-PERF: Limited to 1000 accounts to prevent memory exhaustion
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_user_accounts(
        &self,
        user_id: &str,
    ) -> Result<Vec<freshcredit_types::Account>> {
        info!("Retrieving accounts for user: {}", user_id);

        let mut rows = self
            .connection
            .query(
                "SELECT * FROM accounts WHERE user_id = ? ORDER BY created_at DESC LIMIT 1000",
                libsql::params![user_id],
            )
            .await?;

        let mut accounts = Vec::new();
        while let Some(row) = rows.next().await? {
            let account_type_str: String = row.get(2)?;
            let account_type = match account_type_str.as_str() {
                "Savings" => freshcredit_types::AccountType::Savings,
                "Credit" => freshcredit_types::AccountType::Credit,
                "Investment" => freshcredit_types::AccountType::Investment,
                _ => freshcredit_types::AccountType::Checking,
            };

            let account = freshcredit_types::Account {
                id: row.get(0)?,
                user_id: row.get(1)?,
                account_type,
                balance: row.get(3)?,
                currency: row.get(4)?,
                institution_name: row.get(5)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String>(6)?)
                    .map_err(|e| anyhow::anyhow!("Failed to parse date: {e}"))?
                    .with_timezone(&chrono::Utc),
            };
            accounts.push(account);
        }

        Ok(accounts)
    }

    /// Get all transactions for a user
    /// P0-PERF: Limited to 10000 transactions to prevent memory exhaustion
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn empty_to_none(s: String) -> Option<String> {
        if s.is_empty() { None } else { Some(s) }
    }

    fn parse_transaction_date(s: &str) -> Result<chrono::DateTime<chrono::Utc>> {
        Ok(chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|e| anyhow::anyhow!("Failed to parse date: {e}"))?
            .with_timezone(&chrono::Utc))
    }

    pub async fn get_user_transactions(
        &self,
        user_id: &str,
    ) -> Result<Vec<freshcredit_types::Transaction>> {
        info!("Retrieving transactions for user: {user_id}");

        let mut rows = self
            .connection
            .query(
                "SELECT t.* FROM transactions t
             JOIN accounts a ON t.account_id = a.id
             WHERE a.user_id = ?
             ORDER BY t.date DESC LIMIT 10000",
                libsql::params![user_id],
            )
            .await?;

        let mut transactions = Vec::new();
        while let Some(row) = rows.next().await? {
            let transaction = freshcredit_types::Transaction {
                id: row.get(0)?,
                account_id: row.get(1)?,
                amount: row.get(2)?,
                currency: row.get(3)?,
                description: row.get(4)?,
                category: Self::empty_to_none(row.get::<String>(5)?),
                date: Self::parse_transaction_date(&row.get::<String>(6)?)?,
                merchant_name: Self::empty_to_none(row.get::<String>(7)?),
            };
            transactions.push(transaction);
        }

        Ok(transactions)
    }
}
