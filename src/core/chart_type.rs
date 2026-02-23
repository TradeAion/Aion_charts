//! Main Chart Type — determines how the primary OHLC data is rendered.
//!
//! This module provides the `MainChartType` enum that controls the visual
//! representation of the main price data (the bars in `ChartEngine.bars`).
//!
//! # Supported Chart Types
//!
//! | Type | Description |
//! |------|-------------|
//! | `Candlestick` | Japanese candlesticks (default) |
//! | `OhlcBars` | Traditional OHLC bars with ticks |
//! | `Line` | Simple line connecting close prices |
//! | `Area` | Filled area below the close line |
//! | `HeikinAshi` | Heikin-Ashi candles (smoothed) |
//! | `Baseline` | Line with two-tone fill above/below baseline |
//!
//! # Example
//!
//! ```rust,ignore
//! use raycore::MainChartType;
//!
//! engine.set_main_chart_type(MainChartType::OhlcBars);
//! ```

use serde::{Deserialize, Serialize};

/// The main chart type — how the primary OHLC data is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MainChartType {
    /// Japanese candlesticks (bodies + wicks).
    #[default]
    Candlestick,
    /// Traditional OHLC bars with horizontal ticks for open/close.
    OhlcBars,
    /// Simple line connecting close prices.
    Line,
    /// Filled area below the close line.
    Area,
    /// Heikin-Ashi candles (smoothed trend visualization).
    HeikinAshi,
    /// Line with gradient fill above/below a baseline value.
    Baseline,
}

impl MainChartType {
    /// Parse from a string (for WASM API).
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "candlestick" | "candles" | "candle" => Self::Candlestick,
            "ohlc" | "ohlc_bars" | "bars" | "hlc" => Self::OhlcBars,
            "line" => Self::Line,
            "area" => Self::Area,
            "heikin_ashi" | "heikinashi" | "ha" => Self::HeikinAshi,
            "baseline" => Self::Baseline,
            _ => Self::Candlestick,
        }
    }

    /// Convert to string identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Candlestick => "candlestick",
            Self::OhlcBars => "ohlc",
            Self::Line => "line",
            Self::Area => "area",
            Self::HeikinAshi => "heikin_ashi",
            Self::Baseline => "baseline",
        }
    }

    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Candlestick => "Candlestick",
            Self::OhlcBars => "OHLC Bars",
            Self::Line => "Line",
            Self::Area => "Area",
            Self::HeikinAshi => "Heikin-Ashi",
            Self::Baseline => "Baseline",
        }
    }

    /// Whether this chart type uses full OHLC data (vs just close).
    pub fn uses_ohlc(&self) -> bool {
        matches!(self, Self::Candlestick | Self::OhlcBars | Self::HeikinAshi)
    }

    /// Whether this chart type shows volume by default.
    pub fn shows_volume(&self) -> bool {
        matches!(self, Self::Candlestick | Self::OhlcBars | Self::HeikinAshi)
    }

    /// List of all available chart types.
    pub fn all() -> &'static [MainChartType] {
        &[
            Self::Candlestick,
            Self::OhlcBars,
            Self::Line,
            Self::Area,
            Self::HeikinAshi,
            Self::Baseline,
        ]
    }
}

/// Options for the main chart rendering.
#[derive(Debug, Clone)]
pub struct MainChartOptions {
    /// The chart type.
    pub chart_type: MainChartType,

    // ── Candlestick / OHLC Bar options ──
    /// Up (bullish) color [R, G, B, A].
    pub up_color: [f32; 4],
    /// Down (bearish) color [R, G, B, A].
    pub down_color: [f32; 4],
    /// Border/wick color for up candles. If None, uses up_color.
    pub up_border_color: Option<[f32; 4]>,
    /// Border/wick color for down candles. If None, uses down_color.
    pub down_border_color: Option<[f32; 4]>,
    /// Whether candle bodies have a visible border.
    pub border_visible: bool,
    /// Whether wicks are visible (candlestick only).
    pub wick_visible: bool,

    // ── Line / Area options ──
    /// Line color for Line/Area chart types.
    pub line_color: [f32; 4],
    /// Line width in pixels.
    pub line_width: f32,
    /// Fill color for Area chart type (top area).
    pub area_top_color: [f32; 4],
    /// Fill color for Area chart type (bottom/fade).
    pub area_bottom_color: [f32; 4],

    // ── Baseline options ──
    /// Baseline value (for Baseline chart type).
    pub baseline_value: f32,
    /// Color above the baseline.
    pub baseline_top_fill_color: [f32; 4],
    /// Color below the baseline.
    pub baseline_bottom_fill_color: [f32; 4],
    /// Line color above baseline.
    pub baseline_top_line_color: [f32; 4],
    /// Line color below baseline.
    pub baseline_bottom_line_color: [f32; 4],
}

impl Default for MainChartOptions {
    fn default() -> Self {
        Self {
            chart_type: MainChartType::Candlestick,
            // Default colors (TradingView-style)
            up_color: [0.18, 0.8, 0.44, 1.0],    // Green #2ECC71
            down_color: [0.91, 0.30, 0.24, 1.0], // Red #E74C3C
            up_border_color: None,
            down_border_color: None,
            border_visible: true,
            wick_visible: true,
            // Line/Area defaults
            line_color: [0.161, 0.384, 1.0, 1.0], // Blue #2962FF
            line_width: 2.0,
            area_top_color: [0.161, 0.384, 1.0, 0.4],
            area_bottom_color: [0.161, 0.384, 1.0, 0.0],
            // Baseline defaults
            baseline_value: 0.0,
            baseline_top_fill_color: [0.18, 0.8, 0.44, 0.3],
            baseline_bottom_fill_color: [0.91, 0.30, 0.24, 0.3],
            baseline_top_line_color: [0.18, 0.8, 0.44, 1.0],
            baseline_bottom_line_color: [0.91, 0.30, 0.24, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_type_from_str() {
        assert_eq!(
            MainChartType::from_str("candlestick"),
            MainChartType::Candlestick
        );
        assert_eq!(
            MainChartType::from_str("candles"),
            MainChartType::Candlestick
        );
        assert_eq!(MainChartType::from_str("ohlc"), MainChartType::OhlcBars);
        assert_eq!(MainChartType::from_str("OHLC"), MainChartType::OhlcBars);
        assert_eq!(MainChartType::from_str("line"), MainChartType::Line);
        assert_eq!(MainChartType::from_str("area"), MainChartType::Area);
        assert_eq!(
            MainChartType::from_str("heikin_ashi"),
            MainChartType::HeikinAshi
        );
        assert_eq!(MainChartType::from_str("ha"), MainChartType::HeikinAshi);
        assert_eq!(MainChartType::from_str("baseline"), MainChartType::Baseline);
        assert_eq!(
            MainChartType::from_str("unknown"),
            MainChartType::Candlestick
        );
    }

    #[test]
    fn test_chart_type_as_str_roundtrip() {
        for ct in MainChartType::all() {
            let s = ct.as_str();
            let parsed = MainChartType::from_str(s);
            assert_eq!(*ct, parsed);
        }
    }

    #[test]
    fn test_uses_ohlc() {
        assert!(MainChartType::Candlestick.uses_ohlc());
        assert!(MainChartType::OhlcBars.uses_ohlc());
        assert!(MainChartType::HeikinAshi.uses_ohlc());
        assert!(!MainChartType::Line.uses_ohlc());
        assert!(!MainChartType::Area.uses_ohlc());
    }

    #[test]
    fn test_all_chart_types_count() {
        assert_eq!(MainChartType::all().len(), 6);
    }
}
