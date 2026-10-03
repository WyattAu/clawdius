//! Theme definitions for Spatial Materialism design language.

/// Brand colors matching the Clawdius landing page.
pub mod colors {
    /// App background behind all content.
    pub const BG_PRIMARY: &str = "#0a0a0a";
    /// Sidebar and panel background.
    pub const BG_SECONDARY: &str = "#111111";
    /// Card and input background.
    pub const BG_SURFACE: &str = "#1a1a1a";
    /// Raised elements such as dropdowns and modals.
    pub const BG_ELEVATED: &str = "#222222";
    /// Primary text color.
    pub const TEXT_PRIMARY: &str = "#e8e8e8";
    /// Secondary text for labels and metadata.
    pub const TEXT_SECONDARY: &str = "#888888";
    /// De-emphasized text such as placeholders.
    pub const TEXT_MUTED: &str = "#555555";
    /// Brand accent for highlights and focus rings.
    pub const ACCENT: &str = "#c0ff00";
    /// Darker accent used on hover.
    pub const ACCENT_DIM: &str = "#7ab800";
    /// Tinted accent background for selected items.
    pub const ACCENT_BG: &str = "#1a2e00";
    /// Default hairline border color.
    pub const BORDER: &str = "#2a2a2a";
    /// Border color for focused inputs.
    pub const BORDER_FOCUS: &str = "#3a3a3a";
    /// Error text and icons.
    pub const ERROR: &str = "#ff4444";
    /// Tinted background behind error messages.
    pub const ERROR_BG: &str = "#2a0000";
    /// Warning text and icons.
    pub const WARNING: &str = "#ffaa00";
    /// Tinted background behind warnings.
    pub const WARNING_BG: &str = "#2a1e00";
    /// Success text and icons.
    pub const SUCCESS: &str = "#00cc66";
    /// Tinted background behind success states.
    pub const SUCCESS_BG: &str = "#002a12";
    /// Background for added diff lines.
    pub const DIFF_ADDED: &str = "#1a3a1a";
    /// Background for removed diff lines.
    pub const DIFF_REMOVED: &str = "#3a1a1a";
    /// Text color on added diff lines.
    pub const DIFF_ADDED_TEXT: &str = "#4ade80";
    /// Text color on removed diff lines.
    pub const DIFF_REMOVED_TEXT: &str = "#f87171";
    /// Background of code blocks.
    pub const CODE_BG: &str = "#0d0d0d";
    /// Background of user chat messages.
    pub const USER_MSG_BG: &str = "#1a1a2e";
    /// Background of assistant chat messages.
    pub const ASSISTANT_MSG_BG: &str = "#141414";
}

/// Typography choices.
pub mod typography {
    /// Monospace stack for code and token counts.
    pub const FONT_MONO: &str = "JetBrains Mono, monospace";
    /// Sans-serif stack for body text.
    pub const FONT_SANS: &str = "Inter, sans-serif";
    /// Display stack for headings.
    pub const FONT_DISPLAY: &str = "Space Grotesk, sans-serif";
    /// Extra-small font size.
    pub const SIZE_XS: &str = "0.625rem";
    /// Small font size.
    pub const SIZE_SM: &str = "0.75rem";
    /// Base body font size.
    pub const SIZE_BASE: &str = "0.875rem";
    /// Medium font size.
    pub const SIZE_MD: &str = "1rem";
    /// Large font size.
    pub const SIZE_LG: &str = "1.125rem";
    /// Extra-large font size.
    pub const SIZE_XL: &str = "1.25rem";
    /// 2x font size, section headings.
    pub const SIZE_2XL: &str = "1.5rem";
    /// 3x font size, page headings.
    pub const SIZE_3XL: &str = "2rem";
    /// Tight line height for headings.
    pub const LINE_HEIGHT_TIGHT: &str = "1.25";
    /// Default line height for body text.
    pub const LINE_HEIGHT_NORMAL: &str = "1.5";
    /// Relaxed line height for dense lists.
    pub const LINE_HEIGHT_RELAXED: &str = "1.75";
    /// Regular font weight.
    pub const WEIGHT_NORMAL: &str = "400";
    /// Medium font weight.
    pub const WEIGHT_MEDIUM: &str = "500";
    /// Semibold font weight.
    pub const WEIGHT_SEMIBOLD: &str = "600";
    /// Bold font weight.
    pub const WEIGHT_BOLD: &str = "700";
}

