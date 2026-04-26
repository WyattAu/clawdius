# Clawdius Master Roadmap

**Document ID:** ROADMAP-001
**Version:** 1.1.0
**Status:** APPROVED (revised after codebase validation)
**Created:** 2026-04-26
**Revised:** 2026-04-26
**Author:** Nexus (Principal Systems Architect)
**Traceability:** REQ-MSG-001, BP-MSG-GATEWAY-001, COMP-MATRIX-001, VALIDATION-001

### Revision History

| Version | Date | Change |
|---------|------|--------|
| 1.0.0 | 2026-04-26 | Initial roadmap |
| 1.1.0 | 2026-04-26 | Post-validation adjustments: A-01 +3d (rusqlite coupling deeper), A-08 +1d (tool interface), added A-11 (api/ migration), A-12 (HFT stub cleanup), C-13 (TODO cleanup), F-06 (stub audit); total +10d

---

## 1. Product Vision

Clawdius is a Rust-native agentic coding platform that replaces OpenCode (local TUI)
and Claude Code (cloud agent) while adding remote connectivity via messaging apps,
multi-codebase workspace support, multi-DB persistence, and Lean4 formal verification.

### 1.1 Identity

Clawdius = OpenClaw's remote connectivity + OpenCode's TUI polish +
IronClaw's security + Lean4 formal verification, all in Rust.

### 1.2 Target Users

| Segment | Use Case | Value Proposition |
|---------|----------|-------------------|
| Individual Developers | Daily coding assistant | Replace OpenCode/Claude Code with Rust-native, self-hosted alternative |
| Defense/Aerospace | Regulated software development | Lean4 proofs, air-gapped deployment, STIG/DISA compliance |
| Fintech | Trading system development | Multi-codebase workspace, audit trails, sandboxed execution |
| Cybersecurity | Security tool development | Prompt injection defense, zero-unsafe, formally verified sandbox |
| Enterprise Teams | Multi-developer collaboration | SaaS deployment, multi-tenant isolation, chat-based remote access |

### 1.3 CLI Surface

```bash
clawdius                           # TUI on current directory (local dev)
clawdius /path/to/repo             # TUI on specific repo
clawdius serve                     # Start everything (TUI + gateway + all adapters)
clawdius serve --projects a,b,c    # Multi-repo workspace mode
clawdius gateway                   # Gateway daemon only (headless server)
clawdius config                    # Interactive config / .clawdius.toml editing
```

### 1.4 Deployment Targets

| Target | Use Case | Stack |
|--------|----------|-------|
| Local | Developer laptop | SQLite, single binary, no network |
| Hetzner Bare Metal | Team / SaaS | Docker/K8s/K3s, PostgreSQL/MariaDB |
| Air-Gapped | Defense / Classified | Podman, vendored deps, local LLM only |
| Cloud | Enterprise SaaS | Terraform, K8s, managed PostgreSQL |

---

## 2. Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│                    clawdius serve                             │
│                                                              │
│  ┌────────────┐  ┌────────────┐  ┌────────────────────────┐ │
│  │  TUI/CLI   │  │  Gateway   │  │     Sprint Engine      │ │
│  │  (local)   │  │  Daemon    │  │  (shared across all    │ │
│  │            │  │            │  │   input channels)       │ │
│  └─────┬──────┘  └─────┬──────┘  └───────────┬────────────┘ │
│        │               │                      │              │
│        └───────────────┼──────────────────────┘              │
│                        │                                     │
│  ┌─────────────────────┴─────────────────────────────────┐  │
│  │                  Workspace Layer                        │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐               │  │
│  │  │ Project A│ │ Project B│ │ Project C│  (N repos)    │  │
│  │  └──────────┘ └──────────┘ └──────────┘               │  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │        Unified Chat History                       │  │  │
│  │  └──────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────┘  │
│                        │                                     │
│  ┌─────────────────────┴─────────────────────────────────┐  │
│  │              Storage Layer (pluggable)                 │  │
│  │  SQLite │ PostgreSQL │ MariaDB │ InMemory (tests)      │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
         │              │              │
    ┌────┴────┐   ┌────┴────┐   ┌────┴────┐
    │Telegram │   │ Discord │   │ Slack   │
    └─────────┘   └─────────┘   └─────────┘
    ┌────┬────┐   ┌────┬────┐
    │Teams│ WA │   │Sig │Matrix│
    └────┴────┘   └────┴────┘
