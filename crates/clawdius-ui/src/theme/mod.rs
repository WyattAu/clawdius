//! Theme definitions for Spatial Materialism design language.

/// Brand colors matching the Clawdius landing page.
pub mod colors {
    pub const BG_PRIMARY: &str = "#0a0a0a";
    pub const BG_SECONDARY: &str = "#111111";
    pub const BG_SURFACE: &str = "#1a1a1a";
    pub const BG_ELEVATED: &str = "#222222";
    pub const TEXT_PRIMARY: &str = "#e8e8e8";
    pub const TEXT_SECONDARY: &str = "#888888";
    pub const TEXT_MUTED: &str = "#555555";
    pub const ACCENT: &str = "#c0ff00";
    pub const ACCENT_DIM: &str = "#7ab800";
    pub const ACCENT_BG: &str = "#1a2e00";
    pub const BORDER: &str = "#2a2a2a";
    pub const BORDER_FOCUS: &str = "#3a3a3a";
    pub const ERROR: &str = "#ff4444";
    pub const ERROR_BG: &str = "#2a0000";
    pub const WARNING: &str = "#ffaa00";
    pub const WARNING_BG: &str = "#2a1e00";
    pub const SUCCESS: &str = "#00cc66";
    pub const SUCCESS_BG: &str = "#002a12";
    pub const DIFF_ADDED: &str = "#1a3a1a";
    pub const DIFF_REMOVED: &str = "#3a1a1a";
    pub const DIFF_ADDED_TEXT: &str = "#4ade80";
    pub const DIFF_REMOVED_TEXT: &str = "#f87171";
    pub const CODE_BG: &str = "#0d0d0d";
    pub const USER_MSG_BG: &str = "#1a1a2e";
    pub const ASSISTANT_MSG_BG: &str = "#141414";
}

/// Typography choices.
pub mod typography {
    pub const FONT_MONO: &str = "JetBrains Mono, monospace";
    pub const FONT_SANS: &str = "Inter, sans-serif";
    pub const FONT_DISPLAY: &str = "Space Grotesk, sans-serif";
    pub const SIZE_XS: &str = "0.625rem";
    pub const SIZE_SM: &str = "0.75rem";
    pub const SIZE_BASE: &str = "0.875rem";
    pub const SIZE_MD: &str = "1rem";
    pub const SIZE_LG: &str = "1.125rem";
    pub const SIZE_XL: &str = "1.25rem";
    pub const SIZE_2XL: &str = "1.5rem";
    pub const SIZE_3XL: &str = "2rem";
    pub const LINE_HEIGHT_TIGHT: &str = "1.25";
    pub const LINE_HEIGHT_NORMAL: &str = "1.5";
    pub const LINE_HEIGHT_RELAXED: &str = "1.75";
    pub const WEIGHT_NORMAL: &str = "400";
    pub const WEIGHT_MEDIUM: &str = "500";
    pub const WEIGHT_SEMIBOLD: &str = "600";
    pub const WEIGHT_BOLD: &str = "700";
}

/// Spacing scale (px).
pub mod spacing {
    pub const SPACE_4: &str = "4px";
    pub const SPACE_8: &str = "8px";
    pub const SPACE_12: &str = "12px";
    pub const SPACE_16: &str = "16px";
    pub const SPACE_24: &str = "24px";
    pub const SPACE_32: &str = "32px";
    pub const SPACE_48: &str = "48px";
    pub const SPACE_64: &str = "64px";
    pub const SPACE_96: &str = "96px";
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
    pub const XL: &str = "1rem";
    pub const FULL: &str = "9999px";
}

/// Box shadow tokens.
pub mod shadow {
    pub const SM: &str = "0 1px 2px rgba(0,0,0,0.3)";
    pub const MD: &str = "0 4px 6px rgba(0,0,0,0.4)";
    pub const LG: &str = "0 10px 25px rgba(0,0,0,0.5)";
}

/// Transition duration tokens.
pub mod transition {
    pub const FAST: &str = "100ms";
    pub const NORMAL: &str = "200ms";
    pub const SLOW: &str = "350ms";
    pub const EASING: &str = "cubic-bezier(0.4, 0, 0.2, 1)";
}

/// Z-index scale for layering.
pub mod z_index {
    pub const BASE: i32 = 0;
    pub const DROPDOWN: i32 = 100;
    pub const STICKY: i32 = 200;
    pub const OVERLAY: i32 = 300;
    pub const MODAL: i32 = 400;
    pub const POPOVER: i32 = 500;
    pub const TOAST: i32 = 600;
    pub const TOOLTIP: i32 = 700;
}

/// Responsive breakpoint tokens (min-width).
pub mod breakpoint {
    pub const SM: &str = "640px";
    pub const MD: &str = "768px";
    pub const LG: &str = "1024px";
    pub const XL: &str = "1280px";
    pub const XXL: &str = "1536px";
}
