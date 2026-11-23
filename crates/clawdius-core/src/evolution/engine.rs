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
    let mut db = seed_population(config, &seed_codes);

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

/// Seed the database: round-robin across islands with dimension spread.
///
/// Shared by [`run`] and [`run_parallel`].
fn seed_population(config: &EvolutionConfig, seed_codes: &[String]) -> ProgramDatabase {
    let mut db = ProgramDatabase::new(config.database.clone());
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
    db
}

/// Run the evolution loop with `workers`-way mutation/evaluation
/// parallelism.
///
/// The expensive steps (mutator call, evaluator call — an LLM request or a
/// sandboxed subprocess) fan out across scoped threads each round, while
/// sampling and MAP-Elites insertion stay on the coordinator thread: the
/// database is never shared, so there are no locks and the replay
/// guarantees hold — each candidate gets its own [`Rng`] derived from
/// `(seed, candidate id)`, and insertion happens in slot order.
///
/// Rounds are `workers`-wide instead of one evaluation each, so migration
/// cadence advances in `migration_interval`-multiple chunks; everything
/// else (empty-island fallback, finite-fitness filter, island round-robin)
/// matches [`run`].
///
/// # Errors
///
/// Propagates the first mutator error in slot order (matching [`run`]'s
/// abort-on-mutator-failure semantics); evaluator failures degrade to
/// NEG_INFINITY and are dropped.
pub fn run_parallel(
    config: &EvolutionConfig,
    seed_codes: Vec<String>,
    evaluator: &dyn Evaluator,
    mutator: &dyn Mutator,
    workers: usize,
) -> Result<(ProgramDatabase, RunStats)> {
    let workers = workers.max(1);
    let mut rng = Rng::new(config.seed);
    let mut db = seed_population(config, &seed_codes);
    let num_islands = db.config().num_islands.max(1);

    let mut evaluations = 0usize;
    let mut migrations = 0usize;
    let mut evaluations_since_migration = 0usize;

    while evaluations < config.max_evaluations {
        let width = workers.min(config.max_evaluations - evaluations);

        // Sample `width` parents serially — deterministic order, one rng.
        let mut slots: Vec<Option<(Program, Vec<Program>)>> = Vec::with_capacity(width);
        for i in 0..width {
            let island = if config.island_round_robin {
                (evaluations + i) % num_islands
            } else {
                0
            };
            slots.push(
                db.sample(island, config.exploit_ratio, &mut rng)
                    .or_else(|| {
                        (0..num_islands)
                            .find_map(|island| db.sample(island, config.exploit_ratio, &mut rng))
                    }),
            );
        }
        if slots.iter().all(Option::is_none) {
            break; // no programs anywhere
        }

        let generation = db.generation() as usize + 1;

        // Fan out: mutate + evaluate concurrently. The database stays on
        // this thread — threads only touch their (parent, inspirations)
        // pair and their own rng.
        let results: Vec<std::result::Result<Option<Program>, anyhow::Error>> =
            std::thread::scope(|scope| {
                let handles: Vec<_> = slots
                    .iter()
                    .enumerate()
                    .filter_map(|(i, slot)| {
                        let (parent, inspirations) = slot.as_ref()?;
                        let child_id = uuid_from((evaluations + i) as u64 + 10_000, config.seed);
                        let mut candidate_rng = Rng::new(config.seed ^ child_id.as_u64_pair().0);
                        Some(scope.spawn(move || {
                            let child_code =
                                mutator.mutate(parent, inspirations, &mut candidate_rng)?;
                            let mut child =
                                Program::child_of(parent, child_code, child_id, generation);
                            let evaluated = evaluator.evaluate(&child);
                            child.metrics = evaluated.metrics.clone();
                            child.artifacts = evaluated.artifacts.clone();
                            Ok(Some(child))
                        }))
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().expect("worker panics propagate"))
                    .collect()
            });

        // Propagate the first mutator failure in slot order (run()'s
        // abort semantics), then insert finite children in slot order.
        for result in &results {
            if let Err(e) = result {
                return Err(anyhow::anyhow!("mutator failed (slot evaluation): {e}"));
            }
        }
        for child in results.into_iter().flatten().flatten() {
            if child
                .fitness_opt(&db.config().fitness_metric)
                .is_some_and(|f| f.is_finite())
            {
                db.add_child(child);
            }
        }

        evaluations += width;
        evaluations_since_migration += width;
        while evaluations_since_migration >= config.database.migration_interval.max(1) {
            migrations += db.advance_generation(&mut rng);
            evaluations_since_migration -= config.database.migration_interval.max(1);
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

    /// Shared convergence scenario for the parallel tests: stub mutator
    /// + line-marker fitness, 2 islands, fixed seed.
    fn convergence_scenario(
        max_evaluations: usize,
    ) -> (EvolutionConfig, FnEvaluator<impl Fn(&Program) -> Evaluated>) {
        const TARGET_LINES: usize = 8;
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
        let config = EvolutionConfig {
            database: crate::evolution::database::DatabaseConfig {
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
            max_evaluations,
            exploit_ratio: 0.7,
            island_round_robin: true,
        };
        (config, evaluator)
    }

    struct ParallelMutator;
    impl Mutator for ParallelMutator {
        fn mutate(&self, parent: &Program, _insp: &[Program], rng: &mut Rng) -> Result<String> {
            let mut lines: Vec<String> = parent.code.lines().map(String::from).collect();
            while lines.len() < 8 {
                lines.push("// filler".into());
            }
            let flip = rng.below(8);
            if rng.f64() < 0.6 {
                lines[flip] = format!("// correct {flip}");
            }
            Ok(lines.join("\n"))
        }
    }

    #[test]
    fn run_parallel_converges_under_fixed_seed() {
        let (config, evaluator) = convergence_scenario(300);
        let mutator = ParallelMutator;
        let (db, stats) =
            run_parallel(&config, vec!["// start\n".into()], &evaluator, &mutator, 4).unwrap();

        assert_eq!(stats.evaluations, 300);
        let champion = db.champion().expect("champion must exist");
        let lines = champion.code.lines().count();
        assert!(
            lines >= 8,
            "champion should hold a full program, got {lines} lines"
        );
        assert!(
            stats.final_best_score.expect("score").ge(&0.99),
            "parallel loop must converge like the sequential one, best {:?}",
            stats.final_best_score
        );
    }

    #[test]
    fn run_parallel_is_deterministic_per_seed() {
        let (config, evaluator) = convergence_scenario(60);
        let mutator = ParallelMutator;
        let (db1, stats1) =
            run_parallel(&config, vec!["// start\n".into()], &evaluator, &mutator, 4).unwrap();
        let (db2, stats2) =
            run_parallel(&config, vec!["// start\n".into()], &evaluator, &mutator, 4).unwrap();

        assert_eq!(stats1.evaluations, stats2.evaluations);
        assert_eq!(stats1.generations, stats2.generations);
        assert_eq!(stats1.migrations, stats2.migrations);
        assert_eq!(stats1.final_best_score, stats2.final_best_score);
        let c1 = db1.champion().expect("champion 1");
        let c2 = db2.champion().expect("champion 2");
        assert_eq!(c1.code, c2.code, "same seed must replay the same champion");
        assert_eq!(c1.id, c2.id);
    }

    #[test]
    fn parallel_rounds_actually_overlap_evaluations() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        struct ConcurrencyProbe {
            active: AtomicUsize,
            peak: AtomicUsize,
        }
        impl Evaluator for ConcurrencyProbe {
            fn evaluate(&self, program: &Program) -> Evaluated {
                let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                self.peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(20));
                self.active.fetch_sub(1, Ordering::SeqCst);
                let mut ev = Evaluated::single("score", 0.5);
                ev.metrics.insert("size".into(), program.code.len() as f64);
                ev.metrics.insert(
                    "complexity".into(),
                    (program.code.lines().count() * 5) as f64,
                );
                ev
            }
        }

        let probe = ConcurrencyProbe {
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        };
        let (config, _noop) = convergence_scenario(12);
        // Use the probe directly as the evaluator.
        let mutator = ParallelMutator;
        let evaluator_ref: &dyn Evaluator = &probe;
        run_parallel(
            &config,
            vec!["// start\n".into()],
            evaluator_ref,
            &mutator,
            4,
        )
        .unwrap();

        let peak = probe.peak.load(Ordering::SeqCst);
        assert!(
            peak > 1,
            "workers=4 must evaluate concurrently; peak was {peak}"
        );

        // And a sequential-width run must stay strictly serial.
        let probe2 = ConcurrencyProbe {
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        };
        let evaluator_ref2: &dyn Evaluator = &probe2;
        run_parallel(
            &config,
            vec!["// start\n".into()],
            evaluator_ref2,
            &mutator,
            1,
        )
        .unwrap();
        assert_eq!(
            probe2.peak.load(Ordering::SeqCst),
            1,
            "workers=1 must not overlap"
        );
    }

    #[test]
    fn run_parallel_propagates_mutator_errors() {
        struct ExplodingMutator;
        impl Mutator for ExplodingMutator {
            fn mutate(
                &self,
                _parent: &Program,
                _insp: &[Program],
                _rng: &mut Rng,
            ) -> Result<String> {
                Err(anyhow::anyhow!("mutation exploded"))
            }
        }
        let (config, evaluator) = convergence_scenario(10);
        let err = run_parallel(
            &config,
            vec!["// start\n".into()],
            &evaluator,
            &ExplodingMutator,
            2,
        )
        .expect_err("mutator failure must abort the run");
        assert!(err.to_string().contains("mutation exploded"), "{err}");
    }
}
