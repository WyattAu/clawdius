//! Session Binder
//!
//! Manages user sessions across messaging platforms with persistence support.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::state_store::StateStore;
use super::types::{
    MessagingError, MessagingSession, PermissionSet, Platform, PlatformUserId, Result, SessionState,
};

pub struct SessionBinder {
    sessions: Arc<RwLock<HashMap<String, MessagingSession>>>,
    user_permissions: Arc<RwLock<HashMap<String, PermissionSet>>>,
    db_path: Option<String>,
    pub state_store: Option<Arc<dyn StateStore>>,
}

impl SessionBinder {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            user_permissions: Arc::new(RwLock::new(HashMap::new())),
            db_path: None,
            state_store: None,
        }
    }

    pub fn with_persistence(db_path: impl Into<String>) -> Self {
        let path = db_path.into();
        Self::init_db(&path);
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            user_permissions: Arc::new(RwLock::new(HashMap::new())),
            db_path: Some(path),
            state_store: None,
        }
    }

    pub fn with_state_store(mut self, store: Arc<dyn StateStore>) -> Self {
        self.state_store = Some(store);
        self
    }

    pub fn has_persistence(&self) -> bool {
        self.db_path.is_some()
    }

    pub async fn load_from_db(&self) -> usize {
        if let Some(store) = &self.state_store {
            let keys = match store.keys("messaging_sessions", "*").await {
                Ok(k) => k,
                Err(_) => return 0,
            };
            let count = keys.len();
            let mut sessions_map = self.sessions.write().await;
            for key in &keys {
                if let Ok(Some(value)) = store.get("messaging_sessions", key).await {
                    if let Ok(session) = serde_json::from_slice::<MessagingSession>(&value) {
                        sessions_map.entry(key.clone()).or_insert(session);
                    }
                }
            }
            drop(sessions_map);
            if let Ok(perm_keys) = store.keys("session_permissions", "*").await {
                let mut perms_map = self.user_permissions.write().await;
                for key in &perm_keys {
                    if let Ok(Some(value)) = store.get("session_permissions", key).await {
                        if let Ok(perm) = serde_json::from_slice::<PermissionSet>(&value) {
                            perms_map.entry(key.clone()).or_insert(perm);
                        }
                    }
                }
            }
            return count;
        }

        let Some(ref path) = self.db_path else {
            return 0;
        };
        let path = path.clone();

        let sessions_result = tokio::task::spawn_blocking(move || {
            let conn = match rusqlite::Connection::open(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to open session DB for load");
                    return (Vec::new(), Vec::new());
                }
            };

            let mut sessions: Vec<(String, MessagingSession)> = Vec::new();
            let mut permissions: Vec<(String, PermissionSet)> = Vec::new();

            let mut stmt = match conn.prepare(
                "SELECT key, id, platform, user_id, clawdius_sid, created_at, last_activity, message_count, state, can_generate, can_analyze, can_modify_files, can_execute, can_admin FROM messaging_sessions WHERE state != 'Closed'",
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to prepare session load query");
                    return (sessions, permissions);
                }
            };

            let rows = match stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i32>(9)?,
                    row.get::<_, i32>(10)?,
                    row.get::<_, i32>(11)?,
                    row.get::<_, i32>(12)?,
                    row.get::<_, i32>(13)?,
                ))
            }) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to query sessions from DB");
                    return (sessions, permissions);
                }
            };

            for row_result in rows {
                let row = match row_result {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to read session row from DB");
                        continue;
                    }
                };
                let (
                    key,
                    id,
                    platform,
                    user_id,
                    clawdius_sid,
                    created_at,
                    last_activity,
                    message_count,
                    state,
                    can_generate,
                    can_analyze,
                    can_modify_files,
                    can_execute,
                    can_admin,
                ): (String, String, String, String, Option<String>, String, String, i64, String, i32, i32, i32, i32, i32) = row;

                let platform = match platform.as_str() {
                    "telegram" => Platform::Telegram,
                    "discord" => Platform::Discord,
                    "matrix" => Platform::Matrix,
                    "signal" => Platform::Signal,
                    "rocketchat" => Platform::RocketChat,
                    "whatsapp" => Platform::WhatsApp,
                    "slack" => Platform::Slack,
                    "webhook" => Platform::Webhook,
                    _ => continue,
                };

                let state = match state.as_str() {
                    "Active" => SessionState::Active,
                    "Idle" => SessionState::Idle,
                    "Compacted" => SessionState::Compacted,
                    "Closed" => SessionState::Closed,
                    _ => SessionState::Active,
                };

                let parsed_id = Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::nil());
                let parsed_created =
                    DateTime::parse_from_rfc3339(&created_at).map(|d| d.with_timezone(&Utc));
                let parsed_last = DateTime::parse_from_rfc3339(&last_activity)
                    .map(|d| d.with_timezone(&Utc));
                let parsed_sid =
                    clawdius_sid.and_then(|s| Uuid::parse_str(&s).ok());

                sessions.push((key, MessagingSession {
                    id: parsed_id,
                    platform_user: PlatformUserId::new(platform, user_id),
                    clawdius_session_id: parsed_sid,
                    created_at: parsed_created.unwrap_or_else(|_| Utc::now()),
                    last_activity: parsed_last.unwrap_or_else(|_| Utc::now()),
                    message_count: message_count as u64,
                    state,
                    permissions: PermissionSet {
                        can_generate: can_generate != 0,
                        can_analyze: can_analyze != 0,
                        can_modify_files: can_modify_files != 0,
                        can_execute: can_execute != 0,
                        can_admin: can_admin != 0,
                    },
                }));
            }

            if let Ok(mut stmt) = conn.prepare(
                "SELECT key, can_generate, can_analyze, can_modify_files, can_execute, can_admin FROM session_permissions",
            ) {
                let rows = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)?,
                        row.get::<_, i32>(3)?,
                        row.get::<_, i32>(4)?,
                        row.get::<_, i32>(5)?,
                    ))
                });

                if let Ok(rows) = rows {
                    for row in rows.flatten() {
                        let (key, cg, ca, cmf, ce, ca2): (String, i32, i32, i32, i32, i32) = row;
                        permissions.push((
                            key,
                            PermissionSet {
                                can_generate: cg != 0,
                                can_analyze: ca != 0,
                                can_modify_files: cmf != 0,
                                can_execute: ce != 0,
                                can_admin: ca2 != 0,
                            },
                        ));
                    }
                }
            }

            (sessions, permissions)
        })
        .await;

        match sessions_result {
            Ok((sessions, permissions)) => {
                tracing::info!(
                    session_count = sessions.len(),
                    perm_count = permissions.len(),
                    "Loaded from DB"
                );
                let count = sessions.len();
                let mut sessions_map = self.sessions.write().await;
                for (key, session) in sessions {
                    sessions_map.entry(key).or_insert(session);
                }

                let mut perms_map = self.user_permissions.write().await;
                for (key, perm) in permissions {
                    perms_map.entry(key).or_insert(perm);
                }
                count
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to load sessions from DB");
                0
            }
        }
    }

    fn init_db(path: &str) {
        let conn = rusqlite::Connection::open(path).expect("Failed to open session database");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messaging_sessions (
                key             TEXT PRIMARY KEY,
                id              TEXT NOT NULL,
                platform        TEXT NOT NULL,
                user_id         TEXT NOT NULL,
                clawdius_sid    TEXT,
                created_at      TEXT NOT NULL,
                last_activity   TEXT NOT NULL,
                message_count   INTEGER NOT NULL DEFAULT 0,
                state           TEXT NOT NULL DEFAULT 'Active',
                can_generate    BOOLEAN NOT NULL DEFAULT 1,
                can_analyze     BOOLEAN NOT NULL DEFAULT 1,
                can_modify_files BOOLEAN NOT NULL DEFAULT 0,
                can_execute     BOOLEAN NOT NULL DEFAULT 0,
                can_admin       BOOLEAN NOT NULL DEFAULT 0
            )",
            [],
        )
        .expect("Failed to create sessions table");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_permissions (
                key             TEXT PRIMARY KEY,
                can_generate    BOOLEAN NOT NULL DEFAULT 1,
                can_analyze     BOOLEAN NOT NULL DEFAULT 1,
                can_modify_files BOOLEAN NOT NULL DEFAULT 0,
                can_execute     BOOLEAN NOT NULL DEFAULT 0,
                can_admin       BOOLEAN NOT NULL DEFAULT 0
            )",
            [],
        )
        .expect("Failed to create permissions table");

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_last_activity ON messaging_sessions(last_activity)",
            [],
        )
        .ok();
    }

    async fn persist_session_db(&self, key: &str, session: &MessagingSession) {
        if let Some(store) = &self.state_store {
            let key = key.to_string();
            let value = serde_json::to_vec(session).unwrap_or_default();
            let _ = store.set("messaging_sessions", &key, &value, None).await;
            return;
        }

        let Some(ref path) = self.db_path else {
            return;
        };
        let path = path.clone();
        let key = key.to_string();
        let platform = session.platform_user.platform.as_str().to_string();
        let user_id = session.platform_user.user_id.clone();
        let id = session.id.to_string();
        let clawdius_sid = session.clawdius_session_id.map(|u| u.to_string());
        let created_at = session.created_at.to_rfc3339();
        let last_activity = session.last_activity.to_rfc3339();
        let message_count = session.message_count as i64;
        let state = format!("{:?}", session.state);
        let can_generate = session.permissions.can_generate;
        let can_analyze = session.permissions.can_analyze;
        let can_modify_files = session.permissions.can_modify_files;
        let can_execute = session.permissions.can_execute;
        let can_admin = session.permissions.can_admin;

        match tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute(
                "INSERT OR REPLACE INTO messaging_sessions (key, id, platform, user_id, clawdius_sid, created_at, last_activity, message_count, state, can_generate, can_analyze, can_modify_files, can_execute, can_admin) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                rusqlite::params![key, id, platform, user_id, clawdius_sid, created_at, last_activity, message_count, state, can_generate, can_analyze, can_modify_files, can_execute, can_admin],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "Failed to persist session"),
            Err(e) => tracing::warn!(error = %e, "spawn_blocking failed for session persist"),
        }
    }

    async fn delete_session_db(&self, key: &str) {
        if let Some(store) = &self.state_store {
            let _ = store.delete("messaging_sessions", key).await;
            return;
        }

        let Some(ref path) = self.db_path else {
            return;
        };
        let path = path.clone();
        let key = key.to_string();

        match tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute(
                "DELETE FROM messaging_sessions WHERE key = ?1",
                rusqlite::params![key],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "Failed to delete session from DB"),
            Err(e) => tracing::warn!(error = %e, "spawn_blocking failed for delete"),
        }
    }

    async fn persist_permissions_db(&self, key: &str, perms: &PermissionSet) {
        if let Some(store) = &self.state_store {
            let key = key.to_string();
            let value = serde_json::to_vec(perms).unwrap_or_default();
            let _ = store.set("session_permissions", &key, &value, None).await;
            return;
        }

        let Some(ref path) = self.db_path else {
            return;
        };
        let path = path.clone();
        let key = key.to_string();
        let can_generate = perms.can_generate;
        let can_analyze = perms.can_analyze;
        let can_modify_files = perms.can_modify_files;
        let can_execute = perms.can_execute;
        let can_admin = perms.can_admin;

        match tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute(
                "INSERT OR REPLACE INTO session_permissions (key, can_generate, can_analyze, can_modify_files, can_execute, can_admin) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![key, can_generate, can_analyze, can_modify_files, can_execute, can_admin],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "Failed to persist permissions"),
            Err(e) => tracing::warn!(error = %e, "spawn_blocking failed for permissions"),
        }
    }

    pub async fn bind_session(&self, platform_user: &PlatformUserId) -> Result<MessagingSession> {
        let key = platform_user.composite_key();
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get(&key) {
            let mut session = session.clone();
            session.last_activity = Utc::now();
            session.message_count += 1;
            sessions.insert(key.clone(), session.clone());
            drop(sessions);
            self.persist_session_db(&key, &session).await;
            return Ok(session);
        }

        let permissions = self.get_permissions(&key).await;
        let session = MessagingSession {
            id: Uuid::new_v4(),
            platform_user: platform_user.clone(),
            clawdius_session_id: None,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            message_count: 1,
            state: SessionState::Active,
            permissions,
        };

        sessions.insert(key.clone(), session.clone());
        drop(sessions);
        self.persist_session_db(&key, &session).await;
        Ok(session)
    }

    pub async fn get_session(&self, platform_user: &PlatformUserId) -> Option<MessagingSession> {
        let sessions = self.sessions.read().await;
        sessions.get(&platform_user.composite_key()).cloned()
    }

    pub async fn update_activity(&self, platform_user: &PlatformUserId) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let key = platform_user.composite_key();

        if let Some(session) = sessions.get_mut(&key) {
            session.last_activity = Utc::now();
            session.message_count += 1;
            let session = session.clone();
            drop(sessions);
            self.persist_session_db(&key, &session).await;
            Ok(())
        } else {
            Err(MessagingError::SessionNotFound(key))
        }
    }

    pub async fn close_session(&self, platform_user: &PlatformUserId) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let key = platform_user.composite_key();

        if let Some(session) = sessions.get_mut(&key) {
            session.state = SessionState::Closed;
            let session = session.clone();
            drop(sessions);
            self.persist_session_db(&key, &session).await;
            Ok(())
        } else {
            Err(MessagingError::SessionNotFound(key))
        }
    }

    pub async fn link_clawdius_session(
        &self,
        platform_user: &PlatformUserId,
        clawdius_session_id: Uuid,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let key = platform_user.composite_key();

        if let Some(session) = sessions.get_mut(&key) {
            session.clawdius_session_id = Some(clawdius_session_id);
            let session = session.clone();
            drop(sessions);
            self.persist_session_db(&key, &session).await;
            Ok(())
        } else {
            Err(MessagingError::SessionNotFound(key))
        }
    }

    pub async fn set_permissions(&self, user_key: &str, permissions: PermissionSet) {
        let mut user_perms = self.user_permissions.write().await;
        user_perms.insert(user_key.to_string(), permissions.clone());
        drop(user_perms);
        self.persist_permissions_db(user_key, &permissions).await;
    }

    async fn get_permissions(&self, user_key: &str) -> PermissionSet {
        let user_perms = self.user_permissions.read().await;
        user_perms.get(user_key).cloned().unwrap_or_default()
    }

    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    pub async fn sessions_for_user(&self, user_id: &str) -> usize {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| s.state != SessionState::Closed && s.platform_user.user_id == user_id)
            .count()
    }

    pub async fn cleanup_idle_sessions(&self, max_idle_minutes: i64) {
        let mut sessions = self.sessions.write().await;
        let cutoff = Utc::now() - chrono::Duration::minutes(max_idle_minutes);

        let removed: Vec<(String, MessagingSession)> = {
            let mut removed = Vec::new();
            let keys: Vec<String> = sessions
                .iter()
                .filter(|(_, session)| {
                    session.last_activity <= cutoff || session.state == SessionState::Closed
                })
                .map(|(k, _)| k.clone())
                .collect();
            for key in keys {
                if let Some(session) = sessions.remove(&key) {
                    removed.push((key, session));
                }
            }
            removed
        };

        drop(sessions);

        for (key, _) in &removed {
            self.delete_session_db(key).await;
        }
    }
}

