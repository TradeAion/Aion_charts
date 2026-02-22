//! Line series visual options — color, width, dash style, etc.

/// Line dash style — matches LWC's LineStyle enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
    LargeDashed,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self::Solid
    }
}

impl LineStyle {
    /// Returns (dash_len, gap_len) in CSS px. (0, 0) = solid.
    pub fn dash_pattern(&self) -> (f64, f64) {
        match self {
            Self::Solid => (0.0, 0.0),
            Self::Dashed => (6.0, 4.0),
            Self::Dotted => (1.0, 2.0),
            Self::LargeDashed => (12.0, 6.0),
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
        // TradingView blue: #2962FF
        Self {
            color: [0.161, 0.384, 1.0, 1.0],
            line_width: 2.0,
            line_style: LineStyle::Solid,
            crosshair_marker_visible: true,
            crosshair_marker_radius: 4.0,
            visible: true,
            title: String::new(),
        }
    }
}
