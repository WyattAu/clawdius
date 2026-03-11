//! Team Management for Enterprise
//!
//! Provides team management, roles, and permissions.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Team
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Team ID
    pub id: String,
    /// Team name
    pub name: String,
    /// Team description
    pub description: Option<String>,
    /// Organization ID
    pub organization_id: String,
    /// Team members
    pub members: HashMap<String, TeamMember>,
    /// Team roles
    pub roles: HashMap<String, TeamRole>,
    /// Team settings
    pub settings: TeamSettings,
    /// Created at
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Updated at
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Team member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    /// User ID
    pub user_id: String,
    /// Display name
    pub display_name: String,
    /// Email
    pub email: String,
    /// Role assignments
    pub roles: Vec<String>,
    /// Joined at
    pub joined_at: chrono::DateTime<chrono::Utc>,
    /// Invited by
    pub invited_by: Option<String>,
    /// Status
    pub status: MemberStatus,
}

/// Member status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberStatus {
    /// Active member
    Active,
    /// Pending invitation
    Pending,
    /// Suspended
    Suspended,
    /// Left team
    Left,
}

/// Team role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRole {
    /// Role ID
    pub id: String,
    /// Role name
    pub name: String,
    /// Role description
    pub description: String,
    /// Is system role
    pub is_system: bool,
    /// Permissions
    pub permissions: HashSet<Permission>,
    /// Inherited roles
    pub inherits: Vec<String>,
}

/// Permission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Team management
    /// Create teams
    TeamCreate,
    /// Delete teams
    TeamDelete,
    /// Update team settings
    TeamUpdate,
    /// Invite members
    TeamInviteMembers,
    /// Remove members
    TeamRemoveMembers,
    /// Manage roles
    TeamManageRoles,

    // Session management
    /// Create sessions
    SessionCreate,
    /// View sessions
    SessionView,
    /// View all sessions (including others')
    SessionViewAll,
    /// Delete sessions
    SessionDelete,
    /// Share sessions
    SessionShare,

    // Code access
    /// Read files
    CodeRead,
    /// Write files
    CodeWrite,
    /// Delete files
    CodeDelete,
    /// Execute commands
    CodeExecute,

    // LLM access
    /// Use LLM
    LlmUse,
    /// View LLM usage
    LlmViewUsage,
    /// Manage LLM settings
    LlmManageSettings,

    // Plugin management
    /// Install plugins
    PluginInstall,
    /// Uninstall plugins
    PluginUninstall,
    /// Configure plugins
    PluginConfigure,

    // Audit
    /// View audit logs
    AuditView,
    /// Export audit logs
    AuditExport,

    // Admin
    /// Admin access
    Admin,
    /// Manage billing
    BillingManage,
}

impl Permission {
    /// Get permission description
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::TeamCreate => "Create new teams",
            Self::TeamDelete => "Delete teams",
            Self::TeamUpdate => "Update team settings",
            Self::TeamInviteMembers => "Invite new team members",
            Self::TeamRemoveMembers => "Remove team members",
            Self::TeamManageRoles => "Manage team roles and permissions",
            Self::SessionCreate => "Create new sessions",
            Self::SessionView => "View own sessions",
            Self::SessionViewAll => "View all team sessions",
            Self::SessionDelete => "Delete sessions",
            Self::SessionShare => "Share sessions with team",
            Self::CodeRead => "Read files",
            Self::CodeWrite => "Write/modify files",
            Self::CodeDelete => "Delete files",
            Self::CodeExecute => "Execute shell commands",
            Self::LlmUse => "Use LLM features",
            Self::LlmViewUsage => "View LLM usage statistics",
            Self::LlmManageSettings => "Manage LLM settings",
            Self::PluginInstall => "Install plugins",
            Self::PluginUninstall => "Uninstall plugins",
            Self::PluginConfigure => "Configure plugin settings",
            Self::AuditView => "View audit logs",
            Self::AuditExport => "Export audit logs",
            Self::Admin => "Full administrative access",
            Self::BillingManage => "Manage billing and subscription",
        }
    }

    /// Get permission category
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            Self::TeamCreate
            | Self::TeamDelete
            | Self::TeamUpdate
            | Self::TeamInviteMembers
            | Self::TeamRemoveMembers
            | Self::TeamManageRoles => "Team Management",
            Self::SessionCreate
            | Self::SessionView
            | Self::SessionViewAll
            | Self::SessionDelete
            | Self::SessionShare => "Session Management",
            Self::CodeRead | Self::CodeWrite | Self::CodeDelete | Self::CodeExecute => {
                "Code Access"
            }
            Self::LlmUse | Self::LlmViewUsage | Self::LlmManageSettings => "LLM Access",
            Self::PluginInstall | Self::PluginUninstall | Self::PluginConfigure => {
                "Plugin Management"
            }
            Self::AuditView | Self::AuditExport => "Audit",
            Self::Admin | Self::BillingManage => "Administration",
        }
    }
}

