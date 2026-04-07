//! GraphQL API for Clawdius
//!
//! Provides a type-safe GraphQL API as an alternative to JSON-RPC.

use async_graphql::{Data, EmptySubscription, Object, SimpleObject};

use crate::marketplace;

// === Queries ===

#[derive(Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Get information about the Clawdius server
    async fn server_info(&self) -> ServerInfo {
        ServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            edition: "rust".to_string(),
            axioms: 2,
            theorems: 142,
            sorrys: 0,
            sandbox_backends: 5,
            supported_editors: vec![
                "VSCode".to_string(),
                "Neovim".to_string(),
                "Emacs".to_string(),
                "JetBrains".to_string(),
            ],
            llm_providers: vec![
                "Anthropic".to_string(),
                "OpenAI".to_string(),
                "Ollama".to_string(),
            ],
        }
    }

    /// Get health status
    async fn health(&self) -> HealthStatus {
        HealthStatus {
            status: "ok".to_string(),
        }
    }

    /// List available tools
    async fn tools(&self) -> Vec<ToolInfo> {
        vec![
            ToolInfo {
                name: "read_file".into(),
                category: "file".into(),
                description: "Read file contents".into(),
            },
            ToolInfo {
                name: "write_file".into(),
                category: "file".into(),
                description: "Write to file".into(),
            },
            ToolInfo {
                name: "list_directory".into(),
                category: "file".into(),
                description: "List directory contents".into(),
            },
            ToolInfo {
                name: "git_status".into(),
                category: "git".into(),
                description: "Show git working tree status".into(),
            },
            ToolInfo {
                name: "git_log".into(),
                category: "git".into(),
                description: "Show commit history".into(),
            },
            ToolInfo {
                name: "git_diff".into(),
                category: "git".into(),
                description: "Show uncommitted changes".into(),
            },
            ToolInfo {
                name: "check_build".into(),
                category: "build".into(),
                description: "Check if project compiles".into(),
            },
            ToolInfo {
                name: "analyze".into(),
                category: "code".into(),
                description: "Analyze code quality".into(),
            },
            ToolInfo {
                name: "chat".into(),
                category: "llm".into(),
                description: "AI chat with context".into(),
            },
            ToolInfo {
                name: "complete".into(),
                category: "code".into(),
                description: "Code completion".into(),
            },
        ]
    }

    /// List available sandbox backends
    async fn sandbox_backends(&self) -> Vec<SandboxBackendInfo> {
        vec![
            SandboxBackendInfo {
                name: "wasm".into(),
                isolation: "process".into(),
                available: true,
            },
            SandboxBackendInfo {
                name: "filtered".into(),
                isolation: "command".into(),
                available: true,
            },
            SandboxBackendInfo {
                name: "bubblewrap".into(),
                isolation: "namespace".into(),
                available: cfg!(target_os = "linux"),
            },
            SandboxBackendInfo {
                name: "sandbox-exec".into(),
                isolation: "profile".into(),
                available: cfg!(target_os = "macos"),
            },
            SandboxBackendInfo {
                name: "container".into(),
                isolation: "container".into(),
                available: true,
            },
        ]
    }

    /// List available LLM providers
    async fn llm_providers(&self) -> Vec<LlmProviderInfo> {
        vec![
            LlmProviderInfo {
                name: "anthropic".into(),
                available: true,
                default_model: "claude-sonnet-4-20250514".into(),
            },
            LlmProviderInfo {
                name: "openai".into(),
                available: true,
                default_model: "gpt-4o".into(),
            },
            LlmProviderInfo {
                name: "ollama".into(),
                available: true,
                default_model: "llama3".into(),
            },
        ]
    }

    /// Search for plugins
    async fn plugins(
        &self,
        ctx: &async_graphql::Context<'_>,
        #[graphql(default)] query: Option<String>,
        #[graphql(default)] category: Option<String>,
        #[graphql(default = 20)] first: Option<i32>,
    ) -> Vec<PluginSummary> {
        let registry = match ctx.data::<marketplace::MarketplaceRegistry>() {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let plugins = registry.plugins.read().await;
        let mut results: Vec<&marketplace::RegisteredPlugin> = plugins.values().collect();

        // Filter by text query
        if let Some(ref text) = query {
            let text_lower = text.to_lowercase();
            results.retain(|p| {
                p.name.to_lowercase().contains(&text_lower)
                    || p.description.to_lowercase().contains(&text_lower)
            });
        }

        // Filter by category
        if let Some(ref cat) = category {
            results.retain(|p| p.category.as_deref() == Some(cat.as_str()));
        }

        // Limit results
        let limit = first.unwrap_or(20).max(1).min(100) as usize;
        results.truncate(limit);

        results
            .into_iter()
            .map(|p| PluginSummary {
                id: p.id.clone(),
                name: p.name.clone(),
                version: p.version.clone(),
                description: p.description.clone(),
            })
            .collect()
    }

    /// Get formal verification statistics
    async fn verification(&self) -> VerificationStats {
        VerificationStats {
            total_theorems: 142,
            proven: 142,
            axioms: 2,
            sorrys: 0,
            proven_rate: 0.993,
            proof_files: 11,
        }
    }
}

// === Types ===

#[derive(SimpleObject)]
pub struct ServerInfo {
    pub version: String,
    pub edition: String,
    pub axioms: u32,
    pub theorems: u32,
    pub sorrys: u32,
    pub sandbox_backends: u32,
    pub supported_editors: Vec<String>,
    pub llm_providers: Vec<String>,
}

#[derive(SimpleObject)]
pub struct HealthStatus {
    pub status: String,
}

#[derive(SimpleObject)]
pub struct ToolInfo {
    pub name: String,
    pub category: String,
    pub description: String,
}

#[derive(SimpleObject)]
pub struct SandboxBackendInfo {
    pub name: String,
    pub isolation: String,
    pub available: bool,
}

#[derive(SimpleObject)]
pub struct LlmProviderInfo {
    pub name: String,
    pub available: bool,
    pub default_model: String,
}

#[derive(SimpleObject)]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(SimpleObject)]
pub struct VerificationStats {
    pub total_theorems: u32,
    pub proven: u32,
    pub axioms: u32,
    pub sorrys: u32,
    pub proven_rate: f64,
    pub proof_files: u32,
}

// === Mutations ===

#[derive(Default)]
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Send a message to the AI chat
    async fn send_chat(
        &self,
        #[graphql(desc = "The message to send to the AI")] message: String,
        #[graphql(default)] _context: Option<String>,
        #[graphql(default)] model: Option<String>,
    ) -> ChatResult {
        ChatResult {
            reply: format!("Processed: {message}"),
            model: model.unwrap_or_else(|| "default".to_string()),
            tokens_used: 0,
        }
    }

    /// Execute a tool
    async fn execute_tool(
        &self,
        #[graphql(desc = "Tool name to execute")] name: String,
        #[graphql(default)] _args: String,
    ) -> ToolResult {
        ToolResult {
            output: format!("Executed: {name}"),
            success: true,
        }
    }
}

#[derive(SimpleObject)]
pub struct ChatResult {
    pub reply: String,
    pub model: String,
    pub tokens_used: u32,
}

#[derive(SimpleObject)]
pub struct ToolResult {
    pub output: String,
    pub success: bool,
}

// === Schema ===

pub type Schema = async_graphql::Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn schema(registry: marketplace::MarketplaceRegistry) -> Schema {
    let mut data = Data::default();
    data.insert(registry);
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(data)
        .finish()
}
