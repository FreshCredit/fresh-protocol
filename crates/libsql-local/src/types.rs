//! Type definitions for database records
//!
//! This module contains struct definitions used for database operations.
//! Extracted from lib.rs as part of modular refactoring.
//!
//! Type categories:
//! - Core types: UserProfile, UserPreferences, SchemaValidationResult
//! - File types: UploadedFile, SaveUploadedFileParams
//! - AI types: AiConversation, AiMessage
//! - Scoring types: ScoringModelRecord (provider-defined, not FreshCredit-owned)
//! - Workflow types: WorkflowRecord
//! - Webhook types: WebhookEvent, WebhookEventCounts
//!
//! # Examples
//!
//! ```rust
//! use freshcredit_libsql_local::UserProfile;
//!
//! let profile = UserProfile {
//!     id: "user-123".to_string(),
//!     platform_user_id: "plat-456".to_string(),
//!     azure_id: "azure-789".to_string(),
//!     email: "user@example.com".to_string(),
//!     display_name: "John Doe".to_string(),
//!     given_name: Some("John".to_string()),
//!     family_name: Some("Doe".to_string()),
//!     surname: None,
//!     mobile_phone: None,
//!     job_title: None,
//!     street_address: None,
//!     city: None,
//!     state_province: None,
//!     postal_code: None,
//!     country_region: None,
//!     date_of_birth: None,
//!     ssn_last_four: None,
//!     employment_status: None,
//!     annual_income: None,
//!     role: "consumer".to_string(),
//!     is_admin: false,
//!     provider_onboarding_complete: false,
//!     tenant_id: "tenant-abc".to_string(),
//!     object_id: "obj-def".to_string(),
//!     verified_id_credential_id: None,
//!     verified_id_status: "not_verified".to_string(),
//!     verified_id_issued_at: None,
//!     created_at: "2025-01-01T00:00:00Z".to_string(),
//!     updated_at: "2025-01-01T00:00:00Z".to_string(),
//! };
//! ```

use serde::{Deserialize, Serialize};

// ============================================================================
// Core User Types
// ============================================================================

/// User profile for database storage (unified schema)
/// Combines Entra ID claims with extended profile and Verified ID fields
/// P0p: Added is_admin for first provider user admin rule (§27.4)
/// P0g: Added provider_onboarding_complete for nav visibility (§28.1)
/// ARCH-P2-001: Added phone_number, preferred_name, emergency_contact_name,
///              emergency_contact_phone, employer_name for web schema alignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub platform_user_id: String,
    pub azure_id: String,
    pub email: String,
    pub display_name: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub surname: Option<String>,
    pub mobile_phone: Option<String>,
    pub job_title: Option<String>,
    pub street_address: Option<String>,
    pub city: Option<String>,
    pub state_province: Option<String>,
    pub postal_code: Option<String>,
    pub country_region: Option<String>,
    pub date_of_birth: Option<String>,
    pub ssn_last_four: Option<String>,
    pub employment_status: Option<String>,
    pub annual_income: Option<i32>,
    // ARCH-P2-001: Extended profile fields for web schema alignment
    /// Alternative phone number (separate from mobile_phone)
    pub phone_number: Option<String>,
    /// User's preferred display name (nickname)
    pub preferred_name: Option<String>,
    /// Emergency contact full name
    pub emergency_contact_name: Option<String>,
    /// Emergency contact phone number
    pub emergency_contact_phone: Option<String>,
    /// Employer/company name
    pub employer_name: Option<String>,
    pub role: String,
    /// P0p: First provider user is admin by default (§27.4)
    #[serde(default)]
    pub is_admin: bool,
    /// P0g: Provider onboarding completion status (§28.1)
    #[serde(default)]
    pub provider_onboarding_complete: bool,
    pub tenant_id: String,
    pub object_id: String,
    pub verified_id_credential_id: Option<String>,
    pub verified_id_status: String,
    pub verified_id_issued_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Schema validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationResult {
    pub is_valid: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub checked_at: String,
}

