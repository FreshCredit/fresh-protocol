//! Type definitions for database records
//!
//! This module contains struct definitions used for database operations.
//! Extracted from lib.rs as part of modular refactoring.
//!
// TAG: surface=database owner=platform-team rule=DB-001
//! Type categories:
//! - Core types: `UserProfile`, `UserPreferences`, `SchemaValidationResult`
//! - File types: `UploadedFile`, `SaveUploadedFileParams`
//! - AI types: `AiConversation`, `AiMessage`
//! - Scoring types: `ScoringModelRecord` (provider-defined, not FreshCredit-owned)
//! - Workflow types: `WorkflowRecord`
//! - Webhook types: `WebhookEvent`, `WebhookEventCounts`
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
//!     // ARCH-P2-001: Extended profile fields
//!     phone_number: None,
//!     preferred_name: None,
//!     emergency_contact_name: None,
// TAG: surface=database owner=platform-team rule=DB-001
//!     emergency_contact_phone: None,
//!     employer_name: None,
//!     role: "consumer".to_string(),
//!     is_admin: false,
//!     provider_onboarding_complete: false,
//!     mfa_enabled: false,
//!     mfa_verified_at: None,
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

// TAG: surface=database owner=platform-team rule=DB-001
/// User profile for database storage (unified schema)
///
/// Combines Entra ID claims with extended profile and Verified ID fields.
/// P0p: Added `is_admin` for first provider user admin rule (§27.4).
/// P0g: Added `provider_onboarding_complete` for nav visibility (§28.1).
/// ARCH-P2-001: Added `phone_number`, `preferred_name`, `emergency_contact_name`,
///              `emergency_contact_phone`, `employer_name` for web schema alignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// Unique user identifier
    pub id: String,
    /// Platform-specific user identifier
    pub platform_user_id: String,
    /// Microsoft Entra ID (Azure AD) object ID
    pub azure_id: String,
    /// Primary email address
    pub email: String,
    /// Display name for the user
    pub display_name: String,
    /// Given (first) name
    pub given_name: Option<String>,
    /// Family (last) name
    pub family_name: Option<String>,
    /// Surname (alternative last name field)
    pub surname: Option<String>,
    /// Mobile phone number
    pub mobile_phone: Option<String>,
    /// Job title or occupation
    pub job_title: Option<String>,
    /// Street address
    pub street_address: Option<String>,
    /// City
    pub city: Option<String>,
    /// State or province
    pub state_province: Option<String>,
    /// Postal or ZIP code
    pub postal_code: Option<String>,
    /// Country or region
    pub country_region: Option<String>,
    /// Date of birth (ISO 8601 format)
    pub date_of_birth: Option<String>,
    /// Last four digits of SSN
    pub ssn_last_four: Option<String>,
    /// Employment status
    pub employment_status: Option<String>,
    /// Annual income in whole dollars
    pub annual_income: Option<i32>,
    // ARCH-P2-001: Extended profile fields for web schema alignment
    /// Alternative phone number (separate from `mobile_phone`)
    pub phone_number: Option<String>,
    // TAG: surface=database owner=platform-team rule=DB-001
    /// User's preferred display name (nickname)
    pub preferred_name: Option<String>,
    /// Emergency contact full name
    pub emergency_contact_name: Option<String>,
    /// Emergency contact phone number
    pub emergency_contact_phone: Option<String>,
    /// Employer/company name
    pub employer_name: Option<String>,
    /// User role (e.g., consumer, provider, admin)
    pub role: String,
    /// P0p: First provider user is admin by default (§27.4)
    #[serde(default)]
    pub is_admin: bool,
    /// P0g: Provider onboarding completion status (§28.1)
    #[serde(default)]
    pub provider_onboarding_complete: bool,
    /// COMPLIANCE: MFA enabled flag for admin users
    #[serde(default)]
    pub mfa_enabled: bool,
    /// COMPLIANCE: Timestamp of last MFA verification
    pub mfa_verified_at: Option<String>,
    /// Microsoft Entra tenant ID
    pub tenant_id: String,
    /// Microsoft Entra object ID
    pub object_id: String,
    /// Verified ID credential identifier
    pub verified_id_credential_id: Option<String>,
    /// Verified ID verification status
    pub verified_id_status: String,
    /// Timestamp when Verified ID was issued
    pub verified_id_issued_at: Option<String>,
    /// Consumer side Verified ID credential identifier
    pub consumer_verified_id_credential_id: Option<String>,
    /// Consumer side Verified ID verification status
    #[serde(default)]
    pub consumer_verified_id_status: String,
    /// Timestamp when consumer side Verified ID was issued
    pub consumer_verified_id_issued_at: Option<String>,
    /// Provider side Verified ID credential identifier
    pub provider_verified_id_credential_id: Option<String>,
    /// Provider side Verified ID verification status
    #[serde(default)]
    pub provider_verified_id_status: String,
    /// Timestamp when provider side Verified ID was issued
    pub provider_verified_id_issued_at: Option<String>,
    /// Record creation timestamp
    pub created_at: String,
    /// Record last update timestamp
    pub updated_at: String,
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Schema validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationResult {
    /// Whether the schema validation passed
    pub is_valid: bool,
    /// List of validation errors
    pub issues: Vec<String>,
    /// List of validation warnings
    pub warnings: Vec<String>,
    /// Timestamp when validation was performed
    pub checked_at: String,
}

