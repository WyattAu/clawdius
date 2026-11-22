//! The evolution loop: sample → mutate → evaluate → insert → migrate.
//!
//! Deterministic under a fixed seed: the rng, the database, and the
//! mutator all derive from the same seed, so a full run replays exactly.

use anyhow::Result;

use super::database::{DatabaseConfig, ProgramDatabase};
use super::evaluator::{Evaluated, Evaluator};
use super::mutator::Mutator;
use super::program::Program;
use super::rng::Rng;

/// Evolution loop configuration.
#[derive(Debug, Clone)]
pub struct EvolutionConfig {
    /// Database config (islands, grid, migration).
    pub database: DatabaseConfig,
    /// RNG seed — the whole run replays under a fixed seed.
    pub seed: u64,
    /// Stop after this many evaluated children.
    pub max_evaluations: usize,
    /// Exploit/explore ratio for parent sampling (1.0 = always champion).
    pub exploit_ratio: f64,
    /// Track an island index round-robin per evaluation.
    pub island_round_robin: bool,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            seed: 0,
            max_evaluations: 100,
            exploit_ratio: 0.7,
            island_round_robin: true,
        }
    }
}

/// Statistics from a completed run.
#[derive(Debug, Clone)]
pub struct RunStats {
    pub evaluations: usize,
    pub generations: u64,
    pub migrations: usize,
    pub final_best_score: Option<f64>,
    pub final_best_generation: usize,
}

/// Run the evolution loop to completion.
///
/// # Errors
///
/// Errors when the mutator fails (evaluator failures degrade to
/// NEG_INFINITY scores instead of aborting).
pub fn run(
    config: &EvolutionConfig,
    seed_codes: Vec<String>,
    evaluator: &dyn Evaluator,
    mutator: &dyn Mutator,
) -> Result<(ProgramDatabase, RunStats)> {
    let mut rng = Rng::new(config.seed);
    let mut db = ProgramDatabase::new(config.database.clone());

    // Seed population: round-robin across islands.
    for (i, code) in seed_codes.iter().enumerate() {
        let id = uuid_from(i as u64, config.seed);
        let mut p = Program::new(code.clone(), id);
        p.generation = 0;
        // Give seeds a spread across the feature dimensions so the grid
        // populates distinct bins from the start.
        p.set_metrics([
            ("size".into(), code.len() as f64),
            ("complexity".into(), (code.lines().count() * 7) as f64),
            ("score".into(), f64::NEG_INFINITY),
        ]);
        let island = if config.island_round_robin {
            i % db.config().num_islands.max(1)
        } else {
            0
        };
        db.add_seed(island, p);
    }

    let mut evaluations = 0usize;
    let mut migrations = 0usize;
    let mut evaluations_since_migration = 0usize;

    while evaluations < config.max_evaluations {
        let island = if config.island_round_robin {
            evaluations % db.config().num_islands.max(1)
        } else {
            0
        };

        // Round-robin the island, but fall back to any island with
        // content (e.g. seeds only in island 0 with multiple islands
        // configured): an empty island is skipped, not a loop exit.
        let Some((parent, inspirations)) = db
            .sample(island, config.exploit_ratio, &mut rng)
            .or_else(|| {
                (0..db.config().num_islands.max(1))
                    .find_map(|i| db.sample(i, config.exploit_ratio, &mut rng))
            })
        else {
            break; // no programs anywhere
        };

        let child_code = mutator.mutate(&parent, &inspirations, &mut rng)?;
        let mut child = Program::child_of(
            &parent,
            child_code,
            uuid_from(evaluations as u64 + 10_000, config.seed),
            db.generation() as usize + 1,
        );

        let evaluated: Evaluated = evaluator.evaluate(&child);
        child.metrics = evaluated.metrics.clone();
        child.artifacts = evaluated.artifacts.clone();

        // Only insert children whose fitness is finite (evaluator failures
        // score NEG_INFINITY and are dropped from the ledger, keeping the
        // database clean for MAP-Elites binning).
        if child
            .fitness_opt(&db.config().fitness_metric)
            .is_some_and(|f| f.is_finite())
        {
            db.add_child(child);
        }
        evaluations += 1;
        evaluations_since_migration += 1;

        // Migration keyed on evaluations (the loop's real cadence).
        if evaluations_since_migration >= config.database.migration_interval.max(1) {
            migrations += db.advance_generation(&mut rng);
            evaluations_since_migration = 0;
        }
    }

    let stats = RunStats {
        evaluations,
        generations: db.generation(),
        migrations,
        final_best_score: db
            .champion()
            .map(|c| c.fitness(&db.config().fitness_metric)),
        final_best_generation: db.champion().map(|c| c.generation).unwrap_or_default(),
    };
    Ok((db, stats))
}

/// Deterministic uuid from two u64s (generation/eval counter + seed).
fn uuid_from(a: u64, b: u64) -> uuid::Uuid {
    uuid::Uuid::from_u64_pair(a.wrapping_add(0x9E37_79B9_7F4A_7C15), b)
}