/// User preferences for toggle states
/// P0g: Added onboarding dismissal fields for §27.3 onboarding flow rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub ai_agent_enabled: Option<bool>,
    pub ai_feedback_enabled: Option<bool>,
    pub ai_offers_enabled: Option<bool>,
    pub ai_lenders_enabled: Option<bool>,
    pub cloud_sync_enabled: Option<bool>,
    pub blockchain_enabled: Option<bool>,
    pub email_notifications_enabled: Option<bool>,
    pub kilt_did_enabled: Option<bool>,
    /// AI mode preference: "auto" (default), "cloud", or "local"
    pub ai_mode: Option<String>,
    /// Mock data mode for internal users testing flows
    /// When enabled, pages display prefilled mock data without database persistence
    /// Only available for @freshcredit.com internal team members
    pub mock_data_enabled: Option<bool>,
    /// P0g: Onboarding completed flag (§27.3)
    #[serde(default)]
    pub onboarding_completed: Option<bool>,
    /// P0g: Permanent dismissal flag - "Don't Show Again" (§27.3)
    #[serde(default)]
    pub onboarding_permanently_dismissed: Option<bool>,
    /// P0g: Reminder dismissal timestamp - "Remind Later" re-prompt after 7 days (§27.3)
    #[serde(default)]
    pub onboarding_reminder_dismissed_until: Option<String>,
    /// P0g: Plaid connection skipped flag (§27.3)
    #[serde(default)]
    pub plaid_connection_skipped: Option<bool>,
    /// P0g: Plaid reminder dismissal timestamp (§27.3)
    #[serde(default)]
    pub plaid_reminder_dismissed_until: Option<String>,
}

// ============================================================================
// File Upload Types
// ============================================================================

/// Uploaded file for AI multimodal input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedFile {
    pub id: String,
    pub user_id: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_data: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_analysis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Parameters for saving an uploaded file
///
/// Consolidates function arguments to avoid clippy::too_many_arguments
#[derive(Debug, Clone)]
pub struct SaveUploadedFileParams<'a> {
    pub user_id: &'a str,
    pub filename: &'a str,
    pub mime_type: &'a str,
    pub file_size: i64,
    pub file_data: Option<Vec<u8>>,
    pub text_content: Option<&'a str>,
    pub conversation_id: Option<&'a str>,
}

// ============================================================================
// AI Conversation Memory Types
// ============================================================================

/// AI Conversation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConversation {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub context: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// AI Message record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub file_attachment_id: Option<String>,
    pub tokens_used: Option<u32>,
    pub model: Option<String>,
    pub created_at: String,
}

// ============================================================================
// Scoring Model Types (BlockScore)
// NOTE: FreshCredit does NOT generate scores - providers define their own models
// ============================================================================

/// Provider-defined scoring model record
/// COMPLIANCE: §3 - Scoring logic is owned and defined by the provider, not FreshCredit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringModelRecord {
    pub id: String,
    pub user_id: String,
    pub provider_id: String,
    pub score_model_id: String,
    pub score_model_name: Option<String>,
    pub score_model_version: Option<String>,
    pub data_elements_used: Option<String>,
    pub data_element_weights: Option<String>,
    pub raw_score_data: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

// ============================================================================
// Workflow Types (Flow Builders)
// ============================================================================

/// Workflow record for flow builders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRecord {
    pub id: String,
    pub user_id: String,
    pub workflow_type: String,
    pub workflow_name: String,
    pub workflow_description: Option<String>,
    pub workflow_status: Option<String>,
    pub workflow_data: String,
    pub trigger_type: Option<String>,
    pub trigger_config: Option<String>,
    pub is_active: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub run_count: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

// ============================================================================
// Webhook Event Types (Outbox Pattern)
// ============================================================================

/// Webhook event record for outbox pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    pub user_id: Option<String>,
    pub provider: String,
    pub event_type: String,
    pub event_id: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub retry_count: i64,
    pub processed_at: Option<String>,
    pub created_at: String,
}

impl WebhookEvent {
    /// Create a new pending webhook event
    pub fn new_pending(
        provider: &str,
        event_type: &str,
        event_id: &str,
        payload: serde_json::Value,
        user_id: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            provider: provider.to_string(),
            event_type: event_type.to_string(),
            event_id: event_id.to_string(),
            payload,
            status: "pending".to_string(),
            retry_count: 0,
            processed_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Webhook event counts for admin dashboard
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebhookEventCounts {
    pub pending: u64,
    pub processing: u64,
    pub processed: u64,
    pub failed: u64,
    pub dead_letter: u64,
}

impl WebhookEventCounts {
    pub fn total(&self) -> u64 {
        self.pending + self.processing + self.processed + self.failed + self.dead_letter
    }
}

