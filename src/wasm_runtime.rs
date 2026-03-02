//! WASM Runtime for Brain Component
//!
//! Implements wasmtime integration with security settings per BP-BRAIN-001.
//! Provides sandboxed execution for LLM reasoning logic.

use wasmtime::{AsContext, AsContextMut, Config, Engine, Memory, Module, Store};

/// Default memory limit: 4GB
pub const DEFAULT_MEMORY_LIMIT: usize = 4 * 1024 * 1024 * 1024;
/// Default stack limit: 1MB
pub const DEFAULT_STACK_LIMIT: usize = 1024 * 1024;
/// Default fuel: 1 billion units
pub const DEFAULT_FUEL: u64 = 1_000_000_000;
/// Default timeout: 30 seconds
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Configuration for the WASM runtime
#[derive(Debug, Clone)]
pub struct WasmConfig {
    /// Maximum memory in bytes
    pub memory_limit: usize,
    /// Maximum stack size in bytes
    pub stack_limit: usize,
    /// Fuel for execution limiting
    pub fuel: u64,
    /// Execution timeout in seconds
    pub timeout_secs: u64,
    /// Enable multi-memory support
    pub multi_memory: bool,
    /// Enable 64-bit memory
    pub memory64: bool,
}

impl WasmConfig {
    /// Creates a new WASM configuration with default values
    #[must_use]
    pub fn new() -> Self {
        Self {
            memory_limit: DEFAULT_MEMORY_LIMIT,
            stack_limit: DEFAULT_STACK_LIMIT,
            fuel: DEFAULT_FUEL,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            multi_memory: true,
            memory64: false,
        }
    }

    /// Sets the memory limit
    #[must_use]
    pub fn with_memory_limit(mut self, limit: usize) -> Self {
        self.memory_limit = limit;
        self
    }

    /// Sets the stack limit
    #[must_use]
    pub fn with_stack_limit(mut self, limit: usize) -> Self {
        self.stack_limit = limit;
        self
    }

    /// Sets the fuel amount
    #[must_use]
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// Sets the timeout
    #[must_use]
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a WASM engine with the given configuration
///
/// # Errors
/// Returns an error if engine creation fails
pub fn create_engine(config: &WasmConfig) -> crate::error::Result<Engine> {
    let mut wasm_config = Config::new();

    wasm_config.wasm_multi_memory(config.multi_memory);
    wasm_config.wasm_memory64(config.memory64);
    wasm_config.max_wasm_stack(config.stack_limit);
    wasm_config.consume_fuel(true);

    Engine::new(&wasm_config).map_err(|e| {
        crate::error::BrainError::WasmCompileFailed {
            reason: e.to_string(),
        }
        .into()
    })
}

/// WASM runtime wrapper
pub struct WasmRuntime {
    engine: Engine,
    config: WasmConfig,
}

impl WasmRuntime {
    /// Creates a new WASM runtime
    ///
    /// # Errors
    /// Returns an error if runtime creation fails
    pub fn new(config: WasmConfig) -> crate::error::Result<Self> {
        let engine = create_engine(&config)?;
        Ok(Self { engine, config })
    }

    /// Returns the underlying engine
    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Returns the configuration
    #[must_use]
    pub fn config(&self) -> &WasmConfig {
        &self.config
    }

    /// Compiles a WASM module from bytes
    ///
    /// # Errors
    /// Returns an error if compilation fails
    pub fn compile_module(&self, wasm_bytes: &[u8]) -> crate::error::Result<Module> {
        Module::new(&self.engine, wasm_bytes).map_err(|e| {
            crate::error::BrainError::WasmCompileFailed {
                reason: e.to_string(),
            }
            .into()
        })
    }

    /// Creates a new store with the given data
    #[must_use]
    pub fn create_store<T>(&self, data: T) -> Store<T> {
        Store::new(&self.engine, data)
    }

    /// Creates a new store with fuel configured
    ///
    /// # Errors
    /// Returns an error if fuel configuration fails
    pub fn create_store_with_fuel<T>(&self, data: T) -> crate::error::Result<Store<T>> {
        let mut store = Store::new(&self.engine, data);
        store
            .set_fuel(self.config.fuel)
            .map_err(|e| crate::error::BrainError::WasmTrap {
                message: format!("Failed to set fuel: {e}"),
            })?;
        Ok(store)
    }
}

impl std::fmt::Debug for WasmRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmRuntime")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// WASM export: memory
pub const WASM_EXPORT_MEMORY: &str = "memory";
/// WASM export: `brain_init`
pub const WASM_EXPORT_BRAIN_INIT: &str = "brain_init";
/// WASM export: `brain_invoke`
pub const WASM_EXPORT_BRAIN_INVOKE: &str = "brain_invoke";
/// WASM export: `brain_get_version`
pub const WASM_EXPORT_BRAIN_GET_VERSION: &str = "brain_get_version";
/// WASM export: `brain_shutdown`
pub const WASM_EXPORT_BRAIN_SHUTDOWN: &str = "brain_shutdown";

/// WASM import: `host_log`
pub const WASM_IMPORT_HOST_LOG: &str = "host_log";
/// WASM import: `host_read_file`
pub const WASM_IMPORT_HOST_READ_FILE: &str = "host_read_file";
/// WASM import: `host_llm_call`
pub const WASM_IMPORT_HOST_LLM_CALL: &str = "host_llm_call";
/// WASM import: `host_get_artifact`
pub const WASM_IMPORT_HOST_GET_ARTIFACT: &str = "host_get_artifact";

/// WASM runtime errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmError {
    /// Module not found
    ModuleNotFound,
    /// Export not found
    ExportNotFound,
    /// Import not found
    ImportNotFound,
    /// Memory access error
    MemoryAccess,
    /// Runtime trap
    Trap,
    /// Out of fuel
    OutOfFuel,
    /// Execution timeout
    Timeout,
}

