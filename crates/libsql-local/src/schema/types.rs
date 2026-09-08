// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
/// Validates schema initialization by tracking which categories are initialized
/// ARCH-007: Schema validation tracking for startup validation logging
#[derive(Debug)]

pub struct SchemaValidation {
    /// Schema category name
    pub category: &'static str,
    /// Whether tables for this category were initialized
    pub tables_initialized: bool,
}
