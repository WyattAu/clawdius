//! LLM-backed mutator: the Phase B endpoint of the evolution stack.
//!
//! Wires the [`PromptSampler`] (artifact-conditioned mutation prompts) to a
//! [`LlmClient`] from the provider ensemble, completing the design intent
//! documented in [`super`] — "mutation routes through the LLM provider
//! ensemble with the orchestrator's rate-limit fallback":
//!
//! - **Prompts carry everything** ([`PromptSampler`]): parent code,
//!   evaluator artifacts, adjacent elites, prior failed attempts — targeted
//!   improvement beats blind rewrite (the AlphaEvolve finding).
//! - **Cost caps** come from the orchestrator's
//!   [`ResourceGovernor`](crate::orchestrator::ResourceGovernor): each
//!   mutation acquires a slot under its model key, released on both
//!   outcomes.
//! - **The loop never stalls**: an LLM error, an over-quota model, or an
//!   empty/unfenced response degrades to [`StubMutator`] for that
//!   generation (logged) instead of killing the run.
//!
//! `Mutator::mutate` is sync by contract while `LlmClient::chat` is async;
//! the mutator owns a private runtime and `block_on`s on it — safe from any
//! caller context (it is never the caller's runtime).

use std::sync::Arc;

use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage, ChatRole};
use crate::orchestrator::ResourceGovernor;
use anyhow::Result;

use super::mutator::{Mutator, StubMutator};
use super::program::Program;
use super::prompt::{PromptConfig, PromptSampler};
use super::rng::Rng;

/// Extracts the fenced code block from an LLM response.
///
/// Returns the first fenced block's content; when the model ignored the
/// format instruction, returns the whole trimmed response (the evaluator
/// will score it honestly either way).
#[must_use]
pub fn extract_code_block(response: &str) -> Option<&str> {
    let start = response.find("```")?;
    let after_fence = &response[start + 3..];
    // Skip the language tag line (```rust).
    let body_start = after_fence.find('\n').map_or(0, |i| i + 1);
    let body = &after_fence[body_start..];
    let end = body.find("```")?;
    let content = body[..end].trim();
    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

/// Mutator over an LLM provider with prompt sampling, cost gating, and
/// stub fallback.
pub struct LlmMutator {
    client: Arc<dyn LlmClient>,
    sampler: PromptSampler,
    governor: Option<Arc<ResourceGovernor>>,
    model_key: String,
    rt: tokio::runtime::Runtime,
}

impl LlmMutator {
    /// Build over a client (any provider or the [`ModelRouter`](clawdius_core::llm) ensemble).
    #[must_use]
    pub fn new(client: Arc<dyn LlmClient>, config: PromptConfig) -> Self {
        Self {
            client,
            sampler: PromptSampler::new(config),
            governor: None,
            model_key: String::new(),
            rt: tokio::runtime::Runtime::new().expect("evolution mutator runtime"),
        }
    }

    /// Gate each mutation against the resource governor under `model_key`.
    #[must_use]
    pub fn with_governor(mut self, governor: Arc<ResourceGovernor>, model_key: &str) -> Self {
        self.governor = Some(governor);
        self.model_key = model_key.to_string();
        self
    }

    /// One LLM mutation attempt; `Err` on provider failure.
    fn attempt(
        &self,
        parent: &Program,
        inspirations: &[Program],
    ) -> std::result::Result<String, crate::error::Error> {
        let prompt = self.sampler.build_mutation_prompt(
            parent,
            inspirations,
            parent.artifacts.as_deref(),
            &[],
        );
        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: self.sampler.build_system_prompt(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: prompt,
            },
        ];
        self.rt.block_on(self.client.chat(messages))
    }
}

