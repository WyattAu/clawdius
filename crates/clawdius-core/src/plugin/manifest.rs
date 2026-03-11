//! Plugin Manifest - Defines plugin metadata and configuration
//!
//! The manifest is a TOML file that describes a plugin's metadata,
//! dependencies, and configuration schema.

use serde::{Deserialize, Serialize};

use super::api::{PluginAuthor, PluginCapabilities, PluginDependency, PluginId, PluginMetadata};

/// Plugin manifest file name
pub const MANIFEST_FILE: &str = "plugin.toml";

/// Plugin manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin ID (name@version format)
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Description
    pub description: String,
    /// Author information
    pub author: ManifestAuthor,
    /// Homepage URL
    #[serde(default)]
    pub homepage: Option<String>,
    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,
    /// License (SPDX identifier)
    pub license: String,
    /// Keywords for discovery
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Minimum Clawdius version
    #[serde(default = "default_min_version")]
    pub min_clawdius_version: String,
    /// Plugin capabilities
    #[serde(default)]
    pub capabilities: ManifestCapabilities,
    /// Hooks to subscribe to
    #[serde(default)]
    pub hooks: Vec<String>,
    /// Configuration schema (JSON Schema)
    #[serde(default)]
    pub config: Option<toml::Value>,
    /// Dependencies
    #[serde(default)]
    pub dependencies: Vec<ManifestDependency>,
    /// WASM module path (relative to manifest)
    #[serde(default = "default_wasm_path")]
    pub wasm: String,
    /// Icon path (relative to manifest)
    #[serde(default)]
    pub icon: Option<String>,
    /// Readme path (relative to manifest)
    #[serde(default = "default_readme_path")]
    pub readme: String,
}

fn default_min_version() -> String {
    "0.1.0".to_string()
}

fn default_wasm_path() -> String {
    "plugin.wasm".to_string()
}

fn default_readme_path() -> String {
    "README.md".to_string()
}

/// Author information in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestAuthor {
    /// Author name
    pub name: String,
    /// Email address
    #[serde(default)]
    pub email: Option<String>,
    /// Website URL
    #[serde(default)]
    pub url: Option<String>,
}

/// Capabilities in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestCapabilities {
    /// Can read files
    #[serde(default = "default_true")]
    pub read_files: bool,
    /// Can write files
    #[serde(default)]
    pub write_files: bool,
    /// Can execute commands
    #[serde(default)]
    pub execute: bool,
    /// Can make network requests
    #[serde(default)]
    pub network: bool,
    /// Can access LLM
    #[serde(default)]
    pub access_llm: bool,
    /// Can access session history
    #[serde(default)]
    pub access_history: bool,
    /// Can modify other plugins
    #[serde(default)]
    pub modify_plugins: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ManifestCapabilities {
    fn default() -> Self {
        Self {
            read_files: true,
            write_files: false,
            execute: false,
            network: false,
            access_llm: false,
            access_history: false,
            modify_plugins: false,
        }
    }
}

/// Dependency in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestDependency {
    /// Plugin name
    pub name: String,
    /// Version requirement (semver)
    pub version: String,
    /// Is optional
    #[serde(default)]
    pub optional: bool,
}

impl PluginManifest {
    /// Parse manifest from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Serialize manifest to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Convert to PluginMetadata
    pub fn to_metadata(&self) -> PluginMetadata {
        let parts: Vec<&str> = self.id.split('@').collect();
        let (name, version) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (self.name.clone(), self.version.clone())
        };

