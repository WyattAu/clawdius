//! Debug Adapter Protocol (DAP) types and adapter
//!
//! Skeleton DAP adapter that handles the standard initialization sequence
//! and returns stub responses for other DAP requests.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DapError>,
}

impl Default for DapMessage {
    fn default() -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: None,
            result: None,
            params: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequestArguments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    pub client_name: String,
    pub adapter_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_start_at1: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns_start_at1: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_variable_type: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_memory_references: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_configuration_done_request: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_restart_request: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBreakpointsArguments {
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakpoints: Option<Vec<Breakpoint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cols: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_reference: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<Source>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksums: Option<Vec<Checksum>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checksum {
    pub algorithm: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_restart: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction_pointer_reference: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    pub name: String,
    pub expensive: bool,
    pub variables_reference: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub var_type: Option<String>,
    pub variables_reference: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named_variables: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_variables: Option<u64>,
}

pub struct DapHandler {
    initialized: bool,
}

impl DapHandler {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    pub fn handle(&mut self, message: &DapMessage) -> DapMessage {
        match message.method.as_deref() {
            Some("initialize") => self.handle_initialize(message),
            Some("launch") => self.handle_launch(message),
            Some("setBreakpoints") => self.handle_set_breakpoints(message),
            Some("continue") => self.handle_continue(message),
            Some("next") => self.handle_next(message),
            Some("stepIn") => self.handle_step_in(message),
            Some("stepOut") => self.handle_step_out(message),
            Some("stackTrace") => self.handle_stack_trace(message),
            Some("scopes") => self.handle_scopes(message),
            Some("variables") => self.handle_variables(message),
            Some("threads") => self.handle_threads(message),
            Some("disconnect") => self.handle_disconnect(message),
            Some("evaluate") => self.handle_evaluate(message),
            Some("configurationDone") => self.handle_configuration_done(message),
            _ => self.handle_unknown(message),
        }
    }

    fn handle_initialize(&mut self, msg: &DapMessage) -> DapMessage {
        if let Some(ref params) = msg.params {
            if let Ok(args) = serde_json::from_value::<InitializeRequestArguments>(params.clone()) {
                tracing::info!(
                    client_name = %args.client_name,
                    adapter_id = %args.adapter_id,
                    "DAP initialize request"
                );
            }
        }

        self.initialized = true;
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({
                "capabilities": {
                    "supportsConfigurationDoneRequest": true,
                    "supportsRestartRequest": false,
                }
            })),
            ..Default::default()
        }
    }

    fn handle_launch(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({})),
            ..Default::default()
        }
    }

    fn handle_set_breakpoints(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({ "breakpoints": [] })),
            ..Default::default()
        }
    }

    fn handle_continue(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({
                "allThreadsContinued": true
            })),
            ..Default::default()
        }
    }

    fn handle_next(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({})),
            ..Default::default()
        }
    }

    fn handle_step_in(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        ok_response(msg.id)
    }

    fn handle_step_out(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        ok_response(msg.id)
    }

    fn handle_stack_trace(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({
                "stackFrames": [],
                "totalFrames": 0
            })),
            ..Default::default()
        }
    }

    fn handle_scopes(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({ "scopes": [] })),
            ..Default::default()
        }
    }

    fn handle_variables(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({ "variables": [] })),
            ..Default::default()
        }
    }

    fn handle_threads(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({ "threads": [] })),
            ..Default::default()
        }
    }

    fn handle_disconnect(&mut self, msg: &DapMessage) -> DapMessage {
        self.initialized = false;
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({})),
            ..Default::default()
        }
    }

    fn handle_evaluate(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        DapMessage {
            jsonrpc: "2.0".to_string(),
            id: msg.id,
            result: Some(serde_json::json!({
                "result": "",
                "variablesReference": 0
            })),
            ..Default::default()
        }
    }

    fn handle_configuration_done(&mut self, msg: &DapMessage) -> DapMessage {
        self.ensure_initialized(msg);
        ok_response(msg.id)
    }

    fn handle_unknown(&mut self, msg: &DapMessage) -> DapMessage {
        let method = msg.method.as_deref().unwrap_or("unknown");
        tracing::debug!(method, "Unknown DAP request");

        if msg.id.is_some() {
            DapMessage {
                jsonrpc: "2.0".to_string(),
                id: msg.id,
                error: Some(DapError {
                    code: 32700,
                    message: format!("Method '{method}' not supported"),
                }),
                ..Default::default()
            }
        } else {
            DapMessage::default()
        }
    }

    fn ensure_initialized(&self, msg: &DapMessage) {
        if !self.initialized {
            tracing::warn!(
                method = ?msg.method,
                "DAP request received before initialize"
            );
        }
    }
}