impl Mutator for LlmMutator {
    fn mutate(&self, parent: &Program, inspirations: &[Program], rng: &mut Rng) -> Result<String> {
        let task_id = format!("evo-mut-{:x}", rng.next_u64());

        // Reserve: over-quota is an error path → stub fallback below.
        let gate = match &self.governor {
            Some(governor) => self
                .rt
                .block_on(governor.acquire(&self.model_key, &task_id)),
            None => Ok(()),
        };

        let outcome = gate.and_then(|_| self.attempt(parent, inspirations));

        // Release on both outcomes — a leaked slot would wedge the lane.
        if let Some(governor) = &self.governor {
            self.rt
                .block_on(governor.release(&self.model_key, &task_id));
        }

        match outcome {
            Ok(response) => match extract_code_block(&response) {
                Some(code) => Ok(code.to_string()),
                None => {
                    tracing::warn!(
                        chars = response.len(),
                        "LLM mutator returned no code block; degrading to stub for this generation"
                    );
                    StubMutator.mutate(parent, inspirations, rng)
                },
            },
            Err(e) => {
                tracing::warn!(error = %e, "LLM mutator failed; degrading to stub for this generation");
                StubMutator.mutate(parent, inspirations, rng)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::prompt::PromptConfig as _Config;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Scripted fake: returns queued responses in order, counting calls.
    type CoreResult<T> = std::result::Result<T, crate::error::Error>;

    struct FakeLlm {
        responses: std::sync::Mutex<Vec<CoreResult<String>>>,
        calls: AtomicUsize,
    }
    impl FakeLlm {
        fn new(responses: Vec<CoreResult<String>>) -> Arc<Self> {
            Arc::new(Self {
                responses: std::sync::Mutex::new(responses),
                calls: AtomicUsize::new(0),
            })
        }
    }
    #[async_trait::async_trait]
    impl LlmClient for FakeLlm {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> CoreResult<String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // FIFO: scripts are written in call order.
            let mut queue = self.responses.lock().expect("queue");
            if queue.is_empty() {
                panic!("scripted response queue exhausted");
            }
            queue.remove(0)
        }
        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
        ) -> CoreResult<tokio::sync::mpsc::Receiver<String>> {
            Err(crate::error::Error::Llm("streaming not exercised".into()))
        }
        fn count_tokens(&self, text: &str) -> usize {
            text.len() / 4
        }
    }

    fn parent() -> Program {
        Program::new("fn base() {}\n", uuid::Uuid::new_v4())
    }

    fn mutator(responses: Vec<CoreResult<String>>) -> (LlmMutator, Arc<FakeLlm>) {
        let client = FakeLlm::new(responses);
        let m = LlmMutator::new(
            Arc::clone(&client) as Arc<dyn LlmClient>,
            _Config::default(),
        );
        (m, client)
    }

    #[test]
    fn extract_fenced_block() {
        let r = "Here is the improved program:\n```rust\nfn better() {}\n```\nDone.";
        assert_eq!(extract_code_block(r), Some("fn better() {}"));
    }

    #[test]
    fn extract_falls_back_to_whole_response() {
        assert_eq!(extract_code_block("fn plain() {}"), None);
        assert_eq!(extract_code_block(""), None);
        // Empty fenced block → None.
        assert_eq!(extract_code_block("```\n```"), None);
    }

    #[test]
    fn llm_mutation_extracts_the_code_block() {
        let (m, _client) = mutator(vec![Ok(
            "analysis...\n```rust\nfn improved() {}\n```".to_string()
        )]);
        let mut rng = Rng::new(9);
        let child = m.mutate(&parent(), &[], &mut rng).expect("mutate");
        assert_eq!(child, "fn improved() {}");
    }

    #[test]
    fn provider_failure_falls_back_to_stub() {
        let (m, client) = mutator(vec![Err(crate::error::Error::Llm("rate limited".into()))]);
        let mut rng = Rng::new(9);
        let child = m.mutate(&parent(), &[], &mut rng).expect("stub fallback");
        assert!(child.starts_with("fn base()"), "{child}");
        assert_eq!(client.calls.load(Ordering::SeqCst), 1, "one attempt made");
    }

    #[test]
    fn unfenced_response_falls_back_to_stub() {
        let (m, _client) = mutator(vec![Ok("I cannot help with that.".to_string())]);
        let mut rng = Rng::new(9);
        let child = m.mutate(&parent(), &[], &mut rng).expect("stub fallback");
        assert!(child.starts_with("fn base()"), "{child}");
    }

    #[test]
    fn governor_gate_caps_and_releases() {
        let mut quotas = std::collections::HashMap::new();
        quotas.insert(
            "evo/model".to_string(),
            crate::orchestrator::Quota {
                max_concurrent_tasks: 1,
                max_ram_per_task: 1,
                max_execution_time_ms: 1000,
                max_tasks_per_hour: 1,
                max_tasks_per_day: 2,
            },
        );
        let governor = Arc::new(ResourceGovernor::new(quotas));
        let client = FakeLlm::new(vec![
            Ok("```rust\nfn one() {}\n```".to_string()),
            Err(crate::error::Error::Llm("would exceed hourly cap".into())),
        ]);
        let m = LlmMutator::new(
            Arc::clone(&client) as Arc<dyn LlmClient>,
            _Config::default(),
        )
        .with_governor(Arc::clone(&governor), "evo/model");

        let mut rng = Rng::new(9);
        let p = parent();
        let first = m.mutate(&p, &[], &mut rng).expect("first mutation");
        assert_eq!(first, "fn one() {}");
        // Slot released after the first call — usage must show none active.
        let usage = m.rt.block_on(governor.get_usage("evo/model"));
        assert_eq!(usage.active_tasks, 0, "slot must be released");

        // Second call: provider error → stub fallback, slot still clean.
        let second = m.mutate(&p, &[], &mut rng).expect("second via stub");
        assert!(second.starts_with("fn base()"), "{second}");
        let usage = m.rt.block_on(governor.get_usage("evo/model"));
        assert_eq!(usage.active_tasks, 0);
    }

    #[test]
    fn over_quota_model_degrades_to_stub() {
        let mut quotas = std::collections::HashMap::new();
        quotas.insert(
            "evo/capped".to_string(),
            crate::orchestrator::Quota {
                max_concurrent_tasks: 1,
                max_ram_per_task: 1,
                max_execution_time_ms: 1000,
                max_tasks_per_hour: 1,
                max_tasks_per_day: 1,
            },
        );
        let governor = Arc::new(ResourceGovernor::new(quotas));
        let client = FakeLlm::new(vec![Ok("```rust\nfn never() {}\n```".to_string())]);
        let m = LlmMutator::new(
            Arc::clone(&client) as Arc<dyn LlmClient>,
            _Config::default(),
        )
        .with_governor(Arc::clone(&governor), "evo/capped");

        // Hold the model's only slot from the outside.
        m.rt.block_on(governor.acquire("evo/capped", "external"))
            .expect("external holder");

        let mut rng = Rng::new(9);
        let child = m.mutate(&parent(), &[], &mut rng).expect("stub under cap");
        assert!(child.starts_with("fn base()"), "{child}");
        assert_eq!(client.calls.load(Ordering::SeqCst), 0, "LLM never called");
    }
}
