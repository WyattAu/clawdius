use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAuditEntry {
    pub id: String,
    pub tenant_id: String,
    pub session_id: String,
    pub iteration: usize,
    pub llm_request_tokens: u32,
    pub llm_response_tokens: u32,
    pub tool_calls: Vec<ToolCallAuditRecord>,
    pub timestamp: DateTime<Utc>,
    pub prev_entry_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallAuditRecord {
    pub tool_name: String,
    pub arguments_hash: String,
    pub result_size_bytes: usize,
    pub is_error: bool,
    pub duration_ms: u64,
}

pub struct AgentAuditLog {
    entries: Vec<AgentAuditEntry>,
    tenant_key: String,
}

impl AgentAuditLog {
    #[must_use] 
    pub fn new(tenant_key: &str) -> Self {
        Self {
            entries: Vec::new(),
            tenant_key: tenant_key.to_string(),
        }
    }

    pub fn append(&mut self, mut entry: AgentAuditEntry) -> anyhow::Result<String> {
        entry.prev_entry_hash = self
            .entries
            .last()
            .map(|e| e.entry_hash.clone())
            .unwrap_or_default();

        let hash = self.compute_entry_hash(&entry);
        entry.entry_hash = hash;
        let id = entry.id.clone();
        self.entries.push(entry);
        Ok(id)
    }

    pub fn verify_integrity(&self) -> anyhow::Result<bool> {
        let mut prev_hash = String::new();
        for entry in &self.entries {
            if entry.prev_entry_hash != prev_hash {
                return Ok(false);
            }
            let expected = self.compute_entry_hash(entry);
            if entry.entry_hash != expected {
                return Ok(false);
            }
            prev_hash = entry.entry_hash.clone();
        }
        Ok(true)
    }

    #[must_use] 
    pub fn entries_for_session(&self, session_id: &str) -> Vec<&AgentAuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.session_id == session_id)
            .collect()
    }

    #[must_use] 
    pub fn entries_for_tenant(&self, tenant_id: &str) -> Vec<&AgentAuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.tenant_id == tenant_id)
            .collect()
    }

    fn compute_entry_hash(&self, entry: &AgentAuditEntry) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(entry.id.as_bytes());
        hasher.update(entry.tenant_id.as_bytes());
        hasher.update(entry.session_id.as_bytes());
        hasher.update(&entry.iteration.to_le_bytes());
        hasher.update(&entry.llm_request_tokens.to_le_bytes());
        hasher.update(&entry.llm_response_tokens.to_le_bytes());
        hasher.update(&(entry.tool_calls.len() as u64).to_le_bytes());
        for tc in &entry.tool_calls {
            hasher.update(tc.tool_name.as_bytes());
            hasher.update(tc.arguments_hash.as_bytes());
            hasher.update(&tc.result_size_bytes.to_le_bytes());
            hasher.update(&[u8::from(tc.is_error)]);
            hasher.update(&tc.duration_ms.to_le_bytes());
        }
        hasher.update(entry.timestamp.to_rfc3339().as_bytes());
        hasher.update(entry.prev_entry_hash.as_bytes());
        hasher.update(self.tenant_key.as_bytes());
        hasher.finalize().to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_entry(
        id: &str,
        tenant_id: &str,
        session_id: &str,
        iteration: usize,
    ) -> AgentAuditEntry {
        AgentAuditEntry {
            id: id.to_string(),
            tenant_id: tenant_id.to_string(),
            session_id: session_id.to_string(),
            iteration,
            llm_request_tokens: 100,
            llm_response_tokens: 50,
            tool_calls: vec![ToolCallAuditRecord {
                tool_name: "read_file".to_string(),
                arguments_hash: blake3::hash(b"{}").to_hex().to_string(),
                result_size_bytes: 1024,
                is_error: false,
                duration_ms: 42,
            }],
            timestamp: Utc::now(),
            prev_entry_hash: String::new(),
            entry_hash: String::new(),
        }
    }

    #[test]
    fn test_append_and_verify_integrity() {
        let mut log = AgentAuditLog::new("test-key");
        let entry = make_entry("id-1", "tenant-a", "session-1", 1);
        log.append(entry).unwrap();
        assert!(log.verify_integrity().unwrap());
    }

    #[test]
    fn test_tamper_detection() {
        let mut log = AgentAuditLog::new("test-key");
        log.append(make_entry("id-1", "tenant-a", "session-1", 1))
            .unwrap();
        log.append(make_entry("id-2", "tenant-a", "session-1", 2))
            .unwrap();
        log.entries[1].llm_request_tokens = 9999;
        assert!(!log.verify_integrity().unwrap());
    }

    #[test]
    fn test_filtering_by_session_and_tenant() {
        let mut log = AgentAuditLog::new("test-key");
        log.append(make_entry("id-1", "tenant-a", "session-1", 1))
            .unwrap();
        log.append(make_entry("id-2", "tenant-a", "session-2", 1))
            .unwrap();
        log.append(make_entry("id-3", "tenant-b", "session-1", 1))
            .unwrap();

        assert_eq!(log.entries_for_session("session-1").len(), 2);
        assert_eq!(log.entries_for_session("session-2").len(), 1);
        assert_eq!(log.entries_for_tenant("tenant-a").len(), 2);
        assert_eq!(log.entries_for_tenant("tenant-b").len(), 1);
    }

    #[test]
    fn test_hash_chain_continuity() {
        let mut log = AgentAuditLog::new("test-key");
        log.append(make_entry("id-1", "tenant-a", "session-1", 1))
            .unwrap();
        log.append(make_entry("id-2", "tenant-a", "session-1", 2))
            .unwrap();
        log.append(make_entry("id-3", "tenant-a", "session-1", 3))
            .unwrap();

        assert_eq!(log.entries[0].prev_entry_hash, "");
        assert_eq!(log.entries[1].prev_entry_hash, log.entries[0].entry_hash);
        assert_eq!(log.entries[2].prev_entry_hash, log.entries[1].entry_hash);
    }

    #[test]
    fn test_empty_log_integrity() {
        let log = AgentAuditLog::new("test-key");
        assert!(log.verify_integrity().unwrap());
    }
}
