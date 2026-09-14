// TAG: surface=security owner=security-team rule=SEC-001
use super::*;
use chrono::{Duration, Utc};

#[test]
fn test_session_creation() {
    let session = Session::new(
        "user123".to_string(),
        Duration::minutes(30),
        Duration::hours(24),
    );

    assert_eq!(session.user_id, "user123");
    assert!(!session.is_expired());
    assert!(session.time_remaining() > Duration::zero());
}

#[test]
fn test_session_expiration() {
    let mut session = Session::new(
        "user123".to_string(),
        Duration::minutes(30),
        Duration::hours(24),
    );

    // Set expiration to the past
    session.expires_at = Utc::now() - Duration::hours(1);

    assert!(session.is_expired());
    assert!(session.time_remaining() < Duration::zero());
}