impl std::fmt::Display for WasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModuleNotFound => write!(f, "WASM module not found"),
            Self::ExportNotFound => write!(f, "WASM export not found"),
            Self::ImportNotFound => write!(f, "WASM import not found"),
            Self::MemoryAccess => write!(f, "WASM memory access error"),
            Self::Trap => write!(f, "WASM trap"),
            Self::OutOfFuel => write!(f, "WASM out of fuel"),
            Self::Timeout => write!(f, "WASM timeout"),
        }
    }
}

impl std::error::Error for WasmError {}

/// Reads data from WASM memory
///
/// # Errors
/// Returns an error if memory access fails
pub fn read_wasm_memory(
    memory: &Memory,
    store: &impl AsContext,
    offset: usize,
    len: usize,
) -> crate::error::Result<Vec<u8>> {
    let data = memory
        .data(store)
        .get(offset..offset.saturating_add(len))
        .ok_or_else(|| crate::error::BrainError::MemoryLimitExceeded {
            bytes: offset.saturating_add(len),
        })?;
    Ok(data.to_vec())
}

/// Writes data to WASM memory
///
/// # Errors
/// Returns an error if memory access fails
pub fn write_wasm_memory(
    memory: &mut Memory,
    store: &mut impl AsContextMut,
    offset: usize,
    data: &[u8],
) -> crate::error::Result<()> {
    let memory_data = memory
        .data_mut(store)
        .get_mut(offset..offset.saturating_add(data.len()))
        .ok_or_else(|| crate::error::BrainError::MemoryLimitExceeded {
            bytes: offset.saturating_add(data.len()),
        })?;
    memory_data.copy_from_slice(data);
    Ok(())
}

/// Allocates memory in the WASM module
///
/// # Errors
/// Returns an error if allocation fails
pub fn allocate_wasm_memory(
    memory: &mut Memory,
    store: &mut impl AsContextMut,
    size: usize,
) -> crate::error::Result<i32> {
    let current_size = memory.data_size(&*store);
    let new_pages = (size / 65536) + 1;

    memory
        .grow(
            &mut *store,
            u64::from(u32::try_from(new_pages).unwrap_or(u32::MAX)),
        )
        .map_err(|_| crate::error::BrainError::MemoryLimitExceeded { bytes: size })?;

    Ok(i32::try_from(current_size).unwrap_or(i32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_config_default() {
        let config = WasmConfig::default();
        assert_eq!(config.memory_limit, DEFAULT_MEMORY_LIMIT);
        assert_eq!(config.stack_limit, DEFAULT_STACK_LIMIT);
        assert_eq!(config.fuel, DEFAULT_FUEL);
        assert_eq!(config.timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn test_wasm_config_builder() {
        let config = WasmConfig::new()
            .with_memory_limit(1024 * 1024)
            .with_stack_limit(512 * 1024)
            .with_fuel(100_000)
            .with_timeout(10);

        assert_eq!(config.memory_limit, 1024 * 1024);
        assert_eq!(config.stack_limit, 512 * 1024);
        assert_eq!(config.fuel, 100_000);
        assert_eq!(config.timeout_secs, 10);
    }

    #[test]
    fn test_create_engine() {
        let config = WasmConfig::default();
        let result = create_engine(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wasm_runtime_creation() {
        let config = WasmConfig::default();
        let result = WasmRuntime::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wasm_error_display() {
        assert_eq!(format!("{}", WasmError::Trap), "WASM trap");
        assert_eq!(format!("{}", WasmError::OutOfFuel), "WASM out of fuel");
    }

    #[test]
    fn test_wasm_exports() {
        assert_eq!(WASM_EXPORT_BRAIN_INIT, "brain_init");
        assert_eq!(WASM_EXPORT_BRAIN_INVOKE, "brain_invoke");
        assert_eq!(WASM_EXPORT_BRAIN_GET_VERSION, "brain_get_version");
        assert_eq!(WASM_EXPORT_BRAIN_SHUTDOWN, "brain_shutdown");
    }

    #[test]
    fn test_wasm_imports() {
        assert_eq!(WASM_IMPORT_HOST_LOG, "host_log");
        assert_eq!(WASM_IMPORT_HOST_READ_FILE, "host_read_file");
        assert_eq!(WASM_IMPORT_HOST_LLM_CALL, "host_llm_call");
        assert_eq!(WASM_IMPORT_HOST_GET_ARTIFACT, "host_get_artifact");
    }
}
