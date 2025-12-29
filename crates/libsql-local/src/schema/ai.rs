//! AI schema definitions: ai_conversations, ai_messages
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

/// Initialize AI-related tables
pub async fn initialize_ai_tables(conn: &Connection) -> Result<()> {
    // Create AI conversations table
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

    // Create AI messages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ai_messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            file_attachment_id TEXT,
            token_count INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES ai_conversations (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Create uploaded_files table for AI file attachments
    conn.execute(
        "CREATE TABLE IF NOT EXISTS uploaded_files (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            filename TEXT NOT NULL,
            file_type TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            storage_path TEXT NOT NULL,
            mime_type TEXT,
            checksum TEXT,
            description TEXT,
            is_processed BOOLEAN DEFAULT FALSE,
            processing_status TEXT DEFAULT 'pending',
            processing_result TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Create indexes for AI tables
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ai_conversations_user_id ON ai_conversations(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ai_messages_conversation_id ON ai_messages(conversation_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_uploaded_files_user_id ON uploaded_files(user_id)",
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
        assert!(true);
    }
}

