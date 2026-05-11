//! Schema category enum

/// Schema category for organizing table initialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaCategory {
    Core,
    Financial,
    Identity,
    Plaid,
    Payments,
    Reports,
    Ticketing,
    Compliance,
    Notifications,
    HealthKit,
    LinkedIn,
    Ip,
    Publications,
    Correlation,
    AppleMusic,
    Platform,
    Teams,
    Customers,
    Ai,
    Workflow,
    Webhook,
    Security,
    Governance,
    Indexes,
}

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
