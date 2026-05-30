// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
#[cfg(test)]
mod unit_tests {
    use crate::types::SchemaValidator;

    #[test]
    fn test_schema_validator_new() {
        let validator = SchemaValidator::new();
        assert!(validator.staging_connection.is_none());
        assert!(validator.local_connection.is_none());
        assert!(validator.cloud_connection.is_none());
    }

    #[test]
    fn test_schema_validator_default() {
        let validator: SchemaValidator = Default::default();
        assert!(validator.staging_connection.is_none());
        assert!(validator.local_connection.is_none());
        assert!(validator.cloud_connection.is_none());
    }
}
