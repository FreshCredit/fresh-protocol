//! Webhook event database operations
//!
//! Implements the transactional outbox pattern for reliable webhook delivery.
//! Events are stored in the database and processed asynchronously.
//!
//! COMPLIANCE: §5 Observability, Logging, and Audit Requirements

use anyhow::Result;
use tracing::info;

use crate::{
    LocalClient,
    WebhookEvent,
    WebhookEventCounts,
};

impl LocalClient {
    /// Store a webhook event for later processing (outbox pattern)
    pub async fn store_webhook_event(&self, event: &WebhookEvent) -> Result<bool> {
        info!(
            "Storing webhook event: {} from {}",
            event.event_id, event.provider
        );

        let now = chrono::Utc::now().to_rfc3339();
        let payload_json = serde_json::to_string(&event.payload)?;

        // Use INSERT OR IGNORE for idempotency via event_id UNIQUE constraint
        let affected = self.connection.execute(
            "INSERT OR IGNORE INTO webhook_events (id, user_id, provider, event_type, event_id, payload, status, retry_count, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                event.id.clone(),
                event.user_id.clone(),
                event.provider.clone(),
                event.event_type.clone(),
                event.event_id.clone(),
                payload_json,
                event.status.clone(),
                event.retry_count,
                now
            ],
        ).await?;

