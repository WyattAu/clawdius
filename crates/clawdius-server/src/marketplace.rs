//! Plugin Marketplace Backend
//!
//! In-memory plugin registry serving the REST API contract expected by
//! `clawdius_core::plugin::marketplace::MarketplaceClient`.
//!
//! Endpoints (all under `/api/v1`):
//! - `GET  /plugins/search`        — search with query, category, author, tag, sort, pagination
//! - `GET  /plugins/featured`      — list featured plugins
//! - `GET  /plugins/{id}`          — get plugin details
//! - `POST /plugins/submit`        — submit a new plugin (manifest + WASM base64)
//! - `POST /plugins/check-updates` — check for updates given installed plugin IDs
//! - `POST /plugins/install`       — install a plugin (returns download URL + checksum)
//! - `GET  /categories`            — list plugin categories

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Registry (in-memory store)
// ---------------------------------------------------------------------------

/// A single registered plugin with all metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub stars: f32,
    pub ratings_count: u64,
    pub tags: Vec<String>,
    pub verified: bool,
    pub featured: bool,
    pub icon_url: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub versions: Vec<RegisteredVersion>,
    pub category: Option<String>,
    /// Base64-encoded WASM module bytes.
    pub wasm_base64: Option<String>,
    /// Manifest TOML string.
    pub manifest_toml: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredVersion {
    pub version: String,
    pub notes: String,
    pub download_url: String,
    pub checksum: String,
    pub signature: Option<String>,
    pub min_clawdius_version: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
    pub prerelease: bool,
    pub deprecated: bool,
    pub deprecation_message: Option<String>,
}

/// A plugin category with aggregate count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: String,
    pub plugin_count: u64,
    pub icon: Option<String>,
}

/// Thread-safe in-memory marketplace registry.
#[derive(Debug, Clone)]
pub struct MarketplaceRegistry {
    pub plugins: Arc<RwLock<HashMap<String, RegisteredPlugin>>>,
    pub categories: Arc<RwLock<HashMap<String, Category>>>,
}

