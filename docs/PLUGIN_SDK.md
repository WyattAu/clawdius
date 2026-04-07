# Clawdius Plugin SDK

## Table of Contents

- [1. Overview](#1-overview)
- [2. Quick Start](#2-quick-start)
- [3. Plugin Manifest Reference](#3-plugin-manifest-reference)
- [4. Hook Reference](#4-hook-reference)
- [5. Plugin API (Host Functions)](#5-plugin-api-host-functions)
- [6. Example Plugin](#6-example-plugin)
- [7. Example plugin.toml](#7-example-plugintoml)
- [8. Testing Plugins](#8-testing-plugins)
- [9. Plugin Security Model](#9-plugin-security-model)
- [10. Publishing to Marketplace](#10-publishing-to-marketplace)

---

## 1. Overview

The Clawdius plugin system allows you to extend Clawdius with custom behavior by writing plugins compiled to WebAssembly (WASM). Plugins run in an isolated sandbox using [wasmtime](https://wasmtime.dev/) and interact with the host through a well-defined API of imported and exported functions.

### Key Concepts

- **WASM-based isolation** — Plugins are compiled to `wasm32-unknown-unknown` and executed inside a wasmtime runtime. They have no direct access to the host filesystem, network, or process memory.
- **Hook-driven** — Plugins subscribe to named lifecycle hooks (e.g. `before_edit`, `after_llm_response`) and the host dispatches events to subscribed plugins.
- **Capability-based permissions** — Each plugin declares its required capabilities in `plugin.toml`. The host enforces these at runtime.

### Constants

| Constant | Value | Description |
|---|---|---|
| `PLUGIN_API_VERSION` | `"1.0.0"` | Current plugin API version |
| `MAX_PLUGIN_SIZE` | `10 * 1024 * 1024` (10 MB) | Maximum size of a plugin WASM module |
| `MAX_PLUGINS` | `100` | Maximum number of plugins that can be loaded simultaneously |

---

## 2. Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- The `wasm32-unknown-unknown` target:

```bash
rustup target add wasm32-unknown-unknown
```

### Creating a Plugin from Scratch

1. **Create a new Rust project:**

```bash
cargo new my-plugin --lib
cd my-plugin
```

2. **Edit `Cargo.toml`** — disable the default `std` feature and set the crate type:

```toml
[lib]
crate-type = ["cdylib"]

[profile.release]
opt-level = "s"     # optimize for size
lto = true
strip = true
```

3. **Write your plugin** — see [Section 6](#6-example-plugin) for a full template.

4. **Build the WASM module:**

```bash
cargo build --release --target wasm32-unknown-unknown
```

The output will be at `target/wasm32-unknown-unknown/release/my_plugin.wasm`.

5. **Create `plugin.toml`** — see [Section 7](#7-example-plugintoml).

6. **Install the plugin** by copying the `.wasm` file and `plugin.toml` into the Clawdius plugins directory (default: `./plugins/<plugin-name>/`):

```
plugins/
  my-plugin/
    plugin.toml
    plugin.wasm
```

Clawdius auto-loads all plugins from the plugins directory on startup when `auto_load` is `true` (the default).

---

## 3. Plugin Manifest Reference

The manifest file (`plugin.toml`) describes a plugin's metadata, capabilities, hook subscriptions, dependencies, and WASM module location. It is defined by the `PluginManifest` struct in `crates/clawdius-core/src/plugin/manifest.rs`.

### Top-Level Fields

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `id` | `String` | Yes | — | Unique plugin identifier in `name@version` format (e.g. `"my-plugin@1.0.0"`) |
| `name` | `String` | Yes | — | Human-readable plugin name |
| `version` | `String` | Yes | — | Semantic version (e.g. `"1.0.0"`). Must be valid semver. |
| `description` | `String` | Yes | — | Short description of the plugin |
| `author` | `ManifestAuthor` | Yes | — | Author information (see below) |
| `homepage` | `Option<String>` | No | `None` | Plugin homepage URL |
| `repository` | `Option<String>` | No | `None` | Source repository URL |
| `license` | `String` | Yes | — | SPDX license identifier (e.g. `"MIT"`, `"Apache-2.0"`) |
| `keywords` | `Vec<String>` | No | `[]` | Tags for marketplace discovery |
| `min_clawdius_version` | `String` | No | `"0.1.0"` | Minimum Clawdius version required (semver) |
| `capabilities` | `ManifestCapabilities` | No | (see below) | Declared permissions (see below) |
| `hooks` | `Vec<String>` | No | `[]` | Hook names to subscribe to |
| `config` | `Option<toml::Value>` | No | `None` | JSON Schema describing custom configuration |
| `dependencies` | `Vec<ManifestDependency>` | No | `[]` | Other plugins this one depends on |
| `wasm` | `String` | No | `"plugin.wasm"` | Path to the WASM module relative to the manifest |
| `icon` | `Option<String>` | No | `None` | Path to an icon file relative to the manifest |
| `readme` | `String` | No | `"README.md"` | Path to the readme file relative to the manifest |

### `author` Section

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | `String` | Yes | — | Author name |
| `email` | `Option<String>` | No | `None` | Email address |
| `url` | `Option<String>` | No | `None` | Author website URL |

### `capabilities` Section

Each capability is a `bool`. Default values are shown below.

| Field | Type | Default | Description |
|---|---|---|---|
| `read_files` | `bool` | `true` | Can read files from the host |
| `write_files` | `bool` | `false` | Can write files to the host |
| `execute` | `bool` | `false` | Can execute shell commands |
| `network` | `bool` | `false` | Can make network requests |
| `access_llm` | `bool` | `false` | Can access the LLM directly |
| `access_history` | `bool` | `false` | Can read session history |
| `modify_plugins` | `bool` | `false` | Can install, remove, or modify other plugins |

### `hooks` Field

A list of hook name strings the plugin wants to receive. Only hooks listed here will be dispatched to the plugin. Custom hooks use the `"custom:<name>"` prefix.

Example:

```toml
hooks = ["before_edit", "after_edit", "on_startup", "custom:my_hook"]
```

### `dependencies` Field

A list of dependency declarations. Each entry has:

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | `String` | Yes | — | Dependency plugin name |
| `version` | `String` | Yes | — | Semver version requirement (e.g. `">=1.0.0"`) |
| `optional` | `bool` | No | `false` | Whether this dependency is optional |

Example:

```toml
[[dependencies]]
name = "utils-plugin"
version = ">=1.0.0"
optional = false
```

### Validation

The manifest is validated on load. The following checks are performed:

- `id` must contain an `@` separator (format: `name@version`)
- `version` must be valid semver
- `min_clawdius_version` must be valid semver
- Each entry in `hooks` must be a known hook name or start with `"custom:"`
- Each dependency `version` must be a valid semver range

Validation errors are returned as `ManifestValidationError` variants: `InvalidIdFormat`, `InvalidVersion`, `InvalidHook`, `InvalidDependencyVersion`.

---

## 4. Hook Reference

Hooks are event points where plugins can inject custom behavior. Each hook is identified by a `HookType` enum variant. When the host fires a hook, it creates a `HookContext` and dispatches it to all plugins that subscribe to that hook.

### HookContext

Every hook receives a `HookContext` with the following fields:

| Field | Type | Description |
|---|---|---|
| `hook_type` | `HookType` | Which hook was triggered |
| `session_id` | `Option<String>` | Session ID, if applicable |
| `timestamp` | `chrono::DateTime<chrono::Utc>` | When the hook fired |
| `data` | `HashMap<String, serde_json::Value>` | Hook-specific data payload |
| `cancellable` | `bool` | Whether this event can be cancelled |
| `cancelled` | `bool` | Whether the event has been cancelled by a plugin |

### HookResult

Plugins return a `HookResult` from hooks:

| Method | Description |
|---|---|
| `HookResult::success()` | Hook executed successfully, continue propagation |
| `HookResult::success_with_data(data)` | Success with optional JSON data |
| `HookResult::error(msg)` | Hook failed with an error message |
| `HookResult::stop()` | Success, but stop propagation to remaining plugins |

### All Hook Types (26 total)

#### Lifecycle Hooks

| Hook Name | `HookType` | When It Fires | Context Data | Cancellable |
|---|---|---|---|---|
| `on_startup` | `OnStartup` | Clawdius starts | — | No |
| `on_shutdown` | `OnShutdown` | Clawdius shuts down | — | No |
| `on_session_create` | `OnSessionCreate` | A new session is created | `session_id` | No |
| `on_session_destroy` | `OnSessionDestroy` | A session is destroyed | `session_id` | No |
| `on_session_activate` | `OnSessionActivate` | A session becomes active | `session_id` | No |

#### LLM Hooks

| Hook Name | `HookType` | When It Fires | Context Data | Cancellable |
|---|---|---|---|---|
| `before_llm_request` | `BeforeLLMRequest` | Before sending a prompt to the LLM | `prompt`, `model` | Yes |
| `after_llm_response` | `AfterLLMResponse` | After receiving an LLM response | `response`, `model`, `usage` | No |
| `on_stream_token` | `OnStreamToken` | When streaming a token from the LLM | `token` | No |

#### Tool Hooks

| Hook Name | `HookType` | When It Fires | Context Data | Cancellable |
|---|---|---|---|---|
| `before_tool_execute` | `BeforeToolExecute` | Before executing a tool | `tool_name`, `tool_args` | Yes |
| `after_tool_execute` | `AfterToolExecute` | After a tool completes | `tool_name`, `result` | No |
| `on_tool_register` | `OnToolRegister` | When registering custom tools | — | No |

#### File Hooks

| Hook Name | `HookType` | When It Fires | Context Data | Cancellable |
|---|---|---|---|---|
| `before_file_read` | `BeforeFileRead` | Before reading a file | `path` | Yes |
| `after_file_read` | `AfterFileRead` | After reading a file | `path`, `content` | No |
| `before_file_write` | `BeforeFileWrite` | Before writing a file | `path`, `content` | Yes |
| `after_file_write` | `AfterFileWrite` | After writing a file | `path` | No |
| `before_file_delete` | `BeforeFileDelete` | Before deleting a file | `path` | Yes |

#### Code Hooks

| Hook Name | `HookType` | When It Fires | Context Data | Cancellable |
|---|---|---|---|---|
| `before_edit` | `BeforeEdit` | Before applying a code edit | `file`, `old_text`, `new_text` | Yes |
| `after_edit` | `AfterEdit` | After applying a code edit | `file`, `success` | No |
| `before_analysis` | `BeforeAnalysis` | Before running code analysis | `file`, `analyzer` | No |
| `after_analysis` | `AfterAnalysis` | After code analysis completes | `file`, `results` | No |

#### Command Hooks

| Hook Name | `HookType` | When It Fires | Context Data | Cancellable |
|---|---|---|---|---|
| `before_command` | `BeforeCommand` | Before executing a shell command | `command`, `cwd` | Yes |
| `after_command` | `AfterCommand` | After a shell command completes | `command`, `exit_code`, `stdout`, `stderr` | No |

#### Event Hooks

| Hook Name | `HookType` | When It Fires | Context Data | Cancellable |
|---|---|---|---|---|
| `on_event` | `OnEvent` | On any event | `event_type`, `event_data` | No |
| `on_error` | `OnError` | On an error event | `error` | No |
| `on_warning` | `OnWarning` | On a warning event | `warning` | No |

#### Custom Hooks

| Hook Name | `HookType` | When It Fires | Context Data | Cancellable |
|---|---|---|---|---|
| `custom` | `Custom` | Plugin-defined custom hook | Custom | Depends on hook |

Custom hooks use the `"custom:<name>"` syntax in the manifest. They allow plugins to define and fire their own events that other plugins can subscribe to.

### Hook Subscription Options

When subscribing to hooks (via the Rust `HookSubscription` API), the following options are available:

| Field | Type | Default | Description |
|---|---|---|---|
| `hook_type` | `HookType` | — | The hook to subscribe to |
| `priority` | `i32` | `0` | Execution priority (higher = runs first) |
| `async_execution` | `bool` | `false` | Whether to run the hook asynchronously |
| `filter` | `Option<String>` | `None` | Optional filter expression to limit when the hook fires |

---

## 5. Plugin API (Host Functions)

Plugins call host functions by importing them from the `"clawdius"` WASM module. These are made available to the WASM instance via the wasmtime linker.

### `clawdius.log(level, ptr, len)`

Write a log message from the plugin.

| Parameter | Type | Description |
|---|---|---|
| `level` | `i32` | Log level: `0` = trace, `1` = debug, `2` = info, `3` = warn, `4+` = error |
| `ptr` | `i32` | Pointer to the message string in WASM linear memory |
| `len` | `i32` | Length of the message string in bytes |

**Returns:** `()` (void)

```rust
// In your plugin (Rust pseudocode)
extern "C" {
    fn clawdius_log(level: i32, ptr: *const u8, len: i32);
}
```

### `clawdius.get_config(key_ptr, key_len, val_ptr, val_len)`

Retrieve a configuration value for the plugin.

| Parameter | Type | Description |
|---|---|---|
| `key_ptr` | `i32` | Pointer to the config key string |
| `key_len` | `i32` | Length of the key string |
| `val_ptr` | `i32` | Pointer where the value will be written |
| `val_len` | `i32` | Maximum length of the value buffer |

**Returns:** `i32` — `0` on success, non-zero on failure.

> **Note:** Full config retrieval is planned for v1.7.0. Currently returns `0` unconditionally.

### `clawdius.hook_result_success()`

Signal that a hook executed successfully.

**Returns:** `i32` — `0`

### `clawdius.hook_result_error(ptr, len)`

Signal that a hook failed with an error message.

| Parameter | Type | Description |
|---|---|---|
| `ptr` | `i32` | Pointer to the error message string |
| `len` | `i32` | Length of the error message |

**Returns:** `i32` — `0`

---

## 6. Example Plugin

A minimal Clawdius plugin in Rust. Save as `src/lib.rs`:

```rust
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

#[no_mangle]
pub extern "C" fn clawdius_init() -> u32 {
    0 // success
}

#[no_mangle]
pub extern "C" fn hook_before_llm_request() -> u32 {
    0 // success, allow the request
}

#[no_mangle]
pub extern "C" fn hook_after_llm_response() -> u32 {
    0 // success
}

#[no_mangle]
pub extern "C" fn clawdius_shutdown() -> u32 {
    0 // success
}
```

### Plugin Entry Points

The WASM runtime looks for the following exported functions:

| Export | Called When | Return |
|---|---|---|
| `_initialize` | WASM reactor pattern initialization | void |
| `clawdius_init` | Plugin initialization (after WASM instantiation) | `i32` (`0` = success) |
| `hook_<name>` | When the named hook fires (e.g. `hook_before_edit`) | `i32` (`0` = success) |
| `clawdius_shutdown` | Plugin shutdown | `i32` (`0` = success) |
| `clawdius_config_changed` | When plugin configuration is updated | `i32` (`0` = success) |

Hook function names are derived from the hook name by replacing hyphens with underscores and prefixing with `hook_`. For example, the `before-edit` hook maps to the exported function `hook_before_edit`.

All exports are optional — if a plugin does not export a given function, the host skips it gracefully.

---

## 7. Example plugin.toml

```toml
id = "my-plugin@0.1.0"
name = "my-plugin"
version = "0.1.0"
description = "A sample Clawdius plugin"
license = "MIT"

[author]
name = "Your Name"
email = "you@example.com"

[capabilities]
read_files = true
write_files = false
execute = false
network = false
access_llm = false
access_history = false
modify_plugins = false

hooks = ["before_llm_request", "after_llm_response"]

wasm = "plugin.wasm"
```

---

## 8. Testing Plugins

### Local Testing with PluginHost

You can use the `PluginHost` API to test plugins programmatically in Rust:

```rust
use clawdius_core::plugin::{
    PluginHost, PluginHostBuilder, PluginHostConfig,
    HookType, HookContext,
};

#[tokio::test]
async fn test_my_plugin() {
    let host = PluginHostBuilder::new()
        .plugins_dir("./test-fixtures/plugins")
        .auto_load(true)
        .build();

    host.initialize().await.unwrap();

    let ctx = HookContext::new(HookType::BeforeLLMRequest);
    let results = host.dispatch_hook(HookType::BeforeLLMRequest, &ctx).await;

    for (plugin_id, result) in results {
        assert!(result.success, "Plugin {plugin_id} hook failed: {:?}", result.error);
    }

    host.shutdown().await.unwrap();
}
```

### WASM Testing Strategies

1. **Unit test the plugin logic in native Rust** — compile with `#[cfg(not(target_arch = "wasm32"))]` guards for tests that use `std`.
2. **Validate the WASM output** — use `wasmtime` to load and instantiate the module, then call exports directly.
3. **Use `PluginLoader` for manifest validation:**

```rust
use clawdius_core::plugin::PluginLoader;

let loader = PluginLoader::new();
let result = loader.validate_plugin_dir(std::path::Path::new("./my-plugin"));
assert!(result.valid, "Validation errors: {:?}", result.errors);
```

4. **Test with `wasm-validate`** (command line) to ensure the WASM binary is well-formed:

```bash
wasm-validate target/wasm32-unknown-unknown/release/my_plugin.wasm
```

---

## 9. Plugin Security Model

### WASM Sandboxing

Plugins execute inside a [wasmtime](https://wasmtime.dev/) sandbox with the following constraints:

- **No filesystem access** — the WASM module cannot read or write host files unless the host explicitly provides imported functions.
- **No network access** — plugins cannot open sockets or make HTTP requests directly.
- **No process spawning** — plugins cannot execute host commands.
- **Memory isolation** — each plugin runs in its own linear memory. It cannot access the host's memory or other plugins' memory.

The wasmtime engine is configured with:
- WASM backtraces enabled
- Multi-memory support enabled
- SIMD support enabled
- Cranelift optimizer at `Speed` level

### Capability-Based Permissions

Capabilities are declared in `plugin.toml` and enforced at runtime by the host. The 7 capability flags are:

| Capability | Default | What It Allows |
|---|---|---|
| `read_files` | `true` | Reading files from the host filesystem |
| `write_files` | `false` | Writing files to the host filesystem |
| `execute` | `false` | Executing shell commands on the host |
| `network` | `false` | Making network requests from the host |
| `access_llm` | `false` | Direct access to the LLM API |
| `access_history` | `false` | Reading session history |
| `modify_plugins` | `false` | Installing, removing, or configuring other plugins |

Convenience constructors exist for common capability sets:

- `PluginCapabilities::default()` — read-only (only `read_files = true`)
- `PluginCapabilities::read_only()` — same as default
- `PluginCapabilities::unrestricted()` — all capabilities enabled

### Size and Count Limits

| Limit | Value | Enforced By |
|---|---|---|
| Max plugin WASM size | 10 MB (`MAX_PLUGIN_SIZE`) | `WasmPlugin::load()` |
| Max total plugin directory size | 10 MB (`MAX_PLUGIN_SIZE`) | `PluginLoader::validate_plugin_dir()` |
| Max loaded plugins | 100 (`MAX_PLUGINS`) | `PluginHostConfig` |

Plugins exceeding the size limit are rejected at load time. The host refuses to load more than `MAX_PLUGINS` simultaneously.

---

## 10. Publishing to Marketplace

### Marketplace API

The Clawdius Marketplace API is at `https://marketplace.clawdius.dev/api/v1` by default. It is accessed via `MarketplaceClient`.

### Client Methods

| Method | Endpoint | Description |
|---|---|---|
| `search(query)` | `GET /plugins/search` | Search for plugins with filters, sorting, and pagination |
| `get_plugin(id)` | `GET /plugins/{id}` | Get details for a specific plugin |
| `install(request)` | `POST /plugins/install` | Install a plugin by name and optional version |
| `check_updates(plugins)` | `POST /plugins/check-updates` | Check for updates to installed plugins |
| `get_featured()` | `GET /plugins/featured` | Get featured plugins |
| `get_categories()` | `GET /categories` | List plugin categories |
| `submit_plugin(manifest, wasm)` | `POST /plugins/submit` | Submit a new plugin to the marketplace |

### Search Options

Search queries support:

| Field | Type | Default | Description |
|---|---|---|---|
| `query` | `String` | `""` | Free-text search term |
| `category` | `Option<String>` | `None` | Filter by category |
| `author` | `Option<String>` | `None` | Filter by author |
| `tag` | `Option<String>` | `None` | Filter by tag |
| `sort` | `MarketplaceSort` | `Relevance` | Sort field: `relevance`, `downloads`, `stars`, `updated`, `name` |
| `order` | `MarketplaceOrder` | `Desc` | Sort order: `desc` or `asc` |
| `page` | `u32` | `1` | Page number (1-indexed) |
| `per_page` | `u32` | `20` | Results per page |
| `include_prereleases` | `bool` | `false` | Include pre-release versions |

### CLI Commands

```
# Search the marketplace
clawdius plugin search <query>

# Install a plugin
clawdius plugin install <name> [--version <ver>]

# List installed plugins
clawdius plugin list

# Uninstall a plugin
clawdius plugin uninstall <name>

# Check for updates
clawdius plugin update --check

# Submit a plugin
clawdius plugin submit <path/to/plugin/dir>
```

### Submitting a Plugin

To submit a plugin to the marketplace:

1. Ensure your `plugin.toml` is valid (use `PluginLoader::validate_plugin_dir`).
2. Build the WASM module (`cargo build --release --target wasm32-unknown-unknown`).
3. Use `MarketplaceClient::submit_plugin(manifest, wasm_bytes)` with your API key.

The submit endpoint accepts the manifest as TOML and the WASM module as a base64-encoded payload.

### Plugin Signing (Future)

Plugin signature verification infrastructure exists in `MarketplaceConfig`:

| Field | Type | Default | Description |
|---|---|---|---|
| `verify_signatures` | `bool` | `true` | Whether to verify plugin signatures |
| `trusted_keys` | `Vec<String>` | `[]` | Trusted public keys for verification |

Each `MarketplaceVersion` includes an optional `signature` field. Full signing support is planned for a future release.

### Marketplace Cache

Search results are cached locally using `MarketplaceCache` with a configurable TTL (`cache_duration_secs`, default 1 hour). Expired entries are cleaned automatically.
