# Clawdius Implementation Roadmap v0.7.0

**Generated:** 2026-03-06
**Current Version:** 0.6.0
**Target Version:** 0.7.0
**Estimated Timeline:** 6 weeks

---

## Sprint 1: Critical Fixes (Week 1-2)

### Goal: Eliminate all skeleton implementations and mock code

#### Day 1-2: Command Executor

**File:** `crates/clawdius-core/src/commands/executor.rs`

**Current State:**
```rust
pub async fn execute(_command: &CustomCommand, _args: HashMap<String, String>) -> Result<()> {
    // TODO: Implement command execution
    Ok(())
}
```

**Target State:**
- Parse command templates
- Substitute variables from args
- Execute via appropriate tool
- Handle errors gracefully
- Return structured results

**Implementation:**
1. Add template parsing with handlebars-style syntax
2. Map command types to tools (file, shell, git)
3. Add validation and sanitization
4. Implement result aggregation

**Effort:** 8 hours
**Priority:** P0 (Critical)
**Dependencies:** None

---

#### Day 3-4: Real Completions

**File:** `crates/clawdius-core/src/rpc/handlers/completion.rs`

**Current State:**
```rust
// Lines 141-144: Mock completions
let text = if last_line.trim().starts_with("//") {
    " TODO: Add implementation".to_string()
} else if last_line.trim().starts_with("fn ") && last_line.contains('{') {
    "\n    unimplemented!()\n".to_string()
}
```

**Target State:**
- Use LLM when available
- Fall back to intelligent heuristics
- Cache completions
- Add cancellation support

**Implementation:**
1. Remove mock logic (move to fallback)
2. Ensure LLM path is always used when configured
3. Add completion caching
4. Implement timeout handling

**Effort:** 4 hours
**Priority:** P0 (Critical)
**Dependencies:** None

---

#### Day 5-7: TODO Cleanup

**Files:** Various (22 locations)

**Strategy:**
1. **Remove obsolete TODOs** - Features that aren't planned
2. **Convert to issues** - Track on GitHub
3. **Implement simple ones** - Quick wins
4. **Document deferrals** - Add to roadmap

**Priority Order:**

| File | TODO | Action | Effort |
|------|------|--------|--------|
| `actions/tests.rs` | Test implementation | Remove (not planned) | 0.5h |
| `cli.rs` | Test implementation | Remove (not planned) | 0.5h |
| `checkpoint/snapshot.rs` | Snapshot creation | Implement | 4h |
| `cli.rs` | Send to LLM | Already implemented | 0h |

**Effort:** 8 hours
**Priority:** P1 (High)

---

#### Day 8-10: Error Handling

**Files:** Various

**Current Issues:**
- Some functions return `Result<()>` but don't propagate errors
- Missing error context
- Inconsistent error types

**Target State:**
- Comprehensive error chain
- Error context with source location
- User-friendly error messages
- Error recovery suggestions

**Implementation:**
1. Audit all `Result` types
2. Add `thiserror` derives where missing
3. Add context with `.context()` or `.map_err()`
4. Create error recovery guide

**Effort:** 4 hours
**Priority:** P1 (High)

---

## Sprint 2: Feature Completion (Week 3-4)

### Goal: Complete partially implemented features

#### Week 3: JSON Output

**Files:** 
- `crates/clawdius/src/cli.rs`
- `crates/clawdius-core/src/output/`

**Current State:**
- `--format` flag exists
- Only `metrics` command supports JSON
- Other commands use text output

**Target State:**
- All commands support `--format json`
- Consistent JSON schema
- Streaming JSON output option
- Pretty-print option

**Implementation:**

1. **Define Output Traits:**
```rust
pub trait OutputFormat {
    fn to_text(&self) -> String;
    fn to_json(&self) -> Result<String>;
}

pub struct OutputOptions {
    pub format: OutputType,
    pub pretty: bool,
    pub stream: bool,
}
```

2. **Implement for Each Command:**
- `chat` - Conversation history
- `sessions` - Session list
- `tools` - Tool results
- `config` - Configuration
- `metrics` - Already done

**Effort:** 6 hours
**Priority:** P0 (Critical)

---

#### Week 4: File Timeline

**New Files:**
- `crates/clawdius-core/src/timeline/mod.rs`
- `crates/clawdius-core/src/timeline/manager.rs`
- `crates/clawdius-core/src/timeline/snapshot.rs`

**Features:**
1. Track file changes in real-time
2. Store snapshots at checkpoints
3. Support rollback to any point
4. Show diff between versions

**Implementation:**

```rust
pub struct TimelineManager {
    store: Arc<TimelineStore>,
    watcher: FileWatcher,
}

impl TimelineManager {
    pub async fn track_file(&self, path: &Path) -> Result<()>;
    pub async fn create_checkpoint(&self, name: &str) -> Result<CheckpointId>;
    pub async fn rollback(&self, checkpoint: &CheckpointId) -> Result<()>;
    pub async fn diff(&self, from: &CheckpointId, to: &CheckpointId) -> Result<Diff>;
}
```

**Effort:** 12 hours
**Priority:** P0 (Critical)

---

## Sprint 3: Polish & UX (Week 5-6)

### Goal: Improve user experience and documentation

#### Week 5: External Editor

**Files:**
- `crates/clawdius-core/src/editor/mod.rs` (new)
- `crates/clawdius/src/cli.rs`

