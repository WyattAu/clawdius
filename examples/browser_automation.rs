//! Example: Web scraping with browser automation
//! 
//! This example demonstrates how to use the browser tool for web scraping.

use clawdius_core::tools::browser::{BrowserTool, BrowserToolConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create browser with custom configuration
    let config = BrowserToolConfig {
        headless: true,
        width: 1920,
        height: 1080,
        enable_console_logs: true,
        max_console_logs: 100,
        default_timeout_ms: 10_000,
        user_agent: None,
        accept_insecure_certs: false,
    };
    
    let mut browser = BrowserTool::with_config(config);
    
    // Example 1: Basic page scraping
    println!("=== Example 1: Basic Page Scraping ===");
    browser.navigate("https://example.com").await?;
    
    // Wait for page to load
    browser.wait_for_selector("h1", Duration::from_secs(5)).await?;
    
    // Extract data
    let title = browser.read_text("h1").await?;
    println!("Page title: {}", title);
    
    let url = browser.get_url().await?;
    println!("Current URL: {}", url);
    
    // Take screenshot
    let screenshot = browser.screenshot(false).await?;
    std::fs::write("example_screenshot.png", screenshot)?;
    println!("Screenshot saved to example_screenshot.png");
    
    // Example 2: Form interaction
    println!("\n=== Example 2: Form Interaction ===");
    
    // Simulate form filling (using example.com's form if it had one)
    // This is a demonstration of the API
    
    // Type into input fields
    // browser.type_text("#username", "user@example.com").await?;
    // browser.type_text("#password", "secret123").await?;
    
    // Select from dropdown
    // browser.select("#country", "US").await?;
    
    // Click button
    // browser.click("#submit-button").await?;
    
    // Example 3: JavaScript execution
    println!("\n=== Example 3: JavaScript Execution ===");
    
    let page_title = browser.evaluate("document.title").await?;
    println!("Document title (via JS): {:?}", page_title);
    
    let link_count = browser.evaluate("document.querySelectorAll('a').length").await?;
    println!("Number of links on page: {:?}", link_count);
    
    // Example 4: Advanced waiting
    println!("\n=== Example 4: Advanced Waiting ===");
    
    // Wait for a custom condition
    // browser.wait_for_function(
    //     "window.dataLoaded === true",
    //     Duration::from_secs(10)
    // ).await?;
    
    // Example 5: Console log monitoring
    println!("\n=== Example 5: Console Log Monitoring ===");
    
    // Execute JavaScript that logs to console
    browser.evaluate("console.log('Hello from browser tool!')").await?;
    
    // Retrieve console logs
    let logs = browser.get_console_logs().await;
    println!("Console logs collected: {} entries", logs.len());
    
    // Clear logs
    browser.clear_console_logs().await?;
    
    // Example 6: Navigation
    println!("\n=== Example 6: Navigation ===");
    
    // Navigate to another page
    // browser.navigate("https://example.org").await?;
    
    // Go back
    // browser.go_back().await?;
    
    // Go forward
    // browser.go_forward().await?;
    
    // Reload page
    browser.reload().await?;
    println!("Page reloaded");
    
    // Example 7: Element inspection
    println!("\n=== Example 7: Element Inspection ===");
    
    // Check if element exists
    let exists = browser.element_exists("h1").await?;
    println!("H1 element exists: {}", exists);
    
    // Get element attribute
    if exists {
        let class = browser.get_attribute("h1", "class").await?;
        println!("H1 class attribute: {:?}", class);
    }
    
    // Example 8: Scrolling
    println!("\n=== Example 8: Scrolling ===");
    
    // Scroll to element
    // browser.scroll_to("#footer").await?;
    // println!("Scrolled to footer");
    
    // Example 9: Advanced interaction
    println!("\n=== Example 9: Advanced Interaction ===");
    
    // Hover over element
    // browser.hover("#menu-item").await?;
    
    // Press keyboard key
    // browser.press_key("Enter").await?;
    
    // Cleanup
    browser.close().await?;
    println!("\n=== Browser closed ===");
    
    Ok(())
}