/// Team settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSettings {
    /// Default role for new members
    pub default_role: String,
    /// Require 2FA for members
    pub require_2fa: bool,
    /// Session sharing enabled
    pub session_sharing: bool,
    /// Maximum members
    pub max_members: Option<u32>,
    /// Allowed domains for invitations
    pub allowed_domains: Vec<String>,
    /// Session retention days
    pub session_retention_days: u32,
    /// Audit log retention days
    pub audit_retention_days: u32,
}

impl Default for TeamSettings {
    fn default() -> Self {
        Self {
            default_role: "member".to_string(),
            require_2fa: false,
            session_sharing: true,
            max_members: None,
            allowed_domains: Vec::new(),
            session_retention_days: 90,
            audit_retention_days: 365,
        }
    }
}

/// Predefined roles
impl TeamRole {
    /// Admin role - full access
    #[must_use]
    pub fn admin() -> Self {
        Self {
            id: "admin".to_string(),
            name: "Admin".to_string(),
            description: "Full administrative access".to_string(),
            is_system: true,
            permissions: Permission::all().into_iter().collect(),
            inherits: vec!["member".to_string()],
        }
    }

    /// Developer role - code access
    #[must_use]
    pub fn developer() -> Self {
        let permissions: HashSet<Permission> = [
            Permission::SessionCreate,
            Permission::SessionView,
            Permission::SessionDelete,
            Permission::SessionShare,
            Permission::CodeRead,
            Permission::CodeWrite,
            Permission::CodeDelete,
            Permission::CodeExecute,
            Permission::LlmUse,
            Permission::LlmViewUsage,
            Permission::PluginConfigure,
        ]
        .into_iter()
        .collect();

        Self {
            id: "developer".to_string(),
            name: "Developer".to_string(),
            description: "Full code access".to_string(),
            is_system: true,
            permissions,
            inherits: vec!["member".to_string()],
        }
    }

    /// Analyst role - read only
    #[must_use]
    pub fn analyst() -> Self {
        let permissions: HashSet<Permission> = [
            Permission::SessionView,
            Permission::CodeRead,
            Permission::LlmViewUsage,
            Permission::AuditView,
        ]
        .into_iter()
        .collect();

        Self {
            id: "analyst".to_string(),
            name: "Analyst".to_string(),
            description: "Read-only access".to_string(),
            is_system: true,
            permissions,
            inherits: vec![],
        }
    }

    /// Member role - basic access
    #[must_use]
    pub fn member() -> Self {
        let permissions: HashSet<Permission> = [
            Permission::SessionCreate,
            Permission::SessionView,
            Permission::SessionDelete,
            Permission::CodeRead,
            Permission::LlmUse,
        ]
        .into_iter()
        .collect();

        Self {
            id: "member".to_string(),
            name: "Member".to_string(),
            description: "Basic team member".to_string(),
            is_system: true,
            permissions,
            inherits: vec![],
        }
    }

    /// Billing admin role
    #[must_use]
    pub fn billing_admin() -> Self {
        let permissions: HashSet<Permission> = [Permission::BillingManage, Permission::AuditView]
            .into_iter()
            .collect();

        Self {
            id: "billing_admin".to_string(),
            name: "Billing Admin".to_string(),
            description: "Manage billing and subscription".to_string(),
            is_system: true,
            permissions,
            inherits: vec!["member".to_string()],
        }
    }
}

impl Permission {
    /// Get all permissions
    #[must_use]
    pub fn all() -> Vec<Self> {
        use Permission::{
            Admin, AuditExport, AuditView, BillingManage, CodeDelete, CodeExecute, CodeRead,
            CodeWrite, LlmManageSettings, LlmUse, LlmViewUsage, PluginConfigure, PluginInstall,
            PluginUninstall, SessionCreate, SessionDelete, SessionShare, SessionView,
            SessionViewAll, TeamCreate, TeamDelete, TeamInviteMembers, TeamManageRoles,
            TeamRemoveMembers, TeamUpdate,
        };
        vec![
            TeamCreate,
            TeamDelete,
            TeamUpdate,
            TeamInviteMembers,
            TeamRemoveMembers,
            TeamManageRoles,
            SessionCreate,
            SessionView,
            SessionViewAll,
            SessionDelete,
            SessionShare,
            CodeRead,
            CodeWrite,
            CodeDelete,
            CodeExecute,
            LlmUse,
            LlmViewUsage,
            LlmManageSettings,
            PluginInstall,
            PluginUninstall,
            PluginConfigure,
            AuditView,
            AuditExport,
            Admin,
            BillingManage,
        ]
    }
}

