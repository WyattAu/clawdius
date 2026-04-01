//! Deterministic test infrastructure for Clawdius
//!
//! Provides:
//! - MockLlmProvider: Deterministic mock implementing the LlmClient trait
//! - SeededRng: Reproducible pseudo-random number generation
//! - Deterministic time mocking via MockClock
//!
//! All tests are fully deterministic — no API keys, no network, no flakiness.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use async_trait::async_trait;
use clawdius_core::llm::providers::LlmClient;
use clawdius_core::llm::{ChatMessage, ChatRole};
use clawdius_core::Result;
use tokio::sync::mpsc;

// ============================================================
// Helper: build ChatMessage quickly in tests
// ============================================================

fn user_msg(content: &str) -> ChatMessage {
    ChatMessage {
        role: ChatRole::User,
        content: content.to_string(),
    }
}

fn system_msg(content: &str) -> ChatMessage {
    ChatMessage {
        role: ChatRole::System,
        content: content.to_string(),
    }
}

fn assistant_msg(content: &str) -> ChatMessage {
    ChatMessage {
        role: ChatRole::Assistant,
        content: content.to_string(),
    }
}

// ============================================================
// Mock LLM Provider
// ============================================================

/// Deterministic mock LLM provider implementing [`LlmClient`].
///
/// Returns pre-configured responses based on prompt substring matching.
/// Enables reproducible tests without API keys or network access.
///
/// # Example
///
/// ```
/// let mock = MockLlmProvider::new()
///     .with_response("explain", "Here is the explanation...")
///     .with_default_response("I don't understand");
///
/// let msg = vec![user_msg("Please explain this code")];
/// let result = mock.chat(msg).await.unwrap();
/// assert_eq!(result, "Here is the explanation...");
/// ```
#[derive(Debug)]
pub struct MockLlmProvider {
    responses: HashMap<String, String>,
    default_response: String,
    call_count: AtomicU32,
    stream_call_count: AtomicU32,
    enforce_exhaustion: bool,
    error_on_call: Option<String>,
    delay_ms: u64,
}

impl MockLlmProvider {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            default_response: "Mock LLM response".to_string(),
            call_count: AtomicU32::new(0),
            stream_call_count: AtomicU32::new(0),
            enforce_exhaustion: false,
            error_on_call: None,
            delay_ms: 0,
        }
    }

    pub fn with_response(mut self, prompt_contains: &str, response: &str) -> Self {
        self.responses
            .insert(prompt_contains.to_string(), response.to_string());
        self
    }

    pub fn with_default_response(mut self, response: &str) -> Self {
        self.default_response = response.to_string();
        self
    }

    pub fn with_enforce_exhaustion(mut self) -> Self {
        self.enforce_exhaustion = true;
        self
    }

    pub fn with_error(mut self, message: &str) -> Self {
        self.error_on_call = Some(message.to_string());
        self
    }

    pub fn with_delay_ms(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    pub fn call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }

    pub fn stream_call_count(&self) -> u32 {
        self.stream_call_count.load(Ordering::SeqCst)
    }

    pub fn total_call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst) + self.stream_call_count.load(Ordering::SeqCst)
    }

    fn resolve_response(&self, messages: &[ChatMessage]) -> String {
        let prompt: String = messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        for (key, value) in &self.responses {
            if prompt.contains(key.as_str()) {
                return value.clone();
            }
        }
        self.default_response.clone()
    }

    pub fn assert_exhausted(&self) {
        if self.enforce_exhaustion {
            let total = self.total_call_count();
            assert_eq!(
                total,
                self.responses.len() as u32,
                "MockLlmProvider: expected {} calls but got {} — not all configured responses were used",
                self.responses.len(),
                total,
            );
        }
    }
}

impl Default for MockLlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmClient for MockLlmProvider {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        }

        if let Some(ref err) = self.error_on_call {
            return Err(clawdius_core::Error::Llm(err.clone()));
        }

        Ok(self.resolve_response(&messages))
    }

    async fn chat_stream(&self, messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
        self.stream_call_count.fetch_add(1, Ordering::SeqCst);

        if self.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
        }

        if let Some(ref err) = self.error_on_call {
            return Err(clawdius_core::Error::Llm(err.clone()));
        }

        let response = self.resolve_response(&messages);
        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            let chunks: Vec<&str> = response.split(' ').collect();
            for chunk in chunks {
                if tx.send(chunk.to_string()).await.is_err() {
                    break;
                }
            }
        });

        Ok(rx)
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
}

// ============================================================
// Seeded Randomness
// ============================================================

