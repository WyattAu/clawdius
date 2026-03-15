//! Event debouncing for file watching
//!
//! Provides debouncing to batch rapid file changes into single events.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize}; // Keep for DebounceConfig

/// Debounce configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebounceConfig {
    /// Minimum time between events in milliseconds
    pub min_interval_ms: u64,
    /// Maximum time to wait before flushing
    pub max_wait_ms: u64,
    /// Maximum events to batch
    pub max_batch_size: usize,
}

impl Default for DebounceConfig {
    fn default() -> Self {
        Self {
            min_interval_ms: 50,
            max_wait_ms: 500,
            max_batch_size: 100,
        }
    }
}

/// Debounced event
/// Note: This struct is not serializable because Instant doesn't implement Serialize/Deserialize.
/// It's intended for internal use within the debouncer.
#[derive(Debug, Clone)]
pub struct DebouncedEvent {
    /// Path that changed
    pub path: PathBuf,
    /// Event kind
    pub kind: DebouncedEventKind,
    /// Number of times this event occurred
    pub count: usize,
    /// First occurrence time
    pub first_seen: Instant,
    /// Last occurrence time
    pub last_seen: Instant,
}

/// Kind of debounced event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebouncedEventKind {
    /// File was created
    Created,
    /// File was modified
    Modified,
    /// File was deleted
    Deleted,
    /// Any event (aggregated)
    Any,
}

/// Event debouncer
#[allow(dead_code)]
pub struct EventDebouncer {
    config: DebounceConfig,
    events: HashMap<PathBuf, DebouncedEvent>,
    start_time: Option<Instant>,
}

#[allow(dead_code)]
impl EventDebouncer {
    /// Create a new debouncer
    pub fn new(config: DebounceConfig) -> Self {
        Self {
            config,
            events: HashMap::new(),
            start_time: None,
        }
    }

    /// Add an event to the debouncer
    pub fn add(&mut self, path: PathBuf, kind: DebouncedEventKind) {
        let now = Instant::now();

        if self.start_time.is_none() {
            self.start_time = Some(now);
        }

        self.events
            .entry(path.clone())
            .and_modify(|event| {
                event.count += 1;
                event.last_seen = now;
                // Keep the most significant event kind
                event.kind = Self::merge_kinds(event.kind, kind);
            })
            .or_insert(DebouncedEvent {
                path,
                kind,
                count: 1,
                first_seen: now,
                last_seen: now,
            });
    }

    /// Check if events should be flushed
    pub fn should_flush(&self) -> bool {
        let now = Instant::now();

        // Check max batch size
        if self.events.len() >= self.config.max_batch_size {
            return true;
        }

        // Check max wait time
        if let Some(start) = self.start_time {
            if now.duration_since(start) >= Duration::from_millis(self.config.max_wait_ms) {
                return true;
            }
        }

        false
    }

    /// Flush all events
    pub fn flush(&mut self) -> Vec<DebouncedEvent> {
        let events = self.events.drain().map(|(_, v)| v).collect();
        self.start_time = None;
        events
    }

    /// Get number of pending events
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.events.len()
    }

    /// Merge two event kinds
    fn merge_kinds(a: DebouncedEventKind, b: DebouncedEventKind) -> DebouncedEventKind {
        use DebouncedEventKind::{Any, Created, Deleted, Modified};

        match (a, b) {
            // Same kind stays same
            (Created, Created) => Created,
            (Modified, Modified) => Modified,
            (Deleted, Deleted) => Deleted,
            // Created then modified = created
            (Created, Modified) => Created,
            // Modified then deleted = deleted
            (Modified, Deleted) => Deleted,
            // Created then deleted = any (file came and went)
            (Created, Deleted) => Any,
            // Any with anything = any
            (Any, _) | (_, Any) => Any,
            // Default to any for other combinations
            _ => Any,
        }
    }
}

impl Default for EventDebouncer {
    fn default() -> Self {
        Self::new(DebounceConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debouncer_single_event() {
        let mut debouncer = EventDebouncer::default();

        debouncer.add(PathBuf::from("test.rs"), DebouncedEventKind::Modified);

        assert_eq!(debouncer.pending_count(), 1);

        let events = debouncer.flush();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].count, 1);
    }

    #[test]
    fn test_debouncer_multiple_same() {
        let mut debouncer = EventDebouncer::default();

        for _ in 0..5 {
            debouncer.add(PathBuf::from("test.rs"), DebouncedEventKind::Modified);
        }

        let events = debouncer.flush();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].count, 5);
    }

    #[test]
    fn test_debouncer_merge_created_modified() {
        let mut debouncer = EventDebouncer::default();

        debouncer.add(PathBuf::from("test.rs"), DebouncedEventKind::Created);
        debouncer.add(PathBuf::from("test.rs"), DebouncedEventKind::Modified);

        let events = debouncer.flush();
        assert_eq!(events[0].kind, DebouncedEventKind::Created);
    }

    #[test]
    fn test_debouncer_merge_created_deleted() {
        let mut debouncer = EventDebouncer::default();

        debouncer.add(PathBuf::from("test.rs"), DebouncedEventKind::Created);
        debouncer.add(PathBuf::from("test.rs"), DebouncedEventKind::Deleted);

        let events = debouncer.flush();
        assert_eq!(events[0].kind, DebouncedEventKind::Any);
    }

    #[test]
    fn test_debouncer_max_batch() {
        let config = DebounceConfig {
            max_batch_size: 3,
            ..Default::default()
        };
        let mut debouncer = EventDebouncer::new(config);

        debouncer.add(PathBuf::from("a.rs"), DebouncedEventKind::Modified);
        debouncer.add(PathBuf::from("b.rs"), DebouncedEventKind::Modified);
        assert!(!debouncer.should_flush());

        debouncer.add(PathBuf::from("c.rs"), DebouncedEventKind::Modified);
        assert!(debouncer.should_flush());
    }
}