```

---

## 3. Implementation Phases

### Phase A: Storage Abstraction + Multi-Codebase

**Duration:** 3-4 weeks
**Priority:** P0 — foundational, blocks Phases B, C, D
**Goal:** Persistent storage with pluggable backends; mount multiple codebases in a
single workspace; agent can read/edit across repos.

#### A.1: Storage Backend Trait

Extract a `StorageBackend` trait from the existing `SessionStore` (rusqlite).
All storage consumers (sessions, messages, audit, tenants, workspaces) go through
this trait. Four concrete implementations:

| Backend | Crate | Use Case |
|---------|-------|----------|
| `SqliteBackend` | `rusqlite` | Local dev, single-user, zero ops |
| `PostgresBackend` | `sqlx` + `tokio-postgres` | Enterprise SaaS, horizontal scaling |
| `MariaDbBackend` | `sqlx` + `mysql` | MySQL-compatible enterprise shops |
| `InMemoryBackend` | `HashMap` | Tests only |

**Trait interface:**

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    // --- Sessions ---
    async fn create_session(&self, session: &Session) -> Result<()>;
    async fn get_session(&self, id: &SessionId) -> Result<Option<Session>>;
    async fn update_session(&self, session: &Session) -> Result<()>;
    async fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<Session>>;
    async fn delete_session(&self, id: &SessionId) -> Result<()>;

    // --- Messages ---
    async fn append_message(&self, session_id: &SessionId, msg: &Message) -> Result<MessageId>;
    async fn get_messages(&self, session_id: &SessionId, opts: &MessageQuery) -> Result<Vec<Message>>;
    async fn update_message(&self, id: &MessageId, msg: &Message) -> Result<()>;

    // --- Workspaces (NEW) ---
    async fn create_workspace(&self, workspace: &Workspace) -> Result<()>;
    async fn get_workspace(&self, id: &WorkspaceId) -> Result<Option<Workspace>>;
    async fn list_workspaces(&self) -> Result<Vec<Workspace>>;
    async fn add_project(&self, workspace_id: &WorkspaceId, project: &Project) -> Result<ProjectId>;
    async fn remove_project(&self, workspace_id: &WorkspaceId, project_id: &ProjectId) -> Result<()>;
    async fn get_project(&self, id: &ProjectId) -> Result<Option<Project>>;

    // --- Tenants (SaaS) ---
    async fn create_tenant(&self, tenant: &Tenant) -> Result<TenantId>;
    async fn get_tenant(&self, id: &TenantId) -> Result<Option<Tenant>>;
    async fn update_tenant(&self, tenant: &Tenant) -> Result<()>;

    // --- Audit ---
    async fn log_audit(&self, entry: &AuditEntry) -> Result<()>;
    async fn query_audit(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>>;

    // --- Migrations ---
    async fn migrate(&self) -> Result<()>;
}
```

**Schema additions for multi-codebase:**

```sql
-- Workspaces table
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    tenant_id TEXT REFERENCES tenants(id),
    default_project_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Projects table
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    language TEXT,
    sandbox_config TEXT, -- JSON
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Workspace-project association
CREATE INDEX IF NOT EXISTS idx_projects_workspace
ON projects(workspace_id);
```

#### A.2: Multi-Codebase Workspace Model

```rust
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub tenant_id: Option<Uuid>,
    pub projects: Vec<Project>,
    pub default_project_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Project {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub root_path: PathBuf,
    pub language: Option<String>,       // auto-detected via tree-sitter
    pub sandbox_config: SandboxConfig,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

#### A.3: Sprint Engine Multi-Repo Context Injection

When a workspace has N projects, the sprint engine's context injection changes:

```
## Workspace: my-monorepo (3 projects)

