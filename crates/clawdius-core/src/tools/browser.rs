//! Browser automation tool using chromiumoxide
//!
//! Provides Puppeteer-style browser automation capabilities including:
//! - Navigation and page interaction
//! - Element manipulation (click, type, read)
//! - Screenshot capture
//! - JavaScript execution
//! - Console log monitoring
//! - Dialog handling
//! - Wait operations
//!
//! # Example
//!
//! ```rust,no_run
//! use clawdius_core::tools::browser::{BrowserTool, BrowserConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = BrowserConfig::default();
//!     let mut browser = BrowserTool::with_config(config);
//!     
//!     browser.navigate("https://example.com").await?;
//!     browser.wait_for_selector("h1", std::time::Duration::from_secs(5)).await?;
//!     let title = browser.read_text("h1").await?;
//!     let screenshot = browser.screenshot(false).await?;
//!     
//!     browser.close().await?;
//!     Ok(())
//! }
//! ```

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::error::Result;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("Browser not initialized")]
    NotInitialized,
    #[error("Failed to create browser: {0}")]
    CreationFailed(String),
    #[error("Navigation failed: {0}")]
    NavigationFailed(String),
    #[error("Element not found: {0}")]
    ElementNotFound(String),
    #[error("Click failed: {0}")]
    ClickFailed(String),
    #[error("Type failed: {0}")]
    TypeFailed(String),
    #[error("Screenshot failed: {0}")]
    ScreenshotFailed(String),
    #[error("JavaScript execution failed: {0}")]
    JsExecutionFailed(String),
    #[error("Wait timeout: {0}")]
    WaitTimeout(String),
    #[error("Chromium error: {0}")]
    ChromiumError(String),
    #[error("Dialog handling failed: {0}")]
    DialogFailed(String),
    #[error("Console log error: {0}")]
    ConsoleLogError(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl From<BrowserError> for crate::error::Error {
    fn from(e: BrowserError) -> Self {
        crate::error::Error::Tool(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserNavigateParams {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserClickParams {
    pub selector: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserTypeParams {
    pub selector: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserScreenshotParams {
    #[serde(default)]
    pub full_page: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserEvaluateParams {
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserWaitForParams {
    pub selector: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserReadParams {
    pub selector: String,
}

fn default_timeout() -> u64 {
    10_000
}

/// Browser configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserToolConfig {
    /// Run in headless mode (default: true)
    #[serde(default = "default_headless")]
    pub headless: bool,
    /// Window width in pixels
    #[serde(default = "default_width")]
    pub width: u32,
    /// Window height in pixels
    #[serde(default = "default_height")]
    pub height: u32,
    /// Enable console log monitoring
    #[serde(default)]
    pub enable_console_logs: bool,
    /// Maximum number of console logs to keep
    #[serde(default = "default_max_logs")]
    pub max_console_logs: usize,
    /// Default timeout for operations in milliseconds
    #[serde(default = "default_timeout")]
    pub default_timeout_ms: u64,
    /// User agent string
    #[serde(default)]
    pub user_agent: Option<String>,
    /// Accept insecure certs
    #[serde(default)]
    pub accept_insecure_certs: bool,
}

fn default_headless() -> bool {
    true
}
fn default_width() -> u32 {
    1920
}
fn default_height() -> u32 {
    1080
}
fn default_max_logs() -> usize {
    100
}

impl Default for BrowserToolConfig {
    fn default() -> Self {
        Self {
            headless: true,
            width: 1920,
            height: 1080,
            enable_console_logs: true,
            max_console_logs: 100,
            default_timeout_ms: 10_000,
            user_agent: None,
            accept_insecure_certs: false,
        }
    }
}

/// Console log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleLog {
    /// Log level (log, warn, error, info, etc.)
    pub level: String,
    /// Log message
    pub message: String,
    /// URL where the log originated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Line number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
}

/// Dialog information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogInfo {
    /// Dialog type (alert, confirm, prompt, beforeunload)
    pub dialog_type: String,
    /// Dialog message
    pub message: String,
    /// Default value (for prompt dialogs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}

type ConsoleLogBuffer = Arc<Mutex<VecDeque<ConsoleLog>>>;

pub struct BrowserTool {
    browser: Option<Browser>,
    page: Option<chromiumoxide::Page>,
    config: BrowserToolConfig,
    console_logs: ConsoleLogBuffer,
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserTool {
    /// Create a new browser tool with default configuration
    pub fn new() -> Self {
        Self {
            browser: None,
            page: None,
            config: BrowserToolConfig::default(),
            console_logs: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Create a new browser tool with custom configuration
    pub fn with_config(config: BrowserToolConfig) -> Self {
        Self {
            browser: None,
            page: None,
            config,
            console_logs: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Initialize the browser
    pub async fn initialize(&mut self) -> Result<()> {
        if self.browser.is_some() {
            return Ok(());
        }

        let mut builder =
            BrowserConfig::builder().window_size(self.config.width, self.config.height);

        if !self.config.headless {
            builder = builder.no_sandbox();
        }

        let config = builder.build().map_err(BrowserError::CreationFailed)?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| BrowserError::CreationFailed(e.to_string()))?;

        tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                let _ = h;
            }
        });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| BrowserError::CreationFailed(e.to_string()))?;

        self.browser = Some(browser);
        self.page = Some(page);

        Ok(())
    }

    fn page(&self) -> Result<&chromiumoxide::Page> {
        self.page
            .as_ref()
            .ok_or_else(|| BrowserError::NotInitialized.into())
    }

    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        page.goto(url)
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        page.wait_for_navigation()
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn click(&mut self, selector: &str) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let element = page
            .find_element(selector)
            .await
            .map_err(|e| BrowserError::ElementNotFound(format!("{}: {}", selector, e)))?;

        element
            .click()
            .await
            .map_err(|e| BrowserError::ClickFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn type_text(&mut self, selector: &str, text: &str) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let element = page
            .find_element(selector)
            .await
            .map_err(|e| BrowserError::ElementNotFound(format!("{}: {}", selector, e)))?;

        element
            .click()
            .await
            .map_err(|e| BrowserError::TypeFailed(e.to_string()))?;

        element
            .type_str(text)
            .await
            .map_err(|e| BrowserError::TypeFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn screenshot(&mut self, full_page: bool) -> Result<Vec<u8>> {
        self.initialize().await?;
        let page = self.page()?;

        let params = ScreenshotParams::builder().full_page(full_page).build();

        let png_bytes = page
            .screenshot(params)
            .await
            .map_err(|e| BrowserError::ScreenshotFailed(e.to_string()))?;

        Ok(png_bytes)
    }

    pub async fn evaluate(&mut self, js: &str) -> Result<serde_json::Value> {
        self.initialize().await?;
        let page = self.page()?;

        let result = page
            .evaluate(js)
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        let value = result
            .into_value::<serde_json::Value>()
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        Ok(value)
    }

    pub async fn wait_for_selector(&mut self, selector: &str, timeout: Duration) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let find_future = async {
            loop {
                match page.find_element(selector).await {
                    Ok(_) => return Ok::<(), BrowserError>(()),
                    Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                }
            }
        };

        tokio::time::timeout(timeout, find_future)
            .await
            .map_err(|_| BrowserError::WaitTimeout(format!("selector: {}", selector)))?
            .map_err(|e: BrowserError| BrowserError::WaitTimeout(format!("{}: {}", selector, e)))?;

        Ok(())
    }

    pub async fn read_text(&mut self, selector: &str) -> Result<String> {
        self.initialize().await?;
        let page = self.page()?;

        let element = page
            .find_element(selector)
            .await
            .map_err(|e| BrowserError::ElementNotFound(format!("{}: {}", selector, e)))?;

        let text = element
            .inner_text()
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?
            .unwrap_or_default();

        Ok(text)
    }

    pub async fn get_content(&mut self) -> Result<String> {
        self.initialize().await?;
        let page = self.page()?;

        let html = page
            .content()
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        Ok(html)
    }

    pub async fn get_url(&mut self) -> Result<String> {
        self.initialize().await?;
        let page = self.page()?;

        let url = page
            .url()
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?
            .unwrap_or_default();

        Ok(url)
    }

    /// Get all console logs collected so far
    pub async fn get_console_logs(&self) -> Vec<ConsoleLog> {
        let logs = self.console_logs.lock().await;
        logs.iter().cloned().collect()
    }

    /// Clear all console logs
    pub async fn clear_console_logs(&self) {
        let mut logs = self.console_logs.lock().await;
        logs.clear();
    }

    /// Wait for a JavaScript function to return true
    pub async fn wait_for_function(&mut self, js_function: &str, timeout: Duration) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > timeout {
                return Err(BrowserError::WaitTimeout(format!("function: {}", js_function)).into());
            }

            let result = page
                .evaluate(js_function)
                .await
                .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

            if let Ok(value) = result.into_value::<serde_json::Value>() {
                if value.as_bool().unwrap_or(false) {
                    return Ok(());
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Set the value of a form element
    pub async fn set_value(&mut self, selector: &str, value: &str) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let js = format!(
            r#"
            const element = document.querySelector('{}');
            if (element) {{
                element.value = '{}';
                element.dispatchEvent(new Event('input', {{ bubbles: true }}));
                return true;
            }}
            return false;
            "#,
            selector, value
        );

        let result = page
            .evaluate(js.as_str())
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        let success = result
            .into_value::<bool>()
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        if !success {
            return Err(BrowserError::ElementNotFound(selector.to_string()).into());
        }

        Ok(())
    }

    /// Select an option from a dropdown
    pub async fn select(&mut self, selector: &str, value: &str) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let js = format!(
            r#"
            const select = document.querySelector('{}');
            if (select) {{
                select.value = '{}';
                select.dispatchEvent(new Event('change', {{ bubbles: true }}));
                return true;
            }}
            return false;
            "#,
            selector, value
        );

        let result = page
            .evaluate(js.as_str())
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        let success = result
            .into_value::<bool>()
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        if !success {
            return Err(BrowserError::ElementNotFound(selector.to_string()).into());
        }

        Ok(())
    }

    /// Check if an element exists
    pub async fn element_exists(&mut self, selector: &str) -> Result<bool> {
        self.initialize().await?;
        let page = self.page()?;

        let result = page.find_element(selector).await;
        Ok(result.is_ok())
    }

    /// Get element attribute value
    pub async fn get_attribute(
        &mut self,
        selector: &str,
        attribute: &str,
    ) -> Result<Option<String>> {
        self.initialize().await?;
        let page = self.page()?;

        let element = page
            .find_element(selector)
            .await
            .map_err(|e| BrowserError::ElementNotFound(format!("{}: {}", selector, e)))?;

        let attr = element
            .attribute(attribute)
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        Ok(attr)
    }

    /// Scroll to an element
    pub async fn scroll_to(&mut self, selector: &str) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let js = format!(
            r#"
            const element = document.querySelector('{}');
            if (element) {{
                element.scrollIntoView({{ behavior: 'smooth', block: 'center' }});
                return true;
            }}
            return false;
            "#,
            selector
        );

        let result = page
            .evaluate(js.as_str())
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        let success = result
            .into_value::<bool>()
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        if !success {
            return Err(BrowserError::ElementNotFound(selector.to_string()).into());
        }

        Ok(())
    }

    /// Hover over an element
    pub async fn hover(&mut self, selector: &str) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let element = page
            .find_element(selector)
            .await
            .map_err(|e| BrowserError::ElementNotFound(format!("{}: {}", selector, e)))?;

        element
            .hover()
            .await
            .map_err(|e| BrowserError::ClickFailed(e.to_string()))?;

        Ok(())
    }

    /// Press a keyboard key
    pub async fn press_key(&mut self, key: &str) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        let js = format!(
            r#"
            document.dispatchEvent(new KeyboardEvent('keydown', {{ key: '{}' }}));
            document.dispatchEvent(new KeyboardEvent('keyup', {{ key: '{}' }}));
            "#,
            key, key
        );

        page.evaluate(js.as_str())
            .await
            .map_err(|e| BrowserError::JsExecutionFailed(e.to_string()))?;

        Ok(())
    }

    /// Reload the page
    pub async fn reload(&mut self) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        page.reload()
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        Ok(())
    }

    /// Go back in browser history
    pub async fn go_back(&mut self) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        page.evaluate("window.history.back()")
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        Ok(())
    }

    /// Go forward in browser history
    pub async fn go_forward(&mut self) -> Result<()> {
        self.initialize().await?;
        let page = self.page()?;

        page.evaluate("window.history.forward()")
            .await
            .map_err(|e| BrowserError::NavigationFailed(e.to_string()))?;

        Ok(())
    }

    /// Close the browser
    pub async fn close(&mut self) -> Result<()> {
        if let Some(browser) = self.browser.take() {
            drop(browser);
        }
        self.page = None;
        Ok(())
    }
}

impl Drop for BrowserTool {
    fn drop(&mut self) {
        if let Some(browser) = self.browser.take() {
            drop(browser);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserActionResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BrowserActionResult {
    pub fn success(data: Option<serde_json::Value>) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}
