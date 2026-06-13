# Clawdius Multi-Interface Architecture Plan

> Unified application strategy: TUI (existing), Tauri desktop app, Leptos web app.
> All three interfaces share clawdius-core as the engine.
>
> Version: 1.0.0 | Date: 2026-06-13

---

## 1. Architecture Overview

### Current State

```
clawdius-core (shared library)
    |
    +-- clawdius (CLI + TUI, ratatui)
    +-- clawdius-gateway (9 messaging adapters + admin REST API)
    +-- clawdius-lsp (Language Server Protocol, tower-lsp)
    +-- clawdius-mcp (Model Context Protocol server)
    +-- clawdius-code (VSCode extension binary)
    +-- clawdius-plugin-sdk (WASM + native plugin SDK)
```

### Target State

```
clawdius-core (shared library)
    |
    +-- clawdius (CLI + TUI, ratatui)              [EXISTING]
    +-- clawdius-gateway (REST API + messaging)     [EXISTING]
    +-- clawdius-lsp (Language Server Protocol)     [EXISTING]
    +-- clawdius-mcp (MCP server)                   [EXISTING]
    +-- clawdius-code (VSCode extension binary)     [EXISTING]
    +-- clawdius-plugin-sdk                         [EXISTING]
    |
    +-- clawdius-tauri (Desktop app, Tauri v2)      [NEW]
    |       Uses clawdius-core natively via Rust FFI
    |       Frontend: Leptos 0.7 WASM
    |       Targets: Linux, macOS, Windows
    |
    +-- clawdius-web (Web app, Leptos 0.7)          [NEW]
            Pure WASM + SSR via Axum
            Shared component library with Tauri frontend
            Hosted at app.clawdius.co.uk
```

### Key Principle: Shared Core, Multiple Shells

All three interfaces (TUI, Tauri, Web) share:
- `clawdius-core` for LLM dispatch, session management, tool execution
- `clawdius-gateway` REST API as the primary IPC mechanism
- `clawdius-lsp` for code intelligence
- `clawdius-mcp` for tool protocol

The Tauri and Web apps are thin presentation layers over the same REST API.

---

## 2. Component Inventory: What Exists in TUI Today

The existing TUI (`crates/clawdius/src/tui_app/`) has 2,445 lines across 17 files:

| Component | File | Lines | Purpose |
|---|---|---|---|
| App state | `app.rs` | 2,445 | Main state machine, event handling, LLM dispatch |
| Chat view | `components/chat.rs` | -- | Message rendering, streaming display |
| Code view | `components/code_view.rs` | -- | Syntax-highlighted code display |
| Command autocomplete | `components/command_autocomplete.rs` | -- | `/command` autocomplete popup |
| Diff view | `components/diff_view.rs` | -- | File diff display with syntax highlighting |
| File list | `components/file_list.rs` | -- | File selection/picker |
| Mention autocomplete | `components/mention_autocomplete.rs` | -- | `@file` mention popup |
| Session picker | `components/session_picker.rs` | -- | Session history and selection |
| Spinner | `components/spinner.rs` | -- | Loading indicator |
| Status bar | `components/status_bar.rs` | -- | Provider, model, mode, token count |
| Syntax highlighter | `components/syntax.rs` | -- | Tree-sitter based highlighting |
| Workspace switcher | `components/workspace_switcher.rs` | -- | Multi-workspace management |
| Theme | `theme.rs` | -- | Color scheme, Spatial Materialism |
| Types | `types.rs` | -- | AppMode, InputMode, LayoutMode, Message, TuiEvent |
| Vim keymap | `vim.rs` | -- | Vim-style navigation |
| UI | `ui.rs` | 1 | (placeholder) |

**These components must be replicated in Leptos for both Tauri and Web.**

---

## 3. Tauri Desktop App

### 3.1 Technology Stack

