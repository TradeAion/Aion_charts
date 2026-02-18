//! PaneSeriesRenderer trait — composable Canvas2D series renderers.
//!
//! Each series type (candles, volume, line, area, etc.) implements this trait.
//! The Canvas2D backend holds a list of these and iterates through them.
//! This mirrors LWC's architecture where PaneRendererCandlesticks,
//! PaneRendererHistogram, etc. are independent, composable renderers.

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::ChartStyle;
use web_sys::CanvasRenderingContext2d;

/// Layout of the chart area in physical pixels.
/// Shared between all series renderers and the overlay.
#[derive(Debug, Clone, Copy)]
pub struct ChartLayout {
    /// Width of the chart drawing area (excludes Y-axis).
    pub chart_w: f64,
    /// Height of the candle/price area.
    pub candle_h: f64,
    /// Height of the volume area.
    pub vol_h: f64,
    /// Height of the X-axis.
    pub x_axis_h: f64,
    /// Device pixel ratio.
    pub dpr: f64,
}

impl ChartLayout {
    /// Compute chart layout from physical dimensions and style.
    pub fn from_physical(phys_w: u32, phys_h: u32, dpr: f64, style: &ChartStyle) -> Self {
        let y_axis_w = style.y_axis_width as f64 * dpr;
        let x_axis_h = style.x_axis_height as f64 * dpr;
        let vol_h = phys_h as f64 * 0.15;
        let chart_w = (phys_w as f64 - y_axis_w).max(1.0);
        let candle_h = (phys_h as f64 - x_axis_h - vol_h).max(1.0);
        Self {
            chart_w,
            candle_h,
            vol_h: vol_h.max(1.0),
            x_axis_h,
            dpr,
        }
    }
}

// ── LWC-matching candlestick sizing (pixel-exact) ─────────────────────────

/// Matches LWC `optimalCandlestickWidth(barSpacing, pixelRatio)`.
/// Returns bar body width in physical pixels (integer).
pub fn optimal_candlestick_width(bar_spacing: f64, pixel_ratio: f64) -> f64 {
    let special_from = 2.5;
    let special_to = 4.0;
    let special_coeff = 3.0;
    if bar_spacing >= special_from && bar_spacing <= special_to {
        return (special_coeff * pixel_ratio).floor();
    }
    let reducing_coeff = 0.2;
    let coeff = 1.0
        - reducing_coeff
            * (bar_spacing.max(special_to) - special_to).atan()
            / (std::f64::consts::FRAC_PI_2);
    let res = (bar_spacing * coeff * pixel_ratio).floor();
    let scaled_bar_spacing = (bar_spacing * pixel_ratio).floor();
    let optimal = res.min(scaled_bar_spacing);
    optimal.max(pixel_ratio.floor())
}

/// Compute wick width in physical pixels matching LWC.
/// Always at least floor(pixelRatio) (= 1 CSS pixel), never wider than bar_width.
pub fn wick_width(bar_spacing: f64, pixel_ratio: f64, bar_width: f64) -> f64 {
    let w = (pixel_ratio.floor()).min((bar_spacing * pixel_ratio).floor());
    let w = w.max(pixel_ratio.floor());
    w.min(bar_width)
}

/// Compute border width in physical pixels matching LWC.
pub fn border_width(pixel_ratio: f64, bar_width: f64) -> f64 {
    let mut bw = (1.0 * pixel_ratio).floor();
    if bar_width <= 2.0 * bw {
        bw = ((bar_width - 1.0) * 0.5).floor();
    }
    let res = bw.max(pixel_ratio.floor());
    if bar_width <= res * 2.0 {
        // Can't fit border + body, return original (border-only mode)
        return (1.0 * pixel_ratio).floor().max(pixel_ratio.floor());
    }
    res
}

/// Ensure bar_width parity matches wick_width parity (LWC trick for symmetry).
pub fn parity_fix(bar_width: f64, wick_width: f64) -> f64 {
    if bar_width >= 2.0 {
        let ww = wick_width as i32;
        let bw = bar_width as i32;
        if (ww % 2) != (bw % 2) {
            return bar_width - 1.0;
        }
    }
    bar_width
}

/// All computed candle sizes in physical pixels for a given bar_spacing and dpr.
#[derive(Debug, Clone, Copy)]
pub struct CandleSizing {
    /// Bar body width (physical px, integer).
    pub bar_width: f64,
    /// Wick width (physical px, integer).
    pub wick_width: f64,
    /// Border width (physical px, integer).
    pub border_width: f64,
    /// Whether body fill should be drawn (bar_width > 2*border_width).
    pub draw_body: bool,
    /// Bar spacing in logical (CSS) pixels.
    pub bar_spacing: f64,
}

impl CandleSizing {
    /// Compute all sizing from layout and viewport, matching LWC exactly.
    pub fn compute(layout: &ChartLayout, vp: &Viewport) -> Self {
        let visible_bars = vp.end_bar - vp.start_bar;
        let bar_spacing = layout.chart_w / (visible_bars * layout.dpr); // logical px per bar
        let dpr = layout.dpr;

        let mut bw = optimal_candlestick_width(bar_spacing, dpr);
        let ww = wick_width(bar_spacing, dpr, bw);
        bw = parity_fix(bw, ww);
        let bdw = border_width(dpr, bw);
        let draw_body = bw > bdw * 2.0;

        Self {
            bar_width: bw,
            wick_width: ww,
            border_width: bdw,
            draw_body,
            bar_spacing,
        }
    }
}

/// A composable Canvas2D series renderer.
/// Each series type implements this; the backend iterates through all of them.
pub trait PaneSeriesRenderer {
    /// Draw this series onto the given Canvas2D context.
    fn draw(
        &self,
        ctx: &CanvasRenderingContext2d,
        bars: &[Bar],
        viewport: &Viewport,
        style: &ChartStyle,
        layout: &ChartLayout,
    );
}
