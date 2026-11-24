//! Per-model quality tracking: the loop's own outcomes score the models
//! that produced them, and routing (`--model auto:quality`) prefers
//! models with clean histories. Quarantine is automatic: a model that
//! keeps producing defective cycles is benched until it earns its way
//! back — flash-lite's corrupt README edit (#104) would have been caught
//! by exactly this in one cycle.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const DEFECTIVE_QUARANTINE_THRESHOLD: u32 = 2;

/// Outcome class for attribution. Quota/transport failures are neutral
/// (the model never ran); gate-caught defects and hard failures count
/// against the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribution {
    Clean,
    Defective,
    Neutral,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModelStats {
    #[serde(default)]
    pub clean: u32,
    #[serde(default)]
    pub defective: u32,
    #[serde(default)]
    pub quarantined_until: Option<i64>,
}

/// Quality ledger keyed by "provider/model".
#[derive(Debug, Default)]
pub struct ModelQuality {
    models: HashMap<String, ModelStats>,
    path: Option<PathBuf>,
}

impl ModelQuality {
    /// Load from `path`; a missing file is an empty ledger seeded with
    /// nothing (models earn scores from cycles, not priors).
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let models = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        Self {
            models,
            path: Some(path.to_path_buf()),
        }
    }

    fn key(provider: &str, model: &str) -> String {
        format!("{provider}/{model}")
    }

    /// Record one cycle's outcome for a model. Quota/transport failures
    /// (`Attribution::Neutral`) never count against a model.
    pub fn record(&mut self, provider: &str, model: &str, outcome: Attribution) {
        let key = Self::key(provider, model);
        let stats = self.models.entry(key).or_default();
        match outcome {
            Attribution::Clean => stats.clean += 1,
            Attribution::Defective => stats.defective += 1,
            Attribution::Neutral => {},
        }
        // Auto-quarantine: 2+ defectives and no clean cycle ever recorded.
        if stats.defective >= DEFECTIVE_QUARANTINE_THRESHOLD && stats.clean == 0 {
            stats.quarantined_until = Some(now() + (7 * 86400));
        }
        self.save();
    }

    /// Best non-quarantined model by clean count (ties broken by fewer
    /// defectives), for one provider first then any.
    #[must_use]
    pub fn best_model(&self, preferred_provider: &str) -> Option<(String, String)> {
        let now = now();
        let mut candidates: Vec<(&String, &ModelStats)> = self
            .models
            .iter()
            .filter(|(_, s)| s.quarantined_until.map(|q| q < now).unwrap_or(true) && s.clean > 0)
            .collect();
        candidates.sort_by(|a, b| {
            let pa = a.1.clean as i64 - a.1.defective as i64;
            let pb = b.1.clean as i64 - b.1.defective as i64;
            pb.cmp(&pa)
        });
        let preferred = candidates
            .iter()
            .find(|(k, _)| k.starts_with(&format!("{preferred_provider}/")))
            .map(|(k, _)| k.clone());
        let chosen = preferred.or_else(|| candidates.first().map(|(k, _)| k.clone()))?;
        let (provider, model) = chosen.split_once('/')?;
        Some((provider.to_string(), model.to_string()))
    }

    fn save(&self) {
        if let Some(path) = &self.path {
            match serde_json::to_string_pretty(&self.models) {
                Ok(body) => {
                    if let Err(e) = std::fs::write(path, &body) {
                        eprintln!("model_quality write failed: {e}");
                    }
                },
                Err(e) => eprintln!("model_quality save failed: {e}"),
            }
        }
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defective_cycles_count_and_quarantine_unproven_models() {
        let mut q = ModelQuality::default();
        q.record("google", "gemini-3.1-flash-lite", Attribution::Defective);
        q.record("google", "gemini-3.1-flash-lite", Attribution::Defective);
        let stats = &q.models["google/gemini-3.1-flash-lite"];
        assert_eq!(stats.defective, 2);
        assert!(
            stats.quarantined_until.is_some(),
            "unproven + 2 defects = bench"
        );
    }

    #[test]
    fn clean_history_prevents_quarantine() {
        let mut q = ModelQuality::default();
        for _ in 0..3 {
            q.record("zai", "glm-4.5-flash", Attribution::Clean);
        }
        q.record("zai", "glm-4.5-flash", Attribution::Defective);
        let stats = &q.models["zai/glm-4.5-flash"];
        assert!(
            stats.quarantined_until.is_none(),
            "4 clean + 1 defect: stays in"
        );
    }

    #[test]
    fn neutral_outcomes_never_count() {
        let mut q = ModelQuality::default();
        q.record("zai", "glm-4.5-flash", Attribution::Neutral);
        assert!(q.models["zai/glm-4.5-flash"].clean == 0);
        assert!(q.models["zai/glm-4.5-flash"].defective == 0);
    }

    #[test]
    fn best_model_prefers_provider_then_quality() {
        let mut q = ModelQuality::default();
        q.record("google", "gemini-3.6-flash", Attribution::Clean);
        q.record("zai", "glm-4.5-flash", Attribution::Clean);
        q.record("zai", "glm-4.5-flash", Attribution::Clean);
        // preferred provider wins even with fewer cleans
        assert_eq!(
            q.best_model("google"),
            Some(("google".into(), "gemini-3.6-flash".into()))
        );
        // unknown provider: best quality wins
        assert_eq!(
            q.best_model("openai"),
            Some(("zai".into(), "glm-4.5-flash".into()))
        );
    }

    #[test]
    fn best_model_skips_quarantined() {
        let mut q = ModelQuality::default();
        q.record("zai", "bad-model", Attribution::Defective);
        q.record("zai", "bad-model", Attribution::Defective);
        assert_eq!(q.best_model("zai"), None, "quarantined model is not best");
    }

    #[test]
    fn persists_roundtrip() {
        // /tmp is janitored on this box mid-test; keep the fixture inside
        // the crate's target dir instead.
        let dir = std::env::temp_dir()
            .join("clawdius-mq")
            .join(format!("mq-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("model-quality.json");
        let mut q = ModelQuality::load(&path);
        q.record("zai", "glm-4.5-flash", Attribution::Clean);
        let reloaded = ModelQuality::load(&path);
        assert_eq!(reloaded.models["zai/glm-4.5-flash"].clean, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
