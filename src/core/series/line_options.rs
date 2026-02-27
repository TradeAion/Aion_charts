//! Line series visual options — color, width, dash style, etc.

/// Line dash style — matches LWC's LineStyle enum.
///
/// Dash patterns are width-relative per the LWC specification:
/// - Solid = no dash
/// - Dotted = [w, w]
/// - Dashed = [2w, 2w]
/// - LargeDashed = [6w, 6w]
/// - SparseDotted = [w, 4w]
///
/// Where `w` = line width in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dotted,
    Dashed,
    LargeDashed,
    SparseDotted,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self::Solid
    }
}

impl LineStyle {
    /// Returns (dash_len, gap_len) in physical pixels, scaled by line width.
    /// (0, 0) = solid (no dash pattern).
    ///
    /// LWC dash table (w = line_width in physical px):
    /// - Solid: (0, 0)
    /// - Dotted: (w, w)
    /// - Dashed: (2w, 2w)
    /// - LargeDashed: (6w, 6w)
    /// - SparseDotted: (w, 4w)
    pub fn dash_pattern(&self, line_width: f64) -> (f64, f64) {
        let w = line_width.max(1.0);
        match self {
            Self::Solid => (0.0, 0.0),
            Self::Dotted => (w, w),
            Self::Dashed => (2.0 * w, 2.0 * w),
            Self::LargeDashed => (6.0 * w, 6.0 * w),
            Self::SparseDotted => (w, 4.0 * w),
        }
    }

    /// Whether this style requires Canvas2D strokePath rendering
    /// (as opposed to rect-based rendering).
    #[inline]
    pub fn is_dashed(&self) -> bool {
        !matches!(self, Self::Solid)
    }

    /// Parse from a string (for WASM API).
    /// Accepted values: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
    pub fn from_str(s: &str) -> Self {
        match s {
            "dotted" => Self::Dotted,
            "dashed" => Self::Dashed,
            "large_dashed" | "largeDashed" => Self::LargeDashed,
            "sparse_dotted" | "sparseDotted" => Self::SparseDotted,
            _ => Self::Solid,
        }
    }
}

/// Visual options for a line series.
#[derive(Debug, Clone)]
pub struct LineSeriesOptions {
    /// Line color [R, G, B, A] (0.0–1.0). Default: #2962FF (TradingView blue).
    pub color: [f32; 4],
    /// Line width in CSS px.
    pub line_width: f64,
    /// Dash style.
    pub line_style: LineStyle,
    /// Show circle marker at crosshair intersection.
    pub crosshair_marker_visible: bool,
    /// Crosshair marker radius in CSS px.
    pub crosshair_marker_radius: f64,
    /// Whether the series is visible.
    pub visible: bool,
    /// Display label / title for the series.
    pub title: String,
}

impl Default for LineSeriesOptions {
    fn default() -> Self {
        let theme = crate::core::renderer::theme::ThemeConfig::default();
        Self {
            color: theme.series_defaults.line_color,
            line_width: 2.0,
            line_style: LineStyle::Solid,
            crosshair_marker_visible: true,
            crosshair_marker_radius: 4.0,
            visible: true,
            title: String::new(),
        }
    }
}
