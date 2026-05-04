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
    pub fn all() -> &'static [SchemaCategory] {
        &[
            SchemaCategory::Core,
            SchemaCategory::Financial,
            SchemaCategory::Identity,
            SchemaCategory::Plaid,
            SchemaCategory::Payments,
            SchemaCategory::Reports,
            SchemaCategory::Ticketing,
            SchemaCategory::Compliance,
            SchemaCategory::Notifications,
            SchemaCategory::HealthKit,
            SchemaCategory::LinkedIn,
            SchemaCategory::Ip,
            SchemaCategory::Publications,
            SchemaCategory::Correlation,
            SchemaCategory::AppleMusic,
            SchemaCategory::Platform,
            SchemaCategory::Teams,
            SchemaCategory::Customers,
            SchemaCategory::Ai,
            SchemaCategory::Workflow,
            SchemaCategory::Webhook,
            SchemaCategory::Security,
            SchemaCategory::Governance,
            SchemaCategory::Indexes,
        ]
    }

    /// Get the category name
    pub fn name(&self) -> &'static str {
        match self {
            SchemaCategory::Core => "core",
            SchemaCategory::Financial => "financial",
            SchemaCategory::Identity => "identity",
            SchemaCategory::Plaid => "plaid",
            SchemaCategory::Payments => "payments",
            SchemaCategory::Reports => "reports",
            SchemaCategory::Ticketing => "ticketing",
            SchemaCategory::Compliance => "compliance",
            SchemaCategory::Notifications => "notifications",
            SchemaCategory::HealthKit => "healthkit",
            SchemaCategory::LinkedIn => "linkedin",
            SchemaCategory::Ip => "ip",
            SchemaCategory::Publications => "publications",
            SchemaCategory::Correlation => "correlation",
            SchemaCategory::AppleMusic => "apple_music",
            SchemaCategory::Platform => "platform",
            SchemaCategory::Teams => "teams",
            SchemaCategory::Customers => "customers",
            SchemaCategory::Ai => "ai",
            SchemaCategory::Workflow => "workflow",
            SchemaCategory::Webhook => "webhook",
            SchemaCategory::Security => "security",
            SchemaCategory::Governance => "governance",
            SchemaCategory::Indexes => "indexes",
        }
    }
}
