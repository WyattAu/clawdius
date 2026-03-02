//! Brain WASM Component implementing Component trait for lifecycle management
//!
//! All Brain-Host communication goes through this versioned RPC protocol.

use std::sync::Arc;

use uuid::Uuid;

use crate::component::{Component, ComponentId, ComponentInfo, ComponentState};
use crate::error::{BrainError, Result};
use crate::llm::{ChatRequest, ChatResponse, LlmClient, Provider};
use crate::rpc::{
    ProtocolVersion, RpcError, RpcMethod, RpcRequest, RpcResponse, UsageStats, PROTOCOL_VERSION,
};
use crate::wasm_runtime::{WasmConfig, WasmRuntime};

/// Brain component version
pub const BRAIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Internal state of the Brain component
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BrainState {
    /// Brain is uninitialized
    #[default]
    Uninitialized,
    /// Brain is initialized but not running
    Initialized,
    /// Brain is running and ready for requests
    Running,
    /// Brain has been stopped
    Stopped,
    /// Brain encountered an error
    Error,
}

impl std::fmt::Display for BrainState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uninitialized => write!(f, "uninitialized"),
            Self::Initialized => write!(f, "initialized"),
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Configuration for the Brain component
#[derive(Debug)]
pub struct BrainConfig {
    /// WASM runtime configuration
    pub wasm_config: WasmConfig,
    /// Default LLM provider to use
    pub default_provider: Provider,
    /// RPC protocol version
    pub protocol_version: ProtocolVersion,
}

impl BrainConfig {
    /// Creates a new `BrainConfig` with default values
    #[must_use]
    pub fn new() -> Self {
        Self {
            wasm_config: WasmConfig::default(),
            default_provider: Provider::OpenAI,
            protocol_version: ProtocolVersion::current(),
        }
    }

    /// Sets the default LLM provider
    #[must_use]
    pub fn with_provider(mut self, provider: Provider) -> Self {
        self.default_provider = provider;
        self
    }

    /// Sets the WASM configuration
    #[must_use]
    pub fn with_wasm_config(mut self, config: WasmConfig) -> Self {
        self.wasm_config = config;
        self
    }
}

impl Default for BrainConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Brain WASM Component
///
/// Provides isolated LLM reasoning logic via wasmtime sandbox.
/// Implements the Component trait for lifecycle management.
#[derive(Debug)]
pub struct Brain {
    state: ComponentState,
    internal_state: BrainState,
    config: BrainConfig,
    runtime: Option<WasmRuntime>,
    llm_client: Arc<LlmClient>,
    session_id: Option<Uuid>,
}

