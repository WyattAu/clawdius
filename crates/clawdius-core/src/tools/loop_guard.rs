//! Semantic state-hash loop detection (blueprint item 46 / IX.46).
//!
//! The infinite fix-error-fix-loop failure mode: the model retries
//! variations of the same broken approach until the budget dies. This
//! guard hashes the semantic state (tool results + assistant text) each
//! iteration; a repeated hash means the run is visiting an identical
//! state and must stop or change strategy.

use std::collections::HashSet;

use sha2::{Digest, Sha256};

/// Loop detection verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopStatus {
    /// New semantic state — continue.
    Continue,
    /// State seen before (repeat count supplied).
    Repeat(usize),
}

/// Rolling detector over semantic state hashes.
#[derive(Debug)]
pub struct LoopGuard {
    seen: HashSet<u64>,
    order: Vec<u64>,
    max_repeats: usize,
    repeats: usize,
}

impl LoopGuard {
    /// Guard that halts after `max_repeats` visits of the same state.
    #[must_use]
    pub fn new(max_repeats: usize) -> Self {
        Self {
            seen: HashSet::new(),
            order: Vec::new(),
            max_repeats,
            repeats: 0,
        }
    }

    /// Observe one iteration's semantic state.
    ///
    /// `state` should combine the LLM response text, tool names called,
    /// and (for repair loops) the compiler/test output — anything that
    /// distinguishes "genuinely progressed" from "spinning".
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn observe(&mut self, state: &str) -> LoopStatus {
        let hash = Sha256::digest(state.as_bytes());
        let key = u64::from_be_bytes(hash[..8].try_into().unwrap_or([0; 8]));
        if self.seen.contains(&key) {
            self.repeats += 1;
            if self.repeats >= self.max_repeats {
                return LoopStatus::Repeat(self.repeats);
            }
            // Still count the visit in history ordering.
            self.order.push(key);
            return LoopStatus::Repeat(self.repeats);
        }
        self.seen.insert(key);
        self.order.push(key);
        LoopStatus::Continue
    }

    /// Distinct states observed so far.
    #[must_use]
    pub fn distinct_states(&self) -> usize {
        self.seen.len()
    }

    /// Total observations.
    #[must_use]
    pub fn observations(&self) -> usize {
        self.order.len()
    }

    /// Whether the loop threshold has been reached.
    #[must_use]
    pub const fn is_looping(&self) -> bool {
        self.repeats >= self.max_repeats
    }
}

/// Convenience: hash a slice of message contents into one state key.
#[must_use]
pub fn state_hash(texts: &[String]) -> u64 {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    for t in texts {
        hasher.update(t.as_bytes());
        hasher.update(b"\x00");
    }
    let digest = hasher.finalize();
    u64::from_be_bytes(digest[..8].try_into().unwrap_or([0; 8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_states_continue() {
        let mut guard = LoopGuard::new(2);
        assert_eq!(guard.observe("state A"), LoopStatus::Continue);
        assert_eq!(guard.observe("state B"), LoopStatus::Continue);
        assert_eq!(guard.observe("state C"), LoopStatus::Continue);
        assert!(!guard.is_looping());
        assert_eq!(guard.distinct_states(), 3);
    }

    #[test]
    fn repeated_state_counts_and_triggers() {
        let mut guard = LoopGuard::new(2);
        let _ = guard.observe("same");
        assert_eq!(guard.observe("same"), LoopStatus::Repeat(1));
        assert_eq!(guard.observe("same"), LoopStatus::Repeat(2));
        assert!(guard.is_looping());
    }

    #[test]
    fn state_hash_is_order_sensitive() {
        let a = state_hash(&["one".into(), "two".into()]);
        let b = state_hash(&["two".into(), "one".into()]);
        assert_ne!(a, b);
        assert_eq!(a, state_hash(&["one".into(), "two".into()]));
    }

    #[test]
    fn trivially_different_states_hash_differently() {
        let mut guard = LoopGuard::new(3);
        for i in 0..5 {
            let state = format!("iteration {i} with distinct tool result {i}");
            assert_eq!(guard.observe(&state), LoopStatus::Continue);
        }
    }
}
