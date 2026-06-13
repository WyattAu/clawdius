//! Integration tests for clawdius-plugin-sdk.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;

use clawdius_plugin_sdk::echo_tool::{DummyTool, EchoTool};
use clawdius_plugin_sdk::error::PluginError;
use clawdius_plugin_sdk::prelude::*;
use clawdius_plugin_sdk::registry::{PluginToolContext, ToolInvocation, ToolResult};
use clawdius_plugin_sdk::simple::SimplePlugin;
use clawdius_plugin_sdk::tool::Tool;
use serde_json::json;

// ── EchoTool ──────────────────────────────────────────────────────────

#[test]
fn echo_tool_name() {
    let tool = EchoTool::new();
    assert_eq!(tool.name(), "echo");
}

#[test]
fn echo_tool_description_is_nonempty() {
    let tool = EchoTool::new();
    assert!(!tool.description().is_empty());
}

#[test]
fn echo_tool_schema_has_required_message() {
    let tool = EchoTool::new();
    let schema = tool.schema();
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("schema should have required array");
    assert!(required.contains(&json!("message")));
}

#[test]
fn echo_tool_validate_args_valid() {
    let tool = EchoTool::new();
    let args = json!({ "message": "hello" });
    assert!(tool.validate_args(&args).is_ok());
}

#[test]
fn echo_tool_validate_args_missing_message() {
    let tool = EchoTool::new();
    let args = json!({ "foo": "bar" });
    let err = tool.validate_args(&args).expect_err("should fail");
    assert!(matches!(err, PluginError::ValidationError { .. }));
}

#[test]
fn echo_tool_validate_args_empty_object() {
    let tool = EchoTool::new();
    let args = json!({});
    assert!(tool.validate_args(&args).is_err());
}

#[test]
fn echo_tool_execute_success() {
    let tool = EchoTool::new();
    let invocation = ToolInvocation {
        name: "echo".into(),
        params: json!({ "message": "hi" }),
        context: PluginToolContext {
            session_id: None,
            plugin_name: "test".into(),
        },
    };
    let result = tool.execute(&invocation);
    assert!(!result.is_error);
    assert_eq!(result.content, vec![ToolContent::Text("hi".into())]);
}

#[test]
fn echo_tool_execute_with_prefix() {
    let tool = EchoTool::with_prefix("bot");
    let invocation = ToolInvocation {
        name: "echo".into(),
        params: json!({ "message": "hello" }),
        context: PluginToolContext {
            session_id: None,
            plugin_name: "test".into(),
        },
    };
    let result = tool.execute(&invocation);
    assert!(!result.is_error);
    assert_eq!(result.content, vec![ToolContent::Text("bot: hello".into())]);
}

// ── DummyTool ──────────────────────────────────────────────────────────

#[test]
fn dummy_tool_returns_fixed_response() {
    let tool = DummyTool::new("dummy", "ok");
    let invocation = ToolInvocation {
        name: "dummy".into(),
        params: json!({}),
        context: PluginToolContext {
            session_id: None,
            plugin_name: "test".into(),
        },
    };
    let result = tool.execute(&invocation);
    assert!(!result.is_error);
    assert_eq!(result.content, vec![ToolContent::Text("ok".into())]);
}

#[test]
fn dummy_tool_validate_always_ok() {
    let tool = DummyTool::new("dummy", "ok");
    assert!(tool.validate_args(&json!({})).is_ok());
    assert!(tool.validate_args(&json!("anything")).is_ok());
}

// ── ToolResult ─────────────────────────────────────────────────────────

#[test]
fn tool_result_ok_and_err() {
    let ok = ToolResult::ok("good");
    assert!(!ok.is_error);

    let err = ToolResult::err("bad");
    assert!(err.is_error);
    assert_eq!(err.content, vec![ToolContent::Text("bad".into())]);
}

// ── ToolRegistry ───────────────────────────────────────────────────────

#[test]
fn registry_register_trait_tool() {
    let mut registry = ToolRegistry::new();
    let tool: Arc<dyn Tool> = Arc::new(EchoTool::new());
    assert!(registry.register_tool(tool).is_ok());
    assert_eq!(registry.len(), 1);
    assert!(registry.contains("echo"));
}

