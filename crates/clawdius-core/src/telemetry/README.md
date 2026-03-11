# Telemetry Module

This module provides crash reporting and error tracking for Clawdius using Sentry.

## Features

- **Optional Sentry Integration**: Crash reporting is optional and only enabled when the `crash-reporting` feature is enabled
- **Environment Variable Configuration**: Can be configured via `SENTRY_DSN` environment variable
- **Configuration File Support**: Can be configured via `.clawdius/config.toml`
- **Zero Overhead When Disabled**: No runtime cost when the feature is not enabled

## Usage

### Enable the Feature

Add to your `Cargo.toml`:

```toml
[dependencies]
clawdius-core = { version = "0.1.0", features = ["crash-reporting"] }
```

### Configuration

#### Option 1: Environment Variable

```bash
export SENTRY_DSN="https://your-key@sentry.io/project-id"
```

#### Option 2: Configuration File

In `.clawdius/config.toml`:

```toml
[telemetry]
crash_reporting = true
sentry_dsn = "https://your-key@sentry.io/project-id"
```

### Code Usage

```rust
use clawdius_core::telemetry::CrashReporter;

// Initialize crash reporter (reads from SENTRY_DSN env var)
let reporter = CrashReporter::new();

// Or with explicit DSN
let reporter = CrashReporter::with_dsn(Some("https://your-key@sentry.io/project-id".to_string()));

// Check if enabled
if reporter.is_enabled() {
    println!("Crash reporting is enabled");
}

// Capture errors
reporter.capture_error(&error);

// Capture messages
reporter.capture_message("Something went wrong");
reporter.capture_message_with_level("Critical error", sentry::Level::Error);

// Add context
reporter.add_breadcrumb("User clicked button", "ui");
reporter.set_tag("version", "1.0.0");
reporter.set_extra("user_id", "12345");
reporter.set_user(Some("12345"), Some("user@example.com"), Some("username"));
```

## Feature Flags

- `crash-reporting`: Enables Sentry integration for crash reporting

## Implementation Details

- Uses conditional compilation (`#[cfg(feature = "crash-reporting")]`) to ensure zero overhead when disabled
- Automatically registers panic handler when enabled
- Thread-safe initialization using atomic operations
- Filters empty DSN strings to prevent misconfiguration
