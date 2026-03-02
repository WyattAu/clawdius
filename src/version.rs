//! Version information for Clawdius

/// Current version string
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version components
pub struct VersionInfo {
    /// Major version
    pub major: u32,
    /// Minor version
    pub minor: u32,
    /// Patch version
    pub patch: u32,
}

impl VersionInfo {
    /// Parse version from string
    ///
    /// # Panics
    /// Panics if the version string is malformed.
    #[must_use]
    pub fn parse(version: &str) -> Self {
        let parts: Vec<&str> = version.split('.').collect();
        Self {
            major: parts.first().unwrap_or(&"0").parse().unwrap_or(0),
            minor: parts.get(1).unwrap_or(&"0").parse().unwrap_or(0),
            patch: parts.get(2).unwrap_or(&"0").parse().unwrap_or(0),
        }
    }

    /// Get the current version
    #[must_use]
    pub fn current() -> Self {
        Self::parse(VERSION)
    }
}

impl std::fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse() {
        let v = VersionInfo::parse("0.1.0");
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_version_display() {
        let v = VersionInfo::parse("1.2.3");
        assert_eq!(format!("{}", v), "1.2.3");
    }
}