#[test]
fn registry_register_trait_tool_duplicate_detection() {
    let mut registry = ToolRegistry::new();
    let tool: Arc<dyn Tool> = Arc::new(EchoTool::new());
    assert!(registry.register_tool(Arc::clone(&tool)).is_ok());
    let err = registry
        .register_tool(tool)
        .expect_err("should detect duplicate");
    assert!(matches!(err, PluginError::DuplicateTool(_)));
}

#[test]
fn registry_unregister_trait_tool() {
    let mut registry = ToolRegistry::new();
    let tool: Arc<dyn Tool> = Arc::new(EchoTool::new());
    registry.register_tool(tool).unwrap();
    let removed = registry.unregister_tool("echo");
    assert!(removed);
    assert!(registry.is_empty());
}

#[test]
fn registry_get_trait_tool() {
    let mut registry = ToolRegistry::new();
    let tool: Arc<dyn Tool> = Arc::new(EchoTool::new());
    registry.register_tool(tool).unwrap();
    let fetched = registry.get_trait_tool("echo").expect("should find echo");
    assert_eq!(fetched.name(), "echo");
}

#[test]
fn registry_invoke_trait_tool() {
    let mut registry = ToolRegistry::new();
    let tool: Arc<dyn Tool> = Arc::new(EchoTool::new());
    registry.register_tool(tool).unwrap();
    let result = registry
        .invoke(
            "echo",
            json!({ "message": "test" }),
            PluginToolContext {
                session_id: Some("sess1".into()),
                plugin_name: "test".into(),
            },
        )
        .unwrap();
    assert!(!result.is_error);
}

#[test]
fn registry_invoke_missing_tool() {
    let registry = ToolRegistry::new();
    let err = registry
        .invoke("nope", json!({}), PluginToolContext::default())
        .expect_err("should fail");
    assert!(matches!(err, PluginError::ToolNotFound(_)));
}

// ── PluginContext ──────────────────────────────────────────────────────

#[test]
fn context_builder_defaults() {
    let ctx = PluginContext::new(PathBuf::from("/tmp/plugins/my-plugin"));
    assert_eq!(ctx.plugin_dir, PathBuf::from("/tmp/plugins/my-plugin"));
    assert!(ctx.workspace_path().is_none());
    assert_eq!(*ctx.config(), json!({}));
}

#[test]
fn context_builder_with_all_fields() {
    let ctx = PluginContext::builder(PathBuf::from("/tmp/p"))
        .workspace_path(PathBuf::from("/home/user/project"))
        .config(json!({ "key": "value" }))
        .metadata("session_id", "abc123")
        .build();

    assert_eq!(
        ctx.workspace_path(),
        Some(std::path::Path::new("/home/user/project"))
    );
    assert_eq!(
        ctx.config().get("key").and_then(|v| v.as_str()),
        Some("value")
    );
    assert_eq!(ctx.get_metadata("session_id"), Some("abc123"));
}

#[test]
fn context_metadata_set_get() {
    let mut ctx = PluginContext::new(PathBuf::from("/tmp/p"));
    ctx.set_metadata("foo", "bar");
    assert_eq!(ctx.get_metadata("foo"), Some("bar"));
    assert_eq!(ctx.get_metadata("missing"), None);
}

// ── SimplePlugin ───────────────────────────────────────────────────────

#[test]
fn simple_plugin_name_and_version() {
    let plugin = SimplePlugin::new("test-plugin", "1.0.0");
    assert_eq!(plugin.name(), "test-plugin");
    assert_eq!(plugin.version(), "1.0.0");
}

#[test]
fn simple_plugin_init_and_shutdown_noop() {
    let mut plugin = SimplePlugin::new("p", "0.1.0");
    let ctx = PluginContext::new(PathBuf::from("/tmp/p"));
    assert!(plugin.init(&ctx).is_ok());
    assert!(plugin.shutdown().is_ok());
}