        Ok(affected > 0)
    }

    /// Get pending webhook events for processing
    pub async fn get_pending_webhook_events(&self, limit: u32) -> Result<Vec<WebhookEvent>> {
        info!("Getting pending webhook events (limit: {})", limit);

        let mut rows = self.connection.query(
            "SELECT id, user_id, provider, event_type, event_id, payload, status, retry_count, processed_at, created_at
             FROM webhook_events
             WHERE status = 'pending'
             ORDER BY created_at ASC
             LIMIT ?",
            libsql::params![limit as i64],
        ).await?;

        let mut events = Vec::new();
        while let Some(row) = rows.next().await? {
            let payload_str: String = row.get(5)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

            events.push(WebhookEvent {
                id: row.get(0)?,
                user_id: row.get::<Option<String>>(1)?,
                provider: row.get(2)?,
                event_type: row.get(3)?,
                event_id: row.get(4)?,
                payload,
                status: row.get(6)?,
                retry_count: row.get(7)?,
                processed_at: row.get::<Option<String>>(8)?,
                created_at: row.get(9)?,
            });
        }

        Ok(events)
    }

    /// Update webhook event status after processing
    pub async fn update_webhook_status(
        &self,
        event_id: &str,
        status: &str,
        increment_retry: bool,
    ) -> Result<bool> {
        info!("Updating webhook status: {} -> {}", event_id, status);

        let now = chrono::Utc::now().to_rfc3339();
        let processed_at = if status == "processed" || status == "failed" {
            Some(now.clone())
        } else {
            None
        };

        let affected = if increment_retry {
            self.connection.execute(
                "UPDATE webhook_events SET status = ?, retry_count = retry_count + 1, processed_at = ? WHERE event_id = ?",
                libsql::params![status, processed_at, event_id],
            ).await?
        } else {
            self.connection
                .execute(
                    "UPDATE webhook_events SET status = ?, processed_at = ? WHERE event_id = ?",
                    libsql::params![status, processed_at, event_id],
                )
                .await?
        };

        Ok(affected > 0)
    }

    /// Get webhook events that need retry (failed with retry_count < max_retries)
    pub async fn get_webhook_events_for_retry(
        &self,
        max_retries: u32,
    ) -> Result<Vec<WebhookEvent>> {
        info!(
            "Getting webhook events for retry (max_retries: {})",
            max_retries
        );

        // P0-PERF: Limited to 1000 events to prevent memory exhaustion
        let mut rows = self.connection.query(
            "SELECT id, user_id, provider, event_type, event_id, payload, status, retry_count, processed_at, created_at
             FROM webhook_events
             WHERE status = 'failed' AND retry_count < ?
             ORDER BY created_at ASC LIMIT 1000",
            libsql::params![max_retries as i64],
        ).await?;

        let mut events = Vec::new();
        while let Some(row) = rows.next().await? {
            let payload_str: String = row.get(5)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

            events.push(WebhookEvent {
                id: row.get(0)?,
                user_id: row.get::<Option<String>>(1)?,
                provider: row.get(2)?,
                event_type: row.get(3)?,
                event_id: row.get(4)?,
                payload,
                status: row.get(6)?,
                retry_count: row.get(7)?,
                processed_at: row.get::<Option<String>>(8)?,
                created_at: row.get(9)?,
            });
        }

        Ok(events)
    }

    /// Move webhook to dead letter queue (max retries exceeded)
    pub async fn move_webhook_to_dead_letter(&self, event_id: &str) -> Result<bool> {
        info!("Moving webhook to dead letter: {}", event_id);

        let now = chrono::Utc::now().to_rfc3339();
        let affected = self.connection.execute(
            "UPDATE webhook_events SET status = 'dead_letter', processed_at = ? WHERE event_id = ?",
            libsql::params![now, event_id],
        ).await?;

        Ok(affected > 0)
    }

    /// Get webhook event by event_id
    pub async fn get_webhook_event(&self, event_id: &str) -> Result<Option<WebhookEvent>> {
        let mut rows = self.connection.query(
            "SELECT id, user_id, provider, event_type, event_id, payload, status, retry_count, processed_at, created_at
             FROM webhook_events WHERE event_id = ?",
            libsql::params![event_id],
        ).await?;

        if let Some(row) = rows.next().await? {
            let payload_str: String = row.get(5)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

            Ok(Some(WebhookEvent {
                id: row.get(0)?,
                user_id: row.get::<Option<String>>(1)?,
                provider: row.get(2)?,
                event_type: row.get(3)?,
                event_id: row.get(4)?,
                payload,
                status: row.get(6)?,
                retry_count: row.get(7)?,
                processed_at: row.get::<Option<String>>(8)?,
                created_at: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get recent webhook events for admin dashboard
    pub async fn get_recent_webhook_events(&self, limit: u32) -> Result<Vec<WebhookEvent>> {
        let mut rows = self.connection.query(
            "SELECT id, user_id, provider, event_type, event_id, payload, status, retry_count, processed_at, created_at
             FROM webhook_events
             ORDER BY created_at DESC
             LIMIT ?",
            libsql::params![limit as i64],
        ).await?;

        let mut events = Vec::new();
        while let Some(row) = rows.next().await? {
            let payload_str: String = row.get(5)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

            events.push(WebhookEvent {
                id: row.get(0)?,
                user_id: row.get::<Option<String>>(1)?,
                provider: row.get(2)?,
                event_type: row.get(3)?,
                event_id: row.get(4)?,
                payload,
                status: row.get(6)?,
                retry_count: row.get(7)?,
                processed_at: row.get::<Option<String>>(8)?,
                created_at: row.get(9)?,
            });
        }

        Ok(events)
    }

    /// Get webhook event counts by status for admin dashboard
    pub async fn get_webhook_event_counts(&self) -> Result<WebhookEventCounts> {
        let mut rows = self
            .connection
            .query(
                "SELECT status, COUNT(*) as count FROM webhook_events GROUP BY status",
                libsql::params![],
            )
            .await?;

        let mut counts = WebhookEventCounts::default();
        while let Some(row) = rows.next().await? {
            let status: String = row.get(0)?;
            let count: i64 = row.get(1)?;

            match status.as_str() {
                "pending" => counts.pending = count as u64,
                "processing" => counts.processing = count as u64,
                "processed" => counts.processed = count as u64,
                "failed" => counts.failed = count as u64,
                "dead_letter" => counts.dead_letter = count as u64,
                _ => {}
            }
        }

        Ok(counts)
    }
}
