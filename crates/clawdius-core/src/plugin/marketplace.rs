//! Plugin Marketplace - Discovery and installation of plugins
//!
//! This module provides marketplace integration for discovering,
//! installing, and updating plugins from remote repositories.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::manifest::PluginManifest;

/// Marketplace endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceConfig {
    /// Marketplace API endpoint URL
    pub endpoint: String,
    /// API key for authentication (if required)
    #[serde(default)]
    pub api_key: Option<String>,
    /// Cache duration in seconds
    #[serde(default = "default_cache_duration")]
    pub cache_duration_secs: u64,
    /// Whether to verify plugin signatures
    #[serde(default = "default_true")]
    pub verify_signatures: bool,
    /// Trusted public keys for signature verification
    #[serde(default)]
    pub trusted_keys: Vec<String>,
}

fn default_cache_duration() -> u64 {
    3600 // 1 hour
}

fn default_true() -> bool {
    true
}

impl Default for MarketplaceConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://marketplace.clawdius.dev/api/v1".to_string(),
            api_key: None,
            cache_duration_secs: 3600,
            verify_signatures: true,
            trusted_keys: Vec::new(),
        }
    }
}

/// Marketplace plugin listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePlugin {
    /// Plugin ID
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Latest version
    pub version: String,
    /// Description
    pub description: String,
    /// Author
    pub author: String,
    /// Download count
    pub downloads: u64,
    /// Star rating (0-5)
    pub stars: f32,
    /// Number of ratings
    pub ratings_count: u64,
    /// Categories/tags
    pub tags: Vec<String>,
    /// Whether the plugin is verified
    pub verified: bool,
    /// Whether the plugin is featured
    pub featured: bool,
    /// Icon URL
    pub icon_url: Option<String>,
    /// Last updated timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Available versions
    pub versions: Vec<MarketplaceVersion>,
}

/// Version information in marketplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceVersion {
    /// Version number
    pub version: String,
    /// Release notes
    pub notes: String,
    /// Download URL
    pub download_url: String,
    /// Checksum (SHA-256)
    pub checksum: String,
    /// Signature (if signed)
    pub signature: Option<String>,
    /// Minimum Clawdius version
    pub min_clawdius_version: String,
    /// Published timestamp
    pub published_at: chrono::DateTime<chrono::Utc>,
    /// Whether this is a pre-release
    pub prerelease: bool,
    /// Whether this version is deprecated
    pub deprecated: bool,
    /// Deprecation message
    pub deprecation_message: Option<String>,
}

/// Search query for marketplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceSearchQuery {
    /// Search term
    #[serde(default)]
    pub query: String,
    /// Filter by category
    #[serde(default)]
    pub category: Option<String>,
    /// Filter by author
    #[serde(default)]
    pub author: Option<String>,
    /// Filter by tag
    #[serde(default)]
    pub tag: Option<String>,
    /// Sort by field
    #[serde(default)]
    pub sort: MarketplaceSort,
    /// Sort order
    #[serde(default)]
    pub order: MarketplaceOrder,
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Results per page
    #[serde(default = "default_page_size")]
    pub per_page: u32,
    /// Include pre-releases
    #[serde(default)]
    pub include_prereleases: bool,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

impl Default for MarketplaceSearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            category: None,
            author: None,
            tag: None,
            sort: MarketplaceSort::default(),
            order: MarketplaceOrder::default(),
            page: 1,
            per_page: 20,
            include_prereleases: false,
        }
    }
}

/// Sort field for marketplace search
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSort {
    #[default]
    Relevance,
    Downloads,
    Stars,
    Updated,
    Name,
}

/// Sort order
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceOrder {
    #[default]
    Desc,
    Asc,
}

/// Search results from marketplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceSearchResults {
    /// Total number of results
    pub total: u64,
    /// Current page
    pub page: u32,
    /// Results per page
    pub per_page: u32,
    /// Total pages
    pub total_pages: u32,
    /// Plugin results
    pub plugins: Vec<MarketplacePlugin>,
}

