# Clawdius Feature Implementation Roadmap

**Version:** 1.0.0  
**Date:** March 2026  
**Status:** Planning

---

## Overview

This document outlines the implementation plan for all identified missing features, organized by phase and priority.

---

## Phase 1: Core Infrastructure (Weeks 1-4)

### 1.1 JSON-RPC Protocol

**Goal:** Enable VSCode extension communication with Clawdius binary.

**File:** `src/rpc/mod.rs`

```rust
/// JSON-RPC 2.0 protocol implementation
pub mod types;
pub mod server;
pub mod client;

/// Core RPC types
pub struct Request {
    pub jsonrpc: String, // "2.0"
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

pub struct Response {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<Error>,
}

/// RPC methods exposed by Clawdius
pub enum Method {
    // Session management
    SessionCreate,
    SessionLoad,
    SessionSave,
    SessionList,
    SessionDelete,
    
    // Chat operations
    ChatSend,
    ChatStream,
    ChatCancel,
    
    // Context management
    ContextAdd,
    ContextRemove,
    ContextList,
    ContextCompact,
    
    // File operations
    FileRead,
    FileWrite,
    FileEdit,
    FileDiff,
    
    // Tools
    ToolList,
    ToolExecute,
    
    // State
    StateGet,
    StateCheckpoint,
    StateRestore,
}
```

**Implementation:**
- Use `jsonrpsee` crate for async JSON-RPC
- Stdio transport for VSCode extension
- Optional TCP transport for remote access
- WebSocket for streaming responses

---

### 1.2 Session Persistence

**Goal:** Persist conversations across restarts.

**File:** `src/session/persistence.rs`

```rust
/// Session storage schema
pub struct SessionStore {
    db: rusqlite::Connection,
}

impl SessionStore {
    pub fn create_session(&self, metadata: SessionMeta) -> Result<SessionId>;
    pub fn load_session(&self, id: SessionId) -> Result<Session>;
    pub fn save_message(&self, session_id: SessionId, msg: Message) -> Result<()>;
    pub fn list_sessions(&self, filter: SessionFilter) -> Result<Vec<SessionMeta>>;
    pub fn delete_session(&self, id: SessionId) -> Result<()>;
    pub fn search_messages(&self, query: &str) -> Result<Vec<Message>>;
}

/// Database schema
/// 
/// CREATE TABLE sessions (
///     id TEXT PRIMARY KEY,
///     title TEXT,
///     created_at TIMESTAMP,
///     updated_at TIMESTAMP,
///     provider TEXT,
///     model TEXT,
///     metadata JSON
/// );
/// 
/// CREATE TABLE messages (
///     id TEXT PRIMARY KEY,
///     session_id TEXT REFERENCES sessions(id),
///     role TEXT CHECK(role IN ('user', 'assistant', 'system', 'tool')),
///     content TEXT,
///     tokens_used INTEGER,
///     created_at TIMESTAMP,
///     metadata JSON
/// );
/// 
/// CREATE TABLE checkpoints (
///     id TEXT PRIMARY KEY,
///     session_id TEXT REFERENCES sessions(id),
///     workspace_snapshot BLOB,
///     message_id TEXT REFERENCES messages(id),
///     created_at TIMESTAMP,
///     description TEXT
/// );
```

**Location:** `.clawdius/sessions.db`

---

### 1.3 @Mentions Context System

**Goal:** Quick context addition via @mentions.

**File:** `src/context/mentions.rs`

```rust
/// Mention types
pub enum Mention {
    /// @file:path - Add file contents
    File { path: PathBuf },
    
    /// @folder:path - Add all files in folder
    Folder { path: PathBuf, recursive: bool },
    
    /// @url:https://... - Fetch and convert to markdown
    Url { url: String },
    
    /// @problems - Add workspace diagnostics
    Problems { severity: Severity },
    
    /// @git:diff - Add current git diff
    GitDiff { staged: bool },
    
    /// @git:log - Add recent commits
    GitLog { count: usize },
    
    /// @symbol:name - Add symbol definition and usages
    Symbol { name: String },
    
    /// @search:query - Semantic search codebase
    Search { query: String, limit: usize },
}

impl Mention {
    /// Parse @mention from text
    pub fn parse(text: &str) -> Result<Vec<(usize, usize, Mention)>>;
    
    /// Resolve mention to context content
    pub async fn resolve(&self, ctx: &Context) -> Result<ContextContent>;
}

/// Example parsing:
/// "Fix the bug in @file:src/main.rs at the @symbol:process function"
/// → [Mention::File("src/main.rs"), Mention::Symbol("process")]
```