impl Brain {
    /// Creates a new Brain component with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ComponentState::Uninitialized,
            internal_state: BrainState::Uninitialized,
            config: BrainConfig::default(),
            runtime: None,
            llm_client: Arc::new(LlmClient::new()),
            session_id: None,
        }
    }

    /// Creates a new Brain component with custom configuration
    #[must_use]
    pub fn with_config(config: BrainConfig) -> Self {
        Self {
            state: ComponentState::Uninitialized,
            internal_state: BrainState::Uninitialized,
            config,
            runtime: None,
            llm_client: Arc::new(LlmClient::new()),
            session_id: None,
        }
    }

    /// Returns the internal Brain state
    #[must_use]
    pub fn brain_state(&self) -> BrainState {
        self.internal_state
    }

    /// Returns the session ID if initialized
    #[must_use]
    pub fn session_id(&self) -> Option<Uuid> {
        self.session_id
    }

    /// Returns the protocol version
    #[must_use]
    pub fn protocol_version(&self) -> &ProtocolVersion {
        &self.config.protocol_version
    }

    /// Returns component information
    #[must_use]
    pub fn info(&self) -> ComponentInfo {
        ComponentInfo::new(ComponentId::BRAIN, self.name(), BRAIN_VERSION)
    }

    /// Invokes an RPC method on the Brain
    pub fn invoke(&mut self, request: &RpcRequest) -> RpcResponse {
        if self.internal_state != BrainState::Running {
            return RpcResponse::error(request.id, RpcError::new(0x3000, "Brain not running"));
        }

        if request.version != PROTOCOL_VERSION {
            return RpcResponse::error(
                request.id,
                RpcError::rpc_version_mismatch(PROTOCOL_VERSION, request.version),
            );
        }

        let result: Result<serde_json::Value> = match request.method {
            RpcMethod::ChatCompletion => Self::handle_chat_completion(&request.params),
            RpcMethod::GetPhase => Ok(self.handle_get_phase()),
            RpcMethod::SetPhase => Ok(Self::handle_set_phase(&request.params)),
            RpcMethod::CheckSop => Ok(Self::handle_check_sop(&request.params)),
            RpcMethod::GenerateCode => Self::handle_generate_code(),
            RpcMethod::AnalyzeCode => Self::handle_analyze_code(),
            RpcMethod::BuildPrompt => Ok(Self::handle_build_prompt(&request.params)),
            _ => Err(BrainError::SerializationError {
                reason: format!("Unknown method: {}", request.method),
            }
            .into()),
        };

        match result {
            Ok(value) => RpcResponse::success(request.id, value),
            Err(e) => RpcResponse::error(request.id, RpcError::new(0x3004, e.to_string())),
        }
    }

    fn handle_chat_completion(params: &serde_json::Value) -> Result<serde_json::Value> {
        let request: ChatRequest =
            serde_json::from_value(params.clone()).map_err(|e| BrainError::SerializationError {
                reason: e.to_string(),
            })?;

        let _ = request;
        Err(BrainError::LlmCallFailed {
            reason: "Chat completion stub - not yet implemented".into(),
        }
        .into())
    }

    fn handle_get_phase(&self) -> serde_json::Value {
        serde_json::json!({
            "phase": "discovery",
            "version": self.config.protocol_version.to_string()
        })
    }

    fn handle_set_phase(params: &serde_json::Value) -> serde_json::Value {
        let phase = params
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or("discovery");

        serde_json::json!({
            "success": true,
            "phase": phase
        })
    }

    fn handle_check_sop(params: &serde_json::Value) -> serde_json::Value {
        let code = params.get("code").and_then(|v| v.as_str()).unwrap_or("");

        let has_unwrap = code.contains(".unwrap()");
        let has_expect = code.contains(".expect(");

        let mut violations = Vec::new();
        if has_unwrap {
            violations.push("Use of .unwrap() detected");
        }
        if has_expect {
            violations.push("Use of .expect() detected");
        }

        serde_json::json!({
            "compliant": violations.is_empty(),
            "violations": violations
        })
    }

    fn handle_generate_code() -> Result<serde_json::Value> {
        Err(BrainError::LlmCallFailed {
            reason: "Generate code stub - not yet implemented".into(),
        }
        .into())
    }

    fn handle_analyze_code() -> Result<serde_json::Value> {
        Err(BrainError::LlmCallFailed {
            reason: "Analyze code stub - not yet implemented".into(),
        }
        .into())
    }

    fn handle_build_prompt(params: &serde_json::Value) -> serde_json::Value {
        let context = params.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let task = params.get("task").and_then(|v| v.as_str()).unwrap_or("");

        let prompt = format!(
            "Context:\n{context}\\nTask:\n{task}\n\nPlease complete the task based on the context provided."
        );

        serde_json::json!({
            "prompt": prompt,
            "estimated_tokens": prompt.len() / 4
        })
    }

    /// Performs a chat completion request
    ///
    /// # Errors
    /// Returns an error if the LLM call fails or the client is not configured.
    pub fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.llm_client.chat(request)
    }

    /// Returns the current memory usage
    #[must_use]
    pub fn memory_usage(&self) -> usize {
        self.runtime
            .as_ref()
            .map_or(0, |_| std::mem::size_of::<Self>())
    }

    /// Returns usage statistics
    #[must_use]
    pub fn get_usage_stats(&self) -> UsageStats {
        UsageStats::default()
    }
}

impl Default for Brain {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Brain {
    fn id(&self) -> ComponentId {
        ComponentId::BRAIN
    }

    fn name(&self) -> &'static str {
        "Brain"
    }

    fn state(&self) -> ComponentState {
        self.state
    }