| Layer | Technology | Rationale |
|---|---|---|
| Shell | Tauri v2 (Rust) | Native performance, small binary (~5-15MB), security-first |
| Frontend | Leptos 0.7 (Rust WASM) | Shared language with backend, SSR-capable |
| Styling | Tailwind CSS 4 | Consistent with landing page design system |
| State | Leptos signals + REST API | Reactive UI, server-driven state |
| IPC | Tauri commands + events | Rust-to-frontend communication |
| Storage | Tauri fs + SQLite | Local data persistence |

### 3.2 Why Tauri (not Electron)

| Metric | Tauri | Electron |
|---|---|---|
| Binary size | 5-15MB | 150-300MB |
| Memory usage | 30-80MB | 200-500MB |
| Boot time | <500ms | 2-5s |
| Language | Rust (type-safe) | JavaScript |
| Security | CSP, scope-limited APIs | nodeIntegration risks |
| Bundle | Static linking, no runtime | Chromium bundled |

### 3.3 Crate Structure

```
crates/clawdius-tauri/
    Cargo.toml
    src/
        main.rs              # Tauri app entry point
        commands/
            mod.rs            # Tauri command registry
            chat.rs           # Chat completion commands
            session.rs        # Session management commands
            file.rs           # File operations commands
            git.rs            # Git operations commands
            config.rs         # Configuration commands
            lsp.rs            # LSP integration commands
        tray.rs               # System tray icon + menu
        menu.rs               # Application menu bar
        window.rs             # Window management
    src-tauri/
        Cargo.toml            # Tauri Rust backend config
        tauri.conf.json       # Tauri app configuration
        icons/                # App icons (PNG, ICO, ICNS)
        capabilities/         # Tauri permission capabilities
    frontend/
        src/
            main.rs           # Leptos app entry
            app.rs            # Root component + router
            pages/
                chat.rs       # Chat interface
                sessions.rs   # Session history
                settings.rs   # Configuration
                diff.rs       # Diff viewer
                about.rs      # About dialog
            components/
                message.rs    # Chat message bubble
                input.rs      # Chat input with autocomplete
                sidebar.rs    # Session/file sidebar
                status_bar.rs # Provider, model, tokens
                code_block.rs # Syntax-highlighted code
                diff_view.rs  # Inline diff display
                file_tree.rs  # Workspace file browser
                tool_result.rs # Tool execution result display
                settings/
                    provider.rs  # LLM provider config
                    sandbox.rs   # Sandbox tier config
                    keybinds.rs  # Keyboard shortcut config
                    theme.rs     # Theme selector
            styles/
                tailwind.css  # Tailwind imports
                custom.css    # Spatial Materialism overrides
        style/
            tailwind.config.js
        index.html            # Shell HTML
```

### 3.4 Tauri IPC Commands

The Tauri app uses Tauri commands (Rust functions callable from JS/WASM):

```rust
// src-tauri/src/commands/chat.rs

#[tauri::command]
async fn send_message(
    message: String,
    session_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<ChatResponse, String> {
    // Use clawdius-core LLM dispatch
    // Stream responses back via Tauri events
}

#[tauri::command]
async fn list_sessions() -> Result<Vec<SessionSummary>, String> { ... }

#[tauri::command]
async fn list_files(workspace: String) -> Result<Vec<FileEntry>, String> { ... }

#[tauri::command]
async fn get_diff(file_path: String) -> Result<DiffResult, String> { ... }

#[tauri::command]
async fn get_config() -> Result<Config, String> { ... }

#[tauri::command]
async fn update_config(config: ConfigUpdate) -> Result<(), String> { ... }
```

### 3.5 Streaming Architecture

```
User types message
    |
    v
Leptos frontend (WASM)
    |
    v  [Tauri command]
Rust backend (clawdius-core)
    |
    v  [LLM provider API]
Token stream starts
    |
    v  [Tauri event: "chat-token"]
Frontend receives tokens
    |
    v  [Leptos signal update]
UI renders incrementally
```

### 3.6 Build Targets