impl Default for SessionBinder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::types::Platform;
    use tempfile;

    #[tokio::test]
    async fn test_session_binding() {
        let binder = SessionBinder::new();
        let user = PlatformUserId::new(Platform::Telegram, "user-123");

        let session = binder.bind_session(&user).await.unwrap();

        assert_eq!(session.platform_user.platform, Platform::Telegram);
        assert_eq!(session.message_count, 1);
        assert_eq!(session.state, SessionState::Active);
    }

    #[tokio::test]
    async fn test_session_persistence() {
        let binder = SessionBinder::new();
        let user = PlatformUserId::new(Platform::Telegram, "user-456");

        let session1 = binder.bind_session(&user).await.unwrap();
        let session2 = binder.bind_session(&user).await.unwrap();

        assert_eq!(session1.id, session2.id);
        assert_eq!(session2.message_count, 2);
    }

    #[tokio::test]
    async fn test_session_close() {
        let binder = SessionBinder::new();
        let user = PlatformUserId::new(Platform::Discord, "user-789");

        binder.bind_session(&user).await.unwrap();
        binder.close_session(&user).await.unwrap();

        let session = binder.get_session(&user).await.unwrap();
        assert_eq!(session.state, SessionState::Closed);
    }

    #[tokio::test]
    async fn test_custom_permissions() {
        let binder = SessionBinder::new();
        let user = PlatformUserId::new(Platform::Matrix, "admin-1");

        binder
            .set_permissions(&user.composite_key(), PermissionSet::admin())
            .await;

        let session = binder.bind_session(&user).await.unwrap();
        assert!(session.permissions.can_admin);
        assert!(session.permissions.can_execute);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions.db");
        let path_str = path.to_str().unwrap().to_string();

        let binder = SessionBinder::with_persistence(&path_str);
        assert!(binder.has_persistence());

        let user = PlatformUserId::new(Platform::Telegram, "persist-user");
        let session1 = binder.bind_session(&user).await.unwrap();
        let original_id = session1.id;

        let user2 = PlatformUserId::new(Platform::Discord, "persist-user2");
        binder.bind_session(&user2).await.unwrap();

        binder
            .set_permissions("persist-user", PermissionSet::admin())
            .await;

        drop(binder);

        let path_check = path_str.clone();
        let db_count = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path_check).unwrap();
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM messaging_sessions", [], |r| r.get(0))
                .unwrap();
            count
        })
        .await
        .unwrap();
        assert!(
            db_count >= 2,
            "DB should have at least 2 sessions, has {}",
            db_count
        );

        let binder2 = SessionBinder::with_persistence(&path_str);
        let loaded_count = binder2.load_from_db().await;
        assert!(
            loaded_count >= 2,
            "should have loaded at least 2 sessions, got {}",
            loaded_count
        );

        let loaded = binder2.get_session(&user).await;
        assert!(
            loaded.is_some(),
            "session should be loaded for key {}",
            user.composite_key()
        );
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, original_id);
        assert_eq!(loaded.message_count, 1);

        let loaded2 = binder2.get_session(&user2).await.unwrap();
        assert_eq!(loaded2.message_count, 1);
    }

    #[tokio::test]
    async fn test_no_persistence() {
        let binder = SessionBinder::new();
        assert!(!binder.has_persistence());
    }
}
