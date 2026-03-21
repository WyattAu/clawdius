//! Enterprise features for Clawdius
//!
//! This module provides enterprise-grade features including:
//! - SSO integration (SAML, OIDC)
//! - Audit logging
//! - Compliance templates
//! - Team management

//! - Shared contexts (Phase 4)

pub mod audit;
pub mod compliance;
pub mod shared_context;
pub mod sso;
pub mod teams;

 pub use audit::{AuditEvent, AuditLogger, AuditQuery, AuditStorage};
 pub use compliance::{
    ComplianceControl, ComplianceFramework, ComplianceReport, ComplianceTemplate, ControlAssessment,
};
 pub use shared_context::{AccessLevel, ContextType, SharedContext};
 pub use sso::{OAuthProvider, SAMLConfig, SSOConfig, SSOManager, SSOProvider, SSOUser};
 pub use teams::{Permission, Team, TeamManager, TeamMember, TeamRole, TeamSettings};
