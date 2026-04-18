//! Browser Daemon — Persistent browser with accessibility-tree element references
//!
//! Provides a long-lived browser process shared across parallel sprint sessions.
//! Key features:
//! - Persistent Chromium instance (launch once, reuse)
//! - Accessibility-tree element refs (`@e1`, `@e2`, etc.)
//! - Ref-based interaction (click, type, read by ref instead of CSS selector)
//! - Session-scoped element maps (each session gets its own refs)
//! - Cross-session browser sharing via `Arc<BrowserDaemon>`
//!
//! # Design
//!
//! The daemon uses a trait (`BrowserSession`) to abstract over the actual browser
//! implementation, so it doesn't depend directly on `chromiumoxide`. The concrete
//! implementation (`ChromiumBrowserSession`) lives behind the `browser` feature flag.
//!
//! # Element References
//!
//! ```text
//! @e1  button "Submit"
//! @e2  input   "username"
//! @e3  link    "Dashboard"
//! @e4  heading "Welcome"
//! ```
//!
//! Refs are assigned by walking the accessibility tree in DOM order. Interactive
//! elements (buttons, links, inputs, selects, textareas) get refs first, then
//! headings and landmarks. Refs are stable within a session but reset on navigation.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

// ─── Element Reference System ───────────────────────────────────────────────

/// A reference to a DOM element, e.g. `@e5`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ElementRef {
    /// The reference string, e.g. `@e5`
    pub ref_id: String,
    /// Role of the element (button, link, input, heading, etc.)
    pub role: String,
    /// Accessible name or label
    pub name: String,
    /// CSS selector to locate this element (internal use)
    pub selector: String,
    /// Brief description for context
    pub description: String,
}

impl std::fmt::Display for ElementRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} \"{}\"", self.ref_id, self.role, self.name)
    }
}

/// A snapshot of the accessibility tree at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilitySnapshot {
    /// URL of the page when snapshot was taken
    pub url: String,
    /// Page title
    pub title: String,
    /// Ordered element references
    pub elements: Vec<ElementRef>,
    /// Timestamp of the snapshot (unix millis)
    pub timestamp_ms: u64,
}

impl AccessibilitySnapshot {
    /// Format as a compact reference list for LLM consumption.
    pub fn to_ref_list(&self) -> String {
        let mut lines = vec![format!("Page: {} ({})", self.title, self.url)];
        for elem in &self.elements {
            lines.push(format!("  {} {} \"{}\"", elem.ref_id, elem.role, elem.name));
        }
        lines.join("\n")
    }

    /// Look up an element by its ref string (e.g. `@e5`).
    pub fn find_by_ref(&self, ref_str: &str) -> Option<&ElementRef> {
        let normalized = if ref_str.starts_with('@') {
            ref_str.to_string()
        } else {
            format!("@{ref_str}")
        };
        self.elements.iter().find(|e| e.ref_id == normalized)
    }

    /// Look up an element by its ref string, returning a mutable reference.
    pub fn find_by_ref_mut(&mut self, ref_str: &str) -> Option<&mut ElementRef> {
        let normalized = if ref_str.starts_with('@') {
            ref_str.to_string()
        } else {
            format!("@{ref_str}")
        };
        self.elements.iter_mut().find(|e| e.ref_id == normalized)
    }
}

// ─── Browser Session Trait ─────────────────────────────────────────────────

use std::future::Future;
use std::pin::Pin;

