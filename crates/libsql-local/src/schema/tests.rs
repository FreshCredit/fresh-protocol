use super::*;

#[test]
fn test_schema_category_all() {
    let categories = SchemaCategory::all();
    assert_eq!(categories.len(), 24);
}

#[test]
fn test_schema_category_names() {
    assert_eq!(SchemaCategory::Core.name(), "core");
    assert_eq!(SchemaCategory::Financial.name(), "financial");
    assert_eq!(SchemaCategory::Plaid.name(), "plaid");
    assert_eq!(SchemaCategory::Payments.name(), "payments");
    assert_eq!(SchemaCategory::Reports.name(), "reports");
    assert_eq!(SchemaCategory::Publications.name(), "publications");
    assert_eq!(SchemaCategory::Indexes.name(), "indexes");
}

#[test]
fn test_schema_category_equality() {
    assert_eq!(SchemaCategory::Core, SchemaCategory::Core);
    assert_ne!(SchemaCategory::Core, SchemaCategory::Financial);
}
