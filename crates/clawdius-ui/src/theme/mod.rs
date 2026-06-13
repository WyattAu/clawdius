//! Theme definitions for Spatial Materialism design language.

/// Brand colors matching the Clawdius landing page.
pub mod colors {
    pub const BG_PRIMARY: &str = "#0a0a0a";
    pub const BG_SECONDARY: &str = "#111111";
    pub const BG_SURFACE: &str = "#1a1a1a";
    pub const TEXT_PRIMARY: &str = "#e8e8e8";
    pub const TEXT_SECONDARY: &str = "#888888";
    pub const ACCENT: &str = "#c0ff00";
    pub const ACCENT_DIM: &str = "#7ab800";
    pub const BORDER: &str = "#2a2a2a";
    pub const ERROR: &str = "#ff4444";
    pub const WARNING: &str = "#ffaa00";
    pub const SUCCESS: &str = "#00cc66";
}

/// Typography choices.
pub mod typography {
    pub const FONT_MONO: &str = "JetBrains Mono, monospace";
    pub const FONT_SANS: &str = "Inter, sans-serif";
    pub const FONT_DISPLAY: &str = "Space Grotesk, sans-serif";
}

/// Spacing tokens (rem).
pub mod spacing {
    pub const XS: &str = "0.25rem";
    pub const SM: &str = "0.5rem";
    pub const MD: &str = "1rem";
    pub const LG: &str = "1.5rem";
    pub const XL: &str = "2rem";
    pub const XXL: &str = "3rem";
}

/// Border radius tokens.
pub mod radius {
    pub const NONE: &str = "0";
    pub const SM: &str = "0.25rem";
    pub const MD: &str = "0.5rem";
    pub const LG: &str = "0.75rem";
    pub const FULL: &str = "9999px";
}
