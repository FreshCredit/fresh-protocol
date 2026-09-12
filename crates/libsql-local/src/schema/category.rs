//! Schema category enum

// TAG: surface=database owner=platform-team rule=DB-001
/// Schema category for organizing table initialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaCategory {
    /// Core user and authentication tables
    Core,
    /// Financial account and transaction tables
    Financial,
    /// Identity verification tables
    Identity,
    /// Plaid product tables
    Plaid,
    /// Payment processing tables
    Payments,
    /// Reports and scoring tables
    Reports,
    /// Ticketing system tables
    Ticketing,
    /// Compliance monitoring tables
    Compliance,
    /// Notification tables
    Notifications,
    /// Apple `HealthKit` tables
    HealthKit,
    /// `LinkedIn` professional data tables
    LinkedIn,
    /// IP address tracking tables
    // TAG: surface=database owner=platform-team rule=GENERAL-001
    Ip,
    /// Publication and research tables
    Publications,
    /// Correlation engine tables
    Correlation,
    /// Apple Music tables
    AppleMusic,
    /// Platform-level tables
    Platform,
    /// Team management tables
    Teams,
    /// Customer data tables
    Customers,
    /// AI conversation and file tables
    Ai,
    /// Workflow automation tables
    Workflow,
    /// Webhook event tables
    Webhook,
    /// Security monitoring tables
    Security,
    /// Governance and policy tables
    Governance,
    /// Database index definitions
    Indexes,
}

// TAG: surface=database owner=platform-team rule=DB-001
impl SchemaCategory {
    /// Get all schema categories
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Core,
            Self::Financial,
            Self::Identity,
            Self::Plaid,
            Self::Payments,
            Self::Reports,
            Self::Ticketing,
            Self::Compliance,
            Self::Notifications,
            Self::HealthKit,
            Self::LinkedIn,
            Self::Ip,
            Self::Publications,
            Self::Correlation,
            Self::AppleMusic,
            Self::Platform,
            Self::Teams,
            Self::Customers,
            Self::Ai,
            Self::Workflow,
            Self::Webhook,
            Self::Security,
            Self::Governance,
            Self::Indexes,
        ]
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get the category name
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Financial => "financial",
            Self::Identity => "identity",
            Self::Plaid => "plaid",
            Self::Payments => "payments",
            Self::Reports => "reports",
            Self::Ticketing => "ticketing",
            Self::Compliance => "compliance",
            Self::Notifications => "notifications",
            Self::HealthKit => "healthkit",
            Self::LinkedIn => "linkedin",
            Self::Ip => "ip",
            Self::Publications => "publications",
            Self::Correlation => "correlation",
            Self::AppleMusic => "apple_music",
            Self::Platform => "platform",
            Self::Teams => "teams",
            Self::Customers => "customers",
            Self::Ai => "ai",
            Self::Workflow => "workflow",
            Self::Webhook => "webhook",
            Self::Security => "security",
            Self::Governance => "governance",
            Self::Indexes => "indexes",
        }
    }
}
