//! Bar (OHLC) series visual options.
//!
//! An OHLC bar series renders each data point as a vertical line (high-low)
//! with horizontal ticks for open (left) and close (right). This is the
//! traditional "bar chart" style used in technical analysis.
//!
//! LWC supports `upColor` and `downColor` for bullish/bearish bars.

/// Visual options for a Bar (OHLC) series.
#[derive(Debug, Clone)]
pub struct BarSeriesOptions {
    /// Color for bullish bars (close >= open) [R, G, B, A].
    pub up_color: [f32; 4],
    /// Color for bearish bars (close < open) [R, G, B, A].
    pub down_color: [f32; 4],
    /// Whether to show open ticks. Default: true.
    pub open_visible: bool,
    /// Whether thin bars are used (1px width). Default: true.
    pub thin_bars: bool,
    /// Whether the series is visible.
    pub visible: bool,
    /// Display label / title for the series.
    pub title: String,
}

impl Default for BarSeriesOptions {
    fn default() -> Self {
        let theme = crate::core::renderer::theme::ThemeConfig::default();
        Self {
            up_color: theme.series_defaults.bar_up_color,
            down_color: theme.series_defaults.bar_down_color,
            open_visible: true,
            thin_bars: true,
            visible: true,
            title: String::new(),
        }
    }
}
