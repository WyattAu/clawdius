//! Transport-robustness primitives shared by all platform adapters.
//!
//! These encode the hardening patterns the competitive analysis flagged as
//! the gap between "works" and "works seamlessly": exponential backoff with
//! jitter instead of fixed sleeps, and crash-safe ingestion offsets so a
//! gateway restart neither re-answers old messages nor loses new ones.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Exponential backoff with jittered sleeps.
///
/// `next()` returns a duration that grows from `initial` by `factor` each
/// call, capped at `max`, with each sample jittered by up to `±jitter` of
/// itself so fleet-wide retries do not synchronize. `reset()` returns to the
/// initial delay after a success.
#[derive(Debug, Clone)]
pub struct Backoff {
    initial: Duration,
    max: Duration,
    factor: f64,
    jitter: f64,
    current: f64,
}

impl Backoff {
    /// Create a backoff; panics in debug builds on non-positive `factor`.
    #[must_use]
    pub fn new(initial: Duration, max: Duration, factor: f64, jitter: f64) -> Self {
        debug_assert!(factor > 1.0, "backoff factor must exceed 1.0");
        debug_assert!((0.0..=1.0).contains(&jitter), "jitter must be in [0, 1]");
        Self {
            initial,
            max,
            factor,
            jitter,
            current: initial.as_secs_f64(),
        }
    }

    /// Produce the next delay and advance the sequence.
    #[must_use]
    pub fn next_delay(&mut self) -> Duration {
        let base = self.current;
        let span = base * self.jitter;
        let sample = base + pseudo_jitter(span);
        self.current = base.mul_add(self.factor, 0.0).min(self.max.as_secs_f64());
        Duration::from_secs_f64(sample.max(0.05))
    }

    /// Reset to the initial delay after a success.
    pub fn reset(&mut self) {
        self.current = self.initial.as_secs_f64();
    }

    /// The current un-jittered delay, for tests and status lines.
    #[must_use]
    pub fn current(&self) -> Duration {
        Duration::from_secs_f64(self.current)
    }
}

/// Cheap bounded jitter in `[-span, +span]` derived from the system clock.
///
/// Deterministic-per-call but unpredictable enough to de-synchronize
/// concurrent adapters; not cryptographic and not used for anything
/// security-relevant.
fn pseudo_jitter(span: f64) -> f64 {
    if span <= 0.0 {
        return 0.0;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0u64, |d| d.subsec_nanos() as u64);
    let unit = (nanos % 1000) as f64 / 999.0; // [0, 1]
    unit.mul_add(2.0, -1.0) * span
}

/// Versioned, atomically-written ingestion offset for at-least-once
/// delivery across gateway restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOffset {
    /// Envelope version, currently always 1.
    pub version: u32,
    /// The next update id to request (last processed + 1).
    pub offset: i64,
}

/// Load a stored offset; `None` when absent or unreadable (first run).
#[must_use]
pub fn load_offset(path: &Path) -> Option<StoredOffset> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Store an offset atomically (write temp file, rename over the target).
///
/// A crash mid-write leaves the previous file intact.
pub fn store_offset(path: &Path, offset: i64) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let envelope = StoredOffset { version: 1, offset };
    let tmp = path.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(&envelope).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_caps() {
        let mut b = Backoff::new(Duration::from_secs(2), Duration::from_secs(30), 1.8, 0.0);
        assert_eq!(b.current(), Duration::from_secs(2));
        let mut last = 2.0_f64;
        for _ in 0..8 {
            let d = b.next_delay().as_secs_f64();
            assert!(d >= last * 0.99, "must not shrink: {d} < {last}");
            last = d;
        }
        assert!(b.current().as_secs_f64() <= Duration::from_secs(30).as_secs_f64() * 1.01);
    }

    #[test]
    fn backoff_reset_returns_to_initial() {
        let mut b = Backoff::new(Duration::from_secs(2), Duration::from_secs(30), 2.0, 0.0);
        let _ = b.next_delay();
        let _ = b.next_delay();
        b.reset();
        assert_eq!(b.current(), Duration::from_secs(2));
    }

    #[test]
    fn offset_store_roundtrip_and_atomic() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("tg-offset.json");
        assert!(load_offset(&path).is_none());
        store_offset(&path, 4242).unwrap();
        assert_eq!(load_offset(&path).map(|s| s.offset), Some(4242));
        assert_eq!(load_offset(&path).map(|s| s.version), Some(1));
        // No temp file left behind.
        assert!(!path.with_extension("json.tmp").exists());
    }
}