### Project: auth-service (/repos/auth)
[repo-map for auth-service — symbols, imports, structure]

### Project: api-gateway (/repos/gateway)
[repo-map for api-gateway — symbols, imports, structure]

### Project: web-frontend (/repos/web)
[repo-map for web-frontend — symbols, imports, structure]
```

Tool calls gain an optional `project` parameter:
```json
{"tool": "edit_file", "project": "auth-service", "path": "src/handler.rs", ...}
{"tool": "edit_file", "project": "api-gateway", "path": "src/proxy.rs", ...}
{"tool": "shell", "project": "auth-service", "command": "cargo test"}
```

If `project` is omitted, the `default_project` is used.

#### A.4: Config File

`.clawdius.toml` at workspace root or `~/.config/clawdius/config.toml`:

```toml
[workspace]
name = "my-monorepo"

[[workspace.projects]]
name = "auth-service"
path = "/repos/auth"

[[workspace.projects]]
name = "api-gateway"
path = "/repos/gateway"

[[workspace.projects]]
name = "web-frontend"
path = "/repos/web"

[database]
backend = "sqlite"  # or "postgresql", "mariadb"
path = "~/.local/share/clawdius/clawdius.db"  # for sqlite
url = "postgres://user:pass@localhost/clawdius"  # for postgres

[llm]
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"

[llm.providers.anthropic]
api_key = "sk-ant-..."
model = "claude-sonnet-4-20250514"

[llm.providers.openai]
api_key = "sk-..."
model = "gpt-4o"

[sandbox]
enabled = true
engine = "bubblewrap"  # or "docker", "none"

[gateway]
enabled = false  # enable for `clawdius serve`
headless = false
bind_addr = "0.0.0.0:8080"

[gateway.adapters.telegram]
enabled = false
bot_token = ""

[gateway.adapters.discord]
enabled = false
bot_token = ""

[gateway.adapters.slack]
enabled = false
bot_token = ""
app_token = ""
signing_secret = ""
```

#### A.5: Task Breakdown

| ID | Task | Effort | Depends On | Deliverable | Notes |
|----|------|--------|------------|-------------|-------|
| A-01 | Extract `StorageBackend` trait | 6d | — | `crates/clawdius-core/src/storage/mod.rs`, trait definition | Revised: 18 rusqlite call sites + row mapping to abstract (validated) |
| A-02 | Migrate `SessionStore` to trait | 3d | A-01 | Refactored session/store.rs | Revised: 3d with clean trait boundary |
| A-03 | Implement `PostgresBackend` | 3d | A-01 | `crates/clawdius-core/src/storage/postgres.rs` | |
| A-04 | Implement `MariaDbBackend` | 2d | A-01 | `crates/clawdius-core/src/storage/mariadb.rs` | |
| A-05 | Add workspace/project schema | 2d | A-01 | Schema migrations for all backends | |
| A-06 | Workspace + Project models | 2d | A-05 | `crates/clawdius-core/src/workspace/` module | |
| A-07 | Multi-repo context injection | 3d | A-06 | Sprint engine context builder change | |
| A-08 | Tool calls with project_id | 3d | A-06 | Tool executor + parser changes | Revised: tool executor interface change is non-trivial |
| A-09 | `.clawdius.toml` config | 2d | A-01, A-06 | Config loading, CLI integration | |
| A-10 | Storage integration tests | 2d | A-02, A-03, A-04 | Test suite for all backends | |
| A-11 | Migrate api/ layer to storage trait | 3d | A-01 | REST API handlers use StorageBackend | NEW: 7.5K lines api/ depends on in-memory stores |
| A-12 | Clean up HFT signal_dispatch stubs | 1d | — | Remove or repurpose signal_dispatch.rs | NEW: 379-line HFT stub with logging-only sends |

**Total: ~32 working days (4-5 weeks)**

---

### Phase B: Messaging Gateway

**Duration:** 6-8 weeks
**Priority:** P0 — core differentiator
**Goal:** Separate `clawdius-gateway` binary that connects to 9 messaging platforms,
routes messages to the sprint engine, and streams responses back.

#### B.1: Gateway Binary

```
crates/clawdius-gateway/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point, config, signal handling
    ├── gateway.rs           # Core message routing loop
    ├── adapter.rs           # PlatformAdapter trait definition
    ├── router.rs            # Session binding, command parsing
    ├── formatter.rs         # Platform-aware response chunking
    ├── rate_limit.rs        # Per-user, per-platform rate limiting
    ├── adapters/
    │   ├── mod.rs
    │   ├── telegram.rs      # Telegram Bot API
    │   ├── discord.rs       # Discord Gateway WebSocket
    │   ├── slack.rs         # Slack Bolt SDK
    │   ├── matrix.rs        # Matrix Client-Server sync
    │   ├── signal.rs        # Signal protocol bridge
    │   ├── teams.rs         # MS Teams Bot Framework
    │   ├── whatsapp.rs      # WhatsApp Cloud API
    │   ├── rocketchat.rs    # Rocket.Chat REST + WebSocket
    │   └── webhook.rs       # Generic HTTP webhook
    └── tests/
        ├── adapter_tests.rs
        ├── integration_tests.rs
        └── mock_platform.rs
