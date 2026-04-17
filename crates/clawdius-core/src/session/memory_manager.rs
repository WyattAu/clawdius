use crate::session::types::Message;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

const MESSAGE_OVERHEAD: usize = 256;
const KEEP_RECENT: usize = 4;
const DEFAULT_COMPACTION_THRESHOLD: u64 = 80;

#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub estimated_bytes: usize,
    pub message_count: usize,
    pub compaction_count: usize,
    pub last_compaction: Option<DateTime<Utc>>,
    pub peak_bytes: usize,
}

pub struct SessionMemoryManager {
    stats: RwLock<HashMap<String, MemoryStats>>,
    global_budget: AtomicU64,
    per_session_budget: AtomicU64,
    compaction_threshold: AtomicU64,
}

fn compact_messages(messages: Vec<Message>) -> Vec<Message> {
    if messages.len() <= KEEP_RECENT + 1 {
        return messages;
    }

    let keep_from = messages.len() - KEEP_RECENT;
    let old_count = keep_from;

    let summary = format!("[Previous context summarized - {old_count} messages compacted]");

    let summary_msg = Message::system(summary);

    let mut result = Vec::with_capacity(KEEP_RECENT + 1);
    result.push(summary_msg);
    result.extend(messages[keep_from..].to_vec());

    result
}

impl SessionMemoryManager {
    #[must_use] 
    pub fn new(global_budget: u64, per_session_budget: u64) -> Self {
        Self {
            stats: RwLock::new(HashMap::new()),
            global_budget: AtomicU64::new(global_budget),
            per_session_budget: AtomicU64::new(per_session_budget),
            compaction_threshold: AtomicU64::new(DEFAULT_COMPACTION_THRESHOLD),
        }
    }

    pub fn set_compaction_threshold(&self, percent: u64) {
        self.compaction_threshold
            .store(percent.min(100), Ordering::Relaxed);
    }

    #[must_use] 
    pub fn estimate_message_size(msg: &Message) -> usize {
        let content_len = msg.as_text().map_or(0, str::len);
        content_len * 2 + MESSAGE_OVERHEAD
    }

    pub fn estimate_session_size(messages: &[Message]) -> usize {
        messages.iter().map(Self::estimate_message_size).sum()
    }

    pub fn track_session(&self, session_id: &str, messages: &[Message]) {
        let estimated_bytes = Self::estimate_session_size(messages);
        let message_count = messages.len();

        let mut stats = self.stats.write().unwrap();
        let entry = stats.entry(session_id.to_string()).or_default();
        entry.estimated_bytes = estimated_bytes;
        entry.message_count = message_count;
        if estimated_bytes > entry.peak_bytes {
            entry.peak_bytes = estimated_bytes;
        }
    }

    pub fn remove_session(&self, session_id: &str) {
        let mut stats = self.stats.write().unwrap();
        stats.remove(session_id);
    }

    pub fn get_stats(&self, session_id: &str) -> Option<MemoryStats> {
        let stats = self.stats.read().unwrap();
        stats.get(session_id).cloned()
    }

    pub fn get_all_stats(&self) -> HashMap<String, MemoryStats> {
        let stats = self.stats.read().unwrap();
        stats.clone()
    }

    pub fn total_memory_usage(&self) -> usize {
        let stats = self.stats.read().unwrap();
        stats.values().map(|s| s.estimated_bytes).sum()
    }

    pub fn should_compact(&self, session_id: &str) -> bool {
        let stats = self.stats.read().unwrap();
        let session_stats = match stats.get(session_id) {
            Some(s) => s,
            None => return false,
        };

        let per_session_budget = self.per_session_budget.load(Ordering::Relaxed);
        if per_session_budget == 0 {
            return false;
        }

        let threshold = self.compaction_threshold.load(Ordering::Relaxed) as f64 / 100.0;
        let budget_threshold = (per_session_budget as f64 * threshold) as usize;

        session_stats.estimated_bytes > budget_threshold
    }

    pub fn check_and_compact(&self, session_id: &str, messages: &mut Vec<Message>) -> bool {
        if !self.should_compact(session_id) {
            return false;
        }

        let old_len = messages.len();
        let compacted = compact_messages(std::mem::take(messages));
        *messages = compacted;

        let mut stats = self.stats.write().unwrap();
        if let Some(entry) = stats.get_mut(session_id) {
            entry.compaction_count += 1;
            entry.last_compaction = Some(Utc::now());
            entry.message_count = messages.len();
            entry.estimated_bytes = Self::estimate_session_size(messages);
        }

        messages.len() != old_len
    }

