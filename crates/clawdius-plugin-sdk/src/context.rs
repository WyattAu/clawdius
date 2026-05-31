use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Context provided to a plugin during initialization.
///
/// Contains filesystem paths the plugin can use for configuration,
/// persistent data, and discovering its own location.
#[derive(Debug)]
pub struct PluginContext {
    /// Absolute path to the directory containing the plugin binary or manifest.
    pub plugin_dir: PathBuf,
    config_dir: PathBuf,
    data_dir: PathBuf,
    workspace_path: Option<PathBuf>,
    config: serde_json::Value,
    session_metadata: HashMap<String, String>,
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
            workspace_path: None,
            config: serde_json::Value::Object(serde_json::Map::new()),
            session_metadata: HashMap::new(),
        }
    }

    /// Creates a `PluginContext` with all fields specified.
    #[must_use]
    pub fn builder(plugin_dir: PathBuf) -> PluginContextBuilder {
        PluginContextBuilder {
            inner: Self::new(plugin_dir),
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

    /// Returns the workspace path if set.
    #[must_use]
    pub fn workspace_path(&self) -> Option<&Path> {
        self.workspace_path.as_deref()
    }

    /// Returns a reference to the plugin's configuration as a JSON value.
    #[must_use]
    pub const fn config(&self) -> &serde_json::Value {
        &self.config
    }

    /// Returns a reference to the session metadata map.
    #[must_use]
    pub const fn session_metadata(&self) -> &HashMap<String, String> {
        &self.session_metadata
    }

    /// Retrieves a session metadata value by key.
    #[must_use]
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.session_metadata.get(key).map(String::as_str)
    }

    /// Inserts a session metadata key-value pair.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.session_metadata.insert(key.into(), value.into());
    }
}

/// Builder for constructing a [`PluginContext`] with custom fields.
#[derive(Debug)]
pub struct PluginContextBuilder {
    inner: PluginContext,
}

impl PluginContextBuilder {
    /// Sets the workspace path.
    #[must_use]
    pub fn workspace_path(mut self, path: PathBuf) -> Self {
        self.inner.workspace_path = Some(path);
        self
    }

    /// Sets the plugin configuration as a JSON value.
    #[must_use]
    pub fn config(mut self, config: serde_json::Value) -> Self {
        self.inner.config = config;
        self
    }

    /// Inserts a session metadata entry.
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.session_metadata.insert(key.into(), value.into());
        self
    }

    /// Consumes the builder and returns the constructed [`PluginContext`].
    #[must_use]
    pub fn build(self) -> PluginContext {
        self.inner
    }
}