/// Simple seeded PRNG for reproducible tests.
///
/// Uses xorshift64 for deterministic, fast random number generation.
/// Not cryptographically secure — intended solely for test determinism.
#[derive(Debug, Clone)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    pub fn next_usize(&mut self) -> usize {
        self.next_u64() as usize
    }

    pub fn next_bool(&mut self) -> bool {
        self.next_u64() % 2 == 0
    }

    pub fn next_range(&mut self, min: u64, max: u64) -> u64 {
        assert!(min <= max, "SeededRng::next_range: min must be <= max");
        min + (self.next_u64() % (max - min + 1))
    }

    pub fn next_f64(&mut self) -> f64 {
        self.next_u64() as f64 / u64::MAX as f64
    }

    pub fn next_string(&mut self, len: usize) -> String {
        (0..len)
            .map(|_| {
                let c = self.next_range(97, 122) as u8;
                c as char
            })
            .collect()
    }

    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        let len = slice.len();
        for i in (1..len).rev() {
            let j = self.next_usize() % (i + 1);
            slice.swap(i, j);
        }
    }

    pub fn pick<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            return None;
        }
        let idx = self.next_usize() % slice.len();
        Some(&slice[idx])
    }
}

// ============================================================
// Mock Clock
// ============================================================

/// Deterministic mock clock for testing time-dependent logic.
///
/// Starts at epoch and advances only when explicitly told to,
/// making time-dependent tests fully deterministic.
#[derive(Debug, Clone)]
pub struct MockClock {
    current_ms: u64,
}

impl MockClock {
    pub fn new() -> Self {
        Self { current_ms: 0 }
    }

    pub fn with_offset(offset_ms: u64) -> Self {
        Self {
            current_ms: offset_ms,
        }
    }

    pub fn now_ms(&self) -> u64 {
        self.current_ms
    }

    pub fn advance(&mut self, ms: u64) {
        self.current_ms += ms;
    }