impl MarketplaceRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            categories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Seed the registry with sample data for development / demo purposes.
    pub async fn seed_defaults(&self) {
        let now = Utc::now();
        let sample_plugins: Vec<RegisteredPlugin> = vec![
            RegisteredPlugin {
                id: "clawdius-lint".into(),
                name: "Clawdius Lint".into(),
                version: "1.0.0".into(),
                description: "Runs clippy and custom lint rules on code changes.".into(),
                author: "clawdius-team".into(),
                downloads: 1240,
                stars: 4.5,
                ratings_count: 32,
                tags: vec!["lint".into(), "quality".into()],
                verified: true,
                featured: true,
                icon_url: Some("https://marketplace.clawdius.dev/icons/lint.svg".into()),
                updated_at: now,
                versions: vec![RegisteredVersion {
                    version: "1.0.0".into(),
                    notes: "Initial release".into(),
                    download_url: "https://marketplace.clawdius.dev/plugins/clawdius-lint/v1.0.0/plugin.wasm".into(),
                    checksum: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
                    signature: None,
                    min_clawdius_version: "1.6.0".into(),
                    published_at: now,
                    prerelease: false,
                    deprecated: false,
                    deprecation_message: None,
                }],
                category: Some("quality".into()),
                wasm_base64: None,
                manifest_toml: None,
            },
            RegisteredPlugin {
                id: "clawdius-format".into(),
                name: "Clawdius Format".into(),
                version: "0.9.0".into(),
                description: "Auto-formats Rust code using rustfmt with project conventions.".into(),
                author: "clawdius-team".into(),
                downloads: 890,
                stars: 4.2,
                ratings_count: 18,
                tags: vec!["format".into(), "style".into()],
                verified: true,
                featured: false,
                icon_url: Some("https://marketplace.clawdius.dev/icons/format.svg".into()),
                updated_at: now,
                versions: vec![RegisteredVersion {
                    version: "0.9.0".into(),
                    notes: "Beta release".into(),
                    download_url: "https://marketplace.clawdius.dev/plugins/clawdius-format/v0.9.0/plugin.wasm".into(),
                    checksum: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
                    signature: None,
                    min_clawdius_version: "1.6.0".into(),
                    published_at: now,
                    prerelease: true,
                    deprecated: false,
                    deprecation_message: None,
                }],
                category: Some("quality".into()),
                wasm_base64: None,
                manifest_toml: None,
            },
            RegisteredPlugin {
                id: "clawdius-deps".into(),
                name: "Clawdius Deps".into(),
                version: "2.1.0".into(),
                description: "Analyzes dependency graphs for vulnerabilities and outdated crates.".into(),
                author: "community".into(),
                downloads: 560,
                stars: 3.8,
                ratings_count: 12,
                tags: vec!["dependencies".into(), "security".into()],
                verified: false,
                featured: true,
                icon_url: None,
                updated_at: now,
                versions: vec![RegisteredVersion {
                    version: "2.1.0".into(),
                    notes: "Added cargo-deny integration".into(),
                    download_url: "https://marketplace.clawdius.dev/plugins/clawdius-deps/v2.1.0/plugin.wasm".into(),
                    checksum: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
                    signature: None,
                    min_clawdius_version: "1.7.0".into(),
                    published_at: now,
                    prerelease: false,
                    deprecated: false,
                    deprecation_message: None,
                }],
                category: Some("security".into()),
                wasm_base64: None,
                manifest_toml: None,
            },
        ];

        let mut plugins = self.plugins.write().await;
        for p in sample_plugins {
            plugins.insert(p.id.clone(), p);
        }

        let mut categories = self.categories.write().await;
        categories.insert(
            "quality".into(),
            Category {
                id: "quality".into(),
                name: "Code Quality".into(),
                description: "Linters, formatters, and static analysis tools.".into(),
                plugin_count: 2,
                icon: Some("🛡️".into()),
            },
        );
        categories.insert(
            "security".into(),
            Category {
                id: "security".into(),
                name: "Security".into(),
                description: "Vulnerability scanning and dependency auditing.".into(),
                plugin_count: 1,
                icon: Some("🔒".into()),
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Query / Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub query: Option<String>,
    pub category: Option<String>,
    pub author: Option<String>,
    pub tag: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub include_prereleases: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CheckUpdatesRequest {
    pub plugins: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    pub manifest: String,
    pub wasm_base64: String,
}

#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    pub plugin: String,
    pub version: Option<String>,
    pub allow_prerelease: Option<bool>,
    #[allow(dead_code)]
    pub force: Option<bool>,
    #[allow(dead_code)]
    pub skip_dependencies: Option<bool>,
}

// ---------------------------------------------------------------------------
// Response types (match clawdius-core marketplace types)
// ---------------------------------------------------------------------------

fn json_error(
    status: StatusCode,
    code: &str,
    message: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "status": "error",
            "error": { "code": code, "message": message }
        })),
    )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/v1/plugins/search`
pub async fn search_plugins(
    State(registry): State<MarketplaceRegistry>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).clamp(1, 100);

    let plugins = registry.plugins.read().await;
    let mut results: Vec<&RegisteredPlugin> = plugins.values().collect();

    // Filter by text query (case-insensitive substring match on name + description)
    if let Some(ref text) = q.query {
        let text_lower = text.to_lowercase();
        results.retain(|p| {
            p.name.to_lowercase().contains(&text_lower)
                || p.description.to_lowercase().contains(&text_lower)
        });
    }

    // Filter by category
    if let Some(ref cat) = q.category {
        results.retain(|p| p.category.as_deref() == Some(cat.as_str()));
    }

    // Filter by author
    if let Some(ref author) = q.author {
        results.retain(|p| p.author == *author);
    }

    // Filter by tag
    if let Some(ref tag) = q.tag {
        results.retain(|p| p.tags.iter().any(|t| t == tag));
    }

    // Filter out pre-releases unless requested
    if q.include_prereleases != Some(true) {
        results.retain(|p| {
            p.versions
                .iter()
                .any(|v| v.version == p.version && !v.prerelease)
        });
    }

    // Sort
    match q.sort.as_deref() {
        Some("downloads") => results.sort_by(|a, b| b.downloads.cmp(&a.downloads)),
        Some("stars") => results.sort_by(|a, b| {
            b.stars
                .partial_cmp(&a.stars)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        Some("updated") => results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
        Some("name") => results.sort_by(|a, b| a.name.cmp(&b.name)),
        _ => {}, // relevance = order of insertion (default)
    }

    if q.order.as_deref() == Some("asc") {
        results.reverse();
    }

    let total = results.len() as u64;
    let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;

    // Paginate
    let start = ((page - 1) * per_page) as usize;
    let plugins: Vec<serde_json::Value> = results
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .map(plugin_to_api)
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "total": total,
            "page": page,
            "per_page": per_page,
            "total_pages": total_pages,
            "plugins": plugins,
        })),
    )
}

/// `GET /api/v1/plugins/featured`
pub async fn featured_plugins(State(registry): State<MarketplaceRegistry>) -> impl IntoResponse {
    let plugins = registry.plugins.read().await;
    let featured: Vec<serde_json::Value> = plugins
        .values()
        .filter(|p| p.featured)
        .map(plugin_to_api)
        .collect();

    Json(serde_json::Value::Array(featured))
}

/// `GET /api/v1/plugins/{id}`
pub async fn get_plugin(
    State(registry): State<MarketplaceRegistry>,
    Path(plugin_id): Path<String>,
) -> impl IntoResponse {
    let plugins = registry.plugins.read().await;

    match plugins.get(&plugin_id) {
        Some(p) => (StatusCode::OK, Json(plugin_to_api(p))),
        None => json_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            &format!("Plugin '{plugin_id}' not found"),
        ),
    }
}

/// `POST /api/v1/plugins/submit`
pub async fn submit_plugin(
    State(registry): State<MarketplaceRegistry>,
    Json(body): Json<SubmitRequest>,
) -> impl IntoResponse {
    // Validate manifest is non-empty TOML-like content
    if body.manifest.trim().is_empty() {
        return json_error(
            StatusCode::BAD_REQUEST,
            "INVALID_MANIFEST",
            "Manifest must be non-empty",
        );
    }

    // Validate wasm_base64 is non-empty and valid base64
    if body.wasm_base64.trim().is_empty() {
        return json_error(
            StatusCode::BAD_REQUEST,
            "INVALID_WASM",
            "WASM payload must be non-empty",
        );
    }

    // Validate base64 decoding succeeds
    if base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.wasm_base64,
    )
    .is_err()
    {
        return json_error(
            StatusCode::BAD_REQUEST,
            "INVALID_WASM",
            "WASM payload is not valid base64",
        );
    }

    // Generate plugin ID from manifest name (basic extraction)
    let plugin_id =
        extract_plugin_name(&body.manifest).unwrap_or_else(|| format!("plugin-{}", uuid_simple()));

    // Check for duplicate ID
    {
        let plugins = registry.plugins.read().await;
        if plugins.contains_key(&plugin_id) {
            return json_error(
                StatusCode::CONFLICT,
                "ALREADY_EXISTS",
                &format!("Plugin '{plugin_id}' already exists"),
            );
        }
    }

    let now = Utc::now();
    let plugin = RegisteredPlugin {
        id: plugin_id.clone(),
        name: extract_plugin_name(&body.manifest).unwrap_or("Unnamed Plugin".into()),
        version: extract_plugin_version(&body.manifest).unwrap_or("0.1.0".into()),
        description: extract_plugin_description(&body.manifest).unwrap_or_default(),
        author: extract_plugin_author(&body.manifest).unwrap_or_else(|| "anonymous".into()),
        downloads: 0,
        stars: 0.0,
        ratings_count: 0,
        tags: extract_plugin_tags(&body.manifest),
        verified: false,
        featured: false,
        icon_url: None,
        updated_at: now,
        versions: vec![RegisteredVersion {
            version: extract_plugin_version(&body.manifest).unwrap_or("0.1.0".into()),
            notes: "Initial submission".into(),
            download_url: format!(
                "https://marketplace.clawdius.dev/plugins/{plugin_id}/v0.1.0/plugin.wasm"
            ),
            checksum: format!("{:064x}", sha256_hex(&body.wasm_base64)),
            signature: None,
            min_clawdius_version: "1.6.0".into(),
            published_at: now,
            prerelease: false,
            deprecated: false,
            deprecation_message: None,
        }],
        category: None,
        wasm_base64: Some(body.wasm_base64),
        manifest_toml: Some(body.manifest),
    };

    registry
        .plugins
        .write()
        .await
        .insert(plugin_id.clone(), plugin);

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "plugin_id": plugin_id })),
    )
}

/// `POST /api/v1/plugins/check-updates`
pub async fn check_updates(
    State(registry): State<MarketplaceRegistry>,
    Json(body): Json<CheckUpdatesRequest>,
) -> impl IntoResponse {
    let plugins = registry.plugins.read().await;
    let results: Vec<serde_json::Value> = body
        .plugins
        .iter()
        .filter_map(|id| {
            plugins.get(id).map(|p| {
                serde_json::json!({
                    "plugin_id": p.id,
                    "current_version": p.version,
                    "latest_version": p.version,
                    "update_available": false,
                    "release_notes": p.versions.first().map(|v| v.notes.as_str()).unwrap_or(""),
                    "is_prerelease": p.versions.first().map(|v| v.prerelease).unwrap_or(false),
                })
            })
        })
        .collect();

    Json(serde_json::Value::Array(results))
}

/// `POST /api/v1/plugins/install`
pub async fn install_plugin(
    State(registry): State<MarketplaceRegistry>,
    Json(body): Json<InstallRequest>,
) -> impl IntoResponse {
    let plugins = registry.plugins.read().await;

    match plugins.get(&body.plugin) {
        Some(p) => {
            // Find the requested version, falling back to latest
            let version = body.version.as_deref().unwrap_or(&p.version);
            let found_version = p.versions.iter().find(|v| v.version == version);

            match found_version {
                Some(v) => {
                    // If prerelease not allowed and this is a prerelease, reject
                    if body.allow_prerelease != Some(true) && v.prerelease {
                        return json_error(
                            StatusCode::BAD_REQUEST,
                            "PRERELEASE_BLOCKED",
                            "Pre-release versions require allow_prerelease=true",
                        );
                    }

                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "manifest": p.manifest_toml,
                            "path": format!(".clawdius/plugins/{}", p.id),
                            "download_url": v.download_url,
                            "checksum": v.checksum,
                            "dependencies": [],
                            "was_update": false,
                            "previous_version": null,
                        })),
                    )
                },
                None => json_error(
                    StatusCode::NOT_FOUND,
                    "VERSION_NOT_FOUND",
                    &format!("Version '{version}' not found for plugin '{}'", p.id),
                ),
            }
        },
        None => json_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            &format!("Plugin '{}' not found", body.plugin),
        ),
    }
}

/// `GET /api/v1/categories`
pub async fn list_categories(State(registry): State<MarketplaceRegistry>) -> impl IntoResponse {
    let categories = registry.categories.read().await;
    let list: Vec<&Category> = categories.values().collect();
    Json(serde_json::json!(list))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `RegisteredPlugin` to the JSON shape expected by the core client.
fn plugin_to_api(p: &RegisteredPlugin) -> serde_json::Value {
    serde_json::json!({
        "id": p.id,
        "name": p.name,
        "version": p.version,
        "description": p.description,
        "author": p.author,
        "downloads": p.downloads,
        "stars": p.stars,
        "ratings_count": p.ratings_count,
        "tags": p.tags,
        "verified": p.verified,
        "featured": p.featured,
        "icon_url": p.icon_url,
        "updated_at": p.updated_at.to_rfc3339(),
        "versions": p.versions.iter().map(|v| serde_json::json!({
            "version": v.version,
            "notes": v.notes,
            "download_url": v.download_url,
            "checksum": v.checksum,
            "signature": v.signature,
            "min_clawdius_version": v.min_clawdius_version,
            "published_at": v.published_at.to_rfc3339(),
            "prerelease": v.prerelease,
            "deprecated": v.deprecated,
            "deprecation_message": v.deprecation_message,
        })).collect::<Vec<_>>(),
    })
}

/// Generate a simple random-ish hex ID (no external dependency needed).
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:08x}", nanos % 0xFFFF_FFFF)
}

/// Simple deterministic hash for checksums (not cryptographic, sufficient for demo).
fn sha256_hex(input: &str) -> u64 {
    // FNV-1a hash — consistent, fast, no dependency
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Try to extract the plugin name from a TOML manifest string.
fn extract_plugin_name(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("name") {
            if let Some(name) = name.split('=').nth(1) {
                let name = name.trim().trim_matches('"').trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// Try to extract the plugin version from a TOML manifest string.
fn extract_plugin_version(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let trimmed = line.trim();
        if let Some(ver) = trimmed.strip_prefix("version") {
            if let Some(ver) = ver.split('=').nth(1) {
                let ver = ver.trim().trim_matches('"').trim().to_string();
                if !ver.is_empty() {
                    return Some(ver);
                }
            }
        }
    }
    None
}

/// Try to extract the plugin description from a TOML manifest string.
fn extract_plugin_description(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let trimmed = line.trim();
        if let Some(desc) = trimmed.strip_prefix("description") {
            if let Some(desc) = desc.split('=').nth(1) {
                let desc = desc.trim().trim_matches('"').trim().to_string();
                if !desc.is_empty() {
                    return Some(desc);
                }
            }
        }
    }
    None
}

/// Try to extract the plugin author from a TOML manifest string.
fn extract_plugin_author(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let trimmed = line.trim();
        if let Some(auth) = trimmed.strip_prefix("author") {
            if let Some(auth) = auth.split('=').nth(1) {
                let auth = auth.trim().trim_matches('"').trim().to_string();
                if !auth.is_empty() {
                    return Some(auth);
                }
            }
        }
    }
    None
}

/// Try to extract tags from a TOML manifest string.
fn extract_plugin_tags(toml: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for line in toml.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("tags") {
            continue;
        }
        let Some(rest) = trimmed.split('=').nth(1) else {
            continue;
        };
        // Strip brackets and whitespace
        let rest = rest.trim();
        let inner = rest
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(rest);
        for part in inner.split(',') {
            let tag = part.trim().trim_matches('"').trim().to_string();
            if !tag.is_empty() {
                tags.push(tag);
            }
        }
        break; // only parse the first tags line
    }
    tags
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: convert impl IntoResponse into (StatusCode, serde_json::Value)
    async fn into_status_body(resp: impl IntoResponse) -> (StatusCode, serde_json::Value) {
        let resp = resp.into_response();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("body should be readable");
        let body: serde_json::Value =
            serde_json::from_slice(&body_bytes).unwrap_or(serde_json::Value::Null);
        (status, body)
    }

    #[tokio::test]
    async fn test_registry_seed() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let plugins = registry.plugins.read().await;
        assert!(plugins.contains_key("clawdius-lint"));
        assert!(plugins.contains_key("clawdius-format"));
        assert!(plugins.contains_key("clawdius-deps"));
        assert_eq!(plugins.len(), 3);

        let categories = registry.categories.read().await;
        assert!(categories.contains_key("quality"));
        assert!(categories.contains_key("security"));
    }

    #[tokio::test]
    async fn test_search_all() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let state = State(registry);
        let q = Query(SearchQuery {
            query: None,
            category: None,
            author: None,
            tag: None,
            sort: None,
            order: None,
            page: None,
            per_page: None,
            include_prereleases: None,
        });

        let resp = search_plugins(state, q).await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 2); // format is prerelease, excluded by default
        assert_eq!(body["plugins"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_search_with_query() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let state = State(registry);
        let q = Query(SearchQuery {
            query: Some("lint".into()),
            category: None,
            author: None,
            tag: None,
            sort: None,
            order: None,
            page: None,
            per_page: None,
            include_prereleases: None,
        });

        let resp = search_plugins(state, q).await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 1);
        assert_eq!(body["plugins"][0]["id"], "clawdius-lint");
    }

    #[tokio::test]
    async fn test_search_with_prerelease() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let state = State(registry);
        let q = Query(SearchQuery {
            query: None,
            category: None,
            author: None,
            tag: None,
            sort: None,
            order: None,
            page: None,
            per_page: None,
            include_prereleases: Some(true),
        });

        let resp = search_plugins(state, q).await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 3); // all 3 including prerelease format
    }

    #[tokio::test]
    async fn test_search_sort_by_downloads() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let state = State(registry);
        let q = Query(SearchQuery {
            query: None,
            category: None,
            author: None,
            tag: None,
            sort: Some("downloads".into()),
            order: None,
            page: None,
            per_page: None,
            include_prereleases: Some(true),
        });

        let resp = search_plugins(state, q).await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        let plugins = body["plugins"].as_array().unwrap();
        // lint (1240) > deps (560) > format (890) for prerelease-inclusive
        assert!(
            plugins[0]["downloads"].as_u64().unwrap() >= plugins[1]["downloads"].as_u64().unwrap()
        );
    }

    #[tokio::test]
    async fn test_get_plugin_found() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let state = State(registry);
        let resp = get_plugin(state, Path("clawdius-lint".into())).await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], "clawdius-lint");
        assert_eq!(body["verified"], true);
        assert!(body["versions"].as_array().unwrap().len() >= 1);
    }

    #[tokio::test]
    async fn test_get_plugin_not_found() {
        let registry = MarketplaceRegistry::new();
        let state = State(registry);
        let resp = get_plugin(state, Path("nonexistent".into())).await;
        let (status, _body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_featured_plugins() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let state = State(registry);
        let resp = featured_plugins(state).await;
        let (_status, body) = into_status_body(resp).await;
        let plugins = body.as_array().unwrap();
        assert_eq!(plugins.len(), 2); // lint + deps are featured
        assert!(plugins.iter().all(|p| p["featured"] == true));
    }

    #[tokio::test]
    async fn test_list_categories() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let state = State(registry);
        let resp = list_categories(state).await;
        let (_status, body) = into_status_body(resp).await;
        let cats = body.as_array().unwrap();
        assert_eq!(cats.len(), 2);
    }

    #[tokio::test]
    async fn test_submit_plugin_valid() {
        let registry = MarketplaceRegistry::new();
        let registry_clone = registry.clone();
        let state = State(registry);

        let wasm_bytes = vec![0x00, 0x61, 0x73, 0x6d]; // WASM magic bytes
        let wasm_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &wasm_bytes);
        let manifest = r#"
name = "test-plugin"
version = "0.1.0"
description = "A test plugin"
author = "tester"
tags = ["test"]
"#
        .to_string();

        let resp = submit_plugin(
            state,
            Json(SubmitRequest {
                manifest,
                wasm_base64: wasm_b64,
            }),
        )
        .await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["plugin_id"], "test-plugin");

        // Verify it's in the registry
        let plugins = registry_clone.plugins.read().await;
        assert!(plugins.contains_key("test-plugin"));
    }

    #[tokio::test]
    async fn test_submit_plugin_empty_manifest() {
        let registry = MarketplaceRegistry::new();
        let state = State(registry);

        let resp = submit_plugin(
            state,
            Json(SubmitRequest {
                manifest: "  ".to_string(),
                wasm_base64: "AAAA".to_string(),
            }),
        )
        .await;
        let (status, _body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_submit_plugin_invalid_base64() {
        let registry = MarketplaceRegistry::new();
        let state = State(registry);

        let resp = submit_plugin(
            state,
            Json(SubmitRequest {
                manifest: r#"name = "bad""#.to_string(),
                wasm_base64: "not-valid-base64!!!".to_string(),
            }),
        )
        .await;
        let (status, _body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_submit_plugin_duplicate() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;
        let state = State(registry);

        let wasm_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &[0x00, 0x61, 0x73, 0x6d],
        );

        let resp = submit_plugin(
            state,
            Json(SubmitRequest {
                manifest: r#"name = "clawdius-lint""#.to_string(),
                wasm_base64: wasm_b64,
            }),
        )
        .await;
        let (status, _body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_install_plugin_found() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;
        let state = State(registry);

        let resp = install_plugin(
            state,
            Json(InstallRequest {
                plugin: "clawdius-lint".into(),
                version: None,
                allow_prerelease: None,
                force: None,
                skip_dependencies: None,
            }),
        )
        .await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["was_update"], false);
        assert!(body["download_url"].is_string());
    }

    #[tokio::test]
    async fn test_install_plugin_not_found() {
        let registry = MarketplaceRegistry::new();
        let state = State(registry);

        let resp = install_plugin(
            state,
            Json(InstallRequest {
                plugin: "nonexistent".into(),
                version: None,
                allow_prerelease: None,
                force: None,
                skip_dependencies: None,
            }),
        )
        .await;
        let (status, _body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_install_plugin_prerelease_blocked() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;
        let state = State(registry);

        let resp = install_plugin(
            state,
            Json(InstallRequest {
                plugin: "clawdius-format".into(),
                version: None,
                allow_prerelease: None,
                force: None,
                skip_dependencies: None,
            }),
        )
        .await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "PRERELEASE_BLOCKED");
    }

    #[tokio::test]
    async fn test_install_plugin_prerelease_allowed() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;
        let state = State(registry);

        let resp = install_plugin(
            state,
            Json(InstallRequest {
                plugin: "clawdius-format".into(),
                version: None,
                allow_prerelease: Some(true),
                force: None,
                skip_dependencies: None,
            }),
        )
        .await;
        let (status, _body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_check_updates() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;
        let state = State(registry);

        let resp = check_updates(
            state,
            Json(CheckUpdatesRequest {
                plugins: vec!["clawdius-lint".into(), "nonexistent".into()],
            }),
        )
        .await;
        let (_status, body) = into_status_body(resp).await;
        let results = body.as_array().unwrap();
        assert_eq!(results.len(), 1); // nonexistent not found
        assert_eq!(results[0]["plugin_id"], "clawdius-lint");
        assert_eq!(results[0]["update_available"], false);
    }

    #[tokio::test]
    async fn test_extract_helpers() {
        let toml = r#"
name = "my-plugin"
version = "1.2.3"
description = "Does things"
author = "someone"
tags = ["a", "b"]
"#;
        assert_eq!(extract_plugin_name(toml), Some("my-plugin".into()));
        assert_eq!(extract_plugin_version(toml), Some("1.2.3".into()));
        assert_eq!(extract_plugin_description(toml), Some("Does things".into()));
        assert_eq!(extract_plugin_author(toml), Some("someone".into()));
        assert_eq!(extract_plugin_tags(toml), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn test_pagination() {
        let registry = MarketplaceRegistry::new();
        registry.seed_defaults().await;

        let state = State(registry);
        let q = Query(SearchQuery {
            query: None,
            category: None,
            author: None,
            tag: None,
            sort: None,
            order: None,
            page: Some(2),
            per_page: Some(1),
            include_prereleases: Some(true),
        });

        let resp = search_plugins(state, q).await;
        let (status, body) = into_status_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["page"], 2);
        assert_eq!(body["total_pages"], 3);
        assert_eq!(body["plugins"].as_array().unwrap().len(), 1);
    }
}
