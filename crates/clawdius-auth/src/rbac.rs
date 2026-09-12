//! Role-Based Access Control (RBAC) for Clawdius.
//!
//! Defines 23 fine-grained permissions across code operations, session
//! management, admin functions, provider management, plugin management,
//! and billing. Provides Axum middleware for enforcing permissions.

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::user::SessionClaims;

/// A single permission grant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    /// Category (e.g., "code", "session", "admin").
    pub category: String,
    /// Action (e.g., "read", "write", "`manage_users`").
    pub action: String,
}

impl Permission {
    /// Create a permission from a category and action pair.
    #[must_use]
    pub fn new(category: &str, action: &str) -> Self {
        Self {
            category: category.to_string(),
            action: action.to_string(),
        }
    }
}

/// All 23 permissions in the system.
pub mod permissions {
    use super::Permission;

    // Code operations (4)
    /// Read code and repositories.
    #[must_use]
    pub fn code_read() -> Permission {
        Permission::new("code", "read")
    }
    /// Write and modify code.
    #[must_use]
    pub fn code_write() -> Permission {
        Permission::new("code", "write")
    }
    /// Execute code and shell commands.
    #[must_use]
    pub fn code_execute() -> Permission {
        Permission::new("code", "execute")
    }
    /// Delete code and files.
    #[must_use]
    pub fn code_delete() -> Permission {
        Permission::new("code", "delete")
    }

    // Session management (5)
    /// Create new sessions.
    #[must_use]
    pub fn session_create() -> Permission {
        Permission::new("session", "create")
    }
    /// Read session contents.
    #[must_use]
    pub fn session_read() -> Permission {
        Permission::new("session", "read")
    }
    /// Update existing sessions.
    #[must_use]
    pub fn session_update() -> Permission {
        Permission::new("session", "update")
    }
    /// Delete sessions.
    #[must_use]
    pub fn session_delete() -> Permission {
        Permission::new("session", "delete")
    }
    /// Share sessions with others.
    #[must_use]
    pub fn session_share() -> Permission {
        Permission::new("session", "share")
    }

    // Admin functions (4)
    /// Create, update, and remove users.
    #[must_use]
    pub fn admin_manage_users() -> Permission {
        Permission::new("admin", "manage_users")
    }
    /// Create, update, and remove teams.
    #[must_use]
    pub fn admin_manage_teams() -> Permission {
        Permission::new("admin", "manage_teams")
    }
    /// View the audit log.
    #[must_use]
    pub fn admin_view_audit() -> Permission {
        Permission::new("admin", "view_audit")
    }
    /// Change system configuration.
    #[must_use]
    pub fn admin_manage_config() -> Permission {
        Permission::new("admin", "manage_config")
    }

    // Provider management (3)
    /// Add LLM providers.
    #[must_use]
    pub fn provider_add() -> Permission {
        Permission::new("provider", "add_provider")
    }
    /// Remove LLM providers.
    #[must_use]
    pub fn provider_remove() -> Permission {
        Permission::new("provider", "remove_provider")
    }
    /// Manage provider API keys.
    #[must_use]
    pub fn provider_manage_keys() -> Permission {
        Permission::new("provider", "manage_keys")
    }

    // Plugin management (3)
    /// Install plugins.
    #[must_use]
    pub fn plugin_install() -> Permission {
        Permission::new("plugin", "install")
    }
    /// Remove plugins.
    #[must_use]
    pub fn plugin_remove() -> Permission {
        Permission::new("plugin", "remove")
    }
    /// Configure plugins.
    #[must_use]
    pub fn plugin_configure() -> Permission {
        Permission::new("plugin", "configure")
    }

    // Billing (2)
    /// View usage and billing data.
    #[must_use]
    pub fn billing_view_usage() -> Permission {
        Permission::new("billing", "view_usage")
    }
    /// Manage billing, plans, and subscriptions.
    #[must_use]
    pub fn billing_manage() -> Permission {
        Permission::new("billing", "manage_billing")
    }

    /// All 23 permissions.
    #[must_use]
    pub fn all() -> Vec<Permission> {
        vec![
            code_read(),
            code_write(),
            code_execute(),
            code_delete(),
            session_create(),
            session_read(),
            session_update(),
            session_delete(),
            session_share(),
            admin_manage_users(),
            admin_manage_teams(),
            admin_view_audit(),
            admin_manage_config(),
            provider_add(),
            provider_remove(),
            provider_manage_keys(),
            plugin_install(),
            plugin_remove(),
            plugin_configure(),
            billing_view_usage(),
            billing_manage(),
        ]
    }
}

