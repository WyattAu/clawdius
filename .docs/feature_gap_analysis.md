# Clawdius Feature Gap Analysis

A comprehensive comparison of Clawdius against leading AI coding agents, identifying missing features and opportunities for improvement.

**Analysis Date:** March 2026

---

## Executive Summary

Clawdius has unique strengths in **security (Sentinel sandboxing)**, **formal verification (Lean4)**, **high-performance (Rust)**, and **domain-specific features (HFT/Financial)**. However, compared to mainstream tools, it lacks several user-facing features that improve daily developer experience.

### Priority Ranking

| Priority | Category | Impact | Effort |
|----------|----------|--------|--------|
| 🔴 P0 | IDE Integration | High | High |
| 🔴 P0 | Browser Automation | High | Medium |
| 🟠 P1 | Context Management | High | Medium |
| 🟠 P1 | Session/History | Medium | Low |
| 🟡 P2 | Output Formats | Medium | Low |
| 🟡 P2 | GitHub Integration | Medium | Medium |
| 🟢 P3 | Web Search/Grounding | Low | Low |
| 🟢 P3 | Team Collaboration | Low | High |

---

## Detailed Feature Comparison

### 1. IDE Integration

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| VSCode Extension | ❌ | ✅ | ✅ | ⚠️ | ✅ |
| JetBrains Plugin | ❌ | ❌ | ❌ | ❌ | ❌ |
| LSP Server Mode | ❌ | ✅ | ❌ | ❌ | ⚠️ |
| Inline Completions | ❌ | ❌ | ✅ | ❌ | ❌ |
| Diff View in Editor | ❌ | ✅ | ✅ | ✅ | ❌ |
| Sidebar Panel | ❌ | ✅ | ✅ | ❌ | ❌ |

**What's Missing:**
- **VSCode Extension** - Most requested feature for daily use
- **LSP Integration** - Real-time diagnostics, completions, hover info
- **Diff View** - Show changes before applying (critical for trust)
- **Sidebar UI** - Persistent panel while coding

**Recommendation:** Build a VSCode extension that communicates with Clawdius via stdio/JSON-RPC.

---

### 2. Browser Automation (Computer Use)

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| Headless Browser | ❌ | ✅ | ❌ | ❌ | ❌ |
| Click/Type/Scroll | ❌ | ✅ | ❌ | ❌ | ❌ |
| Screenshot Capture | ❌ | ✅ | ❌ | ❌ | ❌ |
| Console Log Monitor | ❌ | ✅ | ❌ | ❌ | ❌ |
| Visual Bug Fixing | ❌ | ✅ | ❌ | ❌ | ❌ |
| E2E Testing | ❌ | ✅ | ❌ | ❌ | ❌ |

**What's Missing:**
- **Puppeteer/Playwright Integration** - Control browsers programmatically
- **Screenshot Analysis** - Debug visual issues from screenshots
- **Console Log Monitoring** - React to runtime errors
- **Click/Type Actions** - Interact with web apps
- **Dev Server Integration** - Auto-start dev servers and test

**Recommendation:** Add a browser tool using `headless_chrome` or `fantoccini` crate.

---

### 3. Context Management

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| @url Context | ❌ | ✅ | ❌ | ❌ | ⚠️ |
| @file Context | ❌ | ✅ | ✅ | ✅ | ✅ |
| @folder Context | ❌ | ✅ | ✅ | ✅ | ✅ |
| @problems Context | ❌ | ✅ | ✅ | ❌ | ❌ |
| Auto-Compact | ❌ | ✅ | ✅ | ✅ | ✅ |
| Token Caching | ❌ | ❌ | ❌ | ❌ | ✅ |
| Context Window Display | ❌ | ✅ | ✅ | ✅ | ✅ |

