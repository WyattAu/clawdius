//! Plugin Loader - Loads plugins from various sources

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::manifest::{PluginManifest, MANIFEST_FILE};

/// Plugin loader - handles loading plugins from different sources
pub struct PluginLoader {
    /// Base directory for plugins
    base_dir: PathBuf,
}

impl PluginLoader {
    /// Create a new plugin loader
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_dir: PathBuf::from(super::host::PLUGINS_DIR),
        }
    }

    /// Create a loader with a custom base directory
    pub fn with_base_dir(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Load a plugin manifest from a directory
    pub fn load_manifest(&self, dir: &Path) -> Result<PluginManifest> {
        let manifest_path = dir.join(MANIFEST_FILE);

        if !manifest_path.exists() {
            anyhow::bail!("Manifest not found: {manifest_path:?}");
        }

        let content =
            std::fs::read_to_string(&manifest_path).context("Failed to read manifest file")?;

        let manifest = PluginManifest::from_toml(&content).context("Failed to parse manifest")?;

        manifest.validate().context("Manifest validation failed")?;

        Ok(manifest)
    }

    /// Load a plugin manifest from a TOML string
    pub fn load_manifest_from_toml(&self, toml: &str) -> Result<PluginManifest> {
        let manifest = PluginManifest::from_toml(toml).context("Failed to parse manifest")?;

        manifest.validate().context("Manifest validation failed")?;

        Ok(manifest)
    }

    /// Validate a plugin directory
    pub fn validate_plugin_dir(&self, dir: &Path) -> Result<PluginValidationResult> {
        let mut result = PluginValidationResult::valid();

        // Check manifest
        match self.load_manifest(dir) {
            Ok(manifest) => {
                result.manifest = Some(manifest);
            }
            Err(e) => {
                result.add_error(format!("Manifest error: {e}"));
            }
        }

        // Check WASM file
        if let Some(ref manifest) = result.manifest {
            let wasm_path = dir.join(&manifest.wasm);
            if wasm_path.exists() {
                // Validate WASM file
                match self.validate_wasm(&wasm_path) {
                    Ok(info) => result.wasm_info = Some(info),
                    Err(e) => result.add_error(format!("WASM validation error: {e}")),
                }
            } else {
                result.add_error(format!("WASM file not found: {wasm_path:?}"));
            }
        }

        // Check size
        let total_size = self.calculate_dir_size(dir)?;
        if total_size > super::MAX_PLUGIN_SIZE {
            result.add_warning(format!(
                "Plugin size ({}) exceeds recommended maximum ({})",
                total_size,
                super::MAX_PLUGIN_SIZE
            ));
        }

        result.valid = result.errors.is_empty();
        Ok(result)
    }

    /// Validate a WASM file
    fn validate_wasm(&self, path: &Path) -> Result<WasmInfo> {
        let bytes = std::fs::read(path)?;

        // Check magic number
        if bytes.len() < 4 || &bytes[0..4] != b"\x00asm" {
            anyhow::bail!("Invalid WASM magic number");
        }

        // Parse version (bytes 4-8)
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        Ok(WasmInfo {
            size: bytes.len(),
            version,
        })
    }

    /// Calculate directory size
    fn calculate_dir_size(&self, dir: &Path) -> Result<usize> {
        let mut total_size = 0;

        for entry in walkdir::WalkDir::new(dir) {
            let entry = entry?;
            if entry.file_type().is_file() {
                total_size += entry.metadata()?.len() as usize;
            }
        }

        Ok(total_size)
    }

    /// Discover all plugin directories
    pub fn discover_plugins(&self) -> Result<Vec<PathBuf>> {
        let mut plugins = Vec::new();

        if !self.base_dir.exists() {
            return Ok(plugins);
        }

        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let manifest_path = path.join(MANIFEST_FILE);
                if manifest_path.exists() {
                    plugins.push(path);
                }
            }
        }

        Ok(plugins)
    }

    /// Create a new plugin directory structure
    pub fn create_plugin_dir(&self, manifest: &PluginManifest) -> Result<PathBuf> {
        let plugin_dir = self.base_dir.join(&manifest.name);
        std::fs::create_dir_all(&plugin_dir)?;

        // Write manifest
        let manifest_path = plugin_dir.join(MANIFEST_FILE);
        std::fs::write(&manifest_path, manifest.to_toml()?)?;

        // Create README if it doesn't exist
        let readme_path = plugin_dir.join(&manifest.readme);
        if !readme_path.exists() {
            std::fs::write(
                &readme_path,
                format!("# {}\n\n{}\n", manifest.name, manifest.description),
            )?;
        }

        Ok(plugin_dir)
    }

    /// Get the base directory
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// WASM file information
#[derive(Debug, Clone)]
pub struct WasmInfo {
    /// File size in bytes
    pub size: usize,
    /// WASM version
    pub version: u32,
}

