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
        - reducing_coeff * (bar_spacing.max(special_to) - special_to).atan()
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

/// All computed candle sizes in physical pixels for a given bar_spacing and pixel ratios.
#[derive(Debug, Clone, Copy)]
pub struct CandleSizing {
    pub bar_width: f64,
    pub wick_width: f64,
    pub border_width: f64,
    pub draw_body: bool,
    pub bar_spacing: f64,
    /// Horizontal pixel ratio (used for bar widths and x-coordinates).
    pub h_pixel_ratio: f64,
    /// Vertical pixel ratio (used for heights and y-coordinates).
    pub v_pixel_ratio: f64,
}

impl CandleSizing {
    /// Compute candle sizing from pane dimensions.
    /// `pane_w` is the pane width in physical pixels.
    /// `h_ratio` / `v_ratio` are the per-axis pixel ratios from
    /// `device-pixel-content-box` (or `dpr` as fallback).
    pub fn compute_from_pane(pane_w: f64, vp: &Viewport, h_ratio: f64, v_ratio: f64) -> Self {
        let visible_bars = vp.end_bar - vp.start_bar;
        // bar_spacing is in CSS pixels — divide physical width by (bars * h_ratio)
        let bar_spacing = pane_w / (visible_bars * h_ratio);

        // Horizontal sizing uses h_ratio (matches LWC's horizontalPixelRatio)
        let mut bw = optimal_candlestick_width(bar_spacing, h_ratio);
        let ww = wick_width(bar_spacing, h_ratio, bw);
        bw = parity_fix(bw, ww);
        let bdw = border_width(h_ratio, bw);

        let draw_body = bw > bdw * 2.0;

        Self {
            bar_width: bw,
            wick_width: ww,
            border_width: bdw,
            draw_body,
            bar_spacing,
            h_pixel_ratio: h_ratio,
            v_pixel_ratio: v_ratio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lwc_optimal_candlestick_width(bar_spacing: f64, pixel_ratio: f64) -> f64 {
        let bar_spacing_special_case_from = 2.5;
        let bar_spacing_special_case_to = 4.0;
        let bar_spacing_special_case_coeff = 3.0;
        if bar_spacing >= bar_spacing_special_case_from
            && bar_spacing <= bar_spacing_special_case_to
        {
            return (bar_spacing_special_case_coeff * pixel_ratio).floor();
        }

        let bar_spacing_reducing_coeff = 0.2;
        let coeff = 1.0
            - bar_spacing_reducing_coeff
                * (bar_spacing_special_case_to.max(bar_spacing) - bar_spacing_special_case_to)
                    .atan()
                / std::f64::consts::FRAC_PI_2;
        let res = (bar_spacing * coeff * pixel_ratio).floor();
        let scaled_bar_spacing = (bar_spacing * pixel_ratio).floor();
        let optimal = res.min(scaled_bar_spacing);
        optimal.max(pixel_ratio.floor())
    }

    #[test]
    fn optimal_width_matches_lwc_formula_across_zoom_presets() {
        let bar_spacings = [0.5, 0.8, 1.0, 1.7, 2.4, 2.5, 3.0, 4.0, 4.1, 6.0, 10.0, 18.0];
        let pixel_ratios = [1.0, 1.25, 1.5, 2.0, 3.0];

        for spacing in bar_spacings {
            for ratio in pixel_ratios {
                let expected = lwc_optimal_candlestick_width(spacing, ratio);
                let actual = optimal_candlestick_width(spacing, ratio);
                assert!(
                    (actual - expected).abs() < f64::EPSILON,
                    "spacing={} ratio={} expected={} actual={}",
                    spacing,
                    ratio,
                    expected,
                    actual
                );
            }
        }
    }

    #[test]
    fn wick_and_border_widths_are_lwc_compatible() {
        let bar_spacings = [0.5, 1.0, 2.0, 3.0, 6.0, 12.0];
        let pixel_ratios = [1.0, 1.5, 2.0, 3.0];

        for spacing in bar_spacings {
            for ratio in pixel_ratios {
                let mut bw = optimal_candlestick_width(spacing, ratio);
                let ww = wick_width(spacing, ratio, bw);
                bw = parity_fix(bw, ww);
                let border = border_width(ratio, bw);

                assert!(
                    ww >= ratio.floor(),
                    "wick too thin for spacing={}, ratio={}",
                    spacing,
                    ratio
                );
                assert!(
                    ww <= bw,
                    "wick wider than body for spacing={}, ratio={}",
                    spacing,
                    ratio
                );
                assert!(
                    bw >= ratio.floor(),
                    "body below minimum for spacing={}, ratio={}",
                    spacing,
                    ratio
                );
                assert!(
                    border >= ratio.floor(),
                    "border below minimum for spacing={}, ratio={}",
                    spacing,
                    ratio
                );
            }
        }
    }
}
