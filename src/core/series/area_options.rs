//! Area series visual options — line color, fill gradient, etc.
//!
//! An area series is a line series with a filled region between the line
//! and the bottom of the chart (or a custom base value). reference implementation supports
//! gradient fills with separate top/bottom colors.

/// Visual options for an area series.
#[derive(Debug, Clone)]
pub struct AreaSeriesOptions {
    /// Line color [R, G, B, A] (0.0–1.0). Default: #2962FF (default blue).
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
    /// reference implementation calls this `baseValue` with type `BaseValuePrice`.
    pub base_value: Option<f64>,
}

impl Default for AreaSeriesOptions {
    fn default() -> Self {
        let theme = crate::core::renderer::theme::ThemeConfig::default();
        Self {
            line_color: theme.series_defaults.area_line_color,
            line_width: 2.0,
            top_color: theme.series_defaults.area_top_fill,
            bottom_color: theme.series_defaults.area_bottom_fill,
            invert_filled_area: false,
            crosshair_marker_visible: true,
            crosshair_marker_radius: 4.0,
            visible: true,
            title: String::new(),
            base_value: None,
        }
    }
}