**CLI Usage:**
```bash
clawdius chat "Fix @file:src/main.rs and update @file:tests/main_test.rs"
clawdius chat "@url:https://docs.rs/tokio How do I use this with @file:src/server.rs?"
```

---

### 1.4 JSON Output Format

**Goal:** Structured output for scripting.

**File:** `src/output/format.rs`

```rust
/// Output format options
pub enum OutputFormat {
    /// Human-readable text (default)
    Text,
    
    /// Single JSON object
    Json,
    
    /// Newline-delimited JSON events
    StreamJson,
}

/// JSON output structure
#[derive(Serialize)]
pub struct JsonOutput {
    /// Response content
    pub content: String,
    
    /// Tool calls made
    pub tool_calls: Vec<ToolCall>,
    
    /// Files modified
    pub files_changed: Vec<FileChange>,
    
    /// Token usage
    pub usage: TokenUsage,
    
    /// Session ID
    pub session_id: String,
    
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Stream event types
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "start")]
    Start { session_id: String },
    
    #[serde(rename = "token")]
    Token { content: String },
    
    #[serde(rename = "tool_call")]
    ToolCall { name: String, args: Value },
    
    #[serde(rename = "tool_result")]
    ToolResult { name: String, result: Value },
    
    #[serde(rename = "file_change")]
    FileChange { path: String, change_type: ChangeType },
    
    #[serde(rename = "complete")]
    Complete { usage: TokenUsage },
    
    #[serde(rename = "error")]
    Error { message: String, code: String },
}
```

**CLI Usage:**
```bash
# JSON output
clawdius chat "Explain this code" --output-format json

# Stream events for CI/CD
clawdius chat "Run tests and fix failures" --output-format stream-json

# Quiet mode (no spinner)
clawdius chat "Quick question" --quiet
```

---

### 1.5 Auto-Compact

**Goal:** Automatically summarize when approaching context limit.

**File:** `src/context/compactor.rs`

```rust
/// Auto-compact configuration
pub struct CompactConfig {
    /// Trigger at this % of context window
    pub threshold_percent: f32, // default: 0.85
    
    /// Keep this many recent messages
    pub keep_recent: usize, // default: 4
    
    /// Summarization model (cheaper/smaller)
    pub summary_model: Option<String>,
    
    /// Minimum messages before compacting
    pub min_messages: usize, // default: 10
}

impl Session {
    /// Check if compaction needed
    pub fn needs_compaction(&self, config: &CompactConfig) -> bool {
        let usage = self.token_usage();
        let limit = self.model.context_window();
        usage as f32 / limit as f32 >= config.threshold_percent
    }
    
    /// Perform compaction
    pub async fn compact(&mut self, config: &CompactConfig) -> Result<CompactSummary> {
        // 1. Keep recent messages (user's current context)
        // 2. Summarize older messages
        // 3. Replace old messages with summary
        // 4. Update token counts
    }
}

/// Compact summary stored as system message
pub struct CompactSummary {
    pub summarized_count: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub summary: String,
}
```

**Configuration:**
```toml
# clawdius.toml
[session]
auto_compact = true
compact_threshold = 0.85
compact_keep_recent = 4
```

---

## Phase 2: VSCode Extension (Weeks 5-10)

### 2.1 Extension Structure

```
editors/vscode/
├── package.json              # Extension manifest
├── tsconfig.json
├── src/
│   ├── extension.ts          # Entry point
│   ├── rpc/
│   │   ├── client.ts         # JSON-RPC client
│   │   └── protocol.ts       # Type definitions
│   ├── providers/
│   │   ├── chatProvider.ts   # Chat view
│   │   ├── diffProvider.ts   # Diff view
│   │   └── statusProvider.ts # Status bar
│   ├── commands/
│   │   ├── chat.ts
│   │   ├── refactor.ts
│   │   └── explain.ts
│   └── utils/
│       ├── clawdius.ts       # Binary discovery/launch
│       └── context.ts        # VSCode context helpers
├── webview/                  # UI (see 2.2)
│   └── ...
└── README.md
```