**Features:**
1. Open $EDITOR for long prompts
2. Support common editors (vim, nano, code)
3. Preserve formatting
4. Handle editor exit codes

**Implementation:**

```rust
pub struct ExternalEditor {
    editor: String,
    tmpfile: PathBuf,
}

impl ExternalEditor {
    pub fn from_env() -> Result<Self>;
    pub async fn edit(&self, initial: &str) -> Result<String>;
}
```

**CLI Usage:**
```bash
clawd chat --editor
# Opens $EDITOR, waits for save/close
# Sends content as message
```

**Effort:** 4 hours
**Priority:** P1 (High)

---

#### Week 5-6: WASM Webview Polish

**Files:** `crates/clawdius-webview/src/`

**Current State:**
- Basic Leptos components
- Placeholder for history
- Placeholder for settings

**Target State:**
- Complete history component
- Implement settings panel
- Add theme support
- Improve chat UX

**Implementation:**

1. **History Component:**
```rust
#[component]
fn HistoryView(cx: Scope) -> impl IntoView {
    let sessions = use_context::<Signal<Vec<Session>>>();
    // List sessions
    // Search functionality
    // Load session on click
}
```

2. **Settings Component:**
```rust
#[component]
fn SettingsPanel(cx: Scope) -> impl IntoView {
    // Provider configuration
    // Theme selection
    // Keybindings
    // Import/Export settings
}
```

**Effort:** 12 hours
**Priority:** P1 (High)

---

#### Week 6: Enhanced @Mentions

**Files:**
- `crates/clawdius-core/src/context/mentions.rs`
- `crates/clawdius-core/src/tools/image.rs` (new)

**New Features:**
1. `@image:path` - Include image with vision analysis
2. `@code:symbol` - Reference code symbol
3. `@commit:hash` - Include commit diff
4. `@issue:number` - Include GitHub issue

**Implementation:**

```rust
pub enum MentionType {
    File(PathBuf),
    Folder(PathBuf),
    Url(String),
    Image(PathBuf),      // NEW
    CodeSymbol(String),  // NEW
    Commit(String),      // NEW
    Issue(u64),          // NEW
}
```

**Effort:** 8 hours
**Priority:** P1 (High)

---

## Quality Gates

### Sprint 1 Completion Criteria

- [ ] Command executor fully implemented
- [ ] Real completions working (no mocks)
- [ ] All TODOs resolved or documented
- [ ] Error handling comprehensive
- [ ] All tests passing
- [ ] Zero clippy warnings (new code)

### Sprint 2 Completion Criteria

- [ ] JSON output for all commands
- [ ] File timeline functional
- [ ] Timeline tests passing
- [ ] Documentation updated

### Sprint 3 Completion Criteria

- [ ] External editor working
- [ ] WASM webview polished
- [ ] Enhanced @mentions
- [ ] All features documented
- [ ] Performance benchmarks

---

## Testing Strategy

### Unit Tests

**Coverage Target:** 85%

**Priority Areas:**
1. Command executor
2. File timeline
3. Mention parsing
4. Output formatting

### Integration Tests

**New Tests Required:**
1. Timeline workflow
2. External editor
3. JSON output
4. @mentions variations

### Property-Based Tests

**Areas:**
1. Mention parsing (arbitrary strings)
2. JSON serialization (round-trip)
3. Timeline operations (rollback consistency)

---

## Documentation Updates

### Required Updates

1. **README.md**
   - Add file timeline section
   - Add external editor usage
   - Update feature list

2. **User Guide**
   - Timeline usage guide
   - Editor configuration
   - JSON output examples

3. **API Reference**
   - Timeline API
   - Editor API
   - Output formatting

4. **Architecture Docs**
   - Timeline design
   - Editor integration
   - @mentions system

---

## Risk Mitigation

### Technical Risks

| Risk | Mitigation |
|------|------------|
| Timeline performance | Use incremental snapshots |
| Editor compatibility | Test major editors |
| WASM bundle size | Code splitting |
| JSON schema changes | Version schemas |

### Schedule Risks

| Risk | Mitigation |
|------|------------|
| Scope creep | Strict sprint goals |
| Dependencies | Parallel work streams |
| Testing delays | Continuous testing |

---

## Success Metrics

### v0.7.0 Release Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| Features | 100% complete | Feature matrix |
| Test coverage | 85% | cargo-tarpaulin |
| Performance | <1s startup | Benchmark |
| Memory | <150MB | Profiling |
| Documentation | 100% | Doc coverage |
| TODOs | 0 | grep count |
| Warnings | <50 | cargo check |

---

## Rollout Plan

### Alpha Release (Week 4)

- Internal testing
- Feature complete
- Known issues documented

### Beta Release (Week 5)

- Limited external testing
- Feedback collection
- Bug fixes

### Stable Release (Week 6)

- Full release
- Documentation complete
- Migration guide

---

## Post-v0.7.0

### v0.8.0 Focus: Performance

- Profile and optimize hot paths
- Reduce memory footprint
- Improve response latency
- Add caching layers

### v0.9.0 Focus: Security

- Security audit
- Penetration testing
- Supply chain verification
- Compliance preparation

### v1.0.0 Focus: Platform

- Plugin system
- API stability
- Enterprise features
- Final polish

---

*Roadmap generated on 2026-03-06*
*Owner: Development Team*
*Review: Weekly*
