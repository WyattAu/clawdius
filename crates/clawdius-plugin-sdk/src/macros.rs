/// Convenience macro for declaring a [`Plugin`] implementation with a function-pointer tool.
///
/// This macro reduces boilerplate for simple plugins that register one or more
/// function-pointer based tools.
///
/// # Example
///
/// ```ignore
/// use clawdius_plugin_sdk::prelude::*;
///
/// fn greet_handler(invocation: &ToolInvocation) -> ToolResult {
///     ToolResult::ok("Hello!")
/// }
///
/// clawdius_plugin_sdk::declare_plugin!(
///     name = "greet-plugin",
///     version = "0.1.0",
///     tools = [
///         { name = "greet", description = "Says hello", handler = greet_handler },
///     ]
/// );
/// ```
#[macro_export]
macro_rules! declare_plugin {
    (
        name = $name:expr,
        version = $version:expr,
        tools = [ $( { name = $tname:expr, description = $tdesc:expr, handler = $thandler:expr } ),* $(,)? ]
    ) => {
        pub struct DeclaredPlugin;

        impl $crate::plugin::Plugin for DeclaredPlugin {
            fn name(&self) -> &str { $name }
            fn version(&self) -> &str { $version }

            fn init(
                &mut self,
                _ctx: &$crate::context::PluginContext,
            ) -> Result<(), $crate::error::PluginError> {
                Ok(())
            }

            fn register_tools(
                &self,
                registry: &mut $crate::registry::ToolRegistry,
            ) -> Result<(), $crate::error::PluginError> {
                $(
                    registry.register($crate::registry::ToolRegistration {
                        name: $tname.into(),
                        description: $tdesc.into(),
                        handler: $thandler,
                    });
                )*
                Ok(())
            }

            fn shutdown(&mut self) -> Result<(), $crate::error::PluginError> {
                Ok(())
            }
        }
    };
}

/// Convenience macro for creating a [`ToolInvocation`] in tests.
///
/// # Example
///
/// ```ignore
/// let invocation = clawdius_plugin_sdk::invocation!("my_tool", { "key": "value" });
/// ```
#[macro_export]
macro_rules! invocation {
    ($name:expr, { $($key:tt: $val:expr),* $(,)? }) => {
        $crate::registry::ToolInvocation {
            name: $name.into(),
            params: serde_json::json!({ $($key: $val),* }),
            context: $crate::registry::ToolContext {
                session_id: None,
                plugin_name: String::new(),
            },
        }
    };
}
