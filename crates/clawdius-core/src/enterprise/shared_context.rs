//! Shared Context for Enterprise Teams
//!
//! Team members can share context windows (files, code snippets, sessions)
//! that other team members can access and reference.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of shared context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextType {
    Files,
    CodeSnippets,
    SessionHistory,
    Documentation,
    Configuration,
    Custom,
}

impl Default for ContextType {
    fn default() -> Self {
        Self::Custom
    }
}

/// Access level for shared context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    ReadOnly,
    ReadWrite,
    FullAccess,
}

impl Default for AccessLevel {
    fn default() -> Self {
        Self::ReadOnly
    }
}

/// Code snippet with language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub file_path: String,
    pub line_range: (usize, usize),
    pub content: String,
    pub language: String,
}

/// Context data payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextData {
    pub file_paths: Vec<String>,
    pub code_snippets: Vec<CodeSnippet>,
    pub session_ids: Vec<String>,
    pub documentation_urls: Vec<String>,
    pub configuration: HashMap<String, String>,
    pub custom_data: HashMap<String, serde_json::Value>,
}

impl Default for ContextData {
    fn default() -> Self {
        Self {
            file_paths: Vec::new(),
            code_snippets: Vec::new(),
            session_ids: Vec::new(),
            documentation_urls: Vec::new(),
            configuration: HashMap::new(),
            custom_data: HashMap::new(),
        }
    }
}

/// Shared context entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedContext {
    pub id: String,
    pub team_id: String,
    pub created_by: String,
    pub name: String,
    pub description: Option<String>,
    pub context_type: ContextType,
    pub data: ContextData,
    pub tags: Vec<String>,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub access_level: AccessLevel,
}

impl SharedContext {
    pub fn new(team_id: String, created_by: String, name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            team_id,
            created_by,
            name,
            description: None,
            context_type: ContextType::Custom,
            data: ContextData::default(),
            tags: Vec::new(),
            is_pinned: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            expires_at: None,
            access_level: AccessLevel::ReadOnly,
        }
    }

    /// Check if context has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = &self.expires_at {
            return expires_at < &Utc::now();
        }
        false
    }
}

/// Context filter for queries
#[derive(Debug, Clone, Default)]
pub struct ContextFilter {
    pub context_type: Option<ContextType>,
    pub created_by: Option<String>,
    pub tags: Vec<String>,
    pub is_pinned: Option<bool>,
}

/// Context updates
#[derive(Debug, Clone, Default)]
pub struct ContextUpdates {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub is_pinned: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
}
