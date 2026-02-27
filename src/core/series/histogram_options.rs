//! Histogram series visual options — color, base value, etc.
//!
//! A histogram series renders vertical bars from a base value (default 0)
//! to the data value. Used for MACD histogram, volume profile overlays, etc.
//! LWC supports per-bar color overrides via the data array.

/// Visual options for a histogram series.
#[derive(Debug, Clone)]
pub struct HistogramSeriesOptions {
    /// Default bar color [R, G, B, A] (0.0–1.0). Default: #26a69a (teal).
    pub color: [f32; 4],
    /// Base value — bars extend from this value to the data value.
    /// Default: 0.0 (bars grow up from zero for positive values, down for negative).
    pub base: f64,
    /// Whether the series is visible.
    pub visible: bool,
    /// Display label / title for the series.
    pub title: String,
}

impl Default for HistogramSeriesOptions {
    fn default() -> Self {
        let theme = crate::core::renderer::theme::ThemeConfig::default();
        Self {
            color: theme.series_defaults.histogram_color,
            base: 0.0,
            visible: true,
            title: String::new(),
        }
    }
}
