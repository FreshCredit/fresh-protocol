//! AI Conversation database operations
//!
//! Manages AI chatbot conversation history and message storage.
//!
//! COMPLIANCE: §5 Observability, Logging, and Audit Requirements

use anyhow::Result;

use crate::{
    AiConversation,
    AiMessage,
    LocalClient,
};

impl LocalClient {
    /// Create a new conversation
    pub async fn create_conversation(
        &self,
        user_id: &str,
        title: Option<&str>,
        context: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        self.connection
            .execute(
                "INSERT INTO ai_conversations (id, user_id, title, context, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
                libsql::params![id.clone(), user_id, title, context, now.clone(), now],
            )
            .await?;

        Ok(id)
    }

    /// Get conversation by ID
    pub async fn get_conversation(&self, conversation_id: &str) -> Result<Option<AiConversation>> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, title, context, created_at, updated_at
             FROM ai_conversations WHERE id = ?",
                libsql::params![conversation_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(AiConversation {
                id: row.get(0)?,
                user_id: row.get(1)?,
                title: row.get(2)?,
                context: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get recent conversations for a user
    pub async fn get_user_conversations(
        &self,
        user_id: &str,
        limit: u32,
    ) -> Result<Vec<AiConversation>> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, title, context, created_at, updated_at
             FROM ai_conversations WHERE user_id = ?
             ORDER BY updated_at DESC LIMIT ?",
                libsql::params![user_id, limit as i64],
            )
            .await?;

        let mut conversations = Vec::new();
        while let Some(row) = rows.next().await? {
            conversations.push(AiConversation {
                id: row.get(0)?,
                user_id: row.get(1)?,
                title: row.get(2)?,
                context: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            });
        }
        Ok(conversations)
    }

    /// Add a message to a conversation
    pub async fn add_conversation_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        file_attachment_id: Option<&str>,
        tokens_used: Option<u32>,
        model: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        self.connection.execute(
            "INSERT INTO ai_messages (id, conversation_id, role, content, file_attachment_id, tokens_used, model, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                id.clone(),
                conversation_id,
                role,
                content,
                file_attachment_id,
                tokens_used.map(|t| t as i64),
                model,
                now.clone()
            ],
        ).await?;

        // Update conversation's updated_at timestamp
        self.connection
            .execute(
                "UPDATE ai_conversations SET updated_at = ? WHERE id = ?",
                libsql::params![now, conversation_id],
            )
            .await?;

        Ok(id)
    }

    /// Get messages for a conversation
    pub async fn get_conversation_messages(
        &self,
        conversation_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<AiMessage>> {
        // P0-SECURITY: Clamp limit to prevent injection and unreasonable queries
        let limit_val = limit.map(|l| l.clamp(1, 1000)).unwrap_or(100);

        // SECURITY FIX: Use parameterized query for LIMIT (libSQL supports this)
        let query = "SELECT id, conversation_id, role, content, file_attachment_id, tokens_used, model, created_at
             FROM ai_messages WHERE conversation_id = ?
             ORDER BY created_at ASC LIMIT ?";

        let mut rows = self
            .connection
            .query(query, libsql::params![conversation_id, limit_val])
            .await?;

        let mut messages = Vec::new();
        while let Some(row) = rows.next().await? {
            messages.push(AiMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                file_attachment_id: row.get(4)?,
                tokens_used: row.get::<Option<i64>>(5)?.map(|t| t as u32),
                model: row.get(6)?,
                created_at: row.get(7)?,
            });
        }
        Ok(messages)
    }

    /// Delete a conversation and all its messages
    pub async fn delete_conversation(&self, conversation_id: &str) -> Result<()> {
        self.connection
            .execute(
                "DELETE FROM ai_conversations WHERE id = ?",
                libsql::params![conversation_id],
            )
            .await?;
        Ok(())
    }

    /// Update conversation title (auto-generated from first message)
    pub async fn update_conversation_title(
        &self,
        conversation_id: &str,
        title: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .execute(
                "UPDATE ai_conversations SET title = ?, updated_at = ? WHERE id = ?",
                libsql::params![title, now, conversation_id],
            )
            .await?;
        Ok(())
    }
}
