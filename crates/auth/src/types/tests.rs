use super::*;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_authenticated_user_new() {
    let user = AuthenticatedUser::new("user-123", "test@example.com", "Test User", "entra-456");

    // TAG: surface=auth owner=security-team rule=RBAC-001
    assert_eq!(user.user_id, "user-123");
    assert_eq!(user.email, "test@example.com");
    assert_eq!(user.name, "Test User");
    assert_eq!(user.entra_id, "entra-456");
    assert!(user.given_name.is_none());
}

#[test]
fn test_display_name_fallback() {
    let user_with_name = AuthenticatedUser::new("1", "test@example.com", "Test User", "e1");
    assert_eq!(user_with_name.display_name(), "Test User");

    let user_empty_name = AuthenticatedUser::new("1", "test@example.com", "", "e1");
    assert_eq!(user_empty_name.display_name(), "test@example.com");
}

#[test]
fn test_auth_error_status_codes() {
    assert_eq!(
        AuthError::MissingAuthorization.status_code(),
        http::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        AuthError::InvalidTokenFormat.status_code(),
        http::StatusCode::BAD_REQUEST
    );
    assert_eq!(
        AuthError::TokenExpired.status_code(),
        http::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        AuthError::InvalidSignature.status_code(),
        http::StatusCode::UNAUTHORIZED
    );
    // TAG: surface=auth owner=security-team rule=RBAC-001
    assert_eq!(
        AuthError::UserNotFound("u".to_string()).status_code(),
        http::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        AuthError::InsufficientPermissions("test".to_string()).status_code(),
        http::StatusCode::FORBIDDEN
    );
    assert_eq!(
        AuthError::Internal("err".to_string()).status_code(),
        http::StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn test_jwt_claims_to_user() {
    let claims = JwtClaims::new("user-1", "test@example.com", "Test", "entra-1", 3600);
    let user = claims.to_authenticated_user();

    assert_eq!(user.user_id, "user-1");
    assert_eq!(user.email, "test@example.com");
    assert_eq!(user.name, "Test");
    assert_eq!(user.entra_id, "entra-1");
}

#[test]
fn test_authenticated_user_has_role() {
    let mut user = AuthenticatedUser::new("1", "test@example.com", "Test", "e1");
    user.roles = vec!["admin".to_string(), "user".to_string()];
    assert!(user.has_role("admin"));
    assert!(user.has_role("ADMIN"));
    assert!(user.has_role("User"));
    assert!(!user.has_role("viewer"));
}

#[test]
fn test_authenticated_user_is_admin() {
    let mut user = AuthenticatedUser::new("1", "test@example.com", "Test", "e1");
    assert!(!user.is_admin());
    user.roles = vec!["admin".to_string()];
    assert!(user.is_admin());
    user.roles = vec!["internal".to_string()];
    assert!(user.is_admin());
}

// TAG: surface=auth owner=security-team rule=RBAC-001
#[test]
fn test_authenticated_user_has_complete_profile() {
    let mut user = AuthenticatedUser::new("1", "test@example.com", "Test", "e1");
    assert!(!user.has_complete_profile());
    user.given_name = Some("John".to_string());
    assert!(!user.has_complete_profile());
    user.family_name = Some("Doe".to_string());
    assert!(!user.has_complete_profile());
    user.mobile_phone = Some("555-1234".to_string());
    assert!(user.has_complete_profile());
}

#[test]
fn test_authenticated_user_display() {
    let user = AuthenticatedUser::new("1", "test@example.com", "Test User", "e1");
    assert_eq!(format!("{user}"), "Test User <test@example.com>");

    let user_empty = AuthenticatedUser::new("1", "test@example.com", "", "e1");
    assert_eq!(
        format!("{user_empty}"),
        "test@example.com <test@example.com>"
    );
}

#[test]
fn test_auth_method_display() {
    assert_eq!(format!("{}", AuthMethod::JwtBearer), "jwt_bearer");
    assert_eq!(format!("{}", AuthMethod::CookieSession), "cookie_session");
    assert_eq!(format!("{}", AuthMethod::Oauth), "oauth");
    assert_eq!(format!("{}", AuthMethod::ApiKey), "api_key");
    assert_eq!(format!("{}", AuthMethod::WebAuthn), "webauthn");
}

// TAG: surface=auth owner=security-team rule=RBAC-001
#[test]
fn test_verification_status_display() {
    assert_eq!(format!("{}", VerificationStatus::Unverified), "unverified");
    assert_eq!(format!("{}", VerificationStatus::Pending), "pending");
    assert_eq!(format!("{}", VerificationStatus::Verified), "verified");
    assert_eq!(format!("{}", VerificationStatus::Expired), "expired");
    assert_eq!(format!("{}", VerificationStatus::Revoked), "revoked");
}

#[test]
fn test_auth_config_from_env_success() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::set_var("JWT_SECRET", "this_is_a_very_long_secret_key_32chars!");

    let config = AuthConfig::from_env();
    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.jwt_secret, "this_is_a_very_long_secret_key_32chars!");
    assert_eq!(config.token_expiration_secs, 3600);

    std::env::remove_var("JWT_SECRET");
}

