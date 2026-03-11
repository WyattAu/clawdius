//! Plugin System for Clawdius
//!
//! This module provides a WASM-based plugin system that allows extending
//! Clawdius with custom functionality while maintaining security through
//! sandboxing.

pub mod api;
pub mod hooks;
pub mod host;
pub mod loader;
pub mod manifest;
pub mod marketplace;
pub mod registry;
pub mod wasm;

// Re-export specific types to avoid ambiguity
pub use api::{
    HookResult, PluginAuthor, PluginCapabilities, PluginConfig, PluginDependency, PluginId,
    PluginMetadata, PluginState, PluginStats,
};
pub use hooks::{HookContext, HookStats, HookSubscription, HookType};
pub use host::{PluginHost, PluginHostBuilder, PluginHostConfig};
pub use loader::{PluginLoader, PluginPacker, PluginValidationResult, WasmInfo};
pub use manifest::{ManifestValidationError, PluginManifest, MANIFEST_FILE};
pub use marketplace::{
    InstallRequest, InstallResult, MarketplaceCache, MarketplaceCategory, MarketplaceClient,
    MarketplaceConfig, MarketplaceOrder, MarketplacePlugin, MarketplaceSearchQuery,
    MarketplaceSearchResults, MarketplaceSort, MarketplaceVersion, UpdateCheck,
};
pub use registry::PluginRegistry;
pub use wasm::WasmRuntime;

// Re-export the trait as WasmPluginTrait to avoid ambiguity with the struct
pub use api::Plugin as WasmPluginTrait;
// Re-export the struct as PluginEntry
pub use registry::Plugin as PluginEntry;

/// Plugin system version
pub const PLUGIN_API_VERSION: &str = "1.0.0";

/// Maximum plugin WASM module size (10 MB)
pub const MAX_PLUGIN_SIZE: usize = 10 * 1024 * 1024;

/// Maximum number of plugins that can be loaded
pub const MAX_PLUGINS: usize = 100;