```

#### B.2: PlatformAdapter Trait

```rust
/// A unified message from any platform
pub struct InboundMessage {
    pub id: String,
    pub platform: String,
    pub channel_id: String,
    pub user_id: String,
    pub username: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub reply_to: Option<String>,
    pub attachments: Vec<Attachment>,
}

/// A target for outbound messages
pub struct MessageTarget {
    pub platform: String,
    pub channel_id: String,
    pub thread_id: Option<String>,
    pub reply_to: Option<String>,
}

/// Platform-specific response formatting hints
pub struct FormatHint {
    pub supports_markdown: bool,
    pub supports_code_blocks: bool,
    pub max_message_length: usize,
    pub max_file_size: usize,
    pub supports_inline_keyboard: bool,
}

#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    fn platform_name(&self) -> &str;
    fn format_hint(&self) -> &FormatHint;

    /// Start listening for inbound messages
    async fn start(&self, tx: mpsc::Sender<InboundMessage>) -> Result<()>;

    /// Send a complete message
    async fn send_message(&self, target: &MessageTarget, content: &str) -> Result<()>;

    /// Stream a response chunk (typing indicator / incremental display)
    async fn send_chunk(&self, target: &MessageTarget, chunk: &str) -> Result<()>;

    /// Indicate the agent is processing
    async fn indicate_typing(&self, target: &MessageTarget) -> Result<()>;

    /// Send a file attachment
    async fn send_file(&self, target: &MessageTarget, name: &str, content: &[u8]) -> Result<()>;

    /// Graceful shutdown
    async fn shutdown(&self) -> Result<()>;
}
```

#### B.3: Response Chunking

Each platform has different message length limits and markdown support:

| Platform | Max Length | Markdown | Code Blocks | Inline Keyboard |
|----------|:----------:|:--------:|:-----------:|:---------------:|
| Telegram | 4,096 | Yes (partial) | Yes | Yes (inline) |
| Discord | 2,000 | Yes | Yes | No (use buttons) |
| Slack | 40,000 | Yes (mrkdwn) | Yes | Yes (Block Kit) |
| Matrix | 64,000 | Yes (org.matrix) | Yes | No |
| Signal | 64,000 | No | No | No |
| Teams | 28,000 | Yes (Adaptive Cards) | Yes | Yes |
| WhatsApp | 40,000 | Yes (basic) | Yes | Yes (interactive) |
| Rocket.Chat | 64,000 | Yes | Yes | Yes |
| Webhook | Configurable | Configurable | Configurable | No |

The `ResponseFormatter` splits long responses at code block boundaries, adds
platform-specific markdown, and sends typing indicators between chunks.

#### B.4: Session Binding

Each platform user maps to a Clawdius session via composite key `"platform:user_id"`:

```rust
pub struct SessionBinding {
    pub platform_user_key: String,   // "telegram:123456789"
    pub session_id: SessionId,
    pub workspace_id: WorkspaceId,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}
