// TAG: surface=api owner=platform-team rule=API-001
//! ID generation utilities for `FreshCredit`
//!
//! This module provides consistent ID generation patterns across the codebase.
//! All ID generation should use these utilities rather than direct UUID calls.

use uuid::Uuid;

/// Generate a new UUID v4 as a string
///
/// Use this for general-purpose unique identifiers.
#[inline]
#[must_use]
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Generate a prefixed ID with a given prefix
///
/// Example: `prefixed_id("rpt")` returns `"rpt_550e8400-e29b-41d4-a716-446655440000"`
#[inline]
#[must_use]
pub fn prefixed_id(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4())
}

/// Generate a session ID
#[inline]
#[must_use]
pub fn session_id() -> String {
    prefixed_id("session")
}

/// Generate a request ID
#[inline]
#[must_use]
pub fn request_id() -> String {
    prefixed_id("req")

    // TAG: surface=api owner=platform-team rule=API-001
}

/// Generate a report ID
#[inline]
#[must_use]
pub fn report_id() -> String {
    prefixed_id("rpt")
}

/// Generate a payment method ID
#[inline]
#[must_use]
pub fn payment_method_id(processor: &str) -> String {
    format!("pm_{}_{}", processor, Uuid::new_v4())
}

/// Generate a transaction ID
#[inline]
#[must_use]
pub fn transaction_id() -> String {
    prefixed_id("txn")
}

/// Generate a workflow ID
#[inline]
#[must_use]
pub fn workflow_id() -> String {
    prefixed_id("wf")
}

/// Generate a model ID (for `BlockScore` models)
#[inline]
#[must_use]
pub fn model_id() -> String {
    prefixed_id("model")
}

// TAG: surface=api owner=platform-team rule=API-001
/// Generate an item ID (for Plaid items)
#[inline]
#[must_use]
pub fn item_id() -> String {
    prefixed_id("item")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_id() {
        let id = new_id();
        assert!(!id.is_empty());
        // UUID v4 format: 8-4-4-4-12
        assert_eq!(id.len(), 36);
    }

    #[test]
    fn test_prefixed_id() {
        let id = prefixed_id("test");
        assert!(id.starts_with("test_"));
        assert_eq!(id.len(), 5 + 36); // prefix + underscore + UUID
    }

    #[test]
    fn test_session_id() {
        let id = session_id();
        assert!(id.starts_with("session_"));
    }

    #[test]
    fn test_request_id() {
        let id = request_id();
        assert!(id.starts_with("req_"));
        // TAG: surface=api owner=platform-team rule=API-001
    }

    #[test]
    fn test_report_id() {
        let id = report_id();
        assert!(id.starts_with("rpt_"));
    }

    #[test]
    fn test_payment_method_id() {
        let id = payment_method_id("stripe");
        assert!(id.starts_with("pm_stripe_"));
    }

    #[test]
    fn test_transaction_id() {
        let id = transaction_id();
        assert!(id.starts_with("txn_"));
    }

    #[test]
    fn test_workflow_id() {
        let id = workflow_id();
        assert!(id.starts_with("wf_"));
    }

    #[test]
    fn test_model_id() {
        let id = model_id();
        assert!(id.starts_with("model_"));
    }

    #[test]
    fn test_item_id() {
        let id = item_id();
        assert!(id.starts_with("item_"));
    }
    // TAG: surface=api owner=platform-team rule=API-001
}