    pub fn sessions_over_budget(&self) -> Vec<String> {
        let stats = self.stats.read().unwrap();
        let per_session_budget = self.per_session_budget.load(Ordering::Relaxed);

        if per_session_budget == 0 {
            return Vec::new();
        }

        stats
            .iter()
            .filter(|(_, s)| s.estimated_bytes > per_session_budget as usize)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(count: usize, content_len: usize) -> Vec<Message> {
        (0..count)
            .map(|i| {
                let role = if i % 2 == 0 { "user" } else { "assistant" };
                let content = "x".repeat(content_len);
                if role == "user" {
                    Message::user(content)
                } else {
                    Message::assistant(content)
                }
            })
            .collect()
    }

    #[test]
    fn test_new_manager() {
        let mgr = SessionMemoryManager::new(1_000_000, 100_000);
        assert_eq!(mgr.total_memory_usage(), 0);
        assert_eq!(mgr.get_all_stats().len(), 0);
        assert!(!mgr.should_compact("nonexistent"));
        assert!(mgr.sessions_over_budget().is_empty());
    }

    #[test]
    fn test_estimate_message_size() {
        let msg = Message::user("hello");
        let size = SessionMemoryManager::estimate_message_size(&msg);
        assert_eq!(size, 5 * 2 + MESSAGE_OVERHEAD);

        let msg = Message::assistant("");
        let size = SessionMemoryManager::estimate_message_size(&msg);
        assert_eq!(size, MESSAGE_OVERHEAD);
    }

    #[test]
    fn test_track_session() {
        let mgr = SessionMemoryManager::new(1_000_000, 100_000);
        let messages = make_messages(3, 100);

        mgr.track_session("s1", &messages);

        let stats = mgr.get_stats("s1").expect("should have stats");
        assert_eq!(stats.message_count, 3);
        assert_eq!(
            stats.estimated_bytes,
            SessionMemoryManager::estimate_session_size(&messages)
        );
        assert_eq!(stats.peak_bytes, stats.estimated_bytes);
        assert_eq!(stats.compaction_count, 0);
        assert!(stats.last_compaction.is_none());
    }

    #[test]
    fn test_should_compact_under_threshold() {
        let mgr = SessionMemoryManager::new(1_000_000, 10_000);
        let messages = make_messages(2, 10);
        mgr.track_session("s1", &messages);

        assert!(!mgr.should_compact("s1"));
    }

    #[test]
    fn test_should_compact_over_threshold() {
        let mgr = SessionMemoryManager::new(1_000_000, 1_000);
        let messages = make_messages(10, 200);
        mgr.track_session("s1", &messages);

        let size = SessionMemoryManager::estimate_session_size(&messages);
        assert!(size > 0, "sanity: session has non-zero size");
        assert!(mgr.should_compact("s1"));
    }

    #[test]
    fn test_check_and_compact() {
        let mgr = SessionMemoryManager::new(1_000_000, 1_000);
        let mut messages = make_messages(10, 200);
        mgr.track_session("s1", &messages);

        let compacted = mgr.check_and_compact("s1", &mut messages);
        assert!(compacted);
        assert!(messages.len() < 10);

        let stats = mgr.get_stats("s1").expect("should have stats");
        assert_eq!(stats.compaction_count, 1);
        assert!(stats.last_compaction.is_some());
    }

    #[test]
    fn test_remove_session() {
        let mgr = SessionMemoryManager::new(1_000_000, 100_000);
        let messages = make_messages(3, 100);

        mgr.track_session("s1", &messages);
        assert!(mgr.get_stats("s1").is_some());

        mgr.remove_session("s1");
        assert!(mgr.get_stats("s1").is_none());
        assert_eq!(mgr.total_memory_usage(), 0);
    }

    #[test]
    fn test_total_memory_usage() {
        let mgr = SessionMemoryManager::new(1_000_000, 100_000);
        let msgs1 = make_messages(2, 100);
        let msgs2 = make_messages(3, 200);

        mgr.track_session("s1", &msgs1);
        mgr.track_session("s2", &msgs2);

        let expected = SessionMemoryManager::estimate_session_size(&msgs1)
            + SessionMemoryManager::estimate_session_size(&msgs2);
        assert_eq!(mgr.total_memory_usage(), expected);
    }

    #[test]
    fn test_sessions_over_budget() {
        let mgr = SessionMemoryManager::new(1_000_000, 500);
        let small = make_messages(1, 10);
        let big = make_messages(10, 200);

        mgr.track_session("s1", &small);
        mgr.track_session("s2", &big);

        let over = mgr.sessions_over_budget();
        assert_eq!(over, vec!["s2".to_string()]);
    }
}