/// Trait for browser operations, abstracting over the concrete browser implementation.
pub trait BrowserSession: Send + Sync {
    /// Navigate to a URL.
    fn navigate(&self, url: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
    /// Get the current page URL.
    fn current_url(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>>;
    /// Get the current page title.
    fn title(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>>;
    /// Click an element by CSS selector.
    fn click(&self, selector: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
    /// Type text into an element by CSS selector.
    fn type_text(
        &self,
        selector: String,
        text: String,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
    /// Read the inner text of an element by CSS selector.
    fn read_text(
        &self,
        selector: String,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>>;
    /// Get the full page HTML.
    fn get_content(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>>;
    /// Execute JavaScript and return the result.
    fn evaluate(&self, js: String) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>>;
    /// Take a screenshot and return PNG bytes.
    fn screenshot(&self) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>>;
    /// Wait for a selector to appear.
    fn wait_for_selector(
        &self,
        selector: String,
        timeout_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
    /// Reload the current page.
    fn reload(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
    /// Close the browser session.
    fn close(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

// ─── Stub Browser Session (for testing without Chromium) ───────────────────

/// A no-op browser session for testing and environments without Chromium.
#[derive(Debug)]
pub struct StubBrowserSession {
    url: Arc<Mutex<String>>,
    page_title: Arc<Mutex<String>>,
}

impl Default for StubBrowserSession {
    fn default() -> Self {
        Self::new()
    }
}

impl StubBrowserSession {
    pub fn new() -> Self {
        Self {
            url: Arc::new(Mutex::new("about:blank".to_string())),
            page_title: Arc::new(Mutex::new("Stub Page".to_string())),
        }
    }
}

impl BrowserSession for StubBrowserSession {
    fn navigate(&self, url: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let url_clone = url.clone();
        Box::pin(async move {
            *self.url.lock().await = url;
            *self.page_title.lock().await = format!("Page: {url_clone}");
            Ok(())
        })
    }

    fn current_url(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>> {
        let guard = self.url.clone();
        Box::pin(async move {
            let url = guard.lock().await;
            Ok(url.clone())
        })
    }

    fn title(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>> {
        let guard = self.page_title.clone();
        Box::pin(async move {
            let title = guard.lock().await;
            Ok(title.clone())
        })
    }

    fn click(&self, _selector: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn type_text(
        &self,
        _selector: String,
        _text: String,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn read_text(
        &self,
        selector: String,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async move { Ok(format!("[text from {selector}]")) })
    }

    fn get_content(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async { Ok("<html><body>Stub content</body></html>".to_string()) })
    }

    fn evaluate(&self, js: String) -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async move { Ok(format!("[eval: {js}]")) })
    }

    fn screenshot(&self) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn wait_for_selector(
        &self,
        _selector: String,
        _timeout_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn reload(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn close(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

// ─── Browser Daemon ────────────────────────────────────────────────────────

/// Configuration for the browser daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserDaemonConfig {
    /// Whether to run in headless mode (default: true)
    pub headless: bool,
    /// Window width
    pub width: u32,
    /// Window height
    pub height: u32,
    /// Maximum number of element refs per session
    pub max_refs: usize,
    /// Whether to auto-refresh the accessibility tree on navigation
    pub auto_snapshot: bool,
}

impl Default for BrowserDaemonConfig {
    fn default() -> Self {
        Self {
            headless: true,
            width: 1920,
            height: 1080,
            max_refs: 200,
            auto_snapshot: true,
        }
    }
}

/// Session-scoped element map for a single sprint session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionElementMap {
    /// Session ID this map belongs to
    pub session_id: String,
    /// The latest accessibility snapshot for this session
    pub snapshot: Option<AccessibilitySnapshot>,
    /// Ref counter for assigning new refs
    ref_counter: usize,
}

impl SessionElementMap {
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            snapshot: None,
            ref_counter: 0,
        }
    }
}

/// The browser daemon provides persistent browser access across sprint sessions.
///
/// It wraps a `BrowserSession` and manages per-session element reference maps.
/// Each sprint session gets its own set of `@e1`, `@e2`, ... refs that map to
/// DOM elements. The daemon ensures thread-safe access via async mutex.
pub struct BrowserDaemon {
    /// The underlying browser session (trait object)
    browser: Arc<dyn BrowserSession>,
    /// Per-session element maps
    session_maps: RwLock<HashMap<String, SessionElementMap>>,
    /// Daemon configuration
    config: BrowserDaemonConfig,
    /// Whether the daemon has been initialized
    initialized: Mutex<bool>,
}

impl BrowserDaemon {
    /// Create a new browser daemon with a custom browser session implementation.
    pub fn new(browser: Arc<dyn BrowserSession>, config: BrowserDaemonConfig) -> Self {
        Self {
            browser,
            session_maps: RwLock::new(HashMap::new()),
            config,
            initialized: Mutex::new(false),
        }
    }

    /// Create a daemon with a stub browser (for testing).
    pub fn new_stub() -> Self {
        Self::new(
            Arc::new(StubBrowserSession::new()),
            BrowserDaemonConfig::default(),
        )
    }

    /// Initialize the daemon (navigate to a blank page).
    pub async fn initialize(&self) -> Result<()> {
        let mut init = self.initialized.lock().await;
        if *init {
            return Ok(());
        }
        self.browser.navigate("about:blank".to_string()).await?;
        *init = true;
        Ok(())
    }

    /// Register a session for element tracking.
    pub async fn register_session(&self, session_id: &str) {
        let mut maps = self.session_maps.write().await;
        maps.entry(session_id.to_string())
            .or_insert_with(|| SessionElementMap::new(session_id));
    }

    /// Unregister a session and clean up its element map.
    pub async fn unregister_session(&self, session_id: &str) {
        let mut maps = self.session_maps.write().await;
        maps.remove(session_id);
    }

    /// Navigate to a URL and optionally update the accessibility snapshot.
    pub async fn navigate(&self, url: &str, session_id: Option<&str>) -> Result<()> {
        self.initialize().await?;
        self.browser.navigate(url.to_string()).await?;

        if self.config.auto_snapshot {
            if let Some(sid) = session_id {
                self.build_snapshot(sid).await?;
            }
        }

        Ok(())
    }

    /// Build an accessibility snapshot for a session by parsing the page.
    ///
    /// Uses JavaScript to walk the DOM and extract interactive elements,
    /// then assigns `@e1`, `@e2`, ... refs to each.
    pub async fn build_snapshot(&self, session_id: &str) -> Result<AccessibilitySnapshot> {
        self.initialize().await?;

        let url = self.browser.current_url().await.unwrap_or_default();
        let title = self.browser.title().await.unwrap_or_default();

        // JavaScript to extract interactive elements from the page
        let js = r#"
        (function() {
            const elements = [];
            const interactiveRoles = [
                'button', 'link', 'input', 'select', 'textarea',
                'checkbox', 'radio', 'tab', 'menuitem', 'option',
                'heading', 'navigation', 'main', 'banner', 'contentinfo'
            ];

            function getRole(el) {
                const role = el.getAttribute('role');
                if (role) return role.toLowerCase();
                const tag = el.tagName.toLowerCase();
                if (tag === 'a') return 'link';
                if (tag === 'button' || tag === 'input[type="button"]' || tag === 'input[type="submit"]') return 'button';
                if (tag === 'input') {
                    const type = (el.getAttribute('type') || 'text').toLowerCase();
                    if (type === 'checkbox') return 'checkbox';
                    if (type === 'radio') return 'radio';
                    return 'input';
                }
                if (tag === 'select') return 'select';
                if (tag === 'textarea') return 'textarea';
                if (/^h[1-6]$/.test(tag)) return 'heading';
                if (tag === 'nav') return 'navigation';
                if (tag === 'main') return 'main';
                if (tag === 'header') return 'banner';
                if (tag === 'footer') return 'contentinfo';
                return null;
            }

            function getName(el) {
                // ARIA label first
                const ariaLabel = el.getAttribute('aria-label');
                if (ariaLabel) return ariaLabel.trim();
                // Placeholder for inputs
                const placeholder = el.getAttribute('placeholder');
                if (placeholder) return placeholder.trim();
                // Alt text for images
                const alt = el.getAttribute('alt');
                if (alt) return alt.trim();
                // Text content
                const text = el.textContent || '';
                if (text.trim()) return text.trim().substring(0, 100);
                // Value for inputs
                const value = el.getAttribute('value') || el.getAttribute('name') || '';
                return value.trim() || tag;
            }

            function getSelector(el) {
                if (el.id) return '#' + CSS.escape(el.id);
                const tag = el.tagName.toLowerCase();
                if (el.name) return tag + '[name="' + CSS.escape(el.name) + '"]';
                // Build a path
                const path = [];
                let current = el;
                while (current && current !== document.body) {
                    let selector = current.tagName.toLowerCase();
                    if (current.id) {
                        selector = '#' + CSS.escape(current.id);
                        path.unshift(selector);
                        break;
                    }
                    const parent = current.parentElement;
                    if (parent) {
                        const siblings = Array.from(parent.children).filter(c => c.tagName === current.tagName);
                        if (siblings.length > 1) {
                            const index = siblings.indexOf(current) + 1;
                            selector += ':nth-of-type(' + index + ')';
                        }
                    }
                    path.unshift(selector);
                    current = current.parentElement;
                }
                return path.join(' > ');
            }

            function getDescription(el) {
                const tag = el.tagName.toLowerCase();
                const role = getRole(el);
                const name = getName(el);
                let desc = name;
                if (tag === 'input') {
                    const type = (el.getAttribute('type') || 'text').toLowerCase();
                    desc = type + ' "' + name + '"';
                } else if (tag === 'a') {
                    const href = el.getAttribute('href') || '';
                    desc = 'link "' + name + '"' + (href ? ' -> ' + href : '');
                } else if (/^h[1-6]$/.test(tag)) {
                    const level = tag[1];
                    desc = 'h' + level + ' "' + name + '"';
                }
                return desc;
            }

            // Walk all elements in DOM order
            const walker = document.createTreeWalker(
                document.body,
                NodeFilter.SHOW_ELEMENT,
                {
                    acceptNode: function(node) {
                        const role = getRole(node);
                        if (role && interactiveRoles.includes(role)) {
                            // Skip hidden elements
                            const style = window.getComputedStyle(node);
                            if (style.display === 'none' || style.visibility === 'hidden') {
                                return NodeFilter.FILTER_REJECT;
                            }
                            return NodeFilter.FILTER_ACCEPT;
                        }
                        return NodeFilter.FILTER_SKIP;
                    }
                }
            );

            let node;
            while (node = walker.nextNode()) {
                const role = getRole(node);
                const name = getName(node);
                const selector = getSelector(node);
                const description = getDescription(node);
                elements.push({
                    role: role,
                    name: name,
                    selector: selector,
                    description: description
                });
            }

            return JSON.stringify(elements);
        })()
        "#;

        let result = self.browser.evaluate(js.to_string()).await?;
        let extracted: Vec<ExtractedElement> = match serde_json::from_str(&result) {
            Ok(v) => v,
            Err(_) => Vec::new(),
        };

        // Build element refs
        let mut elements = Vec::new();
        for (i, ext) in extracted.into_iter().take(self.config.max_refs).enumerate() {
            elements.push(ElementRef {
                ref_id: format!("@e{}", i + 1),
                role: ext.role,
                name: ext.name,
                selector: ext.selector,
                description: ext.description,
            });
        }

        let snapshot = AccessibilitySnapshot {
            url,
            title,
            elements,
            timestamp_ms: current_timestamp_ms(),
        };

        // Store in session map
        let mut maps = self.session_maps.write().await;
        if let Some(map) = maps.get_mut(session_id) {
            map.ref_counter = snapshot.elements.len();
            map.snapshot = Some(snapshot.clone());
        }

        Ok(snapshot)
    }

    /// Get the latest snapshot for a session.
    pub async fn get_snapshot(&self, session_id: &str) -> Option<AccessibilitySnapshot> {
        let maps = self.session_maps.read().await;
        maps.get(session_id).and_then(|m| m.snapshot.clone())
    }

    /// Get the ref list (formatted string) for a session.
    pub async fn get_ref_list(&self, session_id: &str) -> Option<String> {
        self.get_snapshot(session_id).await.map(|s| s.to_ref_list())
    }

    /// Resolve a ref string to a CSS selector for a given session.
    pub async fn resolve_ref(&self, session_id: &str, ref_str: &str) -> Result<String> {
        let maps = self.session_maps.read().await;
        let map = maps
            .get(session_id)
            .ok_or_else(|| crate::Error::Tool(format!("Session {session_id} not registered")))?;

        let snapshot = map.snapshot.as_ref().ok_or_else(|| {
            crate::Error::Tool(format!(
                "No accessibility snapshot for session {session_id}"
            ))
        })?;

        let elem = snapshot.find_by_ref(ref_str).ok_or_else(|| {
            crate::Error::Tool(format!("Element {ref_str} not found in snapshot"))
        })?;

        Ok(elem.selector.clone())
    }

    /// Click an element by ref.
    pub async fn click_ref(&self, session_id: &str, ref_str: &str) -> Result<()> {
        let selector = self.resolve_ref(session_id, ref_str).await?;
        self.browser.click(selector).await
    }

    /// Type text into an element by ref.
    pub async fn type_ref(&self, session_id: &str, ref_str: &str, text: &str) -> Result<()> {
        let selector = self.resolve_ref(session_id, ref_str).await?;
        self.browser.type_text(selector, text.to_string()).await
    }

    /// Read the text of an element by ref.
    pub async fn read_ref(&self, session_id: &str, ref_str: &str) -> Result<String> {
        let selector = self.resolve_ref(session_id, ref_str).await?;
        self.browser.read_text(selector).await
    }

    /// Click by CSS selector (direct, bypassing refs).
    pub async fn click(&self, selector: &str) -> Result<()> {
        self.browser.click(selector.to_string()).await
    }

    /// Type by CSS selector (direct, bypassing refs).
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<()> {
        self.browser
            .type_text(selector.to_string(), text.to_string())
            .await
    }

    /// Get current URL.
    pub async fn current_url(&self) -> Result<String> {
        self.browser.current_url().await
    }

    /// Get page title.
    pub async fn title(&self) -> Result<String> {
        self.browser.title().await
    }

    /// Take a screenshot.
    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        self.browser.screenshot().await
    }

    /// Execute JavaScript.
    pub async fn evaluate(&self, js: &str) -> Result<String> {
        self.browser.evaluate(js.to_string()).await
    }

    /// Get page HTML content.
    pub async fn get_content(&self) -> Result<String> {
        self.browser.get_content().await
    }

    /// Reload the current page.
    pub async fn reload(&self) -> Result<()> {
        self.browser.reload().await
    }

    /// Get the number of registered sessions.
    pub async fn session_count(&self) -> usize {
        self.session_maps.read().await.len()
    }

    /// Close the browser daemon and all sessions.
    pub async fn close(&self) -> Result<()> {
        self.browser.close().await?;
        self.session_maps.write().await.clear();
        Ok(())
    }
}

// Internal struct for parsing JS-extracted elements
#[derive(Debug, Deserialize)]
struct ExtractedElement {
    role: String,
    name: String,
    selector: String,
    description: String,
}

/// Returns the current timestamp in milliseconds.
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_ref_display() {
        let elem = ElementRef {
            ref_id: "@e1".to_string(),
            role: "button".to_string(),
            name: "Submit".to_string(),
            selector: "#submit-btn".to_string(),
            description: "button \"Submit\"".to_string(),
        };
        assert_eq!(elem.to_string(), "@e1 button \"Submit\"");
    }

    #[test]
    fn test_element_ref_serialization() {
        let elem = ElementRef {
            ref_id: "@e1".to_string(),
            role: "link".to_string(),
            name: "Home".to_string(),
            selector: "a[href='/']".to_string(),
            description: "link \"Home\" -> /".to_string(),
        };
        let json = serde_json::to_string(&elem).unwrap();
        let parsed: ElementRef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ref_id, "@e1");
        assert_eq!(parsed.role, "link");
    }

    #[test]
    fn test_accessibility_snapshot_find_by_ref() {
        let snapshot = AccessibilitySnapshot {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            elements: vec![
                ElementRef {
                    ref_id: "@e1".to_string(),
                    role: "button".to_string(),
                    name: "Click Me".to_string(),
                    selector: "#btn".to_string(),
                    description: "button \"Click Me\"".to_string(),
                },
                ElementRef {
                    ref_id: "@e2".to_string(),
                    role: "input".to_string(),
                    name: "Search".to_string(),
                    selector: "#search".to_string(),
                    description: "text \"Search\"".to_string(),
                },
            ],
            timestamp_ms: 1000,
        };

        assert!(snapshot.find_by_ref("@e1").is_some());
        assert!(snapshot.find_by_ref("e1").is_some()); // without @
        assert!(snapshot.find_by_ref("@e99").is_none());
    }

    #[test]
    fn test_accessibility_snapshot_to_ref_list() {
        let snapshot = AccessibilitySnapshot {
            url: "https://example.com".to_string(),
            title: "Test Page".to_string(),
            elements: vec![ElementRef {
                ref_id: "@e1".to_string(),
                role: "heading".to_string(),
                name: "Welcome".to_string(),
                selector: "h1".to_string(),
                description: "h1 \"Welcome\"".to_string(),
            }],
            timestamp_ms: 1000,
        };

        let list = snapshot.to_ref_list();
        assert!(list.contains("Test Page"));
        assert!(list.contains("@e1 heading \"Welcome\""));
    }

    #[test]
    fn test_session_element_map_new() {
        let map = SessionElementMap::new("sprint-1");
        assert_eq!(map.session_id, "sprint-1");
        assert!(map.snapshot.is_none());
        assert_eq!(map.ref_counter, 0);
    }

    #[test]
    fn test_browser_daemon_config_default() {
        let config = BrowserDaemonConfig::default();
        assert!(config.headless);
        assert_eq!(config.width, 1920);
        assert_eq!(config.max_refs, 200);
        assert!(config.auto_snapshot);
    }

    #[tokio::test]
    async fn test_browser_daemon_stub_creation() {
        let daemon = BrowserDaemon::new_stub();
        assert_eq!(daemon.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_browser_daemon_session_registration() {
        let daemon = BrowserDaemon::new_stub();
        daemon.register_session("sprint-1").await;
        daemon.register_session("sprint-2").await;
        assert_eq!(daemon.session_count().await, 2);

        daemon.unregister_session("sprint-1").await;
        assert_eq!(daemon.session_count().await, 1);
    }

    #[tokio::test]
    async fn test_browser_daemon_navigate_stub() {
        let daemon = BrowserDaemon::new_stub();
        daemon.register_session("sprint-1").await;
        daemon
            .navigate("https://example.com", Some("sprint-1"))
            .await
            .unwrap();

        let url = daemon.current_url().await.unwrap();
        assert_eq!(url, "https://example.com");
    }

    #[tokio::test]
    async fn test_browser_daemon_resolve_ref_missing_snapshot() {
        let daemon = BrowserDaemon::new_stub();
        daemon.register_session("sprint-1").await;

        let result = daemon.resolve_ref("sprint-1", "@e1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_browser_daemon_close() {
        let daemon = BrowserDaemon::new_stub();
        daemon.register_session("sprint-1").await;
        daemon.close().await.unwrap();
        assert_eq!(daemon.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_accessibility_snapshot_serialization() {
        let snapshot = AccessibilitySnapshot {
            url: "https://example.com".to_string(),
            title: "Test".to_string(),
            elements: vec![ElementRef {
                ref_id: "@e1".to_string(),
                role: "button".to_string(),
                name: "Go".to_string(),
                selector: "#go".to_string(),
                description: "button \"Go\"".to_string(),
            }],
            timestamp_ms: 5000,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: AccessibilitySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.elements.len(), 1);
        assert_eq!(parsed.elements[0].ref_id, "@e1");
    }

    #[test]
    fn test_browser_daemon_config_serialization() {
        let config = BrowserDaemonConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: BrowserDaemonConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.width, 1920);
        assert_eq!(parsed.height, 1080);
    }
}
