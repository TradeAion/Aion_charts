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
        // LWC defaults: up=#26a69a, down=#ef5350
        Self {
            up_color: [0.149, 0.651, 0.604, 1.0],   // #26a69a
            down_color: [0.937, 0.325, 0.314, 1.0],  // #ef5350
            open_visible: true,
            thin_bars: true,
            visible: true,
            title: String::new(),
        }
    }
}
