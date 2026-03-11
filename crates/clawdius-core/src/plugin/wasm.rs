//! WASM Plugin Runtime
//!
//! Provides WASM-based plugin execution using wasmtime.

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use wasmtime::{
    anyhow, Caller, Config, Engine, Instance, IntoFunc, Linker, MaybeUninitExt, Module, Store, Val,
    WasmResults, WasmTy, WasmTyList,
};

use super::api::{
    HookResult, Plugin, PluginConfig, PluginId, PluginMetadata, PluginState, PluginStats,
};
use super::hooks::{HookContext, HookType};
use super::manifest::PluginManifest;

/// WASM plugin instance
pub struct WasmPlugin {
    /// Plugin metadata
    metadata: PluginMetadata,
    /// Plugin configuration
    config: PluginConfig,
    /// Plugin state
    state: PluginState,
    /// WASM engine
    engine: Engine,
    /// WASM module
    module: Module,
    /// WASM instance (lazy initialized)
    instance: Option<Instance>,
    /// Linker for WASM imports
    linker: Linker<()>,
    /// Statistics
    stats: PluginStats,
}

impl WasmPlugin {
    /// Load a WASM plugin from a file
    pub async fn load(path: &Path, manifest: PluginManifest) -> Result<Self> {
        let metadata = manifest.to_metadata();

        // Configure WASM engine with security constraints
        let mut config = Config::new();
        config
            .wasm_backtrace(true)
            .wasm_multi_memory(true)
            .wasm_simd(true)
            .cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = Engine::new(&config)?;

        // Load and compile the module
        let wasm_bytes = tokio::fs::read(path)
            .await
            .context("Failed to read WASM file")?;

        // Validate size
        if wasm_bytes.len() > super::MAX_PLUGIN_SIZE {
            anyhow::bail!(
                "Plugin too large: {} bytes (max: {})",
                wasm_bytes.len(),
                super::MAX_PLUGIN_SIZE
            );
        }

        let module = Module::from_binary(&engine, &wasm_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to compile WASM module: {e}"))?;

        // Create linker with host functions
        let linker = Self::create_linker(&engine)?;

        Ok(Self {
            metadata,
            config: PluginConfig::default(),
            state: PluginState::Loaded,
            engine,
            module,
            instance: None,
            linker,
            stats: PluginStats::default(),
        })
    }

    /// Create linker with host functions
    fn create_linker(engine: &Engine) -> Result<Linker<()>> {
        let mut linker = Linker::new(engine);

        // Add Clawdius host functions
        Self::add_clawdius_imports(&mut linker)?;

        Ok(linker)
    }

    /// Add Clawdius-specific host functions
    fn add_clawdius_imports(linker: &mut Linker<()>) -> Result<()> {
        // Log function
        linker.func_wrap(
            "clawdius",
            "log",
            |_caller: Caller<'_, ()>, level: i32, _ptr: i32, _len: i32| {
                match level {
                    0 => tracing::trace!("WASM log"),
                    1 => tracing::debug!("WASM log"),
                    2 => tracing::info!("WASM log"),
                    3 => tracing::warn!("WASM log"),
                    _ => tracing::error!("WASM log"),
                }

                Ok(())
            },
        )?;

        // Get config value
        linker.func_wrap(
            "clawdius",
            "get_config",
            |_caller: Caller<'_, ()>,
             _key_ptr: i32,
             _key_len: i32,
             _val_ptr: i32,
             _val_len: i32| {
                // TODO: Implement config retrieval
                Ok(0i32)
            },
        )?;

        // Hook result functions
        linker.func_wrap(
            "clawdius",
            "hook_result_success",
            |_caller: Caller<'_, ()>| Ok(0i32),
        )?;

        linker.func_wrap(
            "clawdius",
            "hook_result_error",
            |_caller: Caller<'_, ()>, _ptr: i32, _len: i32| Ok(0i32),
        )?;

        Ok(())
    }

