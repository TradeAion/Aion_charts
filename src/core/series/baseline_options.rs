//! Baseline series visual options — two-tone area fill above/below a base value.
//!
//! LWC's BaselineSeries renders:
//!   - A line connecting data points
//!   - Fill above the baseline in `topFillColor1` → `topFillColor2` gradient
//!   - Fill below the baseline in `bottomFillColor1` → `bottomFillColor2` gradient
//!   - Line color changes at the baseline crossing (topLineColor / bottomLineColor)

/// Visual options for a baseline series.
#[derive(Debug, Clone)]
pub struct BaselineSeriesOptions {
    /// Base value (price level) that divides "above" from "below".
    /// Default: 0.0 (user should set this to a meaningful value).
    pub base_value: f64,

    /// Line color above the baseline [R, G, B, A].
    /// LWC default: rgba(38, 166, 154, 1) — teal/green.
    pub top_line_color: [f32; 4],

    /// Line color below the baseline [R, G, B, A].
    /// LWC default: rgba(239, 83, 80, 1) — red.
    pub bottom_line_color: [f32; 4],

    /// Fill color at the line (above baseline) — top of above-gradient.
    /// LWC default: rgba(38, 166, 154, 0.28).
    pub top_fill_color1: [f32; 4],

    /// Fill color at the baseline (above region) — bottom of above-gradient.
    /// LWC default: rgba(38, 166, 154, 0.05).
    pub top_fill_color2: [f32; 4],

    /// Fill color at the baseline (below region) — top of below-gradient.
    /// LWC default: rgba(239, 83, 80, 0.05).
    pub bottom_fill_color1: [f32; 4],

    /// Fill color at the line (below baseline) — bottom of below-gradient.
    /// LWC default: rgba(239, 83, 80, 0.28).
    pub bottom_fill_color2: [f32; 4],

    /// Line width in CSS pixels.
    pub line_width: f64,

    /// Whether the series is visible.
    pub visible: bool,

    /// Display label / title for the series.
    pub title: String,
}

impl Default for BaselineSeriesOptions {
    fn default() -> Self {
        Self {
            base_value: 0.0,
            // Teal/green for above
            top_line_color: [0.149, 0.651, 0.604, 1.0],
            // Red for below
            bottom_line_color: [0.937, 0.325, 0.314, 1.0],
            // Above fill gradient: line → baseline
            top_fill_color1: [0.149, 0.651, 0.604, 0.28],
            top_fill_color2: [0.149, 0.651, 0.604, 0.05],
            // Below fill gradient: baseline → line
            bottom_fill_color1: [0.937, 0.325, 0.314, 0.05],
            bottom_fill_color2: [0.937, 0.325, 0.314, 0.28],
            line_width: 2.0,
            visible: true,
            title: String::new(),
        }
    }
}