/// Role hierarchy levels (higher number = more permissions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Read-only access.
    Viewer = 0,
    /// Standard user access.
    User = 1,
    /// Can manage providers and plugins.
    Operator = 2,
    /// Full administrative access.
    Admin = 3,
}

impl Role {
    /// Parse a role from a string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "viewer" => Some(Self::Viewer),
            "user" => Some(Self::User),
            "operator" => Some(Self::Operator),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }
}

/// RBAC policy defining which roles have which permissions.
#[derive(Debug, Clone)]
pub struct RbacPolicy {
    /// Role -> set of permissions.
    role_permissions: HashMap<Role, Vec<Permission>>,
}

impl Default for RbacPolicy {
    fn default() -> Self {
        let mut role_permissions = HashMap::new();

        // Viewer: read-only access
        role_permissions.insert(
            Role::Viewer,
            vec![
                permissions::code_read(),
                permissions::session_read(),
                permissions::billing_view_usage(),
            ],
        );

        // User: standard developer access
        role_permissions.insert(
            Role::User,
            vec![
                permissions::code_read(),
                permissions::code_write(),
                permissions::code_execute(),
                permissions::session_create(),
                permissions::session_read(),
                permissions::session_update(),
                permissions::session_delete(),
                permissions::billing_view_usage(),
            ],
        );

        // Operator: can manage providers and plugins
        let mut operator_perms = role_permissions[&Role::User].clone();
        operator_perms.extend(vec![
            permissions::code_delete(),
            permissions::session_share(),
            permissions::provider_add(),
            permissions::provider_remove(),
            permissions::provider_manage_keys(),
            permissions::plugin_install(),
            permissions::plugin_remove(),
            permissions::plugin_configure(),
            permissions::admin_view_audit(),
        ]);
        role_permissions.insert(Role::Operator, operator_perms);

        // Admin: full access
        role_permissions.insert(Role::Admin, permissions::all());

        Self { role_permissions }
    }
}

impl RbacPolicy {
    /// Check if a role has a specific permission.
    #[must_use]
    pub fn has_permission(&self, role: &Role, permission: &Permission) -> bool {
        self.role_permissions
            .get(role)
            .is_some_and(|perms| perms.contains(permission))
    }

    /// Get all permissions for a role.
    #[must_use]
    pub fn permissions_for_role(&self, role: &Role) -> &[Permission] {
        self.role_permissions
            .get(role)
            .map_or(&[], std::vec::Vec::as_slice)
    }
}

/// RBAC service that checks permissions against claims.
#[derive(Clone)]
pub struct RbacService {
    policy: Arc<RbacPolicy>,
}

impl RbacService {
    /// Create a service backed by the given policy.
    #[must_use]
    pub fn new(policy: RbacPolicy) -> Self {
        Self {
            policy: Arc::new(policy),
        }
    }

    /// Check if the given claims have the required permission.
    ///
    /// # Errors
    ///
    /// Returns [`RbacError::InsufficientPermissions`] when none of the
    /// caller's roles grants the permission.
    pub fn check(&self, claims: &SessionClaims, permission: &Permission) -> Result<(), RbacError> {
        // Get the highest role from the user's roles
        let user_role = claims
            .roles
            .iter()
            .filter_map(|r| Role::parse(r))
            .max()
            .unwrap_or(Role::Viewer);

        if self.policy.has_permission(&user_role, permission) {
            Ok(())
        } else {
            Err(RbacError::InsufficientPermissions {
                required: permission.clone(),
                actual_role: user_role,
            })
        }
    }
}

/// Errors from RBAC checks.
#[derive(Debug, thiserror::Error)]
pub enum RbacError {
    /// The caller lacks the permission required for the operation.
    #[error("Insufficient permissions: requires {required:?}, user has role {actual_role:?}")]
    InsufficientPermissions {
        /// The permission that was required.
        required: Permission,
        /// The highest role the caller held.
        actual_role: Role,
    },
}