/// Plugin validation result
#[derive(Debug, Clone)]
pub struct PluginValidationResult {
    /// Whether the plugin is valid
    pub valid: bool,
    /// Validation errors
    pub errors: Vec<String>,
    /// Validation warnings
    pub warnings: Vec<String>,
    /// Parsed manifest (if valid)
    pub manifest: Option<PluginManifest>,
    /// WASM file info (if valid)
    pub wasm_info: Option<WasmInfo>,
}

impl PluginValidationResult {
    fn valid() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            manifest: None,
            wasm_info: None,
        }
    }

    fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// Plugin packer - creates distributable plugin packages
pub struct PluginPacker {
    loader: PluginLoader,
}

impl PluginPacker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            loader: PluginLoader::new(),
        }
    }

    /// Pack a plugin directory into a .cpkg file (simple directory copy)
    /// For production, consider using zip compression
    pub fn pack(&self, plugin_dir: &Path, output: &Path) -> Result<()> {
        let manifest = self.loader.load_manifest(plugin_dir)?;

        // Validate the plugin
        let validation = self.loader.validate_plugin_dir(plugin_dir)?;
        if !validation.valid {
            anyhow::bail!("Cannot pack invalid plugin: {:?}", validation.errors);
        }

        // Simple implementation: copy directory to output location
        // In production, this would use zip compression
        if output.exists() {
            std::fs::remove_dir_all(output)?;
        }
        std::fs::create_dir_all(output)?;

        // Copy all files
        fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let ty = entry.file_type()?;
                if ty.is_dir() {
                    copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
                } else {
                    std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
                }
            }
            Ok(())
        }

        copy_dir_all(plugin_dir, output)?;

        tracing::info!("Packed plugin {} to {:?}", manifest.name, output);
        Ok(())
    }

    /// Unpack a .cpkg file (simple directory copy)
    pub fn unpack(&self, package: &Path, output_dir: &Path) -> Result<PluginManifest> {
        if !package.is_dir() {
            anyhow::bail!("Package must be a directory: {package:?}");
        }

        std::fs::create_dir_all(output_dir)?;

        // Copy all files
        fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let ty = entry.file_type()?;
                if ty.is_dir() {
                    copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
                } else {
                    std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
                }
            }
            Ok(())
        }

        copy_dir_all(package, output_dir)?;

        // Find and load manifest
        for entry in std::fs::read_dir(output_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let manifest = self.loader.load_manifest(&entry.path())?;
                return Ok(manifest);
            }
        }

        // Try loading from root
        self.loader.load_manifest(output_dir)
    }
}

impl Default for PluginPacker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::EXAMPLE_MANIFEST;

    #[test]
    fn test_loader_creation() {
        let loader = PluginLoader::new();
        assert!(loader.base_dir().ends_with("plugins"));
    }

    #[test]
    fn test_manifest_from_toml() {
        let loader = PluginLoader::new();
        let result = loader.load_manifest_from_toml(EXAMPLE_MANIFEST);
        assert!(result.is_ok());
    }
}
