//! Series sizing — LWC-matching candlestick sizing algorithms.
//!
//! Used by GeometryGenerator to compute pixel-exact candle dimensions.

use crate::core::viewport::Viewport;

// ── LWC-matching candlestick sizing (pixel-exact) ─────────────────────────

/// Matches LWC `optimalCandlestickWidth(barSpacing, pixelRatio)`.
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

/// Wick width in physical pixels matching LWC.
pub fn wick_width(bar_spacing: f64, pixel_ratio: f64, bar_width: f64) -> f64 {
    let w = (pixel_ratio.floor()).min((bar_spacing * pixel_ratio).floor());
    let w = w.max(pixel_ratio.floor());
    w.min(bar_width)
}

/// Border width in physical pixels matching LWC.
pub fn border_width(pixel_ratio: f64, bar_width: f64) -> f64 {
    let mut bw = (1.0 * pixel_ratio).floor();
    if bar_width <= 2.0 * bw {
        bw = ((bar_width - 1.0) * 0.5).floor();
    }
    let res = bw.max(pixel_ratio.floor());
    if bar_width <= res * 2.0 {
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
    pub bar_width: f64,
    pub wick_width: f64,
    pub border_width: f64,
    pub draw_body: bool,
    pub bar_spacing: f64,
    pub dpr: f64,
}

impl CandleSizing {
    /// Compute candle sizing from pane dimensions (no ChartLayout needed).
    /// `pane_w` is the pane width in physical pixels.
    pub fn compute_from_pane(pane_w: f64, vp: &Viewport, dpr: f64) -> Self {
        let visible_bars = vp.end_bar - vp.start_bar;
        let bar_spacing = pane_w / (visible_bars * dpr);

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
            dpr,
        }
    }
}