/// Team manager
pub struct TeamManager {
    teams: HashMap<String, Team>,
}

impl TeamManager {
    /// Create a new team manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            teams: HashMap::new(),
        }
    }

    /// Create a team
    pub fn create_team(
        &mut self,
        name: String,
        organization_id: String,
        creator_id: String,
        creator_name: String,
        creator_email: String,
    ) -> Result<Team> {
        let team_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        // Create default roles
        let mut roles = HashMap::new();
        let admin_role = TeamRole::admin();
        let member_role = TeamRole::member();
        let developer_role = TeamRole::developer();
        let analyst_role = TeamRole::analyst();
        roles.insert(admin_role.id.clone(), admin_role);
        roles.insert(member_role.id.clone(), member_role);
        roles.insert(developer_role.id.clone(), developer_role);
        roles.insert(analyst_role.id.clone(), analyst_role);

        // Add creator as admin
        let creator = TeamMember {
            user_id: creator_id.clone(),
            display_name: creator_name,
            email: creator_email,
            roles: vec!["admin".to_string()],
            joined_at: now,
            invited_by: None,
            status: MemberStatus::Active,
        };

        let mut members = HashMap::new();
        members.insert(creator_id, creator);

        let team = Team {
            id: team_id.clone(),
            name,
            description: None,
            organization_id,
            members,
            roles,
            settings: TeamSettings::default(),
            created_at: now,
            updated_at: now,
        };

        self.teams.insert(team_id, team.clone());
        Ok(team)
    }

    /// Get a team
    #[must_use]
    pub fn get_team(&self, team_id: &str) -> Option<&Team> {
        self.teams.get(team_id)
    }

    /// Get a mutable team
    pub fn get_team_mut(&mut self, team_id: &str) -> Option<&mut Team> {
        self.teams.get_mut(team_id)
    }

    /// List teams for an organization
    #[must_use]
    pub fn list_teams(&self, organization_id: &str) -> Vec<&Team> {
        self.teams
            .values()
            .filter(|t| t.organization_id == organization_id)
            .collect()
    }

    /// Add member to team
    pub fn add_member(
        &mut self,
        team_id: &str,
        user_id: String,
        display_name: String,
        email: String,
        role: Option<String>,
        invited_by: Option<String>,
    ) -> Result<&TeamMember> {
        let team = self
            .teams
            .get_mut(team_id)
            .ok_or_else(|| anyhow::anyhow!("Team not found"))?;

        let member = TeamMember {
            user_id: user_id.clone(),
            display_name,
            email,
            roles: vec![role.unwrap_or_else(|| team.settings.default_role.clone())],
            joined_at: chrono::Utc::now(),
            invited_by,
            status: MemberStatus::Active,
        };

        team.members.insert(user_id.clone(), member);
        team.updated_at = chrono::Utc::now();

        Ok(team.members.get(&user_id).unwrap())
    }

    /// Remove member from team
    pub fn remove_member(&mut self, team_id: &str, user_id: &str) -> Result<()> {
        let team = self
            .teams
            .get_mut(team_id)
            .ok_or_else(|| anyhow::anyhow!("Team not found"))?;

        team.members.remove(user_id);
        team.updated_at = chrono::Utc::now();

        Ok(())
    }

    /// Update member role
    pub fn update_member_role(
        &mut self,
        team_id: &str,
        user_id: &str,
        roles: Vec<String>,
    ) -> Result<()> {
        let team = self
            .teams
            .get_mut(team_id)
            .ok_or_else(|| anyhow::anyhow!("Team not found"))?;

        let member = team
            .members
            .get_mut(user_id)
            .ok_or_else(|| anyhow::anyhow!("Member not found"))?;

        member.roles = roles;
        team.updated_at = chrono::Utc::now();

        Ok(())
    }

    /// Check if user has permission
    #[must_use]
    pub fn has_permission(&self, team_id: &str, user_id: &str, permission: Permission) -> bool {
        let team = match self.teams.get(team_id) {
            Some(t) => t,
            None => return false,
        };

        let member = match team.members.get(user_id) {
            Some(m) => m,
            None => return false,
        };

        // Check each role
        for role_id in &member.roles {
            if self.check_role_permission(team, role_id, permission) {
                return true;
            }
        }

        false
    }

    fn check_role_permission(&self, team: &Team, role_id: &str, permission: Permission) -> bool {
        let role = match team.roles.get(role_id) {
            Some(r) => r,
            None => return false,
        };

        // Check direct permissions
        if role.permissions.contains(&permission) {
            return true;
        }

        // Check inherited roles
        for inherited_id in &role.inherits {
            if self.check_role_permission(team, inherited_id, permission) {
                return true;
            }
        }

        false
    }

    /// Get all permissions for a user
    #[must_use]
    pub fn get_user_permissions(&self, team_id: &str, user_id: &str) -> HashSet<Permission> {
        let mut permissions = HashSet::new();

        let team = match self.teams.get(team_id) {
            Some(t) => t,
            None => return permissions,
        };

        let member = match team.members.get(user_id) {
            Some(m) => m,
            None => return permissions,
        };

        for role_id in &member.roles {
            self.collect_role_permissions(team, role_id, &mut permissions);
        }

        permissions
    }

    fn collect_role_permissions(
        &self,
        team: &Team,
        role_id: &str,
        permissions: &mut HashSet<Permission>,
    ) {
        let role = match team.roles.get(role_id) {
            Some(r) => r,
            None => return,
        };

        permissions.extend(role.permissions.iter().copied());

        for inherited_id in &role.inherits {
            self.collect_role_permissions(team, inherited_id, permissions);
        }
    }

    /// Create custom role
    pub fn create_role(
        &mut self,
        team_id: &str,
        name: String,
        description: String,
        permissions: HashSet<Permission>,
        inherits: Vec<String>,
    ) -> Result<TeamRole> {
        let team = self
            .teams
            .get_mut(team_id)
            .ok_or_else(|| anyhow::anyhow!("Team not found"))?;

        let role_id = name.to_lowercase().replace(' ', "-");
        let role = TeamRole {
            id: role_id.clone(),
            name,
            description,
            is_system: false,
            permissions,
            inherits,
        };

        team.roles.insert(role_id, role.clone());
        team.updated_at = chrono::Utc::now();

        Ok(role)
    }

    /// Delete team
    pub fn delete_team(&mut self, team_id: &str) -> Result<()> {
        self.teams.remove(team_id);
        Ok(())
    }

    /// Update team settings
    pub fn update_settings(&mut self, team_id: &str, settings: TeamSettings) -> Result<()> {
        let team = self
            .teams
            .get_mut(team_id)
            .ok_or_else(|| anyhow::anyhow!("Team not found"))?;

        team.settings = settings;
        team.updated_at = chrono::Utc::now();

        Ok(())
    }
}