```

A user on Telegram and the same user on Discord (linked by admin config) share the
same session and chat history.

#### B.5: Task Breakdown

| ID | Task | Effort | Depends On | Deliverable |
|----|------|--------|------------|-------------|
| B-01 | `clawdius-gateway` binary scaffold | 2d | — | Cargo.toml, main.rs, config |
| B-02 | `PlatformAdapter` trait + types | 2d | — | adapter.rs, message types |
| B-03 | Core `MessageGateway` router | 3d | B-02 | gateway.rs, session binding |
| B-04 | `ResponseFormatter` | 2d | B-02 | formatter.rs |
| B-05 | `RateLimiter` (per-user, per-platform) | 2d | — | rate_limit.rs |
| B-06 | Telegram adapter | 4d | B-02, B-03 | adapters/telegram.rs |
| B-07 | Discord adapter | 4d | B-02, B-03 | adapters/discord.rs |
| B-08 | Slack adapter | 4d | B-02, B-03 | adapters/slack.rs |
| B-09 | Matrix adapter | 4d | B-02, B-03 | adapters/matrix.rs |
| B-10 | Signal adapter | 5d | B-02, B-03 | adapters/signal.rs |
| B-11 | Teams adapter | 4d | B-02, B-03 | adapters/teams.rs |
| B-12 | WhatsApp adapter | 3d | B-02, B-03 | adapters/whatsapp.rs |
| B-13 | Rocket.Chat adapter | 2d | B-02, B-03 | adapters/rocketchat.rs |
| B-14 | Webhook adapter | 2d | B-02, B-03 | adapters/webhook.rs |
| B-15 | Mock platform for testing | 2d | B-02 | tests/mock_platform.rs |
| B-16 | Integration test suite | 3d | B-06..B-14 | End-to-end adapter tests |

**Total: ~48 working days (6-8 weeks)**

---

### Phase C: TUI Polish

**Duration:** 3-4 weeks
**Priority:** P0 — daily driver must feel exceptional
**Goal:** ratatui-based TUI that matches or exceeds OpenCode/Crush quality.

#### C.1: Layout

```
┌─────────────────────────────────────────────────────────┐
│ ╭─ clawdius ─ auth-service (/repos/auth) ── claude ─╮  │
│ ├─────────────────────┬───────────────────────────────┤  │
│ │                     │                               │  │
│ │   Chat Pane         │   Code Pane                   │  │
│ │   (messages,        │   (inline diffs,              │  │
│ │    tool output)     │    file content,              │  │
│ │                     │    syntax highlighting)        │  │
│ │                     │                               │  │
│ ├─────────────────────┴───────────────────────────────┤  │
│ │ > type your message here...                         │  │
│ ├─────────────────────────────────────────────────────┤  │
│ │ proj: auth-service │ tokens: 12.4k │ phase: Build  │  │
│ ╰─────────────────────────────────────────────────────╯  │
└─────────────────────────────────────────────────────────┘
```

#### C.2: Key Features

| Feature | Detail |
|---------|--------|
| Vim keybindings | `i` insert, `Esc` normal, `/` search, `q` quit |
| Split panes | Chat + code side-by-side, resizable with `Ctrl+W` |
| Inline diff display | Show proposed edits before applying; `y` accept, `n` reject |
| Session picker | `Ctrl+S` to list/switch/restore sessions |
| Workspace switcher | `Ctrl+W` to switch active project within workspace |
| Status bar | Model, token count, phase, project, connection status |
| Syntax highlighting | tree-sitter integration for Rust, TS, Python, Go, etc. |
| Image rendering | kitty, iTerm2, Sixel terminal image protocols |
| Mouse support | Click links, scroll panes, resize splits |
| Theme support | Dark/light, custom color schemes |

#### C.3: Task Breakdown

| ID | Task | Effort | Depends On | Deliverable |
|----|------|--------|------------|-------------|
| C-01 | ratatui app scaffold | 3d | — | App shell, event loop, terminal setup |
| C-02 | Split pane layout | 3d | C-01 | Resizable horizontal/vertical splits |
| C-03 | Chat pane component | 4d | C-01 | Message list, input box, scrollback |
| C-04 | Code pane component | 4d | C-01 | File viewer, syntax highlighting |
| C-05 | Inline diff component | 3d | C-04 | Colored +/- diff, accept/reject keys |
| C-06 | Session picker | 2d | C-03 | Popup list of sessions |
| C-07 | Workspace switcher | 2d | C-03 | Popup list of projects |
| C-08 | Status bar | 2d | C-01 | Model, tokens, phase, project |
| C-09 | Vim keybindings | 3d | C-01 | Modal editing in input box |
| C-10 | Syntax highlighting | 4d | C-04 | tree-sitter integration |
| C-11 | Mouse support | 2d | C-01 | Click, scroll, resize |
| C-12 | Theme system | 2d | C-01 | Dark/light, custom colors |
| C-13 | Clean up TODO/FIXME comments | 2d | — | Resolve 60 TODOs, especially completion.rs (9) |

**Total: ~34 working days (3-4 weeks)**

---

### Phase D: Deployment Infrastructure

**Duration:** 2-3 weeks
**Priority:** P1 — needed for SaaS and enterprise deployment
**Goal:** Docker images, Helm charts, Terraform modules for Hetzner.

#### D.1: Artifacts

| Artifact | Description |
|----------|-------------|
| `Dockerfile` | Multi-stage build: builder (Rust) → runtime (scratch/debian) |
| `Dockerfile.gateway` | Gateway-only image (no TUI deps) |
| `docker-compose.yml` | Local dev: clawdius + PostgreSQL + gateway |
| `helm/clawdius/` | Helm chart: Deployment, Service, Ingress, PVC, ConfigMap, Secret |
| `terraform/hetzner/` | Hetzner cloud: server, firewall, DNS, volumes |
| `terraform/k3s/` | K3s single-node setup for small teams |

#### D.2: Task Breakdown

| ID | Task | Effort | Depends On | Deliverable |
|----|------|--------|------------|-------------|
| D-01 | Multi-stage Dockerfile | 2d | — | Docker images for cli + gateway |
| D-02 | docker-compose.yml | 1d | D-01 | Local dev stack |
| D-03 | Helm chart | 3d | D-01 | Production K8s deployment |
| D-04 | Terraform Hetzner module | 3d | — | Infrastructure provisioning |
| D-05 | Terraform K3s module | 2d | D-04 | Single-node K3s setup |
| D-06 | Health checks + /metrics | 2d | — | Prometheus metrics, /health endpoint |
| D-07 | 0CVE baseline | 2d | D-01 | Audit transitive deps, pin versions |

**Total: ~15 working days (2-3 weeks)**

---

### Phase E: SaaS + Billing

**Duration:** 4-6 weeks
**Priority:** P1 — revenue enablement
**Goal:** Stripe billing, usage metering, admin API, web dashboard.

#### E.1: Billing Model

| Tier | Price | Limits |
|------|-------|--------|
| Free | $0/mo | 1 workspace, 1 project, 50 messages/day, 1 user |
| Pro | $20/mo | 10 workspaces, unlimited projects, unlimited messages, 5 users |
| Enterprise | Custom | Unlimited everything, dedicated support, compliance artifacts |

#### E.2: Task Breakdown

| ID | Task | Effort | Depends On | Deliverable |
|----|------|--------|------------|-------------|
| E-01 | Stripe integration | 5d | A (storage) | Subscription management, webhooks |
| E-02 | Usage metering | 3d | A | Token counting per tenant, billing cycles |
| E-03 | Admin REST API | 5d | A | Tenant CRUD, usage reports, API keys |
| E-04 | Web dashboard (React) | 10d | E-03 | Admin UI for tenant management |
| E-05 | Invoice generation | 3d | E-01 | PDF invoices, Stripe integration |

**Total: ~26 working days (4-6 weeks)**

---

### Phase F: Enterprise Hardening

**Duration:** 4-6 weeks
**Priority:** P2 — defense/fintech contract enabler
**Goal:** Compliance artifacts, air-gapped mode, encryption, SOC2 prep.

#### F.1: Task Breakdown

| ID | Task | Effort | Depends On | Deliverable |
|----|------|--------|------------|-------------|
| F-01 | Compliance artifact generator | 7d | — | Lean4 proofs → STIG/DISA/OWASP evidence |
| F-02 | Air-gapped mode | 5d | A, D | Vendored deps, local-only, no outbound |
| F-03 | Encryption at rest | 3d | A | AES-256 for sessions, messages, audit |
| F-04 | SOC2 Type I prep | 10d | E | Access review, incident response, change mgmt |
| F-05 | FedRAMP artifacts | Ongoing | F-04 | Continuous compliance documentation |

**Total: ~25 working days (4-6 weeks)**

---

## 4. Dependency Graph

```
Phase A (Storage + Multi-Codebase) ─── 3-4 weeks
    ├── Phase B (Messaging Gateway)  ─── 6-8 weeks (depends on A)
    ├── Phase C (TUI Polish)         ─── 3-4 weeks (parallel with B)
    └── Phase D (Deployment)         ─── 2-3 weeks (depends on A)
            └── Phase E (SaaS)       ─── 4-6 weeks (depends on D)
                    └── Phase F (Enterprise) ─── 4-6 weeks (depends on E)

