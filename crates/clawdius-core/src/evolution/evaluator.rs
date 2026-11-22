//! Evaluator contract: anything that can score a [`Program`].
//!
//! Phase A ships a sync [`FnEvaluator`] (wraps a closure — used by the
//! deterministic convergence test with a stub "LLM" mutator) and a
//! [`CommandEvaluator`] (spawns a subprocess, parses `{"metrics": {...}}`
//! JSON from stdout — the crate-execution pattern `test_execution.rs`
//! already uses). Phase B adds the LLM-judge cascade over `llm/providers`.

use serde_json::Value;

use super::program::Program;

/// Scores a program. Higher is better on every metric; the evaluator
/// populates `metrics` (including the fitness metric) and may attach
/// `artifacts` (stderr/logs) for artifact-conditioned prompts.
pub trait Evaluator: Send + Sync {
    fn evaluate(&self, program: &Program) -> Evaluated;
}

/// The result of evaluating one program.
#[derive(Debug, Clone)]
pub struct Evaluated {
    pub metrics: std::collections::BTreeMap<String, f64>,
    pub artifacts: Option<String>,
}

impl Evaluated {
    /// Convenience: a single-metric result.
    #[must_use]
    pub fn single(metric: &str, value: f64) -> Self {
        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert(metric.to_string(), value);
        Self {
            metrics,
            artifacts: None,
        }
    }
}

/// Wraps a closure — the deterministic adapter for stub evaluators and
/// machine-checkable fitness (string-match, test-suite pass rate).
pub struct FnEvaluator<F>
where
    F: Fn(&Program) -> Evaluated + Send + Sync,
{
    f: F,
}

impl<F> FnEvaluator<F>
where
    F: Fn(&Program) -> Evaluated + Send + Sync,
{
    /// Create from a closure.
    #[must_use]
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F> Evaluator for FnEvaluator<F>
where
    F: Fn(&Program) -> Evaluated + Send + Sync,
{
    fn evaluate(&self, program: &Program) -> Evaluated {
        (self.f)(program)
    }
}

/// Spawns a subprocess evaluator: the program code is written to a file,
/// the evaluator script scores it, and stdout must be JSON with a
/// `metrics` object (and optional `artifacts` string). This is the same
/// contract `agentic/test_execution.rs` uses for out-of-process checks,
/// so fleet evaluators can reuse the pattern directly.
pub struct CommandEvaluator {
    /// The evaluator command (argv[0] plus fixed args).
    pub argv: Vec<String>,
    /// Timeout for the evaluator subprocess.
    pub timeout: std::time::Duration,
}

impl CommandEvaluator {
    /// Create from a command line (argv[0] + fixed args).
    #[must_use]
    pub fn new(argv: Vec<String>, timeout: std::time::Duration) -> Self {
        Self { argv, timeout }
    }
}

impl Evaluator for CommandEvaluator {
    fn evaluate(&self, program: &Program) -> Evaluated {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut eval_script = Command::new(&self.argv[0]);
        for arg in &self.argv[1..] {
            eval_script.arg(arg);
        }

        // Write program code to a temp file for the evaluator to read.
        let tmp = std::env::temp_dir().join(format!("evo-eval-{}.rs", program.id));
        if std::fs::write(&tmp, &program.code).is_err() {
            return Evaluated::single("score", f64::NEG_INFINITY);
        }

        let child = eval_script
            .arg(&tmp)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        let Ok(mut child) = child else {
            let _ = std::fs::remove_file(&tmp);
            return Evaluated::single("score", f64::NEG_INFINITY);
        };

        // Bounded wait: an evaluator that hangs scores NEG_INFINITY.
        let deadline = std::time::Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if std::time::Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&tmp);
                    return Evaluated::single("score", f64::NEG_INFINITY);
                },
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                Err(_) => {
                    let _ = std::fs::remove_file(&tmp);
                    return Evaluated::single("score", f64::NEG_INFINITY);
                },
            }
        };

        let output = match child.wait_with_output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
            Err(_) => String::new(),
        };
        let _ = std::fs::remove_file(&tmp);

        if !status.success() {
            // Evaluator script ran but rejected the program: score it
            // failed, and keep its stderr as artifacts for the prompt.
            let mut ev = Evaluated::single("score", f64::NEG_INFINITY);
            ev.artifacts = Some("evaluator rejected the program".into());
            return ev;
        }

        match serde_json::from_str::<Value>(&output) {
            Ok(v) => {
                let mut metrics = std::collections::BTreeMap::new();
                if let Some(obj) = v.get("metrics").and_then(Value::as_object) {
                    for (k, val) in obj {
                        if let Some(n) = val.as_f64() {
                            metrics.insert(k.clone(), n);
                        }
                    }
                }
                let artifacts = v
                    .get("artifacts")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                Evaluated { metrics, artifacts }
            },
            Err(_) => Evaluated::single("score", f64::NEG_INFINITY),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn fn_evaluator_wraps_closure() {
        let eval = FnEvaluator::new(|p: &Program| {
            let score = p.code.matches("fn ").count() as f64;
            Evaluated::single("score", score)
        });
        let mut p = Program::new("fn a() {}\nfn b() {}", Uuid::new_v4());
        p.set_metrics([("score".into(), 0.0)]);
        let ev = eval.evaluate(&p);
        assert_eq!(ev.metrics.get("score"), Some(&2.0));
    }

    #[test]
    fn command_evaluator_parses_json_stdout() {
        let dir = std::env::temp_dir().join(format!("evo-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("eval.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nprintf '{\"metrics\": {\"score\": 3.5}, \"artifacts\": \"ok\"}'\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let eval = CommandEvaluator::new(
            vec![script.to_string_lossy().into_owned()],
            std::time::Duration::from_secs(10),
        );
        let p = Program::new("code", Uuid::new_v4());
        let ev = eval.evaluate(&p);
        assert_eq!(ev.metrics.get("score"), Some(&3.5));
        assert_eq!(ev.artifacts.as_deref(), Some("ok"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn command_evaluator_times_out_hanging_scripts() {
        let dir = std::env::temp_dir().join(format!("evo-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("hang.sh");
        std::fs::write(&script, "#!/bin/sh\nsleep 60\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let eval = CommandEvaluator::new(
            vec![script.to_string_lossy().into_owned()],
            std::time::Duration::from_millis(150),
        );
        let p = Program::new("code", Uuid::new_v4());
        let ev = eval.evaluate(&p);
        assert_eq!(ev.metrics.get("score"), Some(&f64::NEG_INFINITY));
        std::fs::remove_dir_all(&dir).ok();
    }
}