fn ok_response(id: Option<u64>) -> DapMessage {
    DapMessage {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(serde_json::json!({})),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dap_message_default() {
        let msg = DapMessage::default();
        assert_eq!(msg.jsonrpc, "2.0");
        assert!(msg.id.is_none());
        assert!(msg.result.is_none());
    }

    #[test]
    fn test_dap_message_serialization() {
        let msg = DapMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: Some("initialize".to_string()),
            params: Some(serde_json::json!({
                "clientName": "test",
                "adapterID": "test-adapter"
            })),
            result: None,
            error: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: DapMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, Some(1));
        assert_eq!(parsed.method.as_deref(), Some("initialize"));
    }

    #[tokio::test]
    async fn test_initialize_handshake() {
        let mut handler = DapHandler::new();
        let request = DapMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: Some("initialize".to_string()),
            params: Some(serde_json::json!({
                "clientName": "test-client",
                "adapterID": "test"
            })),
            result: None,
            error: None,
        };

        let response = handler.handle(&request);
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        let caps = &response.result.unwrap()["capabilities"];
        assert_eq!(caps["supportsConfigurationDoneRequest"], true);
        assert_eq!(caps["supportsRestartRequest"], false);
    }

    #[tokio::test]
    async fn test_disconnect_resets_state() {
        let mut handler = DapHandler::new();

        let init = DapMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: Some("initialize".to_string()),
            params: Some(serde_json::json!({
                "clientName": "test",
                "adapterID": "test"
            })),
            result: None,
            error: None,
        };
        let _ = handler.handle(&init);

        let disconnect = DapMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: Some("disconnect".to_string()),
            params: None,
            result: None,
            error: None,
        };
        let resp = handler.handle(&disconnect);
        assert_eq!(resp.id, Some(2));
    }

    #[tokio::test]
    async fn test_unknown_method_returns_error() {
        let mut handler = DapHandler::new();
        let request = DapMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(99),
            method: Some("nonexistent".to_string()),
            params: None,
            result: None,
            error: None,
        };

        let response = handler.handle(&request);
        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, 32700);
    }

    #[tokio::test]
    async fn test_set_breakpoints_returns_empty() {
        let mut handler = DapHandler::new();

        let init = DapMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: Some("initialize".to_string()),
            params: Some(serde_json::json!({
                "clientName": "test",
                "adapterID": "test"
            })),
            result: None,
            error: None,
        };
        let _ = handler.handle(&init);

        let set_bp = DapMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: Some("setBreakpoints".to_string()),
            params: Some(serde_json::json!({
                "source": { "path": "/tmp/test.rs" },
                "breakpoints": [{ "line": 10 }]
            })),
            result: None,
            error: None,
        };

        let response = handler.handle(&set_bp);
        assert_eq!(response.id, Some(2));
        let breakpoints = &response.result.unwrap()["breakpoints"];
        assert!(breakpoints.is_array());
        assert_eq!(breakpoints.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_breakpoint_type_serialization() {
        let bp = Breakpoint {
            line: Some(42),
            column: None,
            condition: Some("x > 0".to_string()),
            hit_condition: None,
            log_message: None,
        };
        let json = serde_json::to_string(&bp).unwrap();
        let parsed: Breakpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.line, Some(42));
        assert_eq!(parsed.condition.as_deref(), Some("x > 0"));
    }

    #[test]
    fn test_variable_type_field_rename() {
        let v = Variable {
            name: "x".to_string(),
            value: "42".to_string(),
            var_type: Some("i32".to_string()),
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
        };
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["type"], "i32");
        assert!(json.get("var_type").is_none());
    }
}
