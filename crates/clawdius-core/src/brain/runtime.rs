//! WASM Brain Runtime implementation

use wasmtime::{Caller, Config, Engine, Linker, Module, Store};

use crate::{Error, Result};

use super::rpc::{BrainRequest, BrainResponse};

#[derive(Debug, Clone)]
pub struct BrainConfig {
    pub fuel_limit: u64,
}

impl Default for BrainConfig {
    fn default() -> Self {
        Self {
            fuel_limit: 1_000_000,
        }
    }
}

pub struct HostState {
    pub fuel_used: u64,
}

impl HostState {
    #[must_use]
    pub fn new(_fuel_limit: u64) -> Self {
        Self { fuel_used: 0 }
    }
}

pub struct BrainModule {
    pub module: Module,
}

pub struct BrainRuntime {
    engine: Engine,
    linker: Linker<HostState>,
    fuel_limit: u64,
}

impl BrainRuntime {
    pub fn new(config: BrainConfig) -> Result<Self> {
        let mut wasm_config = Config::new();
        wasm_config.consume_fuel(true);

        let engine = Engine::new(&wasm_config).map_err(|e| Error::Brain(e.to_string()))?;

        let linker = Linker::new(&engine);

        Ok(Self {
            engine,
            linker,
            fuel_limit: config.fuel_limit,
        })
    }

    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<BrainModule> {
        let module = Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| Error::Brain(e.to_string()))?;
        Ok(BrainModule { module })
    }

    pub fn execute(&self, module: &BrainModule, input: &str) -> Result<String> {
        let mut store = Store::new(&self.engine, HostState::new(self.fuel_limit));
        store
            .set_fuel(self.fuel_limit)
            .map_err(|e| Error::Brain(e.to_string()))?;

        let mut linker = self.linker.clone();

        linker
            .func_wrap(
                "env",
                "log",
                |_caller: Caller<'_, HostState>, _ptr: i32, _len: i32| Ok(0),
            )
            .map_err(|e| Error::Brain(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &module.module)
            .map_err(|e| Error::Brain(e.to_string()))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| Error::Brain("memory export not found".to_string()))?;

        let alloc_func = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|e| Error::Brain(format!("'alloc' function not found: {e}")))?;

        let input_bytes = input.as_bytes();
        let input_ptr = alloc_func
            .call(&mut store, input_bytes.len() as i32)
            .map_err(|e| Error::Brain(e.to_string()))?;

        memory
            .write(&mut store, input_ptr as usize, input_bytes)
            .map_err(|e| Error::Brain(format!("Failed to write to memory: {e}")))?;

        let run = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "run")
            .map_err(|e| Error::Brain(format!("'run' function not found: {e}")))?;

        let result_ptr = run
            .call(&mut store, (input_ptr, input_bytes.len() as i32))
            .map_err(|e| Error::Brain(e.to_string()))?;

        let mut len_bytes = [0u8; 4];
        memory
            .read(&mut store, result_ptr as usize, &mut len_bytes)
            .map_err(|e| Error::Brain(format!("Failed to read length: {e}")))?;
        let result_len = i32::from_le_bytes(len_bytes) as usize;

        let mut output_bytes = vec![0u8; result_len];
        memory
            .read(&mut store, (result_ptr + 4) as usize, &mut output_bytes)
            .map_err(|e| Error::Brain(format!("Failed to read output: {e}")))?;

        String::from_utf8(output_bytes)
            .map_err(|e| Error::Brain(format!("Invalid UTF-8 output: {e}")))
    }

    pub fn execute_rpc(
        &self,
        module: &BrainModule,
        request: BrainRequest,
    ) -> Result<BrainResponse> {
        let input = serde_json::to_string(&request)?;
        let output = self.execute(module, &input)?;
        serde_json::from_str(&output)
            .map_err(|e| Error::Brain(format!("Failed to parse response: {e}")))
    }

    #[must_use]
    pub fn fuel_limit(&self) -> u64 {
        self.fuel_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_creation() {
        let config = BrainConfig::default();
        let runtime = BrainRuntime::new(config);
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_custom_fuel_limit() {
        let config = BrainConfig {
            fuel_limit: 500_000,
        };
        let runtime = BrainRuntime::new(config).unwrap();
        assert_eq!(runtime.fuel_limit(), 500_000);
    }

    #[test]
    fn test_host_state_creation() {
        let state = HostState::new(1_000_000);
        assert_eq!(state.fuel_used, 0);
    }

    #[test]
    fn test_load_invalid_module() {
        let runtime = BrainRuntime::new(BrainConfig::default()).unwrap();
        let result = runtime.load_module(b"invalid wasm");
        assert!(result.is_err());
    }
}