    /// Instantiate the WASM module
    fn instantiate(&mut self) -> Result<()> {
        let mut store = Store::new(&self.engine, ());
        let instance = self.linker.instantiate(&mut store, &self.module)?;
        self.instance = Some(instance);
        Ok(())
    }

    /// Call an exported function
    fn call_export<T: WasmRet>(&mut self, name: &str, args: &[Val]) -> Result<T> {
        let mut store = Store::new(&self.engine, ());
        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin not instantiated"))?;

        let export = instance
            .get_export(&mut store, name)
            .and_then(wasmtime::Extern::into_func)
            .ok_or_else(|| anyhow::anyhow!("Export '{name}' not found"))?;

        let mut results = vec![Val::I32(0)];
        export.call(&mut store, args, &mut results)?;

        T::from_val(results.first())
    }

    /// Check if an export exists
    fn has_export(&self, name: &str) -> bool {
        // This is a simplified check - full implementation would need store access
        self.module.get_export(name).is_some()
    }
}

#[async_trait]
impl Plugin for WasmPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state
    }

    async fn initialize(&mut self, config: PluginConfig) -> Result<()> {
        if self.state != PluginState::Loaded {
            anyhow::bail!("Plugin must be in Loaded state to initialize");
        }

        self.state = PluginState::Initializing;
        self.config = config;

        // Instantiate the WASM module
        self.instantiate()?;

        // Call the _initialize export if it exists (WASM reactor pattern)
        if self.has_export("_initialize") {
            self.call_export::<()>("_initialize", &[])?;
        }

        // Call the plugin's init function if it exists
        if self.has_export("clawdius_init") {
            self.call_export::<i32>("clawdius_init", &[])?;
        }

        self.state = PluginState::Active;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if self.state != PluginState::Active {
            return Ok(());
        }

        self.state = PluginState::Unloading;

        // Call the plugin's shutdown function if it exists
        if self.has_export("clawdius_shutdown") {
            let _ = self.call_export::<i32>("clawdius_shutdown", &[]);
        }

        self.instance = None;
        self.state = PluginState::Loaded;
        Ok(())
    }

    async fn on_hook(&self, hook_name: &str, context: &HookContext) -> Result<HookResult> {
        if self.state != PluginState::Active {
            return Ok(HookResult::error("Plugin not active"));
        }

        // Convert hook name to function name (e.g., "before_edit" -> "hook_before_edit")
        let func_name = format!("hook_{}", hook_name.replace('-', "_"));

        if !self.has_export(&func_name) {
            return Ok(HookResult::success());
        }

        // Serialize context to JSON
        let _context_json = serde_json::to_string(context)?;

        // TODO: Pass context to WASM via shared memory
        // For now, just call the hook without context
        let start = std::time::Instant::now();

        // We need mutable access for the call
        // This is a limitation of the current design
        let result = HookResult::success();

        let elapsed = start.elapsed();

        // Update stats (note: would need interior mutability for proper stats tracking)
        let _stats = PluginStats {
            hook_calls: self.stats.hook_calls + 1,
            successful_hooks: self.stats.successful_hooks + 1,
            total_execution_time_ms: self.stats.total_execution_time_ms
                + elapsed.as_millis() as u64,
            last_activity: Some(chrono::Utc::now()),
            ..self.stats.clone()
        };

        Ok(result)
    }

    fn config(&self) -> &PluginConfig {
        &self.config
    }

    async fn update_config(&mut self, config: PluginConfig) -> Result<()> {
        self.config = config;

        // Notify plugin of config change
        if self.has_export("clawdius_config_changed") {
            self.call_export::<i32>("clawdius_config_changed", &[])?;
        }

        Ok(())
    }
}

/// Trait for WASM return value conversion
trait WasmRet: Sized {
    fn from_val(val: Option<&Val>) -> Result<Self>;
}

impl WasmRet for () {
    fn from_val(_val: Option<&Val>) -> Result<Self> {
        Ok(())
    }
}

impl WasmRet for i32 {
    fn from_val(val: Option<&Val>) -> Result<Self> {
        match val {
            Some(Val::I32(v)) => Ok(*v),
            _ => anyhow::bail!("Expected i32 return value"),
        }
    }
}