| Platform | Output | Size Est. |
|---|---|---|
| Linux (x86_64) | `.deb`, `.AppImage` | ~12MB |
| macOS (x86_64 + ARM64) | `.dmg`, universal binary | ~15MB |
| Windows (x86_64) | `.msi`, `.exe` | ~10MB |

---

## 4. Leptos Web App

### 4.1 Technology Stack

| Layer | Technology | Rationale |
|---|---|---|
| Framework | Leptos 0.7 | Full-stack Rust WASM, shared components with Tauri |
| Server | Axum | Production-grade async HTTP, already a dependency |
| Rendering | SSR + CSR hybrid | SSR for initial load, CSR for interactivity |
| Styling | Tailwind CSS 4 | Shared design system |
| State | Leptos signals + server functions | Reactive UI with server RPC |

### 4.2 Architecture

The Leptos web app runs as a separate binary (`clawdius-web`) that:
1. Embeds an Axum HTTP server
2. Serves the Leptos WASM frontend
3. Communicates with `clawdius-gateway` via HTTP for all operations
4. Can run standalone or behind a reverse proxy

```
Browser
    |
    v  [HTTP/HTTPS]
clawdius-web (Axum + Leptos SSR)
    |
    v  [HTTP REST API]
clawdius-gateway (or direct clawdius-core)
    |
    v  [LLM provider APIs]
Anthropic, OpenAI, DeepSeek, MiMo, etc.
```

### 4.3 Crate Structure

```
crates/clawdius-web/
    Cargo.toml
    src/
        main.rs               # Axum server + Leptos integration
        app.rs                 # Leptos root component + router
        server/
            mod.rs             # Axum router setup
            auth.rs            # Session-based auth
            sse.rs             # Server-Sent Events for streaming
        pages/
            chat.rs            # Chat interface (shared with Tauri)
            sessions.rs        # Session history
            settings.rs        # Configuration
            login.rs           # Login page
            admin/
                dashboard.rs   # Admin dashboard
                tenants.rs     # Tenant management
                audit.rs       # Audit log viewer
                usage.rs       # Usage analytics
        components/
            (shared with Tauri -- symlink or workspace shared crate)
        error.rs               # Error handling
        state.rs               # Global app state
    style/
        tailwind.css
    public/
        favicon.ico
        robots.txt
```

### 4.4 Deployment Options

| Option | Description | Use Case |
|---|---|---|
| Self-hosted | Binary on VPS | Enterprise customers |
| Docker | Container on K8s | Cloud deployment |
| Static SPA | CDN-hosted WASM | Public demo |
| Behind Caddy/Nginx | Reverse proxy | Production |

---

## 5. Shared Component Library

Both Tauri and Web use the same Leptos components. Extract to a shared crate:

```
crates/clawdius-ui/
    Cargo.toml               # Shared Leptos component library
    src/
        lib.rs               # Public exports
        components/
            chat_input.rs    # Chat input with autocomplete, mentions
            message.rs       # Chat message with syntax highlighting
            code_block.rs    # Syntax-highlighted code blocks
            diff_view.rs     # Inline diff display
            file_tree.rs     # Workspace file browser
            sidebar.rs       # Session/file sidebar
            status_bar.rs    # Provider, model, tokens, mode
            tool_result.rs   # Tool execution result display
            session_list.rs  # Session history list
            settings_form.rs # Settings configuration form
            provider_select.rs # LLM provider selector
            model_select.rs  # Model selector per provider
            sandbox_select.rs # Sandbox tier selector
        layouts/
            main.rs          # Main application layout
            settings.rs      # Settings page layout
            admin.rs         # Admin dashboard layout
        hooks/
            use_chat.rs      # Chat state management hook
            use_stream.rs    # SSE/streaming hook
            use_config.rs    # Configuration hook
        theme/
            mod.rs           # Theme definitions
            spatial.rs       # Spatial Materialism tokens
```

### 5.1 Design System: Spatial Materialism + Amoebic UI + Brutalism

Colors (from landing page `index.html`):

