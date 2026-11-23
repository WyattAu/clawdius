//! Evolve CLI: run the evolution engine from the terminal.
//!
//! Wraps [`clawdius_core::evolution`]: seeds + evaluator + mutator →
//! `run_parallel` → stats and champion. The evaluator scores candidates by
//! spawning a command (JSON `{"metrics": {...}}` on stdout); candidates can
//! be evaluated inside isolated git worktrees (`--sandbox`, dogfooding the
//! worktree sandbox) and mutated by an LLM (`--mutator llm`, provider via
//! the normal config surface).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clawdius_core::evolution::{
    CommandEvaluator, DatabaseConfig, EvolutionConfig, LlmMutator, PromptConfig,
    SandboxedCommandEvaluator, StubMutator,
};
use clawdius_core::llm::providers::LlmClient;
use clawdius_core::llm::{create_provider, ResolvedLlmConfig};

/// Arguments for `clawdius evolve`.
#[derive(Debug, clap::Args)]
pub struct EvolveArgs {
    /// Seed program files (one entry per seed).
    #[arg(short, long = "seed", required = true)]
    pub seeds: Vec<PathBuf>,

    /// Evaluator command, shell-style (split on whitespace); the candidate
    /// file path is appended as the last argument. Non-zero exit or
    /// Evaluator command, shell-style (split on whitespace); the candidate
    /// file path is appended as the last argument. Non-zero exit or
    /// non-JSON output scores `NEG_INFINITY`.
    #[arg(short, long, required = true, value_name = "CMD")]
    pub evaluator: String,

    /// Mutation source: `stub` (deterministic) or `llm`.
    #[arg(long, default_value = "stub")]
    pub mutator: String,

    /// LLM provider for `--mutator llm` (resolved from clawdius config).
    #[arg(short = 'P', long, default_value = "anthropic")]
    pub provider: String,

    /// Task instruction for the LLM mutator.
    #[arg(long)]
    pub instruction: Option<String>,

    /// Evaluate candidates inside isolated git worktrees of this repo
    /// (defaults to the process cwd).
    #[arg(long)]
    pub sandbox: bool,

    /// Worker threads for mutation/evaluation rounds.
    #[arg(long, default_value_t = 4)]
    pub workers: usize,

    /// Stop after this many evaluated children.
    #[arg(long, default_value_t = 100)]
    pub max_evaluations: usize,

    /// Island populations (independent niches + ring migration).
    #[arg(long, default_value_t = 2)]
    pub islands: usize,

    /// RNG seed — the whole run replays under a fixed seed.
    #[arg(long = "rng-seed", default_value_t = 0)]
    pub seed: u64,

    /// Evaluation timeout per candidate, in seconds.
    #[arg(long, default_value_t = 300)]
    pub timeout_secs: u64,

    /// Print the champion code at the end.
    #[arg(long)]
    pub print_champion: bool,
}

/// `clawdius evolve` — run the engine, print stats (+ champion).
pub(super) fn handle_evolve(args: &EvolveArgs) -> anyhow::Result<()> {
    let seed_codes: Vec<String> = args
        .seeds
        .iter()
        .map(|p| {
            std::fs::read_to_string(p)
                .map_err(|e| anyhow::anyhow!("seed file {}: {e}", p.display()))
        })
        .collect::<anyhow::Result<_>>()?;
    if seed_codes.is_empty() {
        anyhow::bail!("no seeds — pass at least one --seed <file>");
    }

    let eval_argv: Vec<String> = args
        .evaluator
        .split_whitespace()
        .map(String::from)
        .collect();
    if eval_argv.is_empty() {
        anyhow::bail!("empty --evaluator command");
    }

    let evaluator: Box<dyn clawdius_core::evolution::Evaluator> = if args.sandbox {
        Box::new(
            SandboxedCommandEvaluator::new(
                std::env::current_dir()?,
                eval_argv,
                Duration::from_secs(args.timeout_secs),
            )
            .map_err(|e| {
                anyhow::anyhow!("sandbox needs a git repo (run from the repo root or fix: {e})")
            })?,
        )
    } else {
        Box::new(CommandEvaluator::new(
            eval_argv,
            Duration::from_secs(args.timeout_secs),
        ))
    };

    let config = EvolutionConfig {
        database: DatabaseConfig {
            num_islands: args.islands,
            ..DatabaseConfig::default()
        },
        seed: args.seed,
        max_evaluations: args.max_evaluations,
        exploit_ratio: 0.7,
        island_round_robin: true,
    };

    let started = std::time::Instant::now();
    let (db, stats) = match args.mutator.as_str() {
        "stub" => clawdius_core::evolution::run_parallel(
            &config,
            seed_codes,
            evaluator.as_ref(),
            &StubMutator,
            args.workers,
        )?,
        "llm" => {
            let config_llm = clawdius_core::Config::load_default()
                .map_err(|e| anyhow::anyhow!("no clawdius config for LLM mutator: {e}"))?;
            let llm_config = ResolvedLlmConfig::from_config(&config_llm.llm, &args.provider)?;
            let client = create_provider(&llm_config)?;
            let prompt_config = PromptConfig {
                instruction: args.instruction.clone().unwrap_or_default(),
                ..PromptConfig::default()
            };
            let mutator = LlmMutator::new(Arc::new(client) as Arc<dyn LlmClient>, prompt_config);
            clawdius_core::evolution::run_parallel(
                &config,
                seed_codes,
                evaluator.as_ref(),
                &mutator,
                args.workers,
            )?
        },
        other => anyhow::bail!("unknown --mutator '{other}' (stub | llm)"),
    };
    let elapsed = started.elapsed();

    println!(
        "evolve: {} evaluations · {} generations · {} migrations · {:.1}s",
        stats.evaluations,
        stats.generations,
        stats.migrations,
        elapsed.as_secs_f64()
    );
    match stats.final_best_score {
        Some(score) => println!(
            "champion: score {score:.4} (generation {})",
            stats.final_best_generation
        ),
        None => println!("champion: none (every evaluation failed)"),
    }

    if args.print_champion {
        if let Some(champion) = db.champion() {
            println!(
                "\n--- champion code ---\n{}\n---------------------",
                champion.code
            );
        }
    }
    Ok(())
}