        PluginMetadata {
            id: PluginId::new(&name, &version),
            name: self.name.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            author: PluginAuthor {
                name: self.author.name.clone(),
                email: self.author.email.clone(),
                url: self.author.url.clone(),
            },
            homepage: self.homepage.clone(),
            repository: self.repository.clone(),
            license: self.license.clone(),
            keywords: self.keywords.clone(),
            min_clawdius_version: self.min_clawdius_version.clone(),
            capabilities: PluginCapabilities {
                can_read_files: self.capabilities.read_files,
                can_write_files: self.capabilities.write_files,
                can_execute: self.capabilities.execute,
                can_network: self.capabilities.network,
                can_access_llm: self.capabilities.access_llm,
                can_access_history: self.capabilities.access_history,
                can_modify_plugins: self.capabilities.modify_plugins,
            },
            subscribed_hooks: self.hooks.clone(),
            config_schema: self
                .config
                .as_ref()
                .and_then(|c| serde_json::to_value(c.clone()).ok()),
            dependencies: self
                .dependencies
                .iter()
                .map(|d| PluginDependency {
                    name: d.name.clone(),
                    version: d.version.clone(),
                    optional: d.optional,
                })
                .collect(),
        }
    }

    /// Validate the manifest
    pub fn validate(&self) -> Result<(), ManifestValidationError> {
        // Validate ID format
        if !self.id.contains('@') {
            return Err(ManifestValidationError::InvalidIdFormat {
                id: self.id.clone(),
                reason: "ID must be in format 'name@version'".to_string(),
            });
        }

        // Validate version is semver
        if semver::Version::parse(&self.version).is_err() {
            return Err(ManifestValidationError::InvalidVersion {
                version: self.version.clone(),
            });
        }

        // Validate min_clawdius_version is semver
        if semver::Version::parse(&self.min_clawdius_version).is_err() {
            return Err(ManifestValidationError::InvalidVersion {
                version: self.min_clawdius_version.clone(),
            });
        }

        // Validate hooks
        for hook in &self.hooks {
            if super::hooks::HookType::from_str(hook).is_none() && !hook.starts_with("custom:") {
                return Err(ManifestValidationError::InvalidHook { hook: hook.clone() });
            }
        }

        // Validate dependencies
        for dep in &self.dependencies {
            if semver::VersionReq::parse(&dep.version).is_err() {
                return Err(ManifestValidationError::InvalidDependencyVersion {
                    name: dep.name.clone(),
                    version: dep.version.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Manifest validation errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum ManifestValidationError {
    #[error("Invalid ID format '{id}': {reason}")]
    InvalidIdFormat { id: String, reason: String },

    #[error("Invalid version '{version}'")]
    InvalidVersion { version: String },

    #[error("Invalid hook '{hook}'")]
    InvalidHook { hook: String },

    #[error("Invalid dependency version for '{name}': {version}")]
    InvalidDependencyVersion { name: String, version: String },
}

/// Example manifest for documentation
pub const EXAMPLE_MANIFEST: &str = r#"
id = "my-plugin@1.0.0"
name = "My Plugin"
version = "1.0.0"
description = "A sample plugin for Clawdius"
license = "MIT"

[author]
name = "Developer Name"
email = "dev@example.com"
url = "https://example.com"

homepage = "https://example.com/my-plugin"
repository = "https://github.com/example/my-plugin"
keywords = ["productivity", "automation"]
min_clawdius_version = "0.1.0"

[capabilities]
read_files = true
write_files = true
execute = false
network = true
access_llm = false
access_history = false
modify_plugins = false

hooks = ["before_edit", "after_edit", "on_startup"]

wasm = "plugin.wasm"
icon = "icon.png"
readme = "README.md"

[[dependencies]]
name = "utils-plugin"
version = ">=1.0.0"
optional = false

[config]
type = "object"
properties = { enabled = { type = "boolean", default = true } }
"#;

/// Semver types (simplified for manifest validation)
mod semver {
    #[derive(Debug, Clone)]
    pub struct Version {
        major: u64,
        minor: u64,
        patch: u64,
    }

    impl Version {
        pub fn parse(s: &str) -> Result<Self, ()> {
            let parts: Vec<&str> = s.trim_start_matches('v').split('.').collect();
            if parts.len() != 3 {
                return Err(());
            }
            Ok(Self {
                major: parts[0].parse().map_err(|_| ())?,
                minor: parts[1].parse().map_err(|_| ())?,
                patch: parts[2].parse().map_err(|_| ())?,
            })
        }
    }

    #[derive(Debug, Clone)]
    pub struct VersionReq(String);

    impl VersionReq {
        pub fn parse(s: &str) -> Result<Self, ()> {
            // Simplified validation - just check it's not empty
            if s.is_empty() {
                return Err(());
            }
            Ok(Self(s.to_string()))
        }
    }
}
