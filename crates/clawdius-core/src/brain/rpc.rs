//! Brain-Host RPC protocol definitions

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Enum,
    Trait,
    Module,
    Variable,
    Constant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrainRequest {
    Chat { message: String },
    Analyze { code: String, language: String },
    Validate { operation: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrainResponse {
    Chat { response: String },
    Analysis { symbols: Vec<Symbol> },
    Validation { approved: bool, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let request = BrainRequest::Chat {
            message: "Hello".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Chat"));
    }

    #[test]
    fn test_response_serialization() {
        let response = BrainResponse::Validation {
            approved: true,
            reason: "Safe operation".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Validation"));
    }

    #[test]
    fn test_symbol_serialization() {
        let symbol = Symbol {
            name: "my_function".to_string(),
            kind: SymbolKind::Function,
            location: Location {
                file: "test.rs".to_string(),
                line: 10,
                column: 5,
            },
        };
        let json = serde_json::to_string(&symbol).unwrap();
        let parsed: Symbol = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "my_function");
    }
}