Critical path: A → D → E → F = 13-19 weeks
Parallel path: A → B + C = 9-12 weeks
Total (parallel): ~20-27 weeks
```

---

## 5. Risk Register

| Risk | Probability | Impact | Mitigation |
|------|:-----------:|:------:|------------|
| Messaging platform API breaking changes | High | Medium | Pin SDK versions, adapter abstraction layer |
| Multi-codebase context window explosion | Medium | High | Per-project repo-map budgeting, context compaction |
| Storage backend divergence | Medium | Medium | Shared trait tests, integration test matrix |
| TUI complexity (ratatui learning curve) | Medium | Low | Start with simple layout, iterate |
| Competitor releases similar feature | Medium | Medium | Speed via Rust performance, depth via formal verification |
| OpenCode file reversion during development | High | Low | Git-based workflow, CI/CD, avoid concurrent agent access |
| rusqlite coupling deeper than estimated | High | Low | A-01 revised to 6d; trait extraction is well-scoped |
| api/ layer migration scope hidden | Medium | Medium | A-11 explicitly migrates 7.5K-line api/ module |

---

## 6. Quality Gates

| Gate | Criteria | Verification |
|------|----------|-------------|
| A-complete | All storage backends pass integration tests; multi-repo context injection works | `cargo test -p clawdius-core -- storage` |
| B-complete | At least 3 adapters pass end-to-end tests; session binding works | Integration test suite |
| C-complete | TUI renders correctly; vim keys work; split panes resize | Manual testing + screenshots |
| D-complete | Docker image builds; Helm chart deploys; Terraform provisions | `docker build`, `helm install`, `terraform apply` |
| E-complete | Stripe checkout works; usage metering accurate; admin API functional | End-to-end billing test |
| F-complete | Compliance artifacts generated; air-gapped mode verified | Manual audit + pen test |

---

## 7. Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Test count | 902 | 1500+ | `cargo test --workspace` |
| Lean4 theorems | 114 | 200+ | Lean compiler |
| Code coverage (critical paths) | Unknown | >95% | `cargo tarpaulin` |
| Binary size (CLI) | ~15MB | <20MB | `ls -la target/release/clawdius` |
| Binary size (gateway) | N/A | <15MB | `ls -la target/release/clawdius-gateway` |
| Boot time (TUI) | ~200ms | <100ms | `time clawdius` |
| Message routing latency (P99) | N/A | <1ms | Benchmark suite |
| Platforms supported | 0 | 9 | Adapter count |
| Storage backends | 1 (SQLite) | 4 (SQLite, Postgres, MariaDB, InMemory) | Backend count |
| Total roadmap tasks | 56 | 59 | master_plan.toml |
