use std::path::{Path, PathBuf};

/// Context provided to a plugin during initialization.
///
/// Contains filesystem paths the plugin can use for configuration,
/// persistent data, and discovering its own location.
pub struct PluginContext {
    /// Absolute path to the directory containing the plugin binary or manifest.
    pub plugin_dir: PathBuf,
    /// Directory where the plugin may store configuration files.
    config_dir: PathBuf,
    /// Directory where the plugin may store persistent data.
    data_dir: PathBuf,
}

impl PluginContext {
    /// Creates a new `PluginContext` with the given plugin directory.
    ///
    /// The config and data directories are derived from the plugin directory.
    #[must_use]
    pub fn new(plugin_dir: PathBuf) -> Self {
        let config_dir = plugin_dir.join("config");
        let data_dir = plugin_dir.join("data");
        Self {
            plugin_dir,
            config_dir,
            data_dir,
        }
    }

    /// Returns the path to the plugin's configuration directory.
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Returns the path to the plugin's data directory.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}
