# Agent Task Queue Design

## Overview

A persistent, SQLite-backed task queue for autonomous agent orchestration. This closes the
competitive gap with Claw Code, which has lane events, a policy engine, worker lifecycle,
task/team/cron registries, and recovery recipes.

## Architecture

```
                    +------------------+
                    |   TaskQueue      |
                    |   (SQLite)       |
                    +--------+---------+
                             |
           +----------------+----------------+
           |                |                |
    +------v------+  +------v------+  +------v------+
    | TaskWorker  |  | TaskWorker  |  | TaskWorker  |
    | (Agent 1)   |  | (Agent 2)   |  | (Agent N)   |
    +------+------+  +------+------+  +------+------+
           |                |                |
           +----------------+----------------+
                             |
                    +--------v---------+
                    | PolicyEngine     |
                    | (Permissions)    |
                    +------------------+
```

## Data Model

### Task

| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Unique task identifier |
| parent_id | UUID? | Parent task (for subtasks) |
| agent_id | String | Agent assigned to task |
| status | Enum | Pending/Running/Completed/Failed/Cancelled |
| priority | i32 | Priority (higher = more important) |
| task_type | String | Category: code, test, review, deploy, custom |
| input | JSON | Task input payload |
| output | JSON? | Task output payload |
| error | String? | Error message if failed |
| retries | u32 | Number of retry attempts |
| max_retries | u32 | Maximum retry count |
| created_at | DateTime | Creation timestamp |
| started_at | DateTime? | Execution start timestamp |
| completed_at | DateTime? | Completion timestamp |
| deadline | DateTime? | Optional deadline |
| metadata | JSON? | Arbitrary metadata |

### Agent

| Field | Type | Description |
|-------|------|-------------|
| id | String | Agent identifier |
| name | String | Human-readable name |
| capabilities | JSON[] | List of task types this agent can handle |
| max_concurrent | u32 | Maximum concurrent tasks |
| status | Enum | Idle/Busy/Offline |
| policy_id | String | Permission policy |
| metadata | JSON? | Arbitrary metadata |

### Policy

| Field | Type | Description |
|-------|------|-------------|
| id | String | Policy identifier |
| name | String | Human-readable name |
| rules | JSON[] | List of permission rules |
| constraints | JSON? | Resource constraints |

### Schedule (Cron)

| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Schedule identifier |
| cron_expr | String | Cron expression |
| task_template | JSON | Task template to create |
| agent_id | String | Agent to assign |
| enabled | bool | Whether schedule is active |
| last_run | DateTime? | Last execution timestamp |
| next_run | DateTime? | Next execution timestamp |

## SQLite Schema

```sql
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES tasks(id),
    agent_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 0,
    task_type TEXT NOT NULL,
    input TEXT NOT NULL, -- JSON
    output TEXT, -- JSON
    error TEXT,
    retries INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    created_at TEXT NOT NULL, -- ISO8601
    started_at TEXT,
    completed_at TEXT,
    deadline TEXT,
    metadata TEXT -- JSON
);

CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_agent ON tasks(agent_id);
CREATE INDEX idx_tasks_type ON tasks(task_type);
CREATE INDEX idx_tasks_priority ON tasks(priority DESC);
CREATE INDEX idx_tasks_parent ON tasks(parent_id);

CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    capabilities TEXT NOT NULL, -- JSON array
    max_concurrent INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'idle',
    policy_id TEXT,
    metadata TEXT
);

CREATE TABLE IF NOT EXISTS policies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    rules TEXT NOT NULL, -- JSON array
    constraints TEXT -- JSON
);

CREATE TABLE IF NOT EXISTS schedules (
    id TEXT PRIMARY KEY,
    cron_expr TEXT NOT NULL,
    task_template TEXT NOT NULL, -- JSON
    agent_id TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_run TEXT,
    next_run TEXT
);
```

## Core API