    pub fn elapsed_since(&self, start_ms: u64) -> u64 {
        self.current_ms.saturating_sub(start_ms)
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_llm_returns_configured_response() {
        let mock = MockLlmProvider::new().with_response("explain code", "This code does X");

        let result = mock
            .chat(vec![user_msg("Please explain code")])
            .await
            .unwrap();
        assert_eq!(result, "This code does X");
        assert_eq!(mock.call_count(), 1);
    }

    #[tokio::test]
    async fn mock_llm_returns_default_for_unknown() {
        let mock = MockLlmProvider::new().with_default_response("Unknown");

        let result = mock
            .chat(vec![user_msg("something unexpected")])
            .await
            .unwrap();
        assert_eq!(result, "Unknown");
    }

    #[tokio::test]
    async fn mock_llm_tracks_call_count() {
        let mock = MockLlmProvider::new();
        mock.chat(vec![user_msg("test1")]).await.unwrap();
        mock.chat(vec![user_msg("test2")]).await.unwrap();
        mock.chat(vec![user_msg("test3")]).await.unwrap();
        assert_eq!(mock.call_count(), 3);
    }

    #[tokio::test]
    async fn mock_llm_errors_when_configured() {
        let mock = MockLlmProvider::new().with_error("API overload");

        let result = mock.chat(vec![user_msg("test")]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn mock_llm_stream_returns_chunks() {
        let mock = MockLlmProvider::new().with_response("hello", "a b c d");

        let mut rx = mock
            .chat_stream(vec![user_msg("hello world")])
            .await
            .unwrap();

        let mut collected = Vec::new();
        while let Some(chunk) = rx.recv().await {
            collected.push(chunk);
        }

        assert_eq!(collected, vec!["a", "b", "c", "d"]);
        assert_eq!(mock.stream_call_count(), 1);
    }

    #[tokio::test]
    async fn mock_llm_stream_errors_when_configured() {
        let mock = MockLlmProvider::new().with_error("stream fail");

        let result = mock.chat_stream(vec![user_msg("test")]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn mock_llm_matches_any_message_in_conversation() {
        let mock = MockLlmProvider::new().with_response("special keyword", "Found it!");

        let messages = vec![
            system_msg("You are helpful."),
            user_msg("Hi"),
            assistant_msg("Hello!"),
            user_msg("Now use the special keyword please"),
        ];

        let result = mock.chat(messages).await.unwrap();
        assert_eq!(result, "Found it!");
    }

    #[test]
    fn mock_llm_count_tokens() {
        let mock = MockLlmProvider::new();
        assert_eq!(mock.count_tokens("hello world"), 2);
        assert_eq!(mock.count_tokens(""), 0);
        assert_eq!(mock.count_tokens("a  b   c"), 3);
    }

    #[tokio::test]
    async fn mock_llm_total_call_count() {
        let mock = MockLlmProvider::new();
        mock.chat(vec![user_msg("a")]).await.unwrap();
        mock.chat_stream(vec![user_msg("b")]).await.unwrap();
        mock.chat(vec![user_msg("c")]).await.unwrap();

        assert_eq!(mock.call_count(), 2);
        assert_eq!(mock.stream_call_count(), 1);
        assert_eq!(mock.total_call_count(), 3);
    }

    #[test]
    fn mock_llm_default() {
        let mock = MockLlmProvider::default();
        assert_eq!(mock.call_count(), 0);
    }

    #[test]
    fn seeded_rng_is_deterministic() {
        let mut rng1 = SeededRng::new(42);
        let mut rng2 = SeededRng::new(42);

        for _ in 0..1000 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn seeded_rng_different_seeds_produce_different_sequences() {
        let mut rng1 = SeededRng::new(1);
        let mut rng2 = SeededRng::new(2);

        let v1: Vec<u64> = (0..100).map(|_| rng1.next_u64()).collect();
        let v2: Vec<u64> = (0..100).map(|_| rng2.next_u64()).collect();

        assert_ne!(v1, v2);
    }

    #[test]
    fn seeded_rng_range() {
        let mut rng = SeededRng::new(42);
        for _ in 0..1000 {
            let val = rng.next_range(10, 20);
            assert!(val >= 10 && val <= 20);
        }
    }

    #[test]
    fn seeded_rng_range_single_value() {
        let mut rng = SeededRng::new(42);
        for _ in 0..10 {
            assert_eq!(rng.next_range(7, 7), 7);
        }
    }

    #[test]
    fn seeded_rng_range_panics_on_invalid() {
        let mut rng = SeededRng::new(42);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rng.next_range(10, 5);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn seeded_rng_string() {
        let mut rng = SeededRng::new(42);
        let s = rng.next_string(50);
        assert_eq!(s.len(), 50);
        assert!(s.chars().all(|c| c.is_ascii_lowercase()));
    }

    #[test]
    fn seeded_rng_string_empty() {
        let mut rng = SeededRng::new(42);
        assert_eq!(rng.next_string(0), "");
    }

    #[test]
    fn seeded_rng_zero_seed_no_stuck() {
        let mut rng = SeededRng::new(0);
        let val = rng.next_u64();
        assert_ne!(val, 0);
    }

    #[test]
    fn seeded_rng_f64_in_range() {
        let mut rng = SeededRng::new(42);
        for _ in 0..100 {
            let val = rng.next_f64();
            assert!(val >= 0.0 && val <= 1.0);
        }
    }

    #[test]
    fn seeded_rng_shuffle_deterministic() {
        let mut rng1 = SeededRng::new(99);
        let mut rng2 = SeededRng::new(99);

        let mut v1: Vec<u32> = (0..20).collect();
        let mut v2: Vec<u32> = (0..20).collect();

        rng1.shuffle(&mut v1);
        rng2.shuffle(&mut v2);

        assert_eq!(v1, v2);
    }

    #[test]
    fn seeded_rng_pick() {
        let mut rng = SeededRng::new(42);
        let items = vec!["a", "b", "c"];

        for _ in 0..50 {
            let picked = rng.pick(&items).unwrap();
            assert!(items.contains(picked));
        }
    }

    #[test]
    fn seeded_rng_pick_empty() {
        let mut rng = SeededRng::new(42);
        let empty: Vec<i32> = vec![];
        assert!(rng.pick(&empty).is_none());
    }

    #[test]
    fn seeded_rng_u32_is_deterministic() {
        let mut rng1 = SeededRng::new(42);
        let mut rng2 = SeededRng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next_u32(), rng2.next_u32());
        }
    }

    #[test]
    fn mock_clock_advances() {
        let mut clock = MockClock::new();
        assert_eq!(clock.now_ms(), 0);

        clock.advance(100);
        assert_eq!(clock.now_ms(), 100);

        clock.advance(250);
        assert_eq!(clock.now_ms(), 350);
    }

    #[test]
    fn mock_clock_elapsed() {
        let mut clock = MockClock::new();
        let start = clock.now_ms();

        clock.advance(500);
        assert_eq!(clock.elapsed_since(start), 500);
    }

    #[test]
    fn mock_clock_with_offset() {
        let clock = MockClock::with_offset(1_000_000);
        assert_eq!(clock.now_ms(), 1_000_000);
    }

    #[test]
    fn mock_clock_elapsed_before_start() {
        let clock = MockClock::with_offset(100);
        assert_eq!(clock.elapsed_since(200), 0);
    }

    #[test]
    fn mock_clock_default() {
        let clock = MockClock::default();
        assert_eq!(clock.now_ms(), 0);
    }

    #[tokio::test]
    async fn mock_llm_multiple_matches_possible() {
        let mock = MockLlmProvider::new()
            .with_response("hello", "short match")
            .with_response("hello world", "exact match");

        let result = mock.chat(vec![user_msg("hello world")]).await.unwrap();
        assert!(
            result == "short match" || result == "exact match",
            "Response should match one of the configured patterns, got: {result}"
        );
    }

    #[tokio::test]
    async fn mock_llm_system_and_user_combined() {
        let mock = MockLlmProvider::new().with_response("IMPORTANT", "triggered by system prompt");

        let messages = vec![
            system_msg("You must follow IMPORTANT rules."),
            user_msg("Do something"),
        ];

        let result = mock.chat(messages).await.unwrap();
        assert_eq!(result, "triggered by system prompt");
    }
}
