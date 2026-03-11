//! JSON-RPC 2.0 types

use serde::{Deserialize, Serialize};

/// JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID
    pub id: Id,
    /// Method name
    pub method: String,
    /// Method parameters
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl Request {
    /// Create a new request
    pub fn new(id: impl Into<Id>, method: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            method: method.into(),
            params: None,
        }
    }

    /// Create a request with parameters
    pub fn with_params(mut self, params: impl Serialize) -> Self {
        self.params = Some(serde_json::to_value(params).unwrap_or_default());
        self
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID
    pub id: Id,
    /// Result (on success)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (on failure)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

impl Response {
    /// Create a successful response
    pub fn success(id: Id, result: impl Serialize) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(result).unwrap_or_default()),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Id, error: Error) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Create a method not found error
    pub fn method_not_found(id: Id, method: &str) -> Self {
        Self::error(
            id,
            Error::method_not_found(format!("Method not found: {}", method)),
        )
    }

    /// Create an invalid params error
    pub fn invalid_params(id: Id, message: impl Into<String>) -> Self {
        Self::error(id, Error::invalid_params(message.into()))
    }

    /// Create an internal error
    pub fn internal_error(id: Id, message: impl Into<String>) -> Self {
        Self::error(id, Error::internal_error(message.into()))
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Convert to pretty JSON string
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// JSON-RPC error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Error {
    /// Create a new error
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create an error with additional data
    pub fn with_data(mut self, data: impl Serialize) -> Self {
        self.data = Some(serde_json::to_value(data).unwrap_or_default());
        self
    }

    /// Parse error (-32700)
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(-32700, message)
    }

    /// Invalid request (-32600)
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(-32600, message)
    }

    /// Method not found (-32601)
    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self::new(-32601, message)
    }

    /// Invalid params (-32602)
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(-32602, message)
    }

    /// Internal error (-32603)
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(-32603, message)
    }

    /// Server error (-32000 to -32099)
    pub fn server_error(code: i32, message: impl Into<String>) -> Self {
        debug_assert!((-32099..=-32000).contains(&code));
        Self::new(code, message)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC Error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// JSON-RPC request/response ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    /// Numeric ID
    Number(i64),
    /// String ID
    String(String),
    /// Null ID (for notifications)
    Null,
}

impl Id {
    /// Create a numeric ID
    pub fn number(n: i64) -> Self {
        Self::Number(n)
    }

    /// Create a string ID
    pub fn string(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }

    /// Create a null ID
    pub fn null() -> Self {
        Self::Null
    }

    /// Check if null
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl From<i64> for Id {
    fn from(n: i64) -> Self {
        Self::Number(n)
    }
}

impl From<String> for Id {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

/// JSON-RPC notification (no ID, no response expected)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Method name
    pub method: String,
    /// Method parameters
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl Notification {
    /// Create a new notification
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params: None,
        }
    }

    /// Create a notification with parameters
    pub fn with_params(mut self, params: impl Serialize) -> Self {
        self.params = Some(serde_json::to_value(params).unwrap_or_default());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = Request::new(1, "chat/send").with_params(serde_json::json!({"message": "Hello"}));

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""method":"chat/send"#));
        assert!(json.contains(r#""message":"Hello"#));
    }

    #[test]
    fn test_response_success() {
        let res = Response::success(Id::Number(1), serde_json::json!({"status": "ok"}));

        assert!(res.result.is_some());
        assert!(res.error.is_none());
    }

    #[test]
    fn test_response_error() {
        let res = Response::error(Id::Number(1), Error::method_not_found("Unknown method"));

        assert!(res.result.is_none());
        assert!(res.error.is_some());

        let err = res.error.unwrap();
        assert_eq!(err.code, -32601);
    }
}