/// User preferences for toggle states
/// P0g: Added onboarding dismissal fields for §27.3 onboarding flow rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    /// Whether AI agent features are enabled
    pub ai_agent_enabled: Option<bool>,
    /// Whether AI feedback collection is enabled
    pub ai_feedback_enabled: Option<bool>,
    /// Whether AI offer matching is enabled
    pub ai_offers_enabled: Option<bool>,
    /// Whether AI lender suggestions are enabled
    pub ai_lenders_enabled: Option<bool>,
    /// Whether cloud data sync is enabled
    pub cloud_sync_enabled: Option<bool>,
    /// Whether blockchain features are enabled
    pub blockchain_enabled: Option<bool>,
    /// Whether email notifications are enabled
    pub email_notifications_enabled: Option<bool>,
    /// Whether KILT DID integration is enabled
    pub kilt_did_enabled: Option<bool>,
    /// AI mode preference: "auto" (default), "cloud", or "local"
    pub ai_mode: Option<String>,
    // TAG: surface=database owner=platform-team rule=DB-001
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
    /// Whether the user acknowledged the auto-derived vault encryption key
    /// during onboarding (secure → key → backup → connect flow)
    #[serde(default)]
    pub vault_key_acknowledged: Option<bool>,
    /// Whether the user made an explicit cloud-backup (sync) choice during
    /// onboarding — distinct from `cloud_sync_enabled`, which defaults ON and
    /// therefore cannot record that a choice happened at all
    #[serde(default)]
    pub backup_sync_chosen: Option<bool>,
}

// ============================================================================
// File Upload Types
// ============================================================================

// TAG: surface=database owner=platform-team rule=DB-001
/// Uploaded file for AI multimodal input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedFile {
    /// Unique file identifier
    pub id: String,
    /// User who uploaded the file
    pub user_id: String,
    /// Original filename
    pub filename: String,
    /// MIME type of the file
    pub mime_type: String,
    /// File size in bytes
    pub file_size: i64,
    /// Raw file data bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_data: Option<Vec<u8>>,
    /// Extracted text content (for supported formats)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_content: Option<String>,
    /// AI-generated analysis of the file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_analysis: Option<String>,
    /// Associated AI conversation ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    /// Upload timestamp
    pub created_at: String,
    /// Expiration timestamp for temporary files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Parameters for saving an uploaded file
///
/// Consolidates function arguments to avoid `clippy::too_many_arguments`
#[derive(Debug, Clone)]
pub struct SaveUploadedFileParams<'a> {
    /// User who uploaded the file
    pub user_id: &'a str,
    /// Original filename
    pub filename: &'a str,
    /// MIME type of the file
    pub mime_type: &'a str,
    /// File size in bytes
    pub file_size: i64,
    /// Raw file data bytes
    pub file_data: Option<Vec<u8>>,
    /// Extracted text content
    pub text_content: Option<&'a str>,
    /// Associated AI conversation ID
    pub conversation_id: Option<&'a str>,
    /// Optional RFC 3339 expiration timestamp. Defaults to 24 hours if omitted.
    pub expires_at: Option<&'a str>,
}

// ============================================================================
// AI Conversation Memory Types
// ============================================================================

/// AI Conversation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConversation {
    /// Unique conversation identifier
    pub id: String,
    /// User who owns the conversation
    pub user_id: String,
    /// Conversation title (optional)
    pub title: Option<String>,
    /// Conversation context or system prompt
    pub context: Option<String>,
    /// Creation timestamp
    pub created_at: String,
    /// Last update timestamp
    pub updated_at: String,
}

