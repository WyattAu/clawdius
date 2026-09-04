# actor-kit Adoption Assessment: `clawdius-core/src/orchestrator`

**Date:** 2026-09-04
**Verdict: DO NOT PORT.** The orchestrator is a pull-based distributed job
queue, not a spawn-manage-supervise actor system. actor-kit is a push-based,
single-node actor runtime whose own documentation flags our core workload
(long-running async I/O in handlers) as the wrong fit. A port would be a
rewrite of the orchestration model (est. 1-2 weeks), not a 1-2 day mechanical
mapping, with feature regression and no compensating benefit.

## Module inventory (`crates/clawdius-core/src/orchestrator/`, 2,076 LOC)

| File | LOC | Role |
| --- | --- | --- |
| `mod.rs` | 476 | `Orchestrator` (start/stop N workers, enqueue/status/cancel), `OrchestratorConfig`, `QueuedTask`, `TaskStatus` |
| `queue.rs` | 710 | `TaskQueue` trait + `InMemoryTaskQueue`; Redis backend behind `redis-queue` |
| `worker.rs` | 463 | `Worker::spawn` poll loop: claim → execute via `AgenticSystem::execute` → heartbeat → timeout/requeue; `WorkerHandle` (watch channels) |
| `resource_governor.rs` | 427 | `ResourceGovernor`, per-tenant `Quota`, `ResourceUsage` |

## Architecture character

The module's own header says it: workers run "one per CPU core on Hetzner
EX44 servers", fronted by Next.js, coordinated through Redis. This is
horizontal-scale batch job distribution:

- **Pull model.** Workers poll a shared queue and atomically claim tasks.
  There is no per-worker mailbox and no message addressing.
- **Tasks are one-shot jobs**, not messages to stateful actors. Payload is a
  `TaskRequest` (files, mode, trust level); results flow back through the
  queue (`push_result`/`pop_result`).
- **Fault tolerance is task-level, not process-level:** heartbeat staleness
  detection (`stale_tasks`), requeue, and `retry_count`/`max_retries` on the
  task row. No supervision tree, no restart strategies, no escalation.
- **Execution is minutes-long async I/O** (LLM calls, tool runs, git
  operations) inside `AgenticSystem::execute`.

## Mapping table (orchestrator concept → actor-kit type)

| Orchestrator concept | Nearest actor-kit type | Fit |
| --- | --- | --- |
| `Worker` poll loop | `Actor` + mailbox | **Poor.** Actors are push targets with bounded mailboxes; a Worker has no identity as a message receiver. Inverting pull→push destroys the Redis multi-node model. |
| `AgenticSystem::execute` (minutes of `.await`) | `Actor::handle` (state-machine step, state in registry) | **Poor.** actor-kit's docs explicitly warn actors needing to `.await` I/O mid-handler belong in task-per-actor frameworks. Our entire workload is that. |
| Task retry / stale-task requeue | `Supervisor` + `RestartPolicy` (`Permanent`/`Transient`/…), `SupervisionStrategy` | **Wrong semantics.** OTP supervision restarts *actors* on *exit*; we reschedule *work items* on *heartbeat timeout*. No `ChildSpec` equivalent for a queued task. |
| `Orchestrator::start/stop` of N workers | `SupervisorTree`, `SupervisedChild` | **Cosmetic only.** Our workers are homogeneous, never individually restarted, and have no failure hierarchy. |
| `ResourceGovernor` tenant quotas | `ResourcePolicy`/`NoopPolicy` | **No counterpart.** actor-kit's resource monitors are pressure/backpressure signals, not multi-tenant dollar/request quotas. |
| `TaskQueue` (Redis / InMemory) | `WorkQueue`/`PriorityQueue` (in-process crossbeam deques) | **No counterpart.** actor-kit queues are node-local scheduler internals. |
| `WorkerStatus` watch channels | `ActorHandle` | Partial, but adds nothing over tokio `watch`. |

## Risks if ported anyway

1. **Distribution regression:** Redis queue, heartbeats, and multi-node claim
   semantics have no actor-kit analog; we would have to keep the queue layer
   and bolt actors on top — two runtimes, one job.
2. **Handler model mismatch:** converting long-running futures into actor-kit
   state-machine steps means manually checkpointing `AgenticSystem` execution
   between messages — a redesign, not a port.
3. **Concurrency semantics change:** fixed OS-thread work-stealing pool vs.
   today's tokio tasks with `max_concurrent_tasks` semaphores; LLM calls are
   I/O-bound and would exhaust a CPU-sized worker pool.
4. **Quota enforcement loss:** `ResourceGovernor` would need a full reimplementation.

## Effort estimate

- Mechanical mapping (as scoped by the extraction report): **not available** —
  the conceptual mapping does not close (see table).
- Full re-orchestration onto actor-kit: **1-2 weeks**, high regression risk in
  the production path (billing-adjacent multi-tenant execution), zero
  identified performance or correctness upside.

## Recommendation

Keep the queue-based orchestrator. Revisit only if the roadmap adds a
single-node, many-small-stateful-entities workload (e.g. per-session daemon
state machines) — that is the shape actor-kit is built for, and it is not
what `orchestrator/` does.