#[test]
fn simple_plugin_register_tools() {
    let plugin = SimplePlugin::new("echo-plugin", "0.1.0")
        .with_tool(Arc::new(EchoTool::new()))
        .with_tool(Arc::new(DummyTool::new("dummy", "yes")));

    assert_eq!(plugin.tool_count(), 2);
    assert_eq!(plugin.tool_names().len(), 2);

    let mut registry = ToolRegistry::new();
    plugin.register_tools(&mut registry).unwrap();
    assert_eq!(registry.len(), 2);
    assert!(registry.contains("echo"));
    assert!(registry.contains("dummy"));
}

#[test]
fn simple_plugin_duplicate_tool_detection() {
    let plugin = SimplePlugin::new("dup-plugin", "0.1.0")
        .with_tool(Arc::new(EchoTool::new()))
        .with_tool(Arc::new(EchoTool::new()));

    let mut registry = ToolRegistry::new();
    let err = plugin
        .register_tools(&mut registry)
        .expect_err("should detect duplicate");
    assert!(matches!(err, PluginError::RegistrationFailed(_)));
    assert!(err.to_string().contains("duplicate tool"));
}

#[test]
fn simple_plugin_custom_init_callback() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc as StdArc;

    let called = StdArc::new(AtomicBool::new(false));
    let called_clone = StdArc::clone(&called);

    let mut plugin = SimplePlugin::new("p", "0.1.0").on_init(move |_ctx| {
        called_clone.store(true, Ordering::SeqCst);
        Ok(())
    });

    let ctx = PluginContext::new(PathBuf::from("/tmp/p"));
    plugin.init(&ctx).unwrap();
    assert!(called.load(Ordering::SeqCst));
}

// ── PluginError ────────────────────────────────────────────────────────

#[test]
fn plugin_error_display_roundtrip() {
    let err = PluginError::init_failed("oops");
    assert!(err.to_string().contains("oops"));

    let err = PluginError::duplicate_tool("my.tool");
    assert!(err.to_string().contains("my.tool"));

    let err = PluginError::validation_error("foo", "bad arg");
    assert!(err.to_string().contains("foo"));
    assert!(err.to_string().contains("bad arg"));
}

#[test]
fn plugin_error_constructors() {
    let _ = PluginError::init_failed("init");
    let _ = PluginError::registration_failed("reg");
    let _ = PluginError::execution_failed("exec");
    let _ = PluginError::shutdown_failed("shut");
    let _ = PluginError::duplicate_tool("dup");
    let _ = PluginError::tool_not_found("missing");
    let _ = PluginError::validation_error("t", "r");
}

// ── declare_plugin macro ──────────────────────────────────────────────

#[test]
fn declare_plugin_macro_basic() {
    fn handler(inv: &ToolInvocation) -> ToolResult {
        ToolResult::ok(format!("hi from {}", inv.name))
    }

    clawdius_plugin_sdk::declare_plugin!(
        name = "macro-test",
        version = "0.1.0",
        tools = [
            { name = "greet", description = "Says hi", handler = handler },
        ]
    );

    let mut plugin = DeclaredPlugin;
    let ctx = PluginContext::new(PathBuf::from("/tmp/p"));
    assert!(plugin.init(&ctx).is_ok());
    assert_eq!(plugin.name(), "macro-test");
    assert_eq!(plugin.version(), "0.1.0");

    let mut registry = ToolRegistry::new();
    plugin.register_tools(&mut registry).unwrap();
    assert!(registry.contains("greet"));

    let result = registry
        .invoke(
            "greet",
            json!({}),
            PluginToolContext {
                session_id: None,
                plugin_name: "macro-test".into(),
            },
        )
        .unwrap();
    assert!(!result.is_error);
}

// ── invocation macro ─────────────────────────────────────────────────

#[test]
fn invocation_macro_works() {
    let inv = clawdius_plugin_sdk::invocation!("test_tool", { "key": "value", "num": 42 });
    assert_eq!(inv.name, "test_tool");
    assert_eq!(
        inv.params.get("key").and_then(serde_json::Value::as_str),
        Some("value")
    );
    assert_eq!(
        inv.params.get("num").and_then(serde_json::Value::as_i64),
        Some(42)
    );
}
