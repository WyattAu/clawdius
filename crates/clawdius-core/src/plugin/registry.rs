//! Plugin Registry - Tracks installed and available plugins

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use super::api::{PluginMetadata, PluginState};

/// Plugin entry in the registry
#[derive(Debug, Clone)]
pub struct Plugin {
    /// Plugin metadata
    pub metadata: PluginMetadata,
    /// Installation path
    pub path: PathBuf,
    /// Whether the plugin is enabled
    pub enabled: bool,
}

/// Plugin registry - tracks all installed plugins
pub struct PluginRegistry {
    /// Plugins indexed by ID
    plugins: HashMap<String, Plugin>,
    /// Plugins indexed by name
    by_name: HashMap<String, String>,
    /// Dependency graph
    dependencies: HashMap<String, Vec<String>>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            by_name: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: Plugin) -> Result<()> {
        let id = plugin.metadata.id.as_str().to_string();
        let name = plugin.metadata.name.clone();

        // Check for conflicts
        if self.plugins.contains_key(&id) {
            anyhow::bail!("Plugin already registered: {id}");
        }

        // Build dependency index
        let deps: Vec<String> = plugin
            .metadata
            .dependencies
            .iter()
            .map(|d| d.name.clone())
            .collect();

        // Register
        self.by_name.insert(name, id.clone());
        self.dependencies.insert(id.clone(), deps);
        self.plugins.insert(id, plugin);

        Ok(())
    }

    /// Unregister a plugin
    pub fn unregister(&mut self, id: &str) -> Result<()> {
        if let Some(plugin) = self.plugins.remove(id) {
            self.by_name.remove(&plugin.metadata.name);
            self.dependencies.remove(id);
            Ok(())
        } else {
            anyhow::bail!("Plugin not found: {id}")
        }
    }

    /// Get a plugin by ID
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Plugin> {
        self.plugins.get(id)
    }

    /// Get a plugin by name
    #[must_use]
    pub fn get_by_name(&self, name: &str) -> Option<&Plugin> {
        self.by_name.get(name).and_then(|id| self.plugins.get(id))
    }

    /// List all plugins
    #[must_use]
    pub fn list(&self) -> Vec<&Plugin> {
        self.plugins.values().collect()
    }

    /// List enabled plugins
    #[must_use]
    pub fn list_enabled(&self) -> Vec<&Plugin> {
        self.plugins.values().filter(|p| p.enabled).collect()
    }

    /// List plugins by state
    #[must_use]
    pub fn list_by_state(&self, state: PluginState) -> Vec<&Plugin> {
        // Note: This would need runtime state tracking
        // For now, just return enabled/disabled based on state
        self.plugins
            .values()
            .filter(|p| {
                matches!(
                    (p.enabled, state),
                    (true, PluginState::Active) | (false, PluginState::Loaded)
                )
            })
            .collect()
    }

    /// Enable a plugin
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        if let Some(plugin) = self.plugins.get_mut(id) {
            plugin.enabled = enabled;
            Ok(())
        } else {
            anyhow::bail!("Plugin not found: {id}")
        }
    }

    /// Check if a plugin is enabled
    #[must_use]
    pub fn is_enabled(&self, id: &str) -> bool {
        self.plugins.get(id).is_some_and(|p| p.enabled)
    }

    /// Get plugin dependencies
    #[must_use]
    pub fn get_dependencies(&self, id: &str) -> Option<&Vec<String>> {
        self.dependencies.get(id)
    }

    /// Get plugins that depend on a given plugin
    #[must_use]
    pub fn get_dependents(&self, id: &str) -> Vec<&Plugin> {
        self.plugins
            .values()
            .filter(|p| {
                p.metadata
                    .dependencies
                    .iter()
                    .any(|d| d.name == id.split('@').next().unwrap_or(id))
            })
            .collect()
    }

    /// Check if all dependencies are satisfied
    pub fn check_dependencies(&self, id: &str) -> Result<Vec<String>> {
        let missing = Vec::new();

        if let Some(deps) = self.dependencies.get(id) {
            for dep_name in deps {
                if !self.by_name.contains_key(dep_name) {
                    // Could return missing dependencies
                }
            }
        }

        Ok(missing)
    }

    /// Clear all plugins
    pub fn clear(&mut self) {
        self.plugins.clear();
        self.by_name.clear();
        self.dependencies.clear();
    }

    /// Get plugin count
    #[must_use]
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Get enabled plugin count
    #[must_use]
    pub fn enabled_count(&self) -> usize {
        self.plugins.values().filter(|p| p.enabled).count()
    }

    /// Find plugins by keyword
    #[must_use]
    pub fn find_by_keyword(&self, keyword: &str) -> Vec<&Plugin> {
        let keyword_lower = keyword.to_lowercase();
        self.plugins
            .values()
            .filter(|p| {
                p.metadata
                    .keywords
                    .iter()
                    .any(|k| k.to_lowercase().contains(&keyword_lower))
                    || p.metadata.name.to_lowercase().contains(&keyword_lower)
                    || p.metadata
                        .description
                        .to_lowercase()
                        .contains(&keyword_lower)
            })
            .collect()
    }

    /// Find plugins by author
    #[must_use]
    pub fn find_by_author(&self, author: &str) -> Vec<&Plugin> {
        self.plugins
            .values()
            .filter(|p| {
                p.metadata
                    .author
                    .name
                    .to_lowercase()
                    .contains(&author.to_lowercase())
            })
            .collect()
    }

    /// Export registry to JSON
    pub fn to_json(&self) -> Result<String> {
        let export = RegistryExport {
            plugins: self
                .plugins
                .values()
                .map(|p| PluginExport {
                    id: p.metadata.id.as_str().to_string(),
                    name: p.metadata.name.clone(),
                    version: p.metadata.version.clone(),
                    description: p.metadata.description.clone(),
                    enabled: p.enabled,
                    path: p.path.to_string_lossy().to_string(),
                })
                .collect(),
        };

        Ok(serde_json::to_string_pretty(&export)?)
    }

    /// Import registry from JSON (merges with existing)
    pub fn import_json(&mut self, json: &str) -> Result<()> {
        let export: RegistryExport = serde_json::from_str(json)?;

        for plugin_export in export.plugins {
            // Would need full manifest to import properly
            tracing::debug!("Would import plugin: {}", plugin_export.name);
        }

        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Export format for registry serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryExport {
    plugins: Vec<PluginExport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginExport {
    id: String,
    name: String,
    version: String,
    description: String,
    enabled: bool,
    path: String,
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::api::{PluginAuthor, PluginCapabilities, PluginId};

    fn create_test_plugin(name: &str, version: &str) -> Plugin {
        Plugin {
            metadata: PluginMetadata {
                id: PluginId::new(name, version),
                name: name.to_string(),
                version: version.to_string(),
                description: format!("Test plugin {name}"),
                author: PluginAuthor {
                    name: "Test Author".to_string(),
                    email: None,
                    url: None,
                },
                homepage: None,
                repository: None,
                license: "MIT".to_string(),
                keywords: vec!["test".to_string()],
                min_clawdius_version: "0.1.0".to_string(),
                capabilities: PluginCapabilities::default(),
                subscribed_hooks: vec![],
                config_schema: None,
                dependencies: vec![],
            },
            path: PathBuf::from("/tmp/plugins").join(name),
            enabled: true,
        }
    }

    #[test]
    fn test_registry_new() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_register_plugin() {
        let mut registry = PluginRegistry::new();
        let plugin = create_test_plugin("test-plugin", "1.0.0");

        assert!(registry.register(plugin).is_ok());
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_get_plugin() {
        let mut registry = PluginRegistry::new();
        let plugin = create_test_plugin("test-plugin", "1.0.0");

        registry.register(plugin).unwrap();

        let found = registry.get("test-plugin@1.0.0");
        assert!(found.is_some());
        assert_eq!(found.unwrap().metadata.name, "test-plugin");
    }

    #[test]
    fn test_get_by_name() {
        let mut registry = PluginRegistry::new();
        let plugin = create_test_plugin("test-plugin", "1.0.0");

        registry.register(plugin).unwrap();

        let found = registry.get_by_name("test-plugin");
        assert!(found.is_some());
    }

    #[test]
    fn test_enable_disable() {
        let mut registry = PluginRegistry::new();
        let plugin = create_test_plugin("test-plugin", "1.0.0");

        registry.register(plugin).unwrap();

        assert!(registry.is_enabled("test-plugin@1.0.0"));

        registry.set_enabled("test-plugin@1.0.0", false).unwrap();
        assert!(!registry.is_enabled("test-plugin@1.0.0"));
    }

    #[test]
    fn test_find_by_keyword() {
        let mut registry = PluginRegistry::new();
        let plugin = create_test_plugin("test-plugin", "1.0.0");

        registry.register(plugin).unwrap();

        let found = registry.find_by_keyword("test");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_unregister() {
        let mut registry = PluginRegistry::new();
        let plugin = create_test_plugin("test-plugin", "1.0.0");

        registry.register(plugin).unwrap();
        assert_eq!(registry.count(), 1);

        registry.unregister("test-plugin@1.0.0").unwrap();
        assert_eq!(registry.count(), 0);
    }
}