**What's Missing:**
- **@Mentions System** - Quick way to add context (`@file:main.rs`, `@url:docs.rs`)
- **Auto-Compact** - Summarize when approaching context limit
- **Token Caching** - Reuse cached tokens across sessions (Gemini's approach)
- **Context Visualization** - Show current context window usage

**Recommendation:** Implement `@mentions` parser and auto-compact with configurable threshold.

---

### 4. Session & History Management

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| Session Persistence | ❌ | ✅ (SQLite) | ✅ | ✅ | ✅ |
| Session Restore | ❌ | ✅ | ✅ | ✅ | ✅ |
| Conversation History | ❌ | ✅ | ✅ | ✅ | ✅ |
| Checkpoints | ❌ | ✅ | ✅ | ❌ | ✅ |
| Compare/Restore | ❌ | ✅ | ✅ | ❌ | ✅ |
| Multiple Sessions | ❌ | ✅ | ✅ | ✅ | ✅ |

**What's Missing:**
- **SQLite Session Storage** - Persist conversations across restarts
- **Session Switching** - Multiple parallel conversations
- **Checkpoints** - Snapshot workspace state, compare/restore
- **Search History** - Find previous conversations

**Recommendation:** Extend existing SQLite usage to include session storage.

---

### 5. Output & Scripting

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| JSON Output | ❌ | ✅ | ❌ | ❌ | ✅ |
| Stream JSON | ❌ | ❌ | ❌ | ❌ | ✅ |
| Headless Mode | ⚠️ | ✅ | ❌ | ✅ | ✅ |
| Exit Codes | ⚠️ | ✅ | ❌ | ✅ | ✅ |
| Quiet Mode | ❌ | ✅ | ❌ | ❌ | ✅ |
| Progress Indicators | ⚠️ | ✅ | ✅ | ✅ | ✅ |

**What's Missing:**
- **`--output-format json`** - Structured output for scripts
- **Stream JSON Events** - Real-time progress for CI/CD
- **Proper Exit Codes** - Success/failure for automation
- **`--quiet` Mode** - No spinner for piped output

**Recommendation:** Add output format flag with text/json/stream-json options.

---

### 6. GitHub Integration

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| GitHub Actions | ❌ | ❌ | ❌ | ❌ | ✅ |
| PR Reviews | ❌ | ⚠️ | ❌ | ✅ | ✅ |
| Issue Triage | ❌ | ❌ | ❌ | ✅ | ✅ |
| @mentions in GitHub | ❌ | ❌ | ❌ | ✅ | ✅ |
| Sourcegraph Search | ❌ | ✅ | ❌ | ❌ | ❌ |

**What's Missing:**
- **GitHub Action** - Run Clawdius in CI/CD pipelines
- **PR Review Bot** - Automated code review on pull requests
- **GitHub App** - @clawdius mentions in issues/PRs
- **Sourcegraph Integration** - Search across public repos

**Recommendation:** Create a GitHub Action for automated code review.

---

### 7. Modes & Customization

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| Built-in Modes | ❌ | ❌ | ✅ (5) | ⚠️ | ❌ |
| Custom Modes | ❌ | ❌ | ✅ | ✅ | ❌ |
| Custom Commands | ❌ | ✅ | ✅ | ✅ | ✅ |
| Command Arguments | ❌ | ✅ | ❌ | ✅ | ❌ |
| Skills/Plugins | ❌ | ⚠️ | ❌ | ✅ | ❌ |
| Memory Files | ⚠️ | ⚠️ | ✅ | ✅ | ✅ |

**What's Missing:**
- **Agent Modes** - Code, Architect, Ask, Debug modes with different behaviors
- **Custom Commands** - User-defined slash commands with templates
- **Memory Files** - `.clawdius.md` for project-specific context
- **Plugin System** - Extend functionality with plugins

**Recommendation:** Implement mode system with customizable prompts.

---

### 8. External Integrations

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| MCP Servers | ✅ | ✅ | ✅ | ✅ | ✅ |
| Create MCP Tools | ❌ | ✅ | ❌ | ❌ | ❌ |
| Web Fetch | ❌ | ✅ | ❌ | ⚠️ | ✅ |
| Google Search | ❌ | ❌ | ❌ | ❌ | ✅ |
| Slack/Teams | ❌ | ❌ | ❌ | ❌ | ❌ |
| Matrix Bridge | ⚠️ | ❌ | ❌ | ❌ | ❌ |

**What's Missing:**
- **"Add a tool that..."** - Auto-create MCP servers from description
- **Web Fetch Tool** - Fetch and convert URLs to markdown
- **Google/Bing Search** - Ground responses with web search
- **Notification Bridges** - Alert on events (Slack, Discord, Matrix)

**Recommendation:** Add web fetch tool and improve MCP tool creation UX.

---

### 9. UI/UX Features

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| TUI (Terminal UI) | ✅ | ✅ | ❌ | ✅ | ✅ |
| Vim Keybindings | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| External Editor | ❌ | ✅ | ❌ | ❌ | ❌ |
| File Timeline | ❌ | ✅ | ❌ | ❌ | ❌ |
| Help Dialog | ⚠️ | ✅ | ✅ | ✅ | ✅ |
| Keyboard Shortcuts | ⚠️ | ✅ | ✅ | ✅ | ✅ |
| Multi-language UI | ❌ | ❌ | ✅ (18+) | ❌ | ❌ |

**What's Missing:**
- **Vim-like Editor** - Modal editing in TUI
- **External Editor Support** - Open $EDITOR for long prompts
- **File Change Timeline** - Track all changes with rollback
- **Help Overlay** - `Ctrl+?` for keyboard shortcuts
- **Localization** - UI in multiple languages

**Recommendation:** Add vim keybindings and external editor integration.

---

### 10. Enterprise & Team

| Feature | Clawdius | Cline | Roo Code | Claude Code | Gemini CLI |
|---------|:--------:|:-----:|:--------:|:-----------:|:----------:|
| SSO/SAML | ❌ | ✅ | ❌ | ✅ | ⚠️ |
| Audit Logging | ⚠️ | ✅ | ❌ | ✅ | ✅ |
| Team Workspaces | ❌ | ❌ | ❌ | ⚠️ | ⚠️ |
| Policy Controls | ⚠️ | ✅ | ❌ | ✅ | ✅ |
| Private Cloud | ✅ | ⚠️ | ❌ | ⚠️ | ⚠️ |
| Enterprise Docs | ⚠️ | ✅ | ❌ | ✅ | ✅ |

**What's Missing:**
- **SSO Integration** - SAML/OIDC for enterprise auth
- **Team Shared Context** - Shared .clawdius configurations
- **Policy Engine** - Enforce rules across team
- **Enterprise Deployment Guide** - On-prem deployment docs

**Recommendation:** Add SSO support and enterprise deployment documentation.

---

## Unique Clawdius Advantages

These are features Clawdius has that others don't:

| Feature | Description |
|---------|-------------|
| **Sentinel Sandboxing** | 4-tier JIT sandboxing (bubblewrap/sandbox-exec/WASM/gVisor) |
| **Lean4 Verification** | Formal proof generation and verification |
| **Graph-RAG** | AST + Vector hybrid knowledge system |
| **Nexus Lifecycle** | 24-phase formal R&D lifecycle FSM |
| **HFT Broker Mode** | Sub-millisecond trading with Wallet Guard |
| **Multi-lingual Research** | 16 languages with TQA levels |
| **Compliance Generation** | ISO 26262, DO-178C, IEC 62304 matrices |
| **Rust Native** | Zero GC, <20ms boot, native performance |
| **Air-Gap Capable** | Full offline functionality |

---

## Recommended Implementation Roadmap

### Phase 1: Core UX (Weeks 1-4)
1. **Session Persistence** - SQLite storage for conversations
2. **@Mentions System** - `@file`, `@folder`, `@url` context
3. **JSON Output** - `--output-format json` flag
4. **Auto-Compact** - Summarize when context fills

### Phase 2: IDE Integration (Weeks 5-10)
1. **VSCode Extension** - Basic sidebar integration
2. **Diff View** - Show changes before applying
3. **LSP Mode** - Expose diagnostics to IDE

### Phase 3: Browser Automation (Weeks 11-14)
1. **Browser Tool** - Puppeteer-style automation
2. **Screenshot Analysis** - Debug visual issues
3. **Dev Server Integration** - Auto-start/monitor

### Phase 4: GitHub & Team (Weeks 15-18)
1. **GitHub Action** - CI/CD integration
2. **PR Reviews** - Automated code review
3. **Team Shared Config** - `.clawdius/` in repos

### Phase 5: Advanced (Weeks 19-24)
1. **Custom Modes** - Code/Architect/Debug modes
2. **Plugin System** - Extensible architecture
3. **SSO Integration** - Enterprise auth

---

## Feature Parity Checklist

### Must Have (P0)
- [ ] VSCode extension
- [ ] Session persistence
- [ ] JSON output format
- [ ] @mentions context system
- [ ] Auto-compact for context
- [ ] Browser automation tool

### Should Have (P1)
- [ ] Diff view for changes
- [ ] File timeline/history
- [ ] Checkpoints with restore
- [ ] Custom commands system
- [ ] External editor support
- [ ] GitHub Action

### Nice to Have (P2)
- [ ] Multiple agent modes
- [ ] Web search grounding
- [ ] Vim keybindings
- [ ] Localization (i18n)
- [ ] Plugin system
- [ ] Team workspaces

### Future (P3)
- [ ] JetBrains plugin
- [ ] SSO/SAML integration
- [ ] Mobile companion app
- [ ] Cloud sync (optional)
- [ ] Voice input

---

## Competitive Positioning

```
                    High Security
                         │
                    Clawdius ●
                        ╱  │
                       ╱   │
       OpenCode ●    ╱    │
                 ╱  ╱     │
               ╱  ╱       │
             ╱  ╱    Roo  ●
           ╱  ╱      Code
         ╱  ╱
       ╱  ╱  Cline ●
     ╱  ╱
   ╱  ╱        Gemini ●
 ╱  ╱          CLI
●  ╱
Aider
╱
Low Security ──────────────── High Features
```

**Clawdius Position:** High security, medium features. Opportunity to move right (more features) while maintaining security advantage.

---

## Conclusion

Clawdius has a strong foundation with unique security and verification capabilities. To compete with mainstream tools, focus on:

1. **IDE Integration** - This is table stakes for daily use
2. **Session Management** - Users expect persistence
3. **Context Management** - @mentions and auto-compact are expected
4. **Browser Automation** - Key differentiator for web development

The good news: Most missing features are **UX improvements** rather than fundamental architecture changes. The core Rust engine and security model are solid.

---

*Last Updated: March 2026*
