//! RPC Protocol for Brain-Host Communication
//!
//! Implements the versioned RPC protocol per BP-BRAIN-001.
//! All Brain-Host communication goes through this interface.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current protocol version
pub const PROTOCOL_VERSION: u32 = 1;

/// Protocol version identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Major version - breaking changes
    pub major: u8,
    /// Minor version - new features
    pub minor: u8,
    /// Patch version - bug fixes
    pub patch: u8,
}

impl ProtocolVersion {
    /// Creates a new protocol version
    #[must_use]
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Returns the current protocol version
    #[must_use]
    pub const fn current() -> Self {
        Self::new(1, 0, 0)
    }

    /// Checks if this version is compatible with another
    #[must_use]
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.major == other.major
    }

    /// Converts to u32 representation
    #[must_use]
    pub fn to_u32(&self) -> u32 {
        u32::from(self.major) << 16 | u32::from(self.minor) << 8 | u32::from(self.patch)
    }

    /// Creates from u32 representation
    #[must_use]
    pub fn from_u32(value: u32) -> Self {
        Self {
            major: ((value >> 16) & 0xFF) as u8,
            minor: ((value >> 8) & 0xFF) as u8,
            patch: (value & 0xFF) as u8,
        }
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::current()
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Available RPC methods for Brain operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RpcMethod {
    /// LLM chat completion
    ChatCompletion,
    /// Text embedding generation
    EmbedText,
    /// Query knowledge graph
    QueryGraph,
    /// Search vector database
    SearchVector,
    /// Get current phase
    GetPhase,
    /// Set current phase
    SetPhase,
    /// Check SOP compliance
    CheckSop,
    /// Generate ADR document
    GenerateAdr,
    /// Generate code
    GenerateCode,
    /// Analyze code
    AnalyzeCode,
    /// Validate SOP rules
    ValidateSop,
    /// Build a prompt
    BuildPrompt,
    /// Synthesize research findings
    SynthesizeResearch,
    /// Explain code
    ExplainCode,
    /// Refactor code
    RefactorCode,
}

impl RpcMethod {
    /// Returns the string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChatCompletion => "chat_completion",
            Self::EmbedText => "embed_text",
            Self::QueryGraph => "query_graph",
            Self::SearchVector => "search_vector",
            Self::GetPhase => "get_phase",
            Self::SetPhase => "set_phase",
            Self::CheckSop => "check_sop",
            Self::GenerateAdr => "generate_adr",
            Self::GenerateCode => "generate_code",
            Self::AnalyzeCode => "analyze_code",
            Self::ValidateSop => "validate_sop",
            Self::BuildPrompt => "build_prompt",
            Self::SynthesizeResearch => "synthesize_research",
            Self::ExplainCode => "explain_code",
            Self::RefactorCode => "refactor_code",
        }
    }
}

impl std::fmt::Display for RpcMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// RPC error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Error code
    pub code: u32,
    /// Error message
    pub message: String,
    /// Additional error data
    pub data: Option<serde_json::Value>,
}

impl RpcError {
    /// Error code: WASM compilation failed
    #[must_use]
    pub const fn code_wasm_compile_failed() -> u32 {
        0x3001
    }
    /// Error code: WASM trap
    #[must_use]
    pub const fn code_wasm_trap() -> u32 {
        0x3002
    }
    /// Error code: RPC version mismatch
    #[must_use]
    pub const fn code_rpc_version_mismatch() -> u32 {
        0x3003
    }
    /// Error code: LLM call failed
    #[must_use]
    pub const fn code_llm_call_failed() -> u32 {
        0x3004
    }
    /// Error code: Capability insufficient
    #[must_use]
    pub const fn code_capability_insufficient() -> u32 {
        0x3005
    }
    /// Error code: Memory limit exceeded
    #[must_use]
    pub const fn code_memory_limit_exceeded() -> u32 {
        0x3006
    }
    /// Error code: SOP violation
    #[must_use]
    pub const fn code_sop_violation() -> u32 {
        0x3007
    }
    /// Error code: Prompt too long
    #[must_use]
    pub const fn code_prompt_too_long() -> u32 {
        0x3008
    }

