/// Validates schema initialization by tracking which categories are initialized
/// ARCH-007: Schema validation tracking for startup validation logging
#[derive(Debug)]
#[allow(dead_code)]
pub struct SchemaValidation {
    pub category: &'static str,
    pub tables_initialized: bool,
}