### 2.2 Webview UI in Rust (Leptos → WASM)

**Goal:** Build webview UI in Rust, compiled to WASM.

**Directory:** `src/webview/`

```rust
// src/webview/src/lib.rs
use leptos::*;

#[component]
fn ChatView() -> impl IntoView {
    let (messages, set_messages) = create_signal(Vec::<Message>::new());
    let (input, set_input) = create_signal(String::new());
    let (loading, set_loading) = create_signal(false);
    
    view! {
        <div class="chat-container">
            <MessageList messages=messages />
            <ChatInput 
                input=input 
                on_send=move || {
                    // Send to Clawdius via postMessage
                }
            />
            <StatusBar loading=loading />
        </div>
    }
}

#[component]
fn DiffView(original: String, modified: String) -> impl IntoView {
    view! {
        <div class="diff-container">
            <div class="diff-pane original">
                <pre>{original}</pre>
            </div>
            <div class="diff-pane modified">
                <pre>{modified}</pre>
            </div>
        </div>
    }
}

#[component]
fn FileTree(files: Vec<FileInfo>) -> impl IntoView {
    view! {
        <ul class="file-tree">
            {files.iter().map(|f| view! {
                <li class="file-item">
                    <span class="file-icon">{f.icon()}</span>
                    <span class="file-name">{f.name}</span>
                </li>
            }).collect::<Vec<_>>()}
        </ul>
    }
}
```

**Build:**
```toml
# Cargo.toml workspace member
[workspace.members]
members = ["src/webview"]
```

```toml
# src/webview/Cargo.toml
[package]
name = "clawdius-webview"
version = "0.1.0"

[dependencies]
leptos = { version = "0.6", features = ["csr"] }
wasm-bindgen = "0.2"
web-sys = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"
```

### 2.3 TypeScript RPC Client

```typescript
// src/rpc/client.ts
import { spawn, ChildProcess } from 'child_process';
import { EventEmitter } from 'events';

export class ClawdiusClient extends EventEmitter {
    private process: ChildProcess;
    private requestId = 0;
    private pending = new Map<number, { resolve, reject }>();
    
    constructor(binaryPath: string) {
        this.process = spawn(binaryPath, ['--rpc', '--stdio']);
        this.setupListeners();
    }
    
    async request(method: string, params: any): Promise<any> {
        const id = ++this.requestId;
        const request = { jsonrpc: "2.0", id, method, params };
        
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject });
            this.process.stdin!.write(JSON.stringify(request) + '\n');
        });
    }
    
    // Convenience methods
    async chat(message: string, context?: Context): Promise<ChatResponse> {
        return this.request('chat/send', { message, context });
    }
    
    async addContext(type: string, path: string): Promise<void> {
        return this.request('context/add', { type, path });
    }
    
    async createCheckpoint(description?: string): Promise<Checkpoint> {
        return this.request('state/checkpoint', { description });
    }
    
    async restoreCheckpoint(id: string): Promise<void> {
        return this.request('state/restore', { id });
    }
}
```

### 2.4 VSCode Commands

```typescript
// src/commands/chat.ts
import * as vscode from 'vscode';
import { ClawdiusClient } from '../rpc/client';

export function registerCommands(
    context: vscode.ExtensionContext,
    client: ClawdiusClient
) {
    // Chat with selection
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.chatSelection', async () => {
            const editor = vscode.window.activeTextEditor;
            const selection = editor?.document.getText(editor.selection);
            if (selection) {
                await client.chat(`Explain this code:\n\`\`\`\n${selection}\n\`\`\``);
            }
        })
    );
    
    // Add file to context
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.addFileContext', async () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                await client.addContext('file', editor.document.uri.fsPath);
                vscode.window.showInformationMessage('Added to context');
            }
        })
    );
    
    // Create checkpoint
    context.subscriptions.push(
        vscode.commands.registerCommand('clawdius.checkpoint', async () => {
            const desc = await vscode.window.showInputBox({
                prompt: 'Checkpoint description'
            });
            const checkpoint = await client.createCheckpoint(desc);
            vscode.window.showInformationMessage(`Checkpoint created: ${checkpoint.id}`);
        })
    );
}
```

---

## Phase 3: Browser Automation (Weeks 11-14)

### 3.1 Browser Tool

**File:** `src/tools/browser.rs`

```rust
use headless_chrome::{Browser, Tab};