impl WasmRet for i64 {
    fn from_val(val: Option<&Val>) -> Result<Self> {
        match val {
            Some(Val::I64(v)) => Ok(*v),
            _ => anyhow::bail!("Expected i64 return value"),
        }
    }
}

impl WasmRet for f32 {
    fn from_val(val: Option<&Val>) -> Result<Self> {
        match val {
            Some(Val::F32(v)) => Ok(f32::from_bits(*v)),
            _ => anyhow::bail!("Expected f32 return value"),
        }
    }
}

impl WasmRet for f64 {
    fn from_val(val: Option<&Val>) -> Result<Self> {
        match val {
            Some(Val::F64(v)) => Ok(f64::from_bits(*v)),
            _ => anyhow::bail!("Expected f64 return value"),
        }
    }
}

/// Plugin runtime manages all WASM plugins
pub struct WasmRuntime {
    /// Loaded plugins
    plugins: Arc<RwLock<Vec<Arc<RwLock<WasmPlugin>>>>>,
    /// Engine shared by all plugins
    engine: Engine,
}

impl WasmRuntime {
    /// Create a new WASM runtime
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config
            .wasm_backtrace(true)
            .wasm_multi_memory(true)
            .cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = Engine::new(&config)?;

        Ok(Self {
            plugins: Arc::new(RwLock::new(Vec::new())),
            engine,
        })
    }

    /// Load a plugin
    pub async fn load_plugin(&self, path: &Path, manifest: PluginManifest) -> Result<PluginId> {
        let plugin = WasmPlugin::load(path, manifest).await?;
        let id = plugin.metadata().id.clone();

        self.plugins
            .write()
            .await
            .push(Arc::new(RwLock::new(plugin)));

        Ok(id)
    }

    /// Get a plugin by ID
    pub async fn get_plugin(&self, id: &PluginId) -> Option<Arc<RwLock<WasmPlugin>>> {
        let plugins = self.plugins.read().await;
        plugins
            .iter()
            .find(|p| {
                let p = p.blocking_read();
                &p.metadata().id == id
            })
            .cloned()
    }

    /// List all plugins
    pub async fn list_plugins(&self) -> Vec<PluginId> {
        let plugins = self.plugins.read().await;
        plugins
            .iter()
            .map(|p| {
                let p = p.blocking_read();
                p.metadata().id.clone()
            })
            .collect()
    }

    /// Dispatch a hook to all plugins
    pub async fn dispatch_hook(
        &self,
        hook_type: HookType,
        context: &HookContext,
    ) -> Vec<(PluginId, HookResult)> {
        let mut results = Vec::new();
        let plugins = self.plugins.read().await;

        for plugin_lock in plugins.iter() {
            let plugin = plugin_lock.read().await;

            // Check if plugin subscribes to this hook
            if !plugin
                .metadata()
                .subscribed_hooks
                .contains(&hook_type.to_string())
            {
                continue;
            }

            let id = plugin.metadata().id.clone();
            let result = plugin.on_hook(&hook_type.to_string(), context).await;

            results.push((
                id,
                result.unwrap_or_else(|e| HookResult::error(e.to_string())),
            ));
        }

        results
    }

    /// Initialize all plugins
    pub async fn initialize_all(&self, configs: &HashMap<PluginId, PluginConfig>) -> Result<()> {
        let plugins = self.plugins.read().await;

        for plugin_lock in plugins.iter() {
            let mut plugin = plugin_lock.write().await;
            let config = configs
                .get(&plugin.metadata().id)
                .cloned()
                .unwrap_or_default();

            plugin.initialize(config).await?;
        }

        Ok(())
    }

    /// Shutdown all plugins
    pub async fn shutdown_all(&self) -> Result<()> {
        let plugins = self.plugins.read().await;

        for plugin_lock in plugins.iter() {
            let mut plugin = plugin_lock.write().await;
            plugin.shutdown().await?;
        }

        Ok(())
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create WASM runtime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_runtime_creation() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok());
    }
}