/// Spacing scale (px).
pub mod spacing {
    /// 4-pixel spacing step.
    pub const SPACE_4: &str = "4px";
    /// 8-pixel spacing step.
    pub const SPACE_8: &str = "8px";
    /// 12-pixel spacing step.
    pub const SPACE_12: &str = "12px";
    /// 16-pixel spacing step.
    pub const SPACE_16: &str = "16px";
    /// 24-pixel spacing step.
    pub const SPACE_24: &str = "24px";
    /// 32-pixel spacing step.
    pub const SPACE_32: &str = "32px";
    /// 48-pixel spacing step.
    pub const SPACE_48: &str = "48px";
    /// 64-pixel spacing step.
    pub const SPACE_64: &str = "64px";
    /// 96-pixel spacing step.
    pub const SPACE_96: &str = "96px";
    /// Extra-small spacing.
    pub const XS: &str = "0.25rem";
    /// Small spacing.
    pub const SM: &str = "0.5rem";
    /// Medium spacing.
    pub const MD: &str = "1rem";
    /// Large spacing.
    pub const LG: &str = "1.5rem";
    /// Extra-large spacing.
    pub const XL: &str = "2rem";
    /// Double-extra-large spacing.
    pub const XXL: &str = "3rem";
}

/// Border radius tokens.
pub mod radius {
    /// Square corners.
    pub const NONE: &str = "0";
    /// Small corner radius.
    pub const SM: &str = "0.25rem";
    /// Default corner radius.
    pub const MD: &str = "0.5rem";
    /// Large corner radius.
    pub const LG: &str = "0.75rem";
    /// Extra-large corner radius.
    pub const XL: &str = "1rem";
    /// Fully rounded (pill) corners.
    pub const FULL: &str = "9999px";
}

/// Box shadow tokens.
pub mod shadow {
    /// Subtle shadow for slightly raised elements.
    pub const SM: &str = "0 1px 2px rgba(0,0,0,0.3)";
    /// Default shadow for cards and panels.
    pub const MD: &str = "0 4px 6px rgba(0,0,0,0.4)";
    /// Prominent shadow for modals and popovers.
    pub const LG: &str = "0 10px 25px rgba(0,0,0,0.5)";
}

/// Transition duration tokens.
pub mod transition {
    /// Fast duration for micro-interactions.
    pub const FAST: &str = "100ms";
    /// Default duration for UI transitions.
    pub const NORMAL: &str = "200ms";
    /// Slow duration for large panels and overlays.
    pub const SLOW: &str = "350ms";
    /// Standard ease curve shared by all transitions.
    pub const EASING: &str = "cubic-bezier(0.4, 0, 0.2, 1)";
}

/// Z-index scale for layering.
pub mod z_index {
    /// Base stacking layer.
    pub const BASE: i32 = 0;
    /// Dropdown menus layer.
    pub const DROPDOWN: i32 = 100;
    /// Sticky headers layer.
    pub const STICKY: i32 = 200;
    /// Full-screen overlay layer.
    pub const OVERLAY: i32 = 300;
    /// Modal dialogs layer.
    pub const MODAL: i32 = 400;
    /// Popovers, above modals.
    pub const POPOVER: i32 = 500;
    /// Toast notifications layer.
    pub const TOAST: i32 = 600;
    /// Tooltips, the topmost layer.
    pub const TOOLTIP: i32 = 700;
}

/// Responsive breakpoint tokens (min-width).
pub mod breakpoint {
    /// Small screens and up.
    pub const SM: &str = "640px";
    /// Medium screens and up.
    pub const MD: &str = "768px";
    /// Large screens and up.
    pub const LG: &str = "1024px";
    /// Extra-large screens and up.
    pub const XL: &str = "1280px";
    /// Double-extra-large screens and up.
    pub const XXL: &str = "1536px";
}