```css
:root {
    --bg-primary: #0a0a0a;
    --bg-secondary: #111111;
    --bg-surface: #1a1a1a;
    --text-primary: #e8e8e8;
    --text-secondary: #888888;
    --accent: #c0ff00;         /* Lime green -- brand color */
    --accent-dim: #7ab800;
    --border: #2a2a2a;
    --error: #ff4444;
    --warning: #ffaa00;
    --success: #00cc66;
}
```

Typography:
- Monospace: `JetBrains Mono` (code, status bar)
- Sans-serif: `Inter` (UI text)
- Display: `Space Grotesk` (headings)

### 5.2 Component Mapping: TUI to Leptos

| TUI Component | Leptos Component | Differences |
|---|---|---|
| `ChatView` | `<ChatView/>` | HTML rendering, markdown, code blocks |
| `DiffView` | `<DiffView/>` | Side-by-side or inline, syntax highlighting |
| `FileList` | `<FileTree/>` | Collapsible tree, icons, drag-drop |
| `SessionPicker` | `<SessionList/>` | Searchable, date grouping |
| `Spinner` | `<Spinner/>` | SVG animation |
| `StatusBar` | `<StatusBar/>` | Icon-based, hover tooltips |
| `SyntaxHighlighter` | `<CodeBlock/>` | Tree-sitter WASM or highlight.js |
| `CommandAutocomplete` | `<ChatInput/>` | Dropdown, `/command` detection |
| `MentionAutocomplete` | `<ChatInput/>` | `@file` detection, file search |
| `WorkspaceSwitcher` | `<FileTree/>` | Tab-based workspace switching |

---

## 6. Implementation Plan

### Phase 1: Shared UI Library (Weeks 1-3)

| Week | Task | Deliverable |
|---|---|---|
| 1 | Create `clawdius-ui` crate scaffold | Cargo.toml + lib.rs + 15 component stubs |
| 1 | Set up Tailwind CSS build pipeline | PostCSS + tailwind.config.js |
| 1 | Implement design tokens | CSS variables, theme module |
| 2 | Build `<ChatInput/>` component | Autocomplete, mentions, paste handling |
| 2 | Build `<Message/>` component | Markdown, code blocks, streaming |
| 2 | Build `<CodeBlock/>` component | Syntax highlighting (tree-sitter WASM) |
| 3 | Build `<DiffView/>` component | Side-by-side, unified, file header |
| 3 | Build `<StatusBar/>` component | Provider, model, tokens, latency |
| 3 | Build `<FileTree/>` component | Collapsible, icons, multi-root |
| 3 | Build `<SessionList/>` component | Search, date grouping, delete |

### Phase 2: Tauri Desktop App (Weeks 4-7)

| Week | Task | Deliverable |
|---|---|---|
| 4 | Create `clawdius-tauri` crate | Cargo.toml, tauri.conf.json, icons |
| 4 | Implement Tauri commands | chat, session, file, git, config |
| 4 | Wire streaming via Tauri events | Token-by-token SSE to frontend |
| 5 | Build main layout with shared components | Sidebar + chat + status bar |
| 5 | Implement session management | Create, resume, delete, search |
| 5 | Implement configuration UI | Provider, model, sandbox, keys |
| 6 | System tray integration | Background running, notifications |
| 6 | Auto-updater (Tauri built-in) | GitHub releases integration |
| 6 | Platform-specific packaging | .deb, .dmg, .msi, .AppImage |
| 7 | Integration testing | Full chat loop, streaming, tools |
| 7 | Performance optimization | Lazy loading, virtual scroll |

### Phase 3: Leptos Web App (Weeks 5-8, overlaps with Phase 2)