pub struct BrowserTool {
    browser: Browser,
    tab: Arc<Tab>,
}

impl BrowserTool {
    pub async fn navigate(&self, url: &str) -> Result<()>;
    pub async fn click(&self, selector: &str) -> Result<()>;
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<()>;
    pub async fn scroll(&self, direction: ScrollDirection) -> Result<()>;
    pub async fn screenshot(&self) -> Result<Vec<u8>>;
    pub async fn console_logs(&self) -> Result<Vec<ConsoleLog>>;
    pub async fn evaluate(&self, js: &str) -> Result<serde_json::Value>;
    pub async fn wait_for(&self, selector: &str, timeout: Duration) -> Result<()>;
}

/// Tool definition for LLM
pub fn browser_tool_definition() -> Tool {
    Tool {
        name: "browser".to_string(),
        description: "Control a headless browser for testing and debugging".to_string(),
        parameters: json_schema!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["navigate", "click", "type", "scroll", "screenshot", "evaluate", "wait_for"]
                },
                "url": { "type": "string" },
                "selector": { "type": "string" },
                "text": { "type": "string" },
                "direction": { "type": "string", "enum": ["up", "down"] },
                "javascript": { "type": "string" }
            },
            "required": ["action"]
        }),
    }
}
```

**Dependencies:**
```toml
[dependencies]
headless_chrome = "1.0"
```

### 3.2 Screenshot Analysis

```rust
/// Screenshot with metadata
pub struct Screenshot {
    pub image: Vec<u8>, // PNG bytes
    pub url: String,
    pub viewport: Viewport,
    pub timestamp: DateTime<Utc>,
    pub console_errors: Vec<String>,
}

impl Screenshot {
    /// Convert to base64 for LLM vision
    pub fn to_base64(&self) -> String {
        base64::encode(&self.image)
    }
    
    /// Create message for vision model
    pub fn to_vision_message(&self) -> Message {
        Message {
            role: Role::User,
            content: Content::MultiPart(vec![
                Part::Text("Here's a screenshot of the page:".to_string()),
                Part::Image {
                    source: ImageSource::Base64 {
                        media_type: "image/png".to_string(),
                        data: self.to_base64(),
                    },
                },
            ]),
        }
    }
}
```

---

## Phase 4: Checkpoints & History (Weeks 15-16)

### 4.1 Checkpoint System

**File:** `src/checkpoint/mod.rs`

```rust
pub struct Checkpoint {
    pub id: Uuid,
    pub session_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub description: Option<String>,
    pub workspace_snapshot: WorkspaceSnapshot,
    pub message_id: Uuid, // Message at checkpoint
}

pub struct WorkspaceSnapshot {
    pub files: HashMap<PathBuf, FileSnapshot>,
    pub git_status: Option<GitStatus>,
}

pub struct FileSnapshot {
    pub content: String,
    pub hash: Blake3Hash,
}

impl CheckpointManager {
    /// Create checkpoint of current workspace state
    pub async fn create(&self, description: Option<String>) -> Result<Checkpoint>;
    
    /// List all checkpoints for session
    pub fn list(&self, session_id: Uuid) -> Result<Vec<Checkpoint>>;
    
    /// Compare checkpoint with current state
    pub fn compare(&self, checkpoint_id: Uuid) -> Result<Diff>;
    
    /// Restore workspace to checkpoint
    pub async fn restore(&self, checkpoint_id: Uuid, restore_messages: bool) -> Result<()>;
    
    /// Delete checkpoint
    pub fn delete(&self, checkpoint_id: Uuid) -> Result<()>;
}
```

---

## Phase 5: Custom Commands & Modes (Weeks 17-18)

### 5.1 Custom Commands

**File:** `src/commands/custom.rs`

```rust
/// Custom command definition
pub struct CustomCommand {
    pub id: String,
    pub name: String,
    pub description: String,
    pub template: CommandTemplate,
    pub arguments: Vec<CommandArgument>,
}

pub struct CommandArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
}

pub struct CommandTemplate {
    pub steps: Vec<TemplateStep>,
}

pub enum TemplateStep {
    /// Run a bash command
    Run { command: String, args: HashMap<String, String> },
    
    /// Read a file
    Read { path: String },
    
    /// Write to a file
    Write { path: String, content: String },
    
