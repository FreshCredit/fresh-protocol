//! AI schema definitions: `ai_conversations`, `ai_messages`, `uploaded_files`, `ai_usage_metrics`,
//! `ai_request_logs`, `ai_feedback`, `ai_model_configs`
//!
//! Tables in this module:
//! - `ai_conversations`: AI chat conversation records
//! - `ai_messages`: Individual messages within conversations
//! - `uploaded_files`: File attachments for AI multimodal input
//! - `ai_usage_metrics`: Daily aggregated token/cost metrics (replaces in-memory `AiMetrics`)
//! - `ai_request_logs`: Individual request logging for debugging
//! - `ai_feedback`: User feedback collection for model improvement
//! - `ai_model_configs`: Multi-model configuration and pricing
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

// TAG: surface=database owner=platform-team rule=DB-001
use anyhow::Result;

use super::try_create_index;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize AI-related tables
///
/// Creates 7 tables: `ai_conversations`, `ai_messages`, `uploaded_files`,
/// `ai_usage_metrics`, `ai_request_logs`, `ai_feedback`, `ai_model_configs`
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_ai_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing ai tables");
    create_uploaded_files_table(conn).await?;
    create_ai_conversations_table(conn).await?;
    create_ai_messages_table(conn).await?;
    create_ai_usage_metrics_table(conn).await?;
    create_ai_request_logs_table(conn).await?;
    create_ai_feedback_table(conn).await?;
    create_ai_model_configs_table(conn).await?;
    create_ai_indexes(conn).await?;
    Ok(())
}

async fn create_uploaded_files_table(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS uploaded_files (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            filename TEXT NOT NULL,
            file_type TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            mime_type TEXT NOT NULL,
            storage_path TEXT,
            file_hash TEXT,
            file_data BLOB,
            text_content TEXT,
            ai_analysis TEXT,
            is_encrypted BOOLEAN DEFAULT FALSE,
            encryption_key_id TEXT,
            metadata TEXT,
            conversation_id TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;
    Ok(())
}

async fn create_ai_conversations_table(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ai_conversations (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            title TEXT,
            context TEXT,
            model TEXT DEFAULT 'gemini-pro',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;
    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
async fn create_ai_messages_table(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ai_messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            file_attachment_id TEXT,
            tokens_used INTEGER,
            model TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES ai_conversations(id) ON DELETE CASCADE
        )",
    )
    .await?;
    Ok(())
}

async fn create_ai_usage_metrics_table(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ai_usage_metrics (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            metric_date DATE NOT NULL,
            model TEXT NOT NULL DEFAULT 'gemini-2.0-flash',
            total_tokens INTEGER DEFAULT 0,
            input_tokens INTEGER DEFAULT 0,
            output_tokens INTEGER DEFAULT 0,
            successful_requests INTEGER DEFAULT 0,
            failed_requests INTEGER DEFAULT 0,
            rate_limited_requests INTEGER DEFAULT 0,
            fallback_requests INTEGER DEFAULT 0,
            estimated_cost_micro_usd INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;
    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
async fn create_ai_request_logs_table(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ai_request_logs (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            conversation_id TEXT,
            model TEXT NOT NULL,
            request_type TEXT NOT NULL,
            input_tokens INTEGER,
            output_tokens INTEGER,
            latency_ms INTEGER,
            status TEXT NOT NULL,
            error_code TEXT,
            error_message TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (conversation_id) REFERENCES ai_conversations (id) ON DELETE SET NULL
        )",
    )
    .await?;
    Ok(())
}

async fn create_ai_feedback_table(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ai_feedback (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            conversation_id TEXT,
            message_id TEXT,
            feedback_type TEXT NOT NULL,
            feedback_reason TEXT,
            original_response TEXT,
            corrected_response TEXT,
            metadata TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (conversation_id) REFERENCES ai_conversations (id) ON DELETE SET NULL,
            FOREIGN KEY (message_id) REFERENCES ai_messages (id) ON DELETE SET NULL
        )",
    )
    .await?;
    Ok(())
}

async fn create_ai_model_configs_table(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ai_model_configs (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            model_name TEXT NOT NULL,
            model_provider TEXT NOT NULL,
            is_default INTEGER DEFAULT 0,
            is_enabled INTEGER DEFAULT 1,
            max_tokens INTEGER,
            temperature REAL,
            input_cost_per_million_tokens INTEGER,
            output_cost_per_million_tokens INTEGER,
            rate_limit_rpm INTEGER,
            config_json TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;
    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
const AI_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_uploaded_files_user_id ON uploaded_files(user_id)",
    "CREATE INDEX IF NOT EXISTS idx_uploaded_files_conversation ON uploaded_files(conversation_id)",
    "CREATE INDEX IF NOT EXISTS idx_ai_conversations_user_id ON ai_conversations(user_id)",
    "CREATE INDEX IF NOT EXISTS idx_ai_messages_conversation ON ai_messages(conversation_id)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_usage_metrics_user_date_model ON ai_usage_metrics(user_id, metric_date, model)",
    "CREATE INDEX IF NOT EXISTS idx_ai_usage_metrics_date ON ai_usage_metrics(metric_date)",
    "CREATE INDEX IF NOT EXISTS idx_ai_request_logs_user_id ON ai_request_logs(user_id)",
    "CREATE INDEX IF NOT EXISTS idx_ai_request_logs_created_at ON ai_request_logs(created_at)",
    "CREATE INDEX IF NOT EXISTS idx_ai_request_logs_status ON ai_request_logs(status)",
    "CREATE INDEX IF NOT EXISTS idx_ai_feedback_user_id ON ai_feedback(user_id)",
    "CREATE INDEX IF NOT EXISTS idx_ai_feedback_type ON ai_feedback(feedback_type)",
    "CREATE INDEX IF NOT EXISTS idx_ai_feedback_created_at ON ai_feedback(created_at)",
    "CREATE INDEX IF NOT EXISTS idx_ai_model_configs_user_id ON ai_model_configs(user_id)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_model_configs_user_model ON ai_model_configs(user_id, model_name)",
];

async fn create_ai_indexes(conn: &Connection) -> Result<()> {
    for sql in AI_INDEXES {
        try_create_index(conn, sql).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_ai_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        let _ = 1 + 1; // Compile-time verification
    }
}
