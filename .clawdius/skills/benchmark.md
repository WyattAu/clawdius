---
name: benchmark
description: Run performance benchmarks and analyze results
version: 1.0.0
tags: [performance, benchmark, workflow]
arguments:
  - name: target
    description: What to benchmark (function, endpoint, or binary)
    required: true
  - name: iterations
    description: Number of iterations to run
    required: false
    default: "100"
examples:
  - "/benchmark target=API response time"
  - "/benchmark target=database query iterations=1000"
---

# Benchmark Skill

Run performance benchmarks and produce an actionable analysis.

## Instructions

1. Identify the benchmark target and determine the appropriate tool:
   - Rust: `cargo bench` or `criterion`
   - Python: `pytest-benchmark` or `timeit`
   - Node.js: `benchmark.js` or `autocannon` (for HTTP)
   - Generic: `time` command for simple measurements
2. Check for existing benchmark configurations in the project
3. Run the benchmark with the specified iterations
4. Collect metrics:
   - Mean/median/p95/p99 latency
   - Throughput (operations per second)
   - Memory usage (before/after, peak)
   - CPU utilization if measurable
5. Run the benchmark multiple times to establish variance
6. Compare against any previous benchmark results if available
7. Produce a benchmark report:
   - Summary metrics table
   - Variance analysis (is the result stable?)
   - Comparison to baseline (if available)
   - Bottleneck identification
   - Optimization recommendations ranked by expected impact

## Constraints

- Always warm up before measuring (discard first N iterations)
- Report statistical significance — don't claim improvements within noise margin
- If the benchmark is flaky, say so and report the range
- Consider environmental factors (other processes, thermal throttling)
- Focus on actionable insights, not just numbers
