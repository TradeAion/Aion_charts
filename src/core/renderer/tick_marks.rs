//! Shared tick mark computation — single source of truth.
//!
//! Both the GridRenderer (for grid lines) and axis renderers (for labels)
//! consume the same tick marks, computed once per frame.
//!
//! Supports all PriceScaleMode variants: Normal, Logarithmic, Percentage, IndexedTo100.

use crate::core::formatters::{format_timestamp, nice_step};
use crate::core::renderer::traits::TickMark;
use crate::core::renderer::value_projection::{
    candle_area_height_ph, format_scale_value, y_tick_step_internal,
};
use crate::core::viewport::Viewport;

/// Compute price (Y-axis) tick marks.
/// `chart_h` is the pane height in physical pixels.
/// Handles all price scale modes (Normal, Log, Percentage, IndexedTo100).
pub fn compute_y_ticks(vp: &Viewport, chart_h: f64, dpr: f64) -> Vec<TickMark> {
    let range = vp.price_max - vp.price_min;
    if range <= 0.0 || chart_h <= 0.0 {
        return vec![];
    }

    let candle_h = candle_area_height_ph(vp, chart_h);
    if candle_h <= 0.0 {
        return vec![];
    }

    let step = y_tick_step_internal(vp, chart_h, dpr);
    let first = (vp.price_min / step).ceil() * step;

    let mut out = Vec::new();
    let mut v = first;
    while v <= vp.price_max {
        let frac = (v - vp.price_min) / range;
        let px = candle_h * (1.0 - frac);
        let label = format_scale_value(vp, vp.internal_to_price(v), step);

        out.push(TickMark {
            value: v,
            pixel: px,
            label,
            major: true,
        });
        v += step;
    }
    out
}

/// Compute time (X-axis) tick marks.
/// `chart_w` is the pane width in physical pixels.
pub fn compute_x_ticks(
    vp: &Viewport,
    bars: &crate::core::data::BarArray,
    chart_w: f64,
    dpr: f64,
) -> Vec<TickMark> {
    let count = vp.end_bar - vp.start_bar;
    if count <= 0.0 || chart_w <= 0.0 {
        return vec![];
    }

    // Target ~1 tick per 100 CSS px of width
    let target_count = (chart_w / (100.0 * dpr)).max(2.0);
    let step = nice_step(count / target_count).max(1.0);
    let first = (vp.start_bar / step).ceil() * step;

    let mut out = Vec::new();
    let mut v = first;
    while v <= vp.end_bar {
        let px = (v + 0.5 - vp.start_bar) / count * chart_w;
        let bar_i = v as usize;
        let label = if bar_i < bars.len() && bars.timestamp(bar_i) > 0 {
            format_timestamp(bars.timestamp(bar_i))
        } else {
            format!("{}", v as i64)
        };
        // All time ticks are major (same as Y-axis) so grid lines appear at all ticks.
        // Label boldness for year/month is handled separately in the time axis renderer.
        out.push(TickMark {
            value: v,
            pixel: px,
            label,
            major: true,
        });
        v += step;
    }
    out
}
