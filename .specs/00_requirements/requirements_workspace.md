# Requirements Specification: Multi-Codebase Workspace

## Document Header

| Attribute | Value |
|-----------|-------|
| **Document ID** | REQ-WORKSPACE-001 |
| **Version** | 1.0.0 |
| **Status** | APPROVED |
| **Created** | 2026-04-26 |
| **Author** | Nexus |
| **Traceability** | ROADMAP-001 Phase A |

---

## 1. Functional Requirements

### 1.1 Workspace Model

#### REQ-WS-001: Multi-Project Workspace
**EARS Pattern:** Ubiquitous

A workspace SHALL support mounting 1..N codebases (projects) simultaneously.
The agent SHALL be able to read, search, and edit files across any mounted project
within a single conversation.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-001 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Workspace contains 1..N projects
- [ ] Each project has a unique name and root path
- [ ] Agent can read files from any mounted project
- [ ] Agent can edit files in any mounted project
- [ ] Agent can run shell commands in any mounted project's directory
- [ ] Chat history is unified across all projects in the workspace

---

#### REQ-WS-002: Default Project
**EARS Pattern:** State-driven

When a workspace has multiple projects, one SHALL be designated as the default.
Commands that don't specify a project target SHALL use the default.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-002 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] Default project settable via config and CLI
- [ ] Bare tool calls (no project param) target default project
- [ ] Status bar shows current default project
- [ ] TUI workspace switcher changes default project

---

#### REQ-WS-003: Per-Project Isolation
**EARS Pattern:** Ubiquitous

Each project within a workspace SHALL have independent sandbox configuration,
git state, and repo-map.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-003 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Shell commands run in project's root directory (sandboxed)
- [ ] Git operations are per-project (separate .git directories)
- [ ] Repo-map computed independently per project
- [ ] Sandbox mount points are per-project
- [ ] File edits in one project don't affect another project's git state

---

### 1.2 Context Injection

#### REQ-WS-010: Multi-Repo Context
**EARS Pattern:** Ubiquitous

When a workspace has multiple projects, the sprint engine SHALL inject repo-maps
for ALL mounted projects into the LLM context, labeled by project name.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-010 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Context includes project names and root paths
- [ ] Each project's repo-map is labeled with project name
- [ ] Token budget distributed across projects proportionally
- [ ] If total repo-maps exceed context window, largest/most-relevant projects prioritized
- [ ] System prompt includes instructions for cross-project editing

---

#### REQ-WS-011: Tool Call Project Targeting
**EARS Pattern:** Ubiquitous

All file and shell tool calls SHALL accept an optional `project` parameter to
target a specific project.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-011 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] `edit_file` accepts `project` parameter
- [ ] `read_file` accepts `project` parameter
- [ ] `shell` accepts `project` parameter (sets cwd)
- [ ] `list_files` accepts `project` parameter
- [ ] If `project` omitted, default project used
- [ ] If `project` name doesn't exist, error returned with list of valid projects

---

### 1.3 Configuration

#### REQ-WS-020: Workspace Configuration
**EARS Pattern:** State-driven

When the user creates a `.clawdius.toml` with `[[workspace.projects]]` entries,
the system SHALL load all specified projects into the workspace.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-020 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] TOML config with `[[workspace.projects]]` loads N projects
- [ ] `path` validated to exist on filesystem
- [ ] `name` defaults to directory name if omitted
- [ ] CLI flag `--projects /a,/b` overrides TOML config
- [ ] Single directory invocation (`clawdius /path`) creates 1-project workspace
- [ ] Invalid paths produce clear error message

---

### 1.4 Chat History

#### REQ-WS-030: Unified Chat History
**EARS Pattern:** Ubiquitous

All conversations within a workspace SHALL share a single chat history,
regardless of which project(s) were involved in the conversation.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-030 |
| **Priority** | MUST |
| **Verification** | Integration Test |

**Acceptance Criteria:**
- [ ] Messages reference which project(s) they touched
- [ ] Session resume restores full cross-project conversation
- [ ] Chat search finds messages across all projects
- [ ] Session list shows which projects were involved

---

## 2. Non-Functional Requirements

### 2.1 Performance

#### REQ-WS-040: Context Budget Distribution
**EARS Pattern:** Ubiquitous

When multiple projects are mounted, the repo-map token budget SHALL be distributed
proportionally or by relevance, never exceeding the total context window.

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-040 |
| **Priority** | MUST |
| **Verification** | Unit Test |

**Acceptance Criteria:**
- [ ] Total injected tokens ≤ configured context budget
- [ ] Each project gets at least minimum tokens (10% of budget)
- [ ] Distribution algorithm documented and tested
- [ ] Oversized projects truncated gracefully (not error)

---

### 2.2 Scalability

#### REQ-WS-041: Project Count Limits
**EARS Pattern:** Ubiquitous

| Tier | Max Projects | Max Total Repo-Map Tokens |
|------|:------------:|:-------------------------:|
| Free | 1 | 32,000 |
| Pro | 10 | 128,000 |
| Enterprise | 100 | 512,000 |

| Attribute | Value |
|-----------|-------|
| **ID** | REQ-WS-041 |
| **Priority** | MUST |
| **Verification** | Unit Test |

---

## 3. Traceability Matrix

| Requirement ID | Component | Test Case | Standard |
|----------------|-----------|-----------|----------|
| REQ-WS-001 | Workspace, SprintEngine | TC-WS-001 | IEEE 1016 |
| REQ-WS-002 | Workspace, TUI | TC-WS-002 | IEEE 1016 |
| REQ-WS-003 | Sandbox, Git, RepoMap | TC-WS-003 | IEEE 1016 |
| REQ-WS-010 | SprintEngine ContextBuilder | TC-WS-010 | IEEE 1016 |
| REQ-WS-011 | ToolExecutor, Parser | TC-WS-011 | IEEE 1016 |
| REQ-WS-020 | Config, CLI | TC-WS-020 | IEEE 1016 |
| REQ-WS-030 | Session, ChatHistory | TC-WS-030 | IEEE 1016 |
| REQ-WS-040 | ContextBuilder | TC-WS-040 | IEEE 1016 |
| REQ-WS-041 | Workspace, TenantTier | TC-WS-041 | IEEE 1016 |

---

## 4. Document Status

| Quality Gate | Status |
|---------------|--------|
| Requirements Complete | [PASS] |
| Acceptance Criteria Defined | [PASS] |
| Traceability Established | [PASS] |
| Stakeholder Review | [PENDING] Pending |