// TAG: surface=database owner=platform-team rule=DB-001
/// AI Message record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    /// Unique message identifier
    pub id: String,
    /// Parent conversation ID
    pub conversation_id: String,
    /// Message role (user, assistant, system)
    pub role: String,
    /// Message content
    pub content: String,
    /// Attached file identifier
    pub file_attachment_id: Option<String>,
    /// Number of tokens used for this message
    pub tokens_used: Option<u32>,
    /// AI model identifier used
    pub model: Option<String>,
    /// Message timestamp
    pub created_at: String,
}

// ============================================================================
// Scoring Model Types (BlockScore)
// NOTE: FreshCredit does NOT generate scores - providers define their own models
// ============================================================================

/// Provider-defined scoring model record
/// COMPLIANCE: §3 - Scoring logic is owned and defined by the provider, not `FreshCredit`
#[derive(Debug, Clone, Serialize, Deserialize)]
// TAG: surface=database owner=platform-team rule=GENERAL-001
pub struct ScoringModelRecord {
    /// Unique record identifier
    pub id: String,
    /// User this score belongs to
    pub user_id: String,
    /// Provider who calculated the score
    pub provider_id: String,
    /// Provider's score model identifier
    pub score_model_id: String,
    /// Human-readable score model name
    pub score_model_name: Option<String>,
    /// Score model version
    pub score_model_version: Option<String>,
    /// Data elements used in scoring
    pub data_elements_used: Option<String>,
    /// Weights applied to data elements
    pub data_element_weights: Option<String>,
    /// Raw score response data from provider
    pub raw_score_data: String,
    /// Record creation timestamp
    pub created_at: Option<String>,
    /// Record last update timestamp
    pub updated_at: Option<String>,
}

// ============================================================================
// Workflow Types (Flow Builders)
// ============================================================================

// TAG: surface=database owner=platform-team rule=DB-001
/// Workflow record for flow builders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRecord {
    /// Unique workflow identifier
    pub id: String,
    /// User who owns the workflow
    pub user_id: String,
    /// Type of workflow (e.g., automation, integration)
    pub workflow_type: String,
    /// Human-readable workflow name
    pub workflow_name: String,
    /// Workflow description
    pub workflow_description: Option<String>,
    /// Current workflow status
    pub workflow_status: Option<String>,
    /// Serialized workflow definition data
    pub workflow_data: String,
    /// Trigger type (e.g., scheduled, event, manual)
    pub trigger_type: Option<String>,
    /// Serialized trigger configuration
    pub trigger_config: Option<String>,
    /// Whether the workflow is currently active
    pub is_active: bool,
    /// Timestamp of last execution
    pub last_run_at: Option<String>,
    /// Scheduled next run timestamp
    pub next_run_at: Option<String>,
    /// Number of times the workflow has run
    pub run_count: Option<i64>,
    /// Record creation timestamp
    pub created_at: Option<String>,
    /// Record last update timestamp
    pub updated_at: Option<String>,
}

// ============================================================================
// Webhook Event Types (Outbox Pattern)
// ============================================================================

// TAG: surface=database owner=platform-team rule=DB-001
/// Webhook event record for outbox pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    /// Unique event identifier
    pub id: String,
    /// Associated user ID (if applicable)
    pub user_id: Option<String>,
    /// Provider that sent the webhook
    pub provider: String,
    /// Webhook event type
    pub event_type: String,
    /// Provider's event identifier
    pub event_id: String,
    /// Event payload data
    pub payload: serde_json::Value,
    /// Processing status (pending, processing, processed, failed)
    pub status: String,
    /// Number of delivery retry attempts
    pub retry_count: i64,
    /// Timestamp when event was processed
    pub processed_at: Option<String>,
    /// Event receipt timestamp
    pub created_at: String,
}

// TAG: surface=database owner=platform-team rule=DB-001
impl WebhookEvent {
    /// Create a new pending webhook event
    #[must_use]
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
    /// Number of events waiting to be processed
    pub pending: u64,
    /// Number of events currently being processed
    pub processing: u64,
    /// Number of successfully processed events
    pub processed: u64,
    /// Number of events that failed processing
    pub failed: u64,
    /// Number of events moved to dead letter queue
    pub dead_letter: u64,
}

impl WebhookEventCounts {
    /// Total number of webhook events across all statuses
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.pending + self.processing + self.processed + self.failed + self.dead_letter
    }
}