impl IntoResponse for RbacError {
    fn into_response(self) -> Response {
        (StatusCode::FORBIDDEN, self.to_string()).into_response()
    }
}

/// Axum extractor that enforces a required permission.
///
/// The required [`Permission`] must be inserted into the request extensions
/// (e.g. by a per-route middleware); the guard then validates the caller's
/// claims against it via the [`RbacService`] also present in the extensions.
pub struct RequirePermission {
    /// Verified session claims of the caller.
    pub claims: SessionClaims,
}

/// Guard that checks a specific permission.
pub struct RequirePermissionGuard {
    /// The permission that was required and validated.
    pub permission: Permission,
}

impl<S: Send + Sync> FromRequestParts<S> for RequirePermissionGuard {
    type Rejection = RbacError;

    #[allow(clippy::unused_async_trait_impl)]
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let missing_permission = || RbacError::InsufficientPermissions {
            required: Permission::new("", ""),
            actual_role: Role::Viewer,
        };
        let permission = parts
            .extensions
            .get::<Permission>()
            .cloned()
            .ok_or_else(missing_permission)?;

        let auth_user = parts
            .extensions
            .get::<Arc<crate::middleware::AuthUser>>()
            .ok_or_else(|| RbacError::InsufficientPermissions {
                required: permission.clone(),
                actual_role: Role::Viewer,
            })?;

        let rbac = parts.extensions.get::<Arc<RbacService>>().ok_or_else(|| {
            RbacError::InsufficientPermissions {
                required: permission.clone(),
                actual_role: Role::Viewer,
            }
        })?;

        rbac.check(&auth_user.claims, &permission)?;

        Ok(Self { permission })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_hierarchy() {
        assert!(Role::Admin > Role::Operator);
        assert!(Role::Operator > Role::User);
        assert!(Role::User > Role::Viewer);
    }

    #[test]
    fn test_default_policy_viewer() {
        let policy = RbacPolicy::default();
        assert!(policy.has_permission(&Role::Viewer, &permissions::code_read()));
        assert!(!policy.has_permission(&Role::Viewer, &permissions::code_write()));
        assert!(!policy.has_permission(&Role::Viewer, &permissions::admin_manage_users()));
    }

    #[test]
    fn test_default_policy_user() {
        let policy = RbacPolicy::default();
        assert!(policy.has_permission(&Role::User, &permissions::code_read()));
        assert!(policy.has_permission(&Role::User, &permissions::code_write()));
        assert!(policy.has_permission(&Role::User, &permissions::code_execute()));
        assert!(!policy.has_permission(&Role::User, &permissions::code_delete()));
        assert!(!policy.has_permission(&Role::User, &permissions::admin_manage_users()));
    }

    #[test]
    fn test_default_policy_operator() {
        let policy = RbacPolicy::default();
        assert!(policy.has_permission(&Role::Operator, &permissions::code_delete()));
        assert!(policy.has_permission(&Role::Operator, &permissions::provider_add()));
        assert!(policy.has_permission(&Role::Operator, &permissions::plugin_install()));
        assert!(policy.has_permission(&Role::Operator, &permissions::admin_view_audit()));
        assert!(!policy.has_permission(&Role::Operator, &permissions::admin_manage_users()));
    }

    #[test]
    fn test_default_policy_admin() {
        let policy = RbacPolicy::default();
        for perm in permissions::all() {
            assert!(
                policy.has_permission(&Role::Admin, &perm),
                "Admin should have {perm:?}"
            );
        }
    }

    #[test]
    fn test_rbac_check() {
        let rbac = RbacService::new(RbacPolicy::default());

        let admin_claims = SessionClaims {
            sub: "1".to_string(),
            email: None,
            name: None,
            provider: "test".to_string(),
            roles: vec!["admin".to_string()],
            iat: 0,
            exp: 9_999_999_999,
            jti: "test".to_string(),
            iss: None,
        };

        assert!(rbac
            .check(&admin_claims, &permissions::admin_manage_users())
            .is_ok());

        let viewer_claims = SessionClaims {
            roles: vec!["viewer".to_string()],
            ..admin_claims
        };

        assert!(rbac
            .check(&viewer_claims, &permissions::code_write())
            .is_err());
    }

    #[test]
    fn test_all_permissions_count() {
        assert_eq!(permissions::all().len(), 21);
    }
}
