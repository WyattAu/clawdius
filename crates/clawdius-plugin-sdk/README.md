# clawdius-plugin-sdk

SDK for building Clawdius plugins (WASM or native).

## Overview

This crate provides the types, traits, and utilities needed to author
plugins that extend Clawdius with custom tools. Plugins are discovered
and loaded by the Clawdius host at runtime.

For the full architecture specification, see the
[design document](../../.specs/02_architecture/plugin_sdk_design.md).

## Minimal Plugin Example

```rust,ignore
use clawdius_plugin_sdk::prelude::*;

struct GreetingPlugin;

impl Plugin for GreetingPlugin {
    fn name(&self) -> &str { "greeting" }
    fn version(&self) -> &str { "0.1.0" }

    fn init(&mut self, _ctx: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn register_tools(&self, registry: &mut ToolRegistry) -> Result<(), PluginError> {
        registry.register(ToolRegistration {
            name: "greeting.hello".into(),
            description: "Returns a greeting message".into(),
            handler: hello_handler,
        });
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

fn hello_handler(invocation: &ToolInvocation) -> ToolResult {
    let name = invocation.params.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("world");
    ToolResult::ok(format!("Hello, {name}!"))
}
```

## Key Types

| Type | Description |
|------|-------------|
| `Plugin` | Trait that all plugins must implement |
| `PluginContext` | Filesystem paths provided at init |
| `ToolRegistry` | Central registry for tool dispatch |
| `ToolRegistration` | Describes a tool (name, description, handler) |
| `ToolInvocation` | Data passed to a tool handler at call time |
| `ToolResult` | Result produced by a tool handler |
| `PluginError` | Error type for plugin operations |

## License

Apache-2.0