/// Plugin installation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRequest {
    /// Plugin ID or name
    pub plugin: String,
    /// Specific version (optional, defaults to latest)
    #[serde(default)]
    pub version: Option<String>,
    /// Allow pre-releases
    #[serde(default)]
    pub allow_prerelease: bool,
    /// Force reinstall if already installed
    #[serde(default)]
    pub force: bool,
    /// Skip dependency resolution
    #[serde(default)]
    pub skip_dependencies: bool,
}

/// Plugin installation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    /// Installed plugin manifest
    pub manifest: PluginManifest,
    /// Installation path
    pub path: String,
    /// Installed dependencies
    pub dependencies: Vec<String>,
    /// Whether this was an update
    pub was_update: bool,
    /// Previous version (if update)
    pub previous_version: Option<String>,
}

/// Plugin update check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheck {
    /// Plugin ID
    pub plugin_id: String,
    /// Current version
    pub current_version: String,
    /// Latest available version
    pub latest_version: String,
    /// Whether an update is available
    pub update_available: bool,
    /// Release notes for latest version
    pub release_notes: String,
    /// Whether latest is a pre-release
    pub is_prerelease: bool,
}

/// Marketplace client
pub struct MarketplaceClient {
    config: MarketplaceConfig,
    http_client: reqwest::Client,
    cache: HashMap<String, (chrono::DateTime<chrono::Utc>, serde_json::Value)>,
}

impl MarketplaceClient {
    /// Create a new marketplace client
    #[must_use]
    pub fn new(config: MarketplaceConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            cache: HashMap::new(),
        }
    }

    /// Search for plugins
    pub async fn search(
        &mut self,
        query: MarketplaceSearchQuery,
    ) -> Result<MarketplaceSearchResults> {
        let url = format!("{}/plugins/search", self.config.endpoint);

        let response = self
            .http_client
            .get(&url)
            .query(&query)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.as_deref().unwrap_or("")),
            )
            .send()
            .await
            .context("Failed to search marketplace")?;

        if !response.status().is_success() {
            anyhow::bail!("Marketplace search failed: {}", response.status());
        }

        response
            .json()
            .await
            .context("Failed to parse search results")
    }

    /// Get plugin details
    pub async fn get_plugin(&mut self, plugin_id: &str) -> Result<MarketplacePlugin> {
        let url = format!("{}/plugins/{}", self.config.endpoint, plugin_id);

        let response = self
            .http_client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.as_deref().unwrap_or("")),
            )
            .send()
            .await
            .context("Failed to get plugin details")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get plugin: {}", response.status());
        }

        response
            .json()
            .await
            .context("Failed to parse plugin details")
    }

    /// Install a plugin
    pub async fn install(&mut self, request: InstallRequest) -> Result<InstallResult> {
        let url = format!("{}/plugins/install", self.config.endpoint);

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.as_deref().unwrap_or("")),
            )
            .send()
            .await
            .context("Failed to install plugin")?;

        if !response.status().is_success() {
            anyhow::bail!("Plugin installation failed: {}", response.status());
        }

        response
            .json()
            .await
            .context("Failed to parse installation result")
    }

    /// Check for plugin updates
    pub async fn check_updates(
        &mut self,
        installed_plugins: &[String],
    ) -> Result<Vec<UpdateCheck>> {
        let url = format!("{}/plugins/check-updates", self.config.endpoint);

        let response = self
            .http_client
            .post(&url)
            .json(&installed_plugins)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.as_deref().unwrap_or("")),
            )
            .send()
            .await
            .context("Failed to check for updates")?;

        if !response.status().is_success() {
            anyhow::bail!("Update check failed: {}", response.status());
        }

        response
            .json()
            .await
            .context("Failed to parse update check results")
    }

    /// Get featured plugins
    pub async fn get_featured(&mut self) -> Result<Vec<MarketplacePlugin>> {
        let url = format!("{}/plugins/featured", self.config.endpoint);

        let response = self
            .http_client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.as_deref().unwrap_or("")),
            )
            .send()
            .await
            .context("Failed to get featured plugins")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get featured plugins: {}", response.status());
        }

        response
            .json()
            .await
            .context("Failed to parse featured plugins")
    }

    /// Get plugin categories
    pub async fn get_categories(&mut self) -> Result<Vec<MarketplaceCategory>> {
        let url = format!("{}/categories", self.config.endpoint);

        let response = self
            .http_client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.as_deref().unwrap_or("")),
            )
            .send()
            .await
            .context("Failed to get categories")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get categories: {}", response.status());
        }

        response.json().await.context("Failed to parse categories")
    }

    /// Submit a plugin to the marketplace
    /// Note: This requires the "multipart" feature in reqwest
    pub async fn submit_plugin(
        &mut self,
        manifest: &PluginManifest,
        wasm_bytes: &[u8],
    ) -> Result<String> {
        let url = format!("{}/plugins/submit", self.config.endpoint);

        // Use JSON for manifest and base64 for WASM (simpler than multipart)
        let wasm_base64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, wasm_bytes);

        #[derive(Serialize)]
        struct SubmitRequest {
            manifest: String,
            wasm_base64: String,
        }

        let request = SubmitRequest {
            manifest: manifest.to_toml()?,
            wasm_base64,
        };

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.as_deref().unwrap_or("")),
            )
            .send()
            .await
            .context("Failed to submit plugin")?;

        if !response.status().is_success() {
            anyhow::bail!("Plugin submission failed: {}", response.status());
        }

        #[derive(Deserialize)]
        struct SubmitResponse {
            plugin_id: String,
        }

        let result: SubmitResponse = response.json().await?;
        Ok(result.plugin_id)
    }
}