    /// Send message to LLM
    Prompt { message: String },
}

impl CustomCommand {
    /// Load from .clawdius/commands/*.md
    pub fn from_markdown(content: &str) -> Result<Self>;
    
    /// Execute command with provided arguments
    pub async fn execute(&self, args: HashMap<String, String>) -> Result<()>;
}
```

**Example Command File:** `.clawdius/commands/review-pr.md`
```markdown
# Review Pull Request

Review the pull request and provide feedback.

## Arguments
- `PR_NUMBER` (required): The pull request number
- `FOCUS` (optional): Area to focus on (security, performance, style)

## Steps

RUN gh pr view $PR_NUMBER --json title,body,files,additions,deletions
RUN git diff main...HEAD

PROMPT Review this pull request focusing on $FOCUS.
Provide actionable feedback organized by file.
```

### 5.2 Agent Modes

**File:** `src/modes/mod.rs`

```rust
pub enum AgentMode {
    /// Everyday coding, file edits, quick fixes
    Code,
    
    /// System design, migrations, architecture
    Architect,
    
    /// Quick answers, explanations, documentation
    Ask,
    
    /// Debugging, logging, root cause analysis
    Debug,
    
    /// Custom mode with user-defined behavior
    Custom(CustomMode),
}

impl AgentMode {
    /// Get system prompt for mode
    pub fn system_prompt(&self) -> String;
    
    /// Get available tools for mode
    pub fn available_tools(&self) -> Vec<ToolName>;
    
    /// Get response style
    pub fn response_style(&self) -> ResponseStyle;
}

/// Mode configuration in clawdius.toml
/// 
/// [modes.code]
/// system_prompt = "You are an expert programmer..."
/// tools = ["file_read", "file_write", "bash", "search"]
/// auto_approve = false
/// 
/// [modes.architect]
/// system_prompt = "You are a software architect..."
/// tools = ["file_read", "search", "diagram"]
/// auto_approve = true
```

---

## Phase 6: GitHub Integration (Weeks 19-20)

### 6.1 GitHub Action

**File:** `.github/actions/clawdius-review/action.yml`

```yaml
name: 'Clawdius Code Review'
description: 'AI-powered code review using Clawdius'
inputs:
  github-token:
    description: 'GitHub token'
    required: true
  clawdius-api-key:
    description: 'Clawdius API key (if using cloud)'
    required: false
  focus:
    description: 'Review focus areas'
    required: false
    default: 'security,performance,maintainability'
  
runs:
  using: 'docker'
  image: 'docker://clawdius/review:latest'
  env:
    GITHUB_TOKEN: ${{ inputs.github-token }}
    CLAWDIUS_API_KEY: ${{ inputs.clawdius-api-key }}
    REVIEW_FOCUS: ${{ inputs.focus }}
```

**Workflow Example:**
```yaml
# .github/workflows/review.yml
name: Clawdius Review

on:
  pull_request:
    types: [opened, synchronize]

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      
      - name: Clawdius Review
        uses: clawdius/review-action@v1
        with:
          github-token: ${{ secrets.GITHUB_TOKEN }}
          focus: security,performance
```

---

## Phase 7: Additional Features (Weeks 21-24)

### 7.1 Web Search Grounding

```rust
// src/tools/web_search.rs
pub struct WebSearchTool {
    provider: SearchProvider,
}

pub enum SearchProvider {
    Google { api_key: String, cse_id: String },
    Bing { api_key: String },
    DuckDuckGo,
}

impl WebSearchTool {
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
    pub async fn fetch_page(&self, url: &str) -> Result<String>;
}

/// Grounded response
pub struct GroundedResponse {
    pub content: String,
    pub sources: Vec<Source>,
    pub confidence: f32,
}
```

### 7.2 Vim Keybindings (TUI)

```rust
// src/tui/vim.rs
pub enum VimMode {
    Normal,
    Insert,
    Visual,
    Command,
}

pub struct VimKeymap {
    mappings: HashMap<(VimMode, KeyEvent), VimAction>,
}

pub enum VimAction {
    MoveCursor(Direction),
    ChangeMode(VimMode),
    Delete(Motion),
    Yank(Motion),
    Put(Placement),
    Search(String),
    ExecuteCommand(String),
}
```

### 7.3 Localization

```rust
// src/i18n/mod.rs
pub struct Localization {
    current_lang: Language,
    translations: HashMap<Language, Translations>,
}

