# Plugin SDK Design Document

> Design for v1.1.0 stable Plugin API. Status: PLANNING | Last updated: 2026-05-30

## 1. Current State

No plugin infrastructure exists. Tool dispatch is hardcoded across 3 independent paths:
- Agentic/Sprint engine (`agentic/tool_executor.rs`)
- CLI (`clawdius/src/tool_executor.rs`)
- MCP Server (`mcp/handler.rs`)

All 3 paths enumerate the same ~12 tools via static `match` statements. No central tool registry, no dynamic loading, no plugin ABI.

## 2. Existing Building Blocks

### ToolExecutor Trait (agentic/tool_executor.rs)
```rust
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, request: ToolRequest) -> Result<ToolResult>;
    fn has_tool(&self, name: &str) -> bool;
    fn list_tools(&self) -> Vec<ToolDefinition>;
}
```

### Key Types
- `ToolRequest`: `name: String` + `arguments: HashMap<String, serde_json::Value>`
- `ToolResult`: `success: bool`, `content: String`, `is_error: bool`
- `ToolDefinition`: `name`, `description`, `input_schema` (JSON Schema)

### WASM Sandbox Infrastructure
- `WasiSandbox` (sandbox/wasi.rs): Compiles and runs `.wasm` modules with bounded memory (512MB), fuel (1M ops), timeouts (30s)
- 4-tier sandbox: Direct -> Filtered -> Untrusted (containers) -> Hardened (WASM)
- Host imports: `host.read_file`, `host.write_file`, `host.log` with path validation

### Capability Token System (capability.rs)
- `Permission` enum: FsRead, FsWrite, NetTcp, NetUdp, ExecSpawn, SecretAccess, EnvRead, EnvWrite
- `ResourceScope`: paths, hosts, env_vars
- `CapabilityToken`: SHA3-256 signed, supports `derive()` for permission subsets, expiry
- Currently UNUSED but infrastructure-ready

## 3. Proposed Architecture

### 3a. Central Tool Registry
Replace 3 hardcoded dispatch paths with a single `ToolRegistry`:
- Merges tools from core + all loaded plugins
- Single dispatch point for all tool execution
- Maintains permission mapping per tool

### 3b. Plugin Trait
```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn required_permissions(&self) -> Vec<Permission>;
    async fn init(&mut self, ctx: PluginContext) -> Result<()>;
    fn register_tools(&self) -> Vec<ToolRegistration>;
    async fn shutdown(&mut self) -> Result<()>;
}

struct ToolRegistration {
    name: String,
    description: String,
    input_schema: serde_json::Value,
    handler: Box<dyn ToolHandler>,
}

#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn execute(&self, args: HashMap<String, serde_json::Value>) -> Result<ToolResult>;
}
```

### 3c. WASM Plugin Loading
Leverage existing `WasiSandbox` for third-party plugins:
- Plugins compiled as `.wasm` modules with a standard ABI
- Plugin manifest (`plugin.json`): name, version, tools, permissions, entrypoint
- Host imports for tool registration: `plugin.register_tool`, `plugin.log`
- Loaded into existing 4-tier sandbox based on trust level

### 3d. Plugin Discovery
- System plugins: `~/.config/clawdius/plugins/`
- Project plugins: `.clawdius/plugins/`
- Built-in plugins: compiled into `clawdius-core` binary
- Priority: project > system > built-in (later overrides earlier)

### 3e. SDK Crate (`clawdius-plugin-sdk`)
- Macro helpers: `define_plugin!`, `register_tool!`, `plugin_main!`
- WASM shim generation for browser-compatible plugins
- Error types, logging utilities
- Test harness for plugin development

## 4. Permission Integration

Wire `CapabilityToken` into plugin execution:
1. Plugin declares `required_permissions()` at init time
2. User approves/denies via config or interactive prompt
3. CapabilityToken derived with approved permission subset
4. Plugin execution sandboxed to approved capabilities

## 5. Migration Path

1. Extract `ToolRegistry` from existing `ToolExecutor` implementations
2. Convert hardcoded tools to first-party plugins
3. Unify CLI, agentic, and MCP dispatch through registry
4. Add WASM plugin loading
5. Publish `clawdius-plugin-sdk` crate

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Plugin ABI instability | Medium | High | Semantic versioning; deprecation before breaking changes |
| Malicious plugin escaping sandbox | Low | Critical | WASM sandbox + capability tokens + signed plugins |
| Performance overhead from dynamic dispatch | Medium | Low | Benchmark trait dispatch vs static; optimize hot path |
| Breaking changes to ToolExecutor trait | Low | Medium | New trait alongside old; migration period with deprecation warnings |

## 7. Estimated Effort

| Component | Estimated Time |
|-----------|---------------|
| ToolRegistry extraction | 2 days |
| Plugin trait + SDK crate | 3 days |
| WASM plugin ABI | 4 days |
| Permission integration | 2 days |
| CLI/agentic/MCP migration | 3 days |
| Documentation + examples | 2 days |
| **Total** | **16 days** |