/// Marketplace category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceCategory {
    /// Category ID
    pub id: String,
    /// Category name
    pub name: String,
    /// Category description
    pub description: String,
    /// Plugin count
    pub plugin_count: u64,
    /// Icon
    pub icon: Option<String>,
}

/// Local marketplace cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceCache {
    /// Cache entries
    pub entries: HashMap<String, CacheEntry>,
    /// Last full sync
    pub last_sync: chrono::DateTime<chrono::Utc>,
}

/// Cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Plugin data
    pub plugin: MarketplacePlugin,
    /// Cached at
    pub cached_at: chrono::DateTime<chrono::Utc>,
    /// Expires at
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl MarketplaceCache {
    /// Create a new cache
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            last_sync: chrono::DateTime::UNIX_EPOCH,
        }
    }

    /// Check if an entry is valid
    #[must_use]
    pub fn is_valid(&self, plugin_id: &str) -> bool {
        if let Some(entry) = self.entries.get(plugin_id) {
            entry.expires_at > chrono::Utc::now()
        } else {
            false
        }
    }

    /// Get a cached plugin
    #[must_use]
    pub fn get(&self, plugin_id: &str) -> Option<&MarketplacePlugin> {
        if self.is_valid(plugin_id) {
            self.entries.get(plugin_id).map(|e| &e.plugin)
        } else {
            None
        }
    }

    /// Add a plugin to cache
    pub fn insert(&mut self, plugin: MarketplacePlugin, ttl_secs: u64) {
        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl_secs as i64);

        self.entries.insert(
            plugin.id.clone(),
            CacheEntry {
                plugin,
                cached_at: now,
                expires_at,
            },
        );
    }

    /// Clean expired entries
    pub fn clean_expired(&mut self) {
        let now = chrono::Utc::now();
        self.entries.retain(|_, e| e.expires_at > now);
    }
}

impl Default for MarketplaceCache {
    fn default() -> Self {
        Self::new()
    }
}