impl Localization {
    pub fn t(&self, key: &str) -> String;
    pub fn set_language(&mut self, lang: Language);
}

// locales/en.json
// {
//   "chat.placeholder": "Type your message...",
//   "chat.send": "Send",
//   "error.file_not_found": "File not found: {path}",
// }
```

---

## Implementation Priority Matrix

| Feature | User Impact | Effort | Dependencies | Priority |
|---------|-------------|--------|--------------|----------|
| JSON-RPC Protocol | High | Medium | None | P0 |
| Session Persistence | High | Low | SQLite | P0 |
| @Mentions | High | Medium | None | P0 |
| JSON Output | Medium | Low | None | P0 |
| Auto-Compact | High | Medium | Session | P0 |
| VSCode Extension | Critical | High | JSON-RPC | P0 |
| Webview (Rust/WASM) | High | High | VSCode Ext | P1 |
| Browser Automation | High | Medium | headless_chrome | P1 |
| Diff View | High | Medium | VSCode Ext | P1 |
| Checkpoints | Medium | Medium | Session | P1 |
| Custom Commands | Medium | Low | None | P1 |
| Agent Modes | Medium | Low | None | P2 |
| Web Search | Low | Low | API keys | P2 |
| Vim Keybindings | Low | Medium | TUI | P2 |
| Localization | Low | Medium | None | P3 |
| Plugin System | Medium | High | None | P3 |
| GitHub Action | Medium | Low | Docker | P2 |

---

## File Structure After Implementation

```
clawdius/
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── lib.rs
│   │
│   ├── rpc/                    # NEW: JSON-RPC
│   │   ├── mod.rs
│   │   ├── types.rs
│   │   ├── server.rs
│   │   └── transport.rs
│   │
│   ├── session/                 # NEW: Session management
│   │   ├── mod.rs
│   │   ├── persistence.rs
│   │   └── manager.rs
│   │
│   ├── context/                 # NEW: Context management
│   │   ├── mod.rs
│   │   ├── mentions.rs
│   │   ├── compactor.rs
│   │   └── cache.rs
│   │
│   ├── output/                  # NEW: Output formats
│   │   ├── mod.rs
│   │   ├── format.rs
│   │   └── stream.rs
│   │
│   ├── tools/                   # NEW: Browser tool
│   │   ├── mod.rs
│   │   ├── browser.rs
│   │   └── web_search.rs
│   │
│   ├── checkpoint/              # NEW: Checkpoints
│   │   ├── mod.rs
│   │   ├── snapshot.rs
│   │   └── diff.rs
│   │
│   ├── commands/                # NEW: Custom commands
│   │   ├── mod.rs
│   │   ├── custom.rs
│   │   └── templates.rs
│   │
│   ├── modes/                   # NEW: Agent modes
│   │   ├── mod.rs
│   │   ├── code.rs
│   │   ├── architect.rs
│   │   └── debug.rs
│   │
│   ├── i18n/                    # NEW: Localization
│   │   ├── mod.rs
│   │   └── locales/
│   │
│   ├── webview/                 # NEW: WASM webview
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── components/
│   │       └── styles/
│   │
│   └── ... (existing modules)
│
├── editors/
│   └── vscode/                  # NEW: VSCode extension
│       ├── package.json
│       ├── src/
│       │   ├── extension.ts
│       │   ├── rpc/
│       │   ├── commands/
│       │   └── providers/
│       └── webview/
│           └── (built from src/webview)
│
├── .github/
│   └── actions/
│       └── clawdius-review/     # NEW: GitHub Action
│           ├── action.yml
│           └── Dockerfile
│
└── locales/                     # NEW: Translation files
    ├── en.json
    ├── zh.json
    ├── ja.json
    └── ...
```

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Features vs competitors | 60% | 95% |
| VSCode integration | 0% | 100% |
| Session persistence | 0% | 100% |
| User onboarding time | N/A | < 5 min |
| Daily active usage | N/A | Track |

---

## Next Steps

1. **Review this plan** - Confirm priorities and approach
2. **Start Phase 1** - JSON-RPC and session persistence
3. **Set up VSCode extension skeleton**
4. **Begin webview UI with Leptos**

---

*Document Version: 1.0.0*
*Last Updated: March 2026*
