/// Validates schema initialization by tracking which categories are initialized
/// ARCH-007: Schema validation tracking for startup validation logging
#[derive(Debug)]
#[allow(dead_code)]
pub struct SchemaValidation {
    /// Schema category name
    pub category: &'static str,
    /// Whether tables for this category were initialized
    pub tables_initialized: bool,
}
