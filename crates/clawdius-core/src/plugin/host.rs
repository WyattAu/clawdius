//! Plugin Host - Manages plugin lifecycle and execution
//!
//! The host is responsible for loading, initializing, and managing plugins.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::api::{HookResult, Plugin, PluginConfig, PluginId, PluginStats};
use super::hooks::{HookContext, HookType};
use super::manifest::PluginManifest;
use super::marketplace::{MarketplaceClient, MarketplaceConfig};
use super::registry::PluginRegistry;
use super::wasm::WasmRuntime;

/// Plugin directory name
pub const PLUGINS_DIR: &str = "plugins";

/// Plugin host configuration
#[derive(Debug, Clone)]
pub struct PluginHostConfig {
    /// Directory for plugin storage
    pub plugins_dir: PathBuf,
    /// Maximum number of plugins
    pub max_plugins: usize,
    /// Whether to auto-load plugins on startup
    pub auto_load: bool,
    /// Marketplace configuration
    pub marketplace: MarketplaceConfig,
}

impl Default for PluginHostConfig {
    fn default() -> Self {
        Self {
            plugins_dir: PathBuf::from(PLUGINS_DIR),
            max_plugins: super::MAX_PLUGINS,
            auto_load: true,
            marketplace: MarketplaceConfig::default(),
        }
    }
}

/// Plugin host - manages the plugin system
pub struct PluginHost {
    /// Configuration
    config: PluginHostConfig,
    /// Plugin registry
    registry: PluginRegistry,
    /// WASM runtime
    runtime: Arc<WasmRuntime>,
    /// Marketplace client
    marketplace: MarketplaceClient,
    /// Plugin configurations
    configs: HashMap<PluginId, PluginConfig>,
    /// Initialized flag
    initialized: bool,
}