#[test]
fn test_auth_config_from_env_missing() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var("JWT_SECRET");

    let config = AuthConfig::from_env();
    assert!(config.is_err());
    assert!(matches!(
        config.unwrap_err(),
        AuthConfigError::MissingJwtSecret
    ));
}

#[test]
fn test_auth_config_from_env_weak_secret() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::set_var("JWT_SECRET", "short");

    // TAG: surface=auth owner=security-team rule=RBAC-001
    let config = AuthConfig::from_env();
    assert!(config.is_err());
    assert!(matches!(
        config.unwrap_err(),
        AuthConfigError::WeakJwtSecret
    ));

    std::env::remove_var("JWT_SECRET");
}

#[test]
fn test_auth_config_validate_for_production() {
    let config = AuthConfig {
        jwt_secret: "this_is_a_very_long_secret_key_32chars!".to_string(),
        token_expiration_secs: 3600,
        refresh_token_expiration_secs: 86400 * 7,
        oauth_client_id: None,
        oauth_client_secret: None,
        oauth_tenant_id: None,
        allowed_redirect_urls: vec![],
    };
    assert!(config.validate_for_production().is_ok());

    let bad_config = AuthConfig {
        jwt_secret: "INVALID-DEV-SECRET-SET-JWT_SECRET-ENV-VAR".to_string(),
        ..config.clone()
    };
    assert!(bad_config.validate_for_production().is_err());

    let weak_config = AuthConfig {
        jwt_secret: "short".to_string(),
        ..config
    };
    assert!(weak_config.validate_for_production().is_err());
}

// TAG: surface=auth owner=security-team rule=RBAC-001
#[test]
fn test_auth_config_from_env_with_oauth() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::set_var("JWT_SECRET", "this_is_a_very_long_secret_key_32chars!");
    std::env::set_var("OAUTH_CLIENT_ID", "client-id");
    std::env::set_var("OAUTH_CLIENT_SECRET", "client-secret");
    std::env::set_var("OAUTH_TENANT_ID", "tenant-id");
    std::env::set_var(
        "ALLOWED_REDIRECT_URLS",
        "https://a.com/callback,https://b.com/callback",
    );

    let config = AuthConfig::from_env().unwrap();
    assert_eq!(config.oauth_client_id, Some("client-id".to_string()));
    assert_eq!(
        config.oauth_client_secret,
        Some("client-secret".to_string())
    );
    assert_eq!(config.oauth_tenant_id, Some("tenant-id".to_string()));
    assert_eq!(
        config.allowed_redirect_urls,
        vec!["https://a.com/callback", "https://b.com/callback"]
    );

    std::env::remove_var("JWT_SECRET");
    std::env::remove_var("OAUTH_CLIENT_ID");
    std::env::remove_var("OAUTH_CLIENT_SECRET");
    std::env::remove_var("OAUTH_TENANT_ID");
    std::env::remove_var("ALLOWED_REDIRECT_URLS");
}