    /// Creates a new RPC error
    #[must_use]
    pub fn new(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Adds additional data to the error
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Creates a WASM compilation failed error
    #[must_use]
    pub fn wasm_compile_failed(reason: impl Into<String>) -> Self {
        Self::new(Self::code_wasm_compile_failed(), reason)
    }

    /// Creates a WASM trap error
    #[must_use]
    pub fn wasm_trap(message: impl Into<String>) -> Self {
        Self::new(Self::code_wasm_trap(), message)
    }

    /// Creates an RPC version mismatch error
    #[must_use]
    pub fn rpc_version_mismatch(expected: u32, actual: u32) -> Self {
        Self::new(
            Self::code_rpc_version_mismatch(),
            format!("Expected version {expected}, got {actual}"),
        )
    }

    /// Creates an LLM call failed error
    #[must_use]
    pub fn llm_call_failed(reason: impl Into<String>) -> Self {
        Self::new(Self::code_llm_call_failed(), reason)
    }

    /// Creates a capability insufficient error
    #[must_use]
    pub fn capability_insufficient(required: impl Into<String>) -> Self {
        Self::new(Self::code_capability_insufficient(), required)
    }

    /// Creates a memory limit exceeded error
    #[must_use]
    pub fn memory_limit_exceeded(bytes: usize) -> Self {
        Self::new(
            Self::code_memory_limit_exceeded(),
            format!("Memory limit exceeded: {bytes} bytes"),
        )
    }

    /// Creates an SOP violation error
    #[must_use]
    pub fn sop_violation(violation: impl Into<String>) -> Self {
        Self::new(Self::code_sop_violation(), violation)
    }

    /// Creates a prompt too long error
    #[must_use]
    pub fn prompt_too_long(tokens: usize) -> Self {
        Self::new(
            Self::code_prompt_too_long(),
            format!("Prompt too long: {tokens} tokens"),
        )
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:#06x}] {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

/// RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// Protocol version
    pub version: u32,
    /// Unique request ID
    pub id: Uuid,
    /// Method to invoke
    pub method: RpcMethod,
    /// Method parameters
    pub params: serde_json::Value,
}

impl RpcRequest {
    /// Creates a new RPC request
    #[must_use]
    pub fn new(method: RpcMethod, params: serde_json::Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id: Uuid::new_v4(),
            method,
            params,
        }
    }

    /// Sets a custom request ID
    #[must_use]
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Serializes the request to JSON bytes
    ///
    /// # Errors
    /// Returns an error if serialization fails
    pub fn serialize(&self) -> crate::error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| {
            crate::error::BrainError::SerializationError {
                reason: e.to_string(),
            }
            .into()
        })
    }

    /// Deserializes a request from JSON bytes
    ///
    /// # Errors
    /// Returns an error if deserialization fails
    pub fn deserialize(data: &[u8]) -> crate::error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| {
            crate::error::BrainError::SerializationError {
                reason: e.to_string(),
            }
            .into()
        })
    }
}

/// RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// Protocol version
    pub version: u32,
    /// Request ID this response corresponds to
    pub id: Uuid,
    /// Result or error
    pub result: Result<serde_json::Value, RpcError>,
}

impl RpcResponse {
    /// Creates a successful response
    #[must_use]
    pub fn success(id: Uuid, result: serde_json::Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id,
            result: Ok(result),
        }
    }

    /// Creates an error response
    #[must_use]
    pub fn error(id: Uuid, error: RpcError) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id,
            result: Err(error),
        }
    }

    /// Serializes the response to JSON bytes
    ///
    /// # Errors
    /// Returns an error if serialization fails
    pub fn serialize(&self) -> crate::error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| {
            crate::error::BrainError::SerializationError {
                reason: e.to_string(),
            }
            .into()
        })
    }

    /// Deserializes a response from JSON bytes
    ///
    /// # Errors
    /// Returns an error if deserialization fails
    pub fn deserialize(data: &[u8]) -> crate::error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| {
            crate::error::BrainError::SerializationError {
                reason: e.to_string(),
            }
            .into()
        })
    }

    /// Returns true if this is a successful response
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    /// Returns true if this is an error response
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.result.is_err()
    }
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    /// Tokens used in prompt
    pub prompt_tokens: u64,
    /// Tokens in completion
    pub completion_tokens: u64,
    /// Total tokens used
    pub total_tokens: u64,
}

impl UsageStats {
    /// Creates new usage statistics
    #[must_use]
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version() {
        let v = ProtocolVersion::current();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_protocol_version_compatibility() {
        let v1 = ProtocolVersion::new(1, 0, 0);
        let v2 = ProtocolVersion::new(1, 5, 3);
        let v3 = ProtocolVersion::new(2, 0, 0);

        assert!(v1.is_compatible(&v2));
        assert!(!v1.is_compatible(&v3));
    }

    #[test]
    fn test_protocol_version_u32_roundtrip() {
        let v = ProtocolVersion::new(1, 2, 3);
        let encoded = v.to_u32();
        let decoded = ProtocolVersion::from_u32(encoded);
        assert_eq!(v, decoded);
    }

    #[test]
    fn test_rpc_request_serialization() {
        let request = RpcRequest::new(
            RpcMethod::ChatCompletion,
            serde_json::json!({"prompt": "test"}),
        );

        let serialized = request.serialize().unwrap();
        let deserialized = RpcRequest::deserialize(&serialized).unwrap();

        assert_eq!(request.method, deserialized.method);
        assert_eq!(request.id, deserialized.id);
    }

    #[test]
    fn test_rpc_response_success() {
        let id = Uuid::new_v4();
        let response = RpcResponse::success(id, serde_json::json!({"result": "ok"}));

        assert!(response.is_success());
        assert!(!response.is_error());
    }

    #[test]
    fn test_rpc_response_error() {
        let id = Uuid::new_v4();
        let response = RpcResponse::error(id, RpcError::llm_call_failed("API error"));

        assert!(!response.is_success());
        assert!(response.is_error());
    }

    #[test]
    fn test_rpc_error_codes() {
        assert_eq!(RpcError::code_wasm_compile_failed(), 0x3001);
        assert_eq!(RpcError::code_wasm_trap(), 0x3002);
        assert_eq!(RpcError::code_llm_call_failed(), 0x3004);
    }

    #[test]
    fn test_usage_stats() {
        let stats = UsageStats::new(100, 50);
        assert_eq!(stats.prompt_tokens, 100);
        assert_eq!(stats.completion_tokens, 50);
        assert_eq!(stats.total_tokens, 150);
    }

    #[test]
    fn test_rpc_method_display() {
        assert_eq!(format!("{}", RpcMethod::ChatCompletion), "chat_completion");
        assert_eq!(format!("{}", RpcMethod::GenerateCode), "generate_code");
    }
}