impl PluginHost {
    /// Create a new plugin host
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(PluginHostConfig::default())
    }

    /// Create a plugin host with custom configuration
    #[must_use]
    pub fn with_config(config: PluginHostConfig) -> Self {
        let runtime = WasmRuntime::new().expect("Failed to create WASM runtime");
        let marketplace = MarketplaceClient::new(config.marketplace.clone());

        Self {
            config,
            registry: PluginRegistry::new(),
            runtime: Arc::new(runtime),
            marketplace,
            configs: HashMap::new(),
            initialized: false,
        }
    }

    /// Get the plugin registry
    #[must_use]
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Get the plugin registry (mutable)
    pub fn registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.registry
    }

    /// Get the WASM runtime
    #[must_use]
    pub fn runtime(&self) -> &Arc<WasmRuntime> {
        &self.runtime
    }

    /// Initialize the plugin host
    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Ensure plugins directory exists
        tokio::fs::create_dir_all(&self.config.plugins_dir).await?;

        // Auto-load plugins if configured
        if self.config.auto_load {
            self.load_all_plugins().await?;
        }

        self.initialized = true;
        Ok(())
    }

    /// Shutdown the plugin host
    pub async fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        self.runtime.shutdown_all().await?;
        self.initialized = false;
        Ok(())
    }

    /// Load all plugins from the plugins directory
    async fn load_all_plugins(&mut self) -> Result<()> {
        let mut dir = tokio::fs::read_dir(&self.config.plugins_dir).await?;

        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                // Look for plugin.toml manifest
                let manifest_path = path.join(super::manifest::MANIFEST_FILE);

                if manifest_path.exists() {
                    if let Err(e) = self.load_plugin_from_dir(&path).await {
                        tracing::warn!("Failed to load plugin from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a plugin from a directory
    async fn load_plugin_from_dir(&mut self, dir: &PathBuf) -> Result<PluginId> {
        // Read manifest
        let manifest_path = dir.join(super::manifest::MANIFEST_FILE);
        let manifest_content = tokio::fs::read_to_string(&manifest_path).await?;
        let manifest = PluginManifest::from_toml(&manifest_content)?;

        // Validate manifest
        manifest.validate()?;

        // Get WASM path
        let wasm_path = dir.join(&manifest.wasm);
        if !wasm_path.exists() {
            anyhow::bail!("WASM file not found: {wasm_path:?}");
        }

        // Load into runtime
        let plugin_id = self
            .runtime
            .load_plugin(&wasm_path, manifest.clone())
            .await?;

        // Register in registry
        self.registry.register(super::registry::Plugin {
            metadata: manifest.to_metadata(),
            path: dir.clone(),
            enabled: true,
        })?;

        Ok(plugin_id)
    }

    /// Install a plugin from the marketplace
    pub async fn install_plugin(
        &mut self,
        plugin_name: &str,
        version: Option<&str>,
    ) -> Result<PluginId> {
        let request = super::marketplace::InstallRequest {
            plugin: plugin_name.to_string(),
            version: version.map(std::string::ToString::to_string),
            allow_prerelease: false,
            force: false,
            skip_dependencies: false,
        };

        let result = self.marketplace.install(request).await?;

        // Download and extract the plugin
        let plugin_dir = self.config.plugins_dir.join(&result.manifest.name);
        tokio::fs::create_dir_all(&plugin_dir).await?;

        // Write manifest
        let manifest_path = plugin_dir.join(super::manifest::MANIFEST_FILE);
        tokio::fs::write(&manifest_path, result.manifest.to_toml()?).await?;

        // Download WASM module
        // TODO: Implement actual download from result.download_url
        tracing::info!("Plugin {} installed to {:?}", plugin_name, plugin_dir);

        // Load the plugin
        self.load_plugin_from_dir(&plugin_dir).await
    }

    /// Uninstall a plugin
    pub async fn uninstall_plugin(&mut self, plugin_id: &PluginId) -> Result<()> {
        // Get plugin info from registry
        let plugin = self
            .registry
            .get(plugin_id.as_str())
            .ok_or_else(|| anyhow::anyhow!("Plugin not found: {plugin_id}"))?;

        // Remove plugin directory
        tokio::fs::remove_dir_all(&plugin.path).await?;

        // Unregister
        self.registry.unregister(plugin_id.as_str())?;

        Ok(())
    }

    /// Enable a plugin
    pub async fn enable_plugin(&mut self, plugin_id: &PluginId) -> Result<()> {
        self.registry.set_enabled(plugin_id.as_str(), true)?;
        Ok(())
    }

    /// Disable a plugin
    pub async fn disable_plugin(&mut self, plugin_id: &PluginId) -> Result<()> {
        self.registry.set_enabled(plugin_id.as_str(), false)?;
        Ok(())
    }

    /// Update plugin configuration
    pub async fn configure_plugin(
        &mut self,
        plugin_id: &PluginId,
        config: PluginConfig,
    ) -> Result<()> {
        self.configs.insert(plugin_id.clone(), config);
        Ok(())
    }

    /// Dispatch a hook to all plugins
    pub async fn dispatch_hook(
        &self,
        hook_type: HookType,
        context: &HookContext,
    ) -> Vec<(PluginId, HookResult)> {
        self.runtime.dispatch_hook(hook_type, context).await
    }

    /// Check for plugin updates
    pub async fn check_updates(&mut self) -> Result<Vec<super::marketplace::UpdateCheck>> {
        let installed: Vec<String> = self
            .registry
            .list()
            .iter()
            .map(|p| p.metadata.id.as_str().to_string())
            .collect();

        self.marketplace.check_updates(&installed).await
    }

    /// Search the marketplace
    pub async fn search_marketplace(
        &mut self,
        query: &str,
    ) -> Result<super::marketplace::MarketplaceSearchResults> {
        let search_query = super::marketplace::MarketplaceSearchQuery {
            query: query.to_string(),
            ..Default::default()
        };

        self.marketplace.search(search_query).await
    }

    /// Get marketplace client
    #[must_use]
    pub fn marketplace(&self) -> &MarketplaceClient {
        &self.marketplace
    }

    /// Get marketplace client (mutable)
    pub fn marketplace_mut(&mut self) -> &mut MarketplaceClient {
        &mut self.marketplace
    }

    /// Reload all plugins
    pub async fn reload_all(&mut self) -> Result<()> {
        self.shutdown().await?;
        self.registry.clear();
        self.initialize().await
    }

    /// Get plugin statistics
    pub async fn get_stats(&self, _plugin_id: &PluginId) -> Option<PluginStats> {
        // TODO: Get actual stats from runtime
        None
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin host builder for custom configuration
pub struct PluginHostBuilder {
    config: PluginHostConfig,
}

impl PluginHostBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: PluginHostConfig::default(),
        }
    }

    pub fn plugins_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.plugins_dir = path.into();
        self
    }

    #[must_use]
    pub fn max_plugins(mut self, max: usize) -> Self {
        self.config.max_plugins = max;
        self
    }

    #[must_use]
    pub fn auto_load(mut self, auto_load: bool) -> Self {
        self.config.auto_load = auto_load;
        self
    }

    pub fn marketplace_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.marketplace.endpoint = endpoint.into();
        self
    }

    #[must_use]
    pub fn build(self) -> PluginHost {
        PluginHost::with_config(self.config)
    }
}

impl Default for PluginHostBuilder {
    fn default() -> Self {
        Self::new()
    }
}