impl Default for TeamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_team() {
        let mut manager = TeamManager::new();
        let team = manager.create_team(
            "Test Team".to_string(),
            "org123".to_string(),
            "user1".to_string(),
            "Test User".to_string(),
            "test@example.com".to_string(),
        );

        assert!(team.is_ok());
        assert_eq!(team.unwrap().name, "Test Team");
    }

    #[test]
    fn test_add_member() {
        let mut manager = TeamManager::new();
        let team = manager
            .create_team(
                "Test Team".to_string(),
                "org123".to_string(),
                "user1".to_string(),
                "Test User".to_string(),
                "test@example.com".to_string(),
            )
            .unwrap();

        let member = manager.add_member(
            &team.id,
            "user2".to_string(),
            "Second User".to_string(),
            "second@example.com".to_string(),
            Some("developer".to_string()),
            Some("user1".to_string()),
        );

        assert!(member.is_ok());
    }

    #[test]
    fn test_has_permission() {
        let mut manager = TeamManager::new();
        let team = manager
            .create_team(
                "Test Team".to_string(),
                "org123".to_string(),
                "user1".to_string(),
                "Admin User".to_string(),
                "admin@example.com".to_string(),
            )
            .unwrap();

        // Admin should have all permissions
        assert!(manager.has_permission(&team.id, "user1", Permission::TeamCreate));
        assert!(manager.has_permission(&team.id, "user1", Permission::CodeExecute));
    }

    #[test]
    fn test_get_user_permissions() {
        let mut manager = TeamManager::new();
        let team = manager
            .create_team(
                "Test Team".to_string(),
                "org123".to_string(),
                "user1".to_string(),
                "Admin User".to_string(),
                "admin@example.com".to_string(),
            )
            .unwrap();

        let permissions = manager.get_user_permissions(&team.id, "user1");
        assert!(permissions.contains(&Permission::Admin));
    }
}