/// Runs a convergence scenario: seeds, evaluator, mutator, config → stats.
///
/// Utility for tests and the CLI.
#[must_use]
pub fn converge(
    config: &EvolutionConfig,
    seed_codes: Vec<String>,
    evaluator: &dyn Evaluator,
    mutator: &dyn Mutator,
) -> RunStats {
    match run(config, seed_codes, evaluator, mutator) {
        Ok((_, stats)) => stats,
        Err(_) => RunStats {
            evaluations: 0,
            generations: 0,
            migrations: 0,
            final_best_score: None,
            final_best_generation: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::evaluator::FnEvaluator;
    use crate::evolution::mutator::StubMutator;

    /// The Phase-A acceptance scenario: a stub mutator appends "correct"
    /// lines; fitness counts matched lines; the loop must converge to a
    /// champion whose output is fully correct under a fixed seed.
    #[test]
    fn convergence_under_fixed_seed() {
        const TARGET_LINES: usize = 8;
        let seed_code = "// start\n".repeat(1);

        // Fitness: fraction of lines that are "correct N" markers, capped 1.0.
        let evaluator = FnEvaluator::new(|p: &Program| {
            let lines: Vec<&str> = p.code.lines().collect();
            let correct = lines.iter().filter(|l| l.starts_with("// correct")).count();
            let score = if lines.is_empty() {
                0.0
            } else {
                (correct as f64 / TARGET_LINES as f64).min(1.0)
            };
            let mut ev = Evaluated::single("score", score);
            ev.metrics.insert("size".into(), p.code.len() as f64);
            ev.metrics
                .insert("complexity".into(), (lines.len() * 5) as f64);
            ev
        });

        // Mutator: with rising probability per generation, replace a line
        // with a correct one (deterministic given the rng).
        struct ConvergeMutator;
        impl Mutator for ConvergeMutator {
            fn mutate(&self, parent: &Program, _insp: &[Program], rng: &mut Rng) -> Result<String> {
                let mut lines: Vec<String> = parent.code.lines().map(String::from).collect();
                while lines.len() < TARGET_LINES {
                    lines.push("// filler".into());
                }
                let flip = rng.below(TARGET_LINES);
                if rng.f64() < 0.6 {
                    lines[flip] = format!("// correct {flip}");
                }
                Ok(lines.join("\n"))
            }
        }

        let config = EvolutionConfig {
            database: DatabaseConfig {
                num_islands: 2,
                migration_interval: 10,
                migration_rate: 0.2,
                grid_bins: 4,
                feature_dimensions: vec![
                    crate::evolution::database::FeatureDimension {
                        metric: "size".into(),
                        bins: 4,
                        min: 0.0,
                        max: 1_000.0,
                    },
                    crate::evolution::database::FeatureDimension {
                        metric: "complexity".into(),
                        bins: 4,
                        min: 0.0,
                        max: 100.0,
                    },
                ],
                fitness_metric: "score".into(),
            },
            seed: 0xAC0,
            max_evaluations: 300,
            exploit_ratio: 0.7,
            island_round_robin: true,
        };

        let mutator = ConvergeMutator;
        let (db, stats) = run(&config, vec![seed_code], &evaluator, &mutator).unwrap();

        assert_eq!(stats.evaluations, 300);
        let champion = db.champion().expect("champion must exist");
        let correct = champion
            .code
            .lines()
            .filter(|l| l.starts_with("// correct"))
            .count();
        eprintln!(
            "[dbg] converged: correct={correct}/{TARGET_LINES} fitness={}",
            champion.fitness("score")
        );
        assert!(
            correct >= TARGET_LINES - 2,
            "expected near-full convergence, got {correct}/{TARGET_LINES} correct lines, fitness {}",
            champion.fitness("score")
        );
        assert!(
            champion.fitness("score") >= 0.75,
            "converged fitness {} below 0.75",
            champion.fitness("score")
        );
    }

    #[test]
    fn same_seed_replays_identically() {
        let evaluator =
            FnEvaluator::new(|p: &Program| Evaluated::single("score", p.code.len() as f64));
        let mutator = StubMutator;
        let config = EvolutionConfig {
            seed: 123,
            max_evaluations: 20,
            ..EvolutionConfig::default()
        };

        let (db1, s1) = run(&config, vec!["seed".into()], &evaluator, &mutator).unwrap();
        let (db2, s2) = run(&config, vec!["seed".into()], &evaluator, &mutator).unwrap();

        assert_eq!(s1.evaluations, s2.evaluations);
        assert_eq!(s1.final_best_score, s2.final_best_score);
        assert_eq!(
            db1.champion().map(|c| c.code.clone()),
            db2.champion().map(|c| c.code.clone())
        );
    }

    #[test]
    fn failed_evaluations_score_negative_infinity_and_do_not_crash() {
        let evaluator =
            FnEvaluator::new(|_p: &Program| Evaluated::single("score", f64::NEG_INFINITY));
        let mutator = StubMutator;
        let config = EvolutionConfig {
            seed: 9,
            max_evaluations: 5,
            ..EvolutionConfig::default()
        };
        let (db, _) = run(&config, vec!["seed".into()], &evaluator, &mutator).unwrap();
        // Failed children drop out of the ledger; the seeds remain.
        assert!(db.len() <= 2);
    }
}
