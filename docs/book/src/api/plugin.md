# Plugin API

Clawdius supports a WASM-based plugin system with 26 hook types.

## Overview

Plugins extend Clawdius's functionality by running custom logic in an isolated WASM runtime. This ensures plugins cannot compromise the host system.

## Plugin Architecture

```
┌─────────────────────────┐
│    Clawdius Host         │
│  ┌───────────────────┐  │
│  │  Plugin Manager   │  │
│  │  ┌─────────────┐  │  │
│  │  │  WASM Runtime│  │  │
│  │  │  (Wasmtime)  │  │  │
│  │  └─────────────┘  │  │
│  └───────────────────┘  │
└─────────────────────────┘
```

## Hook Types

Plugins can register handlers for 26 different lifecycle hooks:

| Category | Hooks |
|----------|-------|
| **Session** | `session_start`, `session_end`, `message_sent`, `message_received` |
| **Tool** | `tool_before_execute`, `tool_after_execute`, `tool_error` |
| **LLM** | `llm_before_request`, `llm_after_response`, `llm_error` |
| **File** | `file_read`, `file_write`, `file_delete` |
| **Checkpoint** | `checkpoint_created`, `checkpoint_restored` |
| **Git** | `git_commit`, `git_push`, `git_checkout` |
| **Sandbox** | `sandbox_enter`, `sandbox_exit`, `sandbox_violation` |
| **Config** | `config_loaded`, `config_changed` |
| **System** | `startup`, `shutdown`, `error` |

## Creating a Plugin

Plugins are WASM modules written in Rust (or any language that compiles to WASM):

```rust
// plugin/src/lib.rs
use clawdius_plugin_api::{register_hook, HookContext, HookResult};

#[register_hook("tool_after_execute")]
fn log_tool_usage(ctx: &HookContext) -> HookResult {
    println!("Tool {} executed in {}ms",
        ctx.params["tool"],
        ctx.params["duration_ms"]);
    HookResult::ok()
}
```

## Plugin Configuration

Plugins are configured in `.clawdius/config.toml` or a separate plugins file:

```toml
[plugins]
load = ["./plugins/my-plugin.wasm"]

[plugins.hooks]
tool_after_execute = ["my-plugin"]
```

## Plugin API Surface

Plugins receive a `HookContext` containing:

| Field | Type | Description |
|-------|------|-------------|
| `hook_name` | `&str` | Name of the triggered hook |
| `params` | `Map<String, Value>` | Hook-specific parameters |
| `session_id` | `Option<&str>` | Active session ID |
| `config` | `&Value` | Current configuration |

Plugins return a `HookResult`:

| Variant | Description |
|---------|-------------|
| `ok()` | Allow the operation to proceed |
| `block(reason)` | Block the operation |
| `modify(data)` | Modify the operation's data |

## Security Model

- Plugins run in WASM with capability-based permissions
- No filesystem access unless explicitly granted
- No network access unless explicitly granted
- Fuel-based execution limits prevent infinite loops
- Memory isolation prevents host memory corruption

## Distribution

Plugins can be distributed as `.wasm` files and loaded from:

- Local filesystem paths
- The plugin marketplace (planned)
- Remote URLs (with integrity verification)
