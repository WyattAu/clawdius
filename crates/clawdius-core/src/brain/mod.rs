//! Brain WASM Runtime Module
//!
//! Provides a sandboxed WASM runtime for executing Brain modules
//! with fuel limiting and RPC support.
//!
//! # Overview
//!
//! The Brain module implements a secure execution environment for LLM reasoning
//! and code analysis using WebAssembly (WASM) with the Wasmtime runtime. This
//! provides strong isolation guarantees while allowing for flexible, updatable
//! reasoning modules.
//!
//! # Features
//!
//! - **WASM Isolation**: Code runs in a sandboxed WASM environment
//! - **Fuel Limiting**: Prevents infinite loops and resource exhaustion
//! - **RPC Communication**: Structured request/response protocol
//! - **Hot Reloading**: Update Brain modules without restarting
//! - **Multi-language Support**: Write modules in Rust, C, or other WASM-compatible languages
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use clawdius_core::brain::{BrainRuntime, BrainConfig, BrainRequest};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! // Create runtime with configuration
//! let config = BrainConfig {
//!     max_fuel: 1_000_000,
//!     max_memory: 10 * 1024 * 1024, // 10MB
//!     timeout_ms: 5000,
//! };
//!
//! let mut runtime = BrainRuntime::new(config)?;
//!
//! // Load a Brain module
//! runtime.load_module("path/to/brain.wasm")?;
//!
//! // Execute a request
//! let request = BrainRequest::AnalyzeCode {
//!     code: "fn main() {}".to_string(),
//!     language: "rust".to_string(),
//! };
//!
//! let response = runtime.execute(request)?;
//! println!("Result: {:?}", response);
//! # Ok(())
//! # }
//! ```
//!
//! # Architecture
//!
//! The Brain system consists of:
//!
//! - **`BrainRuntime`**: The WASM runtime host
//! - **`BrainModule`**: Loaded WASM modules
//! - **BrainRequest/BrainResponse**: RPC protocol types
//! - **`HostState`**: Shared state between host and WASM
//!
//! # Security
//!
//! Brain modules execute with:
//! - No filesystem access
//! - No network access
//! - Limited memory (configurable)
//! - Limited execution time (fuel-based)
//! - No access to host environment variables
//!
//! # Example: Creating a Brain Module
//!
//! ```rust,ignore
//! // In your WASM module (Rust)
//! use brain_api::{BrainRequest, BrainResponse};
//! use postcard;
//!
//! #[no_mangle]
//! pub extern "C" fn handle_request(ptr: *const u8, len: usize) -> *mut u8 {
//!     let request: BrainRequest = unsafe {
//!         let slice = std::slice::from_raw_parts(ptr, len);
//!         postcard::from_bytes(slice).unwrap()
//!     };
//!
//!     let response = match request {
//!         BrainRequest::AnalyzeCode { code, language } => {
//!             // Perform analysis
//!             BrainResponse::Analysis { /* ... */ }
//!         }
//!         _ => BrainResponse::Error("Unsupported request".to_string()),
//!     };
//!
//!     // Return response to host
//!     let bytes = postcard::to_allocvec(&response).unwrap();
//!     let result = bytes.leak();
//!     result.as_mut_ptr()
//! }
//! ```
//!
//! # Performance
//!
//! - Cold start: ~10ms
//! - Request latency: <1ms (after warmup)
//! - Memory overhead: ~5MB per module
//! - Fuel consumption: 1 fuel ≈ 1 WASM instruction
//!
//! # Modules
//!
//! - [`rpc`]: RPC protocol types (requests/responses)
//! - [`runtime`]: WASM runtime implementation

pub mod rpc;
pub mod runtime;

pub use rpc::{BrainRequest, BrainResponse, Location, Symbol, SymbolKind};
pub use runtime::{BrainConfig, BrainModule, BrainRuntime, HostState};
