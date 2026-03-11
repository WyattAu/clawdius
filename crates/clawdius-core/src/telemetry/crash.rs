//! Crash reporting and error tracking

use crate::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};

#[allow(dead_code)]
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Crash reporter for tracking errors and panics
pub struct CrashReporter {
    #[allow(dead_code)]
    dsn: Option<String>,
    enabled: bool,
}

impl CrashReporter {
    /// Create a new crash reporter
    pub fn new() -> Self {
        let dsn = std::env::var("SENTRY_DSN").ok().filter(|s| !s.is_empty());
        let enabled = dsn.is_some();

        #[cfg(feature = "crash-reporting")]
        if let Some(ref dsn_str) = dsn {
            if !INITIALIZED.swap(true, Ordering::SeqCst) {
                let _ = sentry::init((
                    dsn_str.as_str(),
                    sentry::ClientOptions {
                        release: Some(env!("CARGO_PKG_VERSION").into()),
                        ..Default::default()
                    },
                ));
            }
        }

        Self { dsn, enabled }
    }

    /// Create a crash reporter with explicit DSN
    pub fn with_dsn(dsn: Option<String>) -> Self {
        let dsn = dsn.filter(|s| !s.is_empty());
        let enabled = dsn.is_some();

        #[cfg(feature = "crash-reporting")]
        if let Some(ref dsn_str) = dsn {
            if !INITIALIZED.swap(true, Ordering::SeqCst) {
                let _ = sentry::init((
                    dsn_str.as_str(),
                    sentry::ClientOptions {
                        release: Some(env!("CARGO_PKG_VERSION").into()),
                        ..Default::default()
                    },
                ));
            }
        }

        Self { dsn, enabled }
    }

    /// Check if crash reporting is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Capture an error
    pub fn capture_error(&self, error: &Error) {
        #[cfg(feature = "crash-reporting")]
        if self.enabled {
            sentry::capture_error(error);
        }
        let _ = error; // Suppress unused warning when feature disabled
    }

    /// Capture a message
    pub fn capture_message(&self, msg: &str) {
        #[cfg(feature = "crash-reporting")]
        if self.enabled {
            sentry::capture_message(msg, sentry::Level::Info);
        }
        let _ = msg; // Suppress unused warning when feature disabled
    }

    /// Capture a message with a specific level
    #[cfg(feature = "crash-reporting")]
    pub fn capture_message_with_level(&self, msg: &str, level: sentry::Level) {
        if self.enabled {
            sentry::capture_message(msg, level);
        }
    }

    /// Add a breadcrumb for context
    pub fn add_breadcrumb(&self, message: &str, category: &str) {
        #[cfg(feature = "crash-reporting")]
        if self.enabled {
            sentry::add_breadcrumb(sentry::Breadcrumb {
                ty: "default".into(),
                level: sentry::Level::Info,
                category: Some(category.into()),
                message: Some(message.into()),
                ..Default::default()
            });
        }
        let _ = (message, category); // Suppress unused warning when feature disabled
    }

    /// Set user context
    pub fn set_user(&self, id: Option<&str>, email: Option<&str>, username: Option<&str>) {
        #[cfg(feature = "crash-reporting")]
        if self.enabled {
            sentry::configure_scope(|scope| {
                scope.set_user(Some(sentry::User {
                    id: id.map(|s| s.into()),
                    email: email.map(|s| s.into()),
                    username: username.map(|s| s.into()),
                    ..Default::default()
                }));
            });
        }
        let _ = (id, email, username); // Suppress unused warning when feature disabled
    }

    /// Set a custom tag
    pub fn set_tag(&self, key: &str, value: &str) {
        #[cfg(feature = "crash-reporting")]
        if self.enabled {
            sentry::configure_scope(|scope| {
                scope.set_tag(key, value);
            });
        }
        let _ = (key, value); // Suppress unused warning when feature disabled
    }

    /// Set extra context data
    pub fn set_extra(&self, key: &str, value: &str) {
        #[cfg(feature = "crash-reporting")]
        if self.enabled {
            sentry::configure_scope(|scope| {
                scope.set_extra(key, value.into());
            });
        }
        let _ = (key, value); // Suppress unused warning when feature disabled
    }
}

impl Default for CrashReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_reporter_with_none_dsn() {
        let reporter = CrashReporter::with_dsn(None);
        assert!(!reporter.is_enabled());
    }
}