    fn initialize(&mut self) -> Result<()> {
        if self.internal_state != BrainState::Uninitialized {
            return Err(BrainError::AlreadyInitialized.into());
        }

        let runtime = WasmRuntime::new(self.config.wasm_config.clone())?;
        self.runtime = Some(runtime);
        self.session_id = Some(Uuid::new_v4());
        self.internal_state = BrainState::Initialized;
        self.state = ComponentState::Initialized;

        tracing::info!(
            session_id = ?self.session_id,
            protocol_version = %self.config.protocol_version,
            "Brain initialized"
        );

        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        if self.internal_state == BrainState::Uninitialized {
            return Err(BrainError::NotInitialized.into());
        }

        if self.internal_state == BrainState::Running {
            return Ok(());
        }

        self.internal_state = BrainState::Running;
        self.state = ComponentState::Running;

        tracing::info!(
            session_id = ?self.session_id,
            "Brain started"
        );

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if self.internal_state != BrainState::Running {
            return Ok(());
        }

        self.internal_state = BrainState::Stopped;
        self.state = ComponentState::Stopped;
        self.runtime = None;

        tracing::info!(
            session_id = ?self.session_id,
            "Brain stopped"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brain_component_trait() {
        let mut brain = Brain::new();
        assert_eq!(brain.id(), ComponentId::BRAIN);
        assert_eq!(brain.name(), "Brain");
        assert_eq!(Component::state(&brain), ComponentState::Uninitialized);
        assert!(brain.initialize().is_ok());
        assert_eq!(Component::state(&brain), ComponentState::Initialized);
        assert!(brain.start().is_ok());
        assert_eq!(Component::state(&brain), ComponentState::Running);
        assert!(brain.stop().is_ok());
        assert_eq!(Component::state(&brain), ComponentState::Stopped);
    }

    #[test]
    fn test_brain_double_initialize() {
        let mut brain = Brain::new();
        brain.initialize().unwrap();
        let result = brain.initialize();
        assert!(result.is_err());
    }

    #[test]
    fn test_brain_start_without_init() {
        let mut brain = Brain::new();
        let result = brain.start();
        assert!(result.is_err());
    }

    #[test]
    fn test_brain_state_display() {
        assert_eq!(format!("{}", BrainState::Running), "running");
        assert_eq!(format!("{}", BrainState::Error), "error");
    }

    #[test]
    fn test_brain_config_default() {
        let config = BrainConfig::default();
        assert_eq!(config.default_provider, Provider::OpenAI);
    }

    #[test]
    fn test_brain_info() {
        let brain = Brain::new();
        let info = brain.info();
        assert_eq!(info.id, ComponentId::BRAIN);
        assert_eq!(info.name, "Brain");
    }

    #[test]
    fn test_brain_invoke_not_running() {
        let mut brain = Brain::new();
        let request = RpcRequest::new(RpcMethod::GetPhase, serde_json::json!({}));
        let response = brain.invoke(&request);
        assert!(response.is_error());
    }

    #[test]
    fn test_brain_invoke_version_mismatch() {
        let mut brain = Brain::new();
        brain.initialize().unwrap();
        brain.start().unwrap();

        let mut request = RpcRequest::new(RpcMethod::GetPhase, serde_json::json!({}));
        request.version = 99;

        let response = brain.invoke(&request);
        assert!(response.is_error());
    }

    #[test]
    fn test_brain_invoke_get_phase() {
        let mut brain = Brain::new();
        brain.initialize().unwrap();
        brain.start().unwrap();

        let request = RpcRequest::new(RpcMethod::GetPhase, serde_json::json!({}));
        let response = brain.invoke(&request);

        assert!(response.is_success());
    }

    #[test]
    fn test_brain_invoke_set_phase() {
        let mut brain = Brain::new();
        brain.initialize().unwrap();
        brain.start().unwrap();

        let request = RpcRequest::new(
            RpcMethod::SetPhase,
            serde_json::json!({"phase": "implementation"}),
        );
        let response = brain.invoke(&request);

        assert!(response.is_success());
    }

    #[test]
    fn test_brain_invoke_check_sop_compliant() {
        let mut brain = Brain::new();
        brain.initialize().unwrap();
        brain.start().unwrap();

        let request = RpcRequest::new(
            RpcMethod::CheckSop,
            serde_json::json!({"code": "let x = 1;"}),
        );
        let response = brain.invoke(&request);

        assert!(response.is_success());
        if let Ok(value) = &response.result {
            assert!(value.get("compliant").unwrap().as_bool().unwrap());
        }
    }

    #[test]
    fn test_brain_invoke_check_sop_violation() {
        let mut brain = Brain::new();
        brain.initialize().unwrap();
        brain.start().unwrap();

        let request = RpcRequest::new(
            RpcMethod::CheckSop,
            serde_json::json!({"code": "let x = y.unwrap();"}),
        );
        let response = brain.invoke(&request);

        assert!(response.is_success());
        if let Ok(value) = &response.result {
            assert!(!value.get("compliant").unwrap().as_bool().unwrap());
        }
    }

    #[test]
    fn test_brain_invoke_build_prompt() {
        let mut brain = Brain::new();
        brain.initialize().unwrap();
        brain.start().unwrap();

        let request = RpcRequest::new(
            RpcMethod::BuildPrompt,
            serde_json::json!({
                "context": "This is a test",
                "task": "Do something"
            }),
        );
        let response = brain.invoke(&request);

        assert!(response.is_success());
    }

    #[test]
    fn test_brain_session_id() {
        let mut brain = Brain::new();
        assert!(brain.session_id().is_none());
        brain.initialize().unwrap();
        assert!(brain.session_id().is_some());
    }

    #[test]
    fn test_brain_protocol_version() {
        let brain = Brain::new();
        let version = brain.protocol_version();
        assert_eq!(version.major, 1);
    }
}
