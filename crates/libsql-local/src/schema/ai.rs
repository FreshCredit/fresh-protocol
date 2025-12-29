//! AI schema definitions: ai_conversations, ai_messages, uploaded_files
//!
//! Tables in this module:
//! - ai_conversations: AI chat conversation records
//! - ai_messages: Individual messages within conversations
//! - uploaded_files: File attachments for AI multimodal input
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

/// Initialize AI-related tables
///
/// Creates 3 tables: ai_conversations, ai_messages, uploaded_files
pub async fn initialize_ai_tables(conn: &Connection) -> Result<()> {
    // Create uploaded_files table for AI multimodal input
    // NOTE: Created before ai_conversations to allow foreign key reference
    conn.execute(
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
        (),
    )
    .await?;

    // Create AI conversations table for conversation memory
    conn.execute(
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
        (),
    )
    .await?;

    // Create AI messages table for conversation history
    conn.execute(
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
        (),
    )
    .await?;

    // Create indexes for AI tables
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_uploaded_files_user_id ON uploaded_files(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_uploaded_files_conversation ON uploaded_files(conversation_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ai_conversations_user_id ON ai_conversations(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ai_messages_conversation ON ai_messages(conversation_id)",
        (),
    )
    .await?;

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

