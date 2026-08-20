//! Role-Based Access Control (RBAC) module
//!
//! This module provides role-based access control functionality.

use serde::{Deserialize, Serialize};

// TAG: surface=auth owner=security-team rule=RBAC-001
/// User role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Administrator with full access
    Admin,
    /// Regular user
    User,
    /// Read-only access
    Viewer,
    /// API client
    ApiClient,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::User => write!(f, "user"),
            Self::Viewer => write!(f, "viewer"),
            Self::ApiClient => write!(f, "api_client"),
        }
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Permission for a specific resource/action
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permission {
    /// Resource name
    pub resource: String,
    /// Action (e.g., "read", "write", "delete")
    pub action: String,
}

impl Permission {
    /// Create a new permission
    pub fn new(resource: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            action: action.into(),
        }
    }
}

/// RBAC manager
#[derive(Debug, Clone)]
pub struct Rbac {
    roles: std::collections::HashMap<Role, Vec<Permission>>,
}

impl Rbac {
    // TAG: surface=auth owner=security-team rule=RBAC-001
    /// Create a new RBAC manager with default roles
    #[must_use]
    pub fn new() -> Self {
        let mut roles = std::collections::HashMap::new();

        // Admin has all permissions
        roles.insert(Role::Admin, vec![Permission::new("*", "*")]);

        // User has limited permissions
        roles.insert(
            Role::User,
            vec![
                Permission::new("profile", "read"),
                Permission::new("profile", "write"),
                Permission::new("applications", "read"),
                Permission::new("applications", "write"),
            ],
        );

        // Viewer is read-only
        roles.insert(
            Role::Viewer,
            vec![
                Permission::new("profile", "read"),
                Permission::new("applications", "read"),
            ],
        );

        // API client has API-specific permissions
        roles.insert(
            Role::ApiClient,
            vec![
                Permission::new("api", "read"),
                Permission::new("api", "write"),
            ],
        );

        Self { roles }
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    /// Check if a role has a specific permission
    #[must_use]
    pub fn has_permission(&self, role: Role, resource: &str, action: &str) -> bool {
        if let Some(permissions) = self.roles.get(&role) {
            permissions.iter().any(|p| {
                (p.resource == "*" || p.resource == resource)
                    && (p.action == "*" || p.action == action)
            })
        } else {
            false
        }
    }

    /// Get permissions for a role
    #[must_use]
    pub fn get_permissions(&self, role: Role) -> Option<&Vec<Permission>> {
        self.roles.get(&role)
    }
}

impl Default for Rbac {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
// TAG: surface=auth owner=platform-team rule=GENERAL-001
mod tests {
    use super::*;

    #[test]
    fn test_role_display() {
        assert_eq!(Role::Admin.to_string(), "admin");
        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Role::Viewer.to_string(), "viewer");
        assert_eq!(Role::ApiClient.to_string(), "api_client");
    }

    #[test]
    fn test_permission_new() {
        let perm = Permission::new("profile", "read");
        assert_eq!(perm.resource, "profile");
        assert_eq!(perm.action, "read");
    }

    #[test]
    fn test_rbac_new() {
        let rbac = Rbac::new();
        assert!(rbac.get_permissions(Role::Admin).is_some());
        assert!(rbac.get_permissions(Role::User).is_some());
        assert!(rbac.get_permissions(Role::Viewer).is_some());
        assert!(rbac.get_permissions(Role::ApiClient).is_some());
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    #[test]
    fn test_rbac_default() {
        let rbac = Rbac::default();
        assert!(rbac.get_permissions(Role::Admin).is_some());
    }

    #[test]
    fn test_rbac_admin_has_all_permissions() {
        let rbac = Rbac::new();
        assert!(rbac.has_permission(Role::Admin, "anything", "anything"));
        assert!(rbac.has_permission(Role::Admin, "profile", "read"));
        assert!(rbac.has_permission(Role::Admin, "api", "delete"));
    }

    #[test]
    fn test_rbac_user_permissions() {
        let rbac = Rbac::new();
        assert!(rbac.has_permission(Role::User, "profile", "read"));
        assert!(rbac.has_permission(Role::User, "profile", "write"));
        assert!(rbac.has_permission(Role::User, "applications", "read"));
        assert!(rbac.has_permission(Role::User, "applications", "write"));
        assert!(!rbac.has_permission(Role::User, "profile", "delete"));
        assert!(!rbac.has_permission(Role::User, "api", "read"));
    }

    #[test]
    fn test_rbac_viewer_permissions() {
        let rbac = Rbac::new();
        assert!(rbac.has_permission(Role::Viewer, "profile", "read"));
        assert!(rbac.has_permission(Role::Viewer, "applications", "read"));
        assert!(!rbac.has_permission(Role::Viewer, "profile", "write"));
        assert!(!rbac.has_permission(Role::Viewer, "applications", "write"));
    }

    #[test]
    fn test_rbac_api_client_permissions() {
        let rbac = Rbac::new();
        assert!(rbac.has_permission(Role::ApiClient, "api", "read"));
        assert!(rbac.has_permission(Role::ApiClient, "api", "write"));
        assert!(!rbac.has_permission(Role::ApiClient, "profile", "read"));
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    #[test]
    fn test_rbac_unknown_role() {
        // Since Role is an enum with all variants covered, this tests the fallback
        // through get_permissions for a role that exists but has no perms
        let rbac = Rbac::new();
        // All roles have permissions in default Rbac, so we test get_permissions directly
        assert!(!rbac.get_permissions(Role::Admin).unwrap().is_empty());
    }

    #[test]
    fn test_permission_wildcard_resource() {
        let perm = Permission::new("*", "read");
        assert!(perm.resource == "*");
        assert!(perm.action == "read");
    }

    #[test]
    fn test_permission_wildcard_action() {
        let perm = Permission::new("profile", "*");
        assert!(perm.resource == "profile");
        assert!(perm.action == "*");
    }

    #[test]
    fn test_role_serde_roundtrip() {
        for role in [Role::Admin, Role::User, Role::Viewer, Role::ApiClient] {
            let json = serde_json::to_string(&role).unwrap();
            let deserialized: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(role, deserialized);
        }
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    #[test]
    fn test_role_deserialize_pascal_case() {
        // The enum derives Serialize/Deserialize without explicit rename_all
        assert_eq!(
            serde_json::from_str::<Role>("\"ApiClient\"").unwrap(),
            Role::ApiClient
        );
        assert_eq!(
            serde_json::from_str::<Role>("\"Admin\"").unwrap(),
            Role::Admin
        );
    }

    #[test]
    fn test_rbac_custom_permission() {
        let mut rbac = Rbac::new();
        rbac.roles.insert(
            Role::User,
            vec![Permission::new("custom/resource", "custom_action")],
        );
        assert!(rbac.has_permission(Role::User, "custom/resource", "custom_action"));
        assert!(!rbac.has_permission(Role::User, "other", "read"));
    }

    #[test]
    fn test_rbac_empty_permissions() {
        let mut rbac = Rbac::new();
        rbac.roles.insert(Role::Viewer, vec![]);
        assert!(!rbac.has_permission(Role::Viewer, "profile", "read"));
    }
}