```rust
/// Task queue operations
pub trait TaskQueue: Send + Sync {
    /// Enqueue a new task
    async fn enqueue(&self, task: NewTask) -> Result<TaskId>;

    /// Dequeue the next pending task for a given agent
    async fn dequeue(&self, agent_id: &str) -> Result<Option<Task>>;

    /// Update task status
    async fn update_status(&self, task_id: &TaskId, status: TaskStatus) -> Result<()>;

    /// Complete a task with output
    async fn complete(&self, task_id: &TaskId, output: Value) -> Result<()>;

    /// Fail a task with error
    async fn fail(&self, task_id: &TaskId, error: &str) -> Result<()>;

    /// Cancel a task
    async fn cancel(&self, task_id: &TaskId) -> Result<()>;

    /// Retry a failed task
    async fn retry(&self, task_id: &TaskId) -> Result<()>;

    /// List tasks with optional filter
    async fn list(&self, filter: TaskFilter) -> Result<Vec<Task>>;

    /// Get task by ID
    async fn get(&self, task_id: &TaskId) -> Result<Task>;

    /// Create subtask
    async fn create_subtask(&self, parent_id: &TaskId, task: NewTask) -> Result<TaskId>;
}

/// Agent management
pub trait AgentRegistry: Send + Sync {
    /// Register a new agent
    async fn register(&self, agent: AgentDef) -> Result<()>;

    /// Deregister an agent
    async fn deregister(&self, agent_id: &str) -> Result<()>;

    /// List all agents
    async fn list(&self) -> Result<Vec<Agent>>;

    /// Update agent status
    async fn update_status(&self, agent_id: &str, status: AgentStatus) -> Result<()>;

    /// Find capable agents for a task type
    async fn find_capable(&self, task_type: &str) -> Result<Vec<Agent>>;
}

/// Policy engine
pub trait PolicyEngine: Send + Sync {
    /// Check if an agent is allowed to perform an action
    async fn check(&self, agent_id: &str, action: &str, resource: &str) -> Result<bool>;

    /// Enforce policy on a task
    async fn enforce(&self, agent_id: &str, task: &Task) -> Result<()>;
}
```

## Recovery Recipes

Inspired by Claw Code's recovery recipes. Each recipe defines a recovery strategy:

```rust
pub enum RecoveryStrategy {
    /// Retry the task up to N times with exponential backoff
    Retry { max_retries: u32, backoff_ms: u64 },
    /// Fall back to a different agent
    FallbackAgent { agent_id: String },
    /// Fall back to a different task type
    FallbackTask { task_type: String },
    /// Escalate to a human
    Escalate { message: String },
    /// Mark as failed and create a follow-up task
    CreateFollowUp { template: NewTask },
}
```

## Concurrency Model

- Each `TaskWorker` runs in its own tokio task
- Workers poll the queue via `dequeue()` (long-poll with 1s interval)
- SQLite access is serialized through `tokio::sync::Mutex`
- Agent concurrency limits enforced by the worker before dequeue

## Integration Points

### TUI Integration

- New `:tasks` command to list task queue
- New `:agents` command to list registered agents
- Status bar shows active task count
- Task output rendered in chat view

### Sprint Integration

- `clawdius sprint` creates task queue with subtasks
- Each subtask assigned to appropriate agent
- Progress tracked in task queue

### MCP Integration

- Task queue exposed as MCP tools: `task_create`, `task_list`, `task_cancel`
- Agents can create subtasks via MCP

## Implementation Plan

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| 1. Schema + TaskQueue trait | 2d | SQLite migration, CRUD operations |
| 2. TaskWorker + polling | 2d | Worker loop, dequeuing, status updates |
| 3. AgentRegistry | 1d | Agent registration, capability matching |
| 4. PolicyEngine | 2d | Permission rules, enforcement |
| 5. RecoveryRecipes | 1d | Retry/fallback/escalation strategies |
| 6. TUI integration | 1d | :tasks, :agents commands, status bar |
| 7. Sprint integration | 1d | Sprint -> TaskQueue adapter |
| 8. Cron scheduling | 1d | Schedule table, cron parser, executor |
| 9. Tests + benchmarks | 2d | Unit tests, integration tests, benchmarks |

**Total: ~13 days (2.5 weeks)**

## File Locations

```
crates/clawdius-core/src/
  agentic/
    task_queue.rs          -- TaskQueue trait + SQLite implementation
    task_worker.rs         -- TaskWorker loop
    agent_registry.rs      -- AgentRegistry trait + SQLite implementation
    policy_engine.rs       -- PolicyEngine trait + rule evaluation
    recovery.rs            -- RecoveryStrategy definitions
    schedule.rs            -- Cron scheduling
```

## Dependencies

- `cron` crate for cron expression parsing
- `uuid` for task IDs (already in workspace)
- `serde_json` for JSON payloads (already in workspace)
- `tokio` for async runtime (already in workspace)