| Week | Task | Deliverable |
|---|---|---|
| 5 | Create `clawdius-web` crate | Cargo.toml, Axum server, SSR setup |
| 5 | Implement auth middleware | Session-based, API key, optional SSO |
| 6 | Build public pages | Login, chat, sessions, settings |
| 6 | Implement SSE streaming | Server-Sent Events for chat tokens |
| 7 | Build admin pages | Dashboard, tenants, audit, usage |
| 7 | Implement responsive design | Mobile-friendly layout |
| 8 | Docker packaging | Evergreen-compliant container image |
| 8 | Deploy to 192.168.1.191 | Production test deployment |

### Phase 4: Polish and Ship (Weeks 9-10)

| Week | Task | Deliverable |
|---|---|---|
| 9 | Cross-platform testing | Linux (Wayland/X11), macOS, Windows |
| 9 | Accessibility audit | WCAG 2.1 AA compliance |
| 9 | Keyboard shortcuts | Vim-style navigation (consistent with TUI) |
| 10 | Documentation | User guide, developer guide |
| 10 | CI/CD pipeline | Build all 3 targets in GitHub Actions |

---

## 7. Dependency Analysis

### New Workspace Dependencies

```toml
# Added to root Cargo.toml [workspace.dependencies]

# === Desktop ===
tauri = { version = "2", features = ["tray-icon", "devtools"] }
tauri-build = "2"
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-process = "2"
tauri-plugin-updater = "2"

# === Web ===
leptos = { version = "0.7", features = ["ssr"] }
leptos_axum = "0.7"
leptos_meta = "0.7"
leptos_router = "0.7"

# === WASM ===
wasm-bindgen = "0.2"
web-sys = "0.3"
js-sys = "0.3"
console_log = "1"
```

### Binary Size Budget

| Binary | Target | Budget |
|---|---|---|
| `clawdius` (CLI+TUI) | 25MB | Current: 25MB |
| `clawdius-tauri` (desktop) | 15MB (Tauri) + 3MB (WASM) | ~18MB |
| `clawdius-web` (web server) | 20MB | ~20MB |
| `clawdius-gateway` (server) | 15MB | Current: 15MB |

---

## 8. Updated Workspace Structure

```toml
[workspace]
members = [
    "crates/clawdius",           # CLI + TUI
    "crates/clawdius-core",      # Shared library
    "crates/clawdius-gateway",   # REST API + messaging
    "crates/clawdius-lsp",       # Language Server
    "crates/clawdius-mcp",       # MCP server
    "crates/clawdius-code",      # VSCode extension
    "crates/clawdius-plugin-sdk", # Plugin SDK
    "crates/clawdius-ui",        # Shared Leptos components [NEW]
    "crates/clawdius-tauri",     # Desktop app [NEW]
    "crates/clawdius-web",       # Web app [NEW]
]
```

**Total crates: 10** (7 existing + 3 new)

---

## 9. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Tauri v2 breaking changes | Medium | Medium | Pin Tauri version, follow stable releases |
| Leptos 0.7 SSR complexity | Medium | Medium | Start with CSR-only, add SSR later |
| Tree-sitter WASM size | Low | Low | Lazy-load per-language grammars |
| Cross-platform rendering diffs | Medium | Low | Tailwind normalize, test on all 3 OS |
| WASM binary size | Low | Low | wasm-opt, trunk build optimizations |
| Shared component crate coupling | Medium | Medium | Feature flags for Tauri-only vs Web-only |

---

## 10. Competitive Positioning

After implementation, Clawdius will be the **only** AI coding agent with:

| Interface | Clawdius | Cursor | Aider | Claude Code | Devin |
|---|---|---|---|---|---|
| Terminal TUI | Yes (ratatui) | No | No | Yes | No |
| Desktop app | Yes (Tauri) | Yes (Electron) | No | No | No |
| Web app | Yes (Leptos) | No | No | No | Yes |
| CLI | Yes | No | Yes | Yes | No |
| IDE extension | Yes (VSCode) | Native | No | Yes | No |
| Messaging bots | 9 platforms | No | No | No | No |
| Total interfaces | **6** | **2** | **2** | **3** | **2** |

This makes Clawdius the most versatile AI coding agent by interface count.
