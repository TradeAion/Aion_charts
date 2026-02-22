//! Area series visual options — line color, fill gradient, etc.
//!
//! An area series is a line series with a filled region between the line
//! and the bottom of the chart (or a custom base value). LWC supports
//! gradient fills with separate top/bottom colors.

/// Visual options for an area series.
#[derive(Debug, Clone)]
pub struct AreaSeriesOptions {
    /// Line color [R, G, B, A] (0.0–1.0). Default: #2962FF (TradingView blue).
    pub line_color: [f32; 4],
    /// Line width in CSS px.
    pub line_width: f64,
    /// Top fill color (at the line). Default: semi-transparent blue.
    pub top_color: [f32; 4],
    /// Bottom fill color (at the base). Default: transparent.
    pub bottom_color: [f32; 4],
    /// Whether to invert the fill direction (fill above the line instead of below).
    pub invert_filled_area: bool,
    /// Show circle marker at crosshair intersection.
    pub crosshair_marker_visible: bool,
    /// Crosshair marker radius in CSS px.
    pub crosshair_marker_radius: f64,
    /// Whether the series is visible.
    pub visible: bool,
    /// Display label / title for the series.
    pub title: String,
    /// Base value for the fill area. If None, fills to the bottom of the pane.
    /// LWC calls this `baseValue` with type `BaseValuePrice`.
    pub base_value: Option<f64>,
}

impl Default for AreaSeriesOptions {
    fn default() -> Self {
        // LWC defaults: line #2962FF, topColor rgba(41,98,255,0.28),
        // bottomColor rgba(41,98,255,0.0)
        Self {
            line_color: [0.161, 0.384, 1.0, 1.0],
            line_width: 2.0,
            top_color: [0.161, 0.384, 1.0, 0.28],
            bottom_color: [0.161, 0.384, 1.0, 0.0],
            invert_filled_area: false,
            crosshair_marker_visible: true,
            crosshair_marker_radius: 4.0,
            visible: true,
            title: String::new(),
            base_value: None,
        }
    }
}
