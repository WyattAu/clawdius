use crate::error::Result;
use crate::session::SessionId;
use crate::Error;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionRole {
    Owner,
    Editor,
    Viewer,
}

impl SessionRole {
    const fn level(&self) -> u8 {
        match self {
            Self::Owner => 3,
            Self::Editor => 2,
            Self::Viewer => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMember {
    pub user_id: String,
    pub role: SessionRole,
    pub joined_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

pub struct SessionMembership {
    session_id: SessionId,
    members: RwLock<HashMap<String, SessionMember>>,
}

impl SessionMembership {
    #[must_use] 
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            members: RwLock::new(HashMap::new()),
        }
    }

    pub const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn add_member(&self, user_id: &str, role: SessionRole) -> Result<()> {
        let mut members = self
            .members
            .write()
            .map_err(|e| Error::Session(format!("membership lock poisoned: {e}")))?;

        if members.contains_key(user_id) {
            return Err(Error::Session(format!(
                "user '{user_id}' is already a member"
            )));
        }

        if role == SessionRole::Owner
            && members.values().any(|m| m.role == SessionRole::Owner) {
                return Err(Error::Session(
                    "cannot add a second owner; only one owner is allowed".to_string(),
                ));
            }

        let now = Utc::now();
        members.insert(
            user_id.to_string(),
            SessionMember {
                user_id: user_id.to_string(),
                role,
                joined_at: now,
                last_active_at: now,
            },
        );
        Ok(())
    }

    pub fn remove_member(&self, user_id: &str) -> Result<()> {
        let mut members = self
            .members
            .write()
            .map_err(|e| Error::Session(format!("membership lock poisoned: {e}")))?;

        let member = members
            .get(user_id)
            .ok_or_else(|| Error::Session(format!("user '{user_id}' is not a member")))?;

        if member.role == SessionRole::Owner {
            return Err(Error::Session(
                "cannot remove the session owner".to_string(),
            ));
        }

        members.remove(user_id);
        Ok(())
    }

    pub fn change_role(&self, user_id: &str, new_role: SessionRole) -> Result<()> {
        let mut members = self
            .members
            .write()
            .map_err(|e| Error::Session(format!("membership lock poisoned: {e}")))?;

        if !members.contains_key(user_id) {
            return Err(Error::Session(format!("user '{user_id}' is not a member")));
        }

        if new_role == SessionRole::Owner
            && members
                .get(user_id)
                .is_some_and(|m| m.role != SessionRole::Owner)
            && members.values().any(|m| m.role == SessionRole::Owner)
        {
            return Err(Error::Session(
                "only one owner is allowed; cannot promote to owner".to_string(),
            ));
        }

        members.get_mut(user_id).expect("key checked above").role = new_role;
        Ok(())
    }

    pub fn get_member(&self, user_id: &str) -> Option<SessionMember> {
        let members = self.members.read().ok()?;
        members.get(user_id).cloned()
    }

    pub fn list_members(&self) -> Vec<SessionMember> {
        let members = self.members.read().ok();
        match members {
            Some(guard) => guard.values().cloned().collect(),
            None => Vec::new(),
        }
    }

    pub fn is_member(&self, user_id: &str) -> bool {
        let members = self.members.read().ok();
        match members {
            Some(guard) => guard.contains_key(user_id),
            None => false,
        }
    }

    pub fn check_permission(&self, user_id: &str, required_role: SessionRole) -> Result<()> {
        let members = self
            .members
            .read()
            .map_err(|e| Error::Session(format!("membership lock poisoned: {e}")))?;

        let member = members.get(user_id).ok_or_else(|| {
            Error::Session(format!(
                "permission denied: user '{user_id}' is not a member"
            ))
        })?;

        if member.role.level() < required_role.level() {
            return Err(Error::Session(format!(
                "permission denied: user '{user_id}' has {:?} role but {:?} is required",
                member.role, required_role
            )));
        }

        Ok(())
    }

    pub fn update_presence(&self, user_id: &str) {
        if let Ok(mut members) = self.members.write() {
            if let Some(member) = members.get_mut(user_id) {
                member.last_active_at = Utc::now();
            }
        }
    }

    pub fn active_members(&self, duration: chrono::Duration) -> Vec<SessionMember> {
        let cutoff = Utc::now() - duration;
        let members = self.members.read().ok();
        match members {
            Some(guard) => guard
                .values()
                .filter(|m| m.last_active_at > cutoff)
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn member_count(&self) -> usize {
        self.members.read().map(|g| g.len()).unwrap_or(0)
    }

    pub fn owner_id(&self) -> Option<String> {
        let members = self.members.read().ok()?;
        members
            .values()
            .find(|m| m.role == SessionRole::Owner)
            .map(|m| m.user_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn setup_membership() -> SessionMembership {
        SessionMembership::new(SessionId::new())
    }

    #[test]
    fn test_new_membership() {
        let membership = setup_membership();
        assert_eq!(membership.member_count(), 0);
        assert_eq!(membership.owner_id(), None);
        assert!(membership.list_members().is_empty());
    }

    #[test]
    fn test_add_owner() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        assert_eq!(membership.member_count(), 1);
        assert_eq!(membership.owner_id(), Some("alice".to_string()));
        let member = membership.get_member("alice").unwrap();
        assert_eq!(member.role, SessionRole::Owner);
    }

    #[test]
    fn test_add_editor() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Editor).unwrap();
        assert_eq!(membership.member_count(), 2);
        let member = membership.get_member("bob").unwrap();
        assert_eq!(member.role, SessionRole::Editor);
    }

    #[test]
    fn test_add_viewer() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("carol", SessionRole::Viewer).unwrap();
        assert_eq!(membership.member_count(), 2);
        let member = membership.get_member("carol").unwrap();
        assert_eq!(member.role, SessionRole::Viewer);
    }

    #[test]
    fn test_cannot_add_second_owner() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        let result = membership.add_member("bob", SessionRole::Owner);
        assert!(result.is_err());
        assert_eq!(membership.member_count(), 1);
    }

    #[test]
    fn test_remove_member() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Editor).unwrap();
        membership.remove_member("bob").unwrap();
        assert_eq!(membership.member_count(), 1);
        assert!(!membership.is_member("bob"));
    }

    #[test]
    fn test_cannot_remove_owner() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        let result = membership.remove_member("alice");
        assert!(result.is_err());
        assert_eq!(membership.member_count(), 1);
    }

    #[test]
    fn test_change_role() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Viewer).unwrap();
        membership.change_role("bob", SessionRole::Editor).unwrap();
        assert_eq!(
            membership.get_member("bob").unwrap().role,
            SessionRole::Editor
        );
    }

    #[test]
    fn test_permission_check_viewer() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Viewer).unwrap();
        membership
            .check_permission("bob", SessionRole::Viewer)
            .unwrap();
    }

    #[test]
    fn test_permission_check_editor() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Editor).unwrap();
        membership
            .check_permission("bob", SessionRole::Editor)
            .unwrap();
        membership
            .check_permission("bob", SessionRole::Viewer)
            .unwrap();
    }

    #[test]
    fn test_permission_check_owner() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership
            .check_permission("alice", SessionRole::Owner)
            .unwrap();
        membership
            .check_permission("alice", SessionRole::Editor)
            .unwrap();
        membership
            .check_permission("alice", SessionRole::Viewer)
            .unwrap();
    }

    #[test]
    fn test_viewer_cannot_edit() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Viewer).unwrap();
        let result = membership.check_permission("bob", SessionRole::Editor);
        assert!(result.is_err());
    }

    #[test]
    fn test_editor_cannot_manage() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Editor).unwrap();
        let result = membership.check_permission("bob", SessionRole::Owner);
        assert!(result.is_err());
    }

    #[test]
    fn test_non_member_denied() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        let result = membership.check_permission("stranger", SessionRole::Viewer);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_presence() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        let original = membership.get_member("alice").unwrap().last_active_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        membership.update_presence("alice");
        let updated = membership.get_member("alice").unwrap().last_active_at;
        assert!(updated > original);
    }

    #[test]
    fn test_active_members() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Editor).unwrap();
        let active = membership.active_members(Duration::seconds(5));
        assert_eq!(active.len(), 2);
        let active_ids: Vec<&str> = active.iter().map(|m| m.user_id.as_str()).collect();
        assert!(active_ids.contains(&"alice"));
        assert!(active_ids.contains(&"bob"));

        let none = membership.active_members(Duration::nanoseconds(1));
        assert!(none.is_empty());
    }

    #[test]
    fn test_list_members() {
        let membership = setup_membership();
        membership.add_member("alice", SessionRole::Owner).unwrap();
        membership.add_member("bob", SessionRole::Editor).unwrap();
        membership.add_member("carol", SessionRole::Viewer).unwrap();
        let all = membership.list_members();
        assert_eq!(all.len(), 3);
        let ids: Vec<&str> = all.iter().map(|m| m.user_id.as_str()).collect();
        assert!(ids.contains(&"alice"));
        assert!(ids.contains(&"bob"));
        assert!(ids.contains(&"carol"));
    }
}
