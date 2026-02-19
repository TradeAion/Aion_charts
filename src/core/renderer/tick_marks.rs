//! Shared tick mark computation — single source of truth.
//!
//! Both the GridRenderer (for grid lines) and axis renderers (for labels)
//! consume the same tick marks, computed once per frame.

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::TickMark;
use crate::core::formatters::{format_price, format_timestamp, nice_step};

/// Compute price (Y-axis) tick marks.
/// `chart_h` is the pane height in physical pixels.
pub fn compute_y_ticks(vp: &Viewport, chart_h: f64, dpr: f64) -> Vec<TickMark> {
    let range = vp.price_max - vp.price_min;
    if range <= 0.0 || chart_h <= 0.0 { return vec![]; }

    // Target ~1 tick per 40 CSS px of height
    let target_count = (chart_h / (40.0 * dpr)).max(3.0).min(15.0);
    let step = nice_step(range / target_count);
    let first = (vp.price_min / step).ceil() * step;

    let mut out = Vec::new();
    let mut v = first;
    while v <= vp.price_max {
        let frac = (v - vp.price_min) / range;
        let px = chart_h * (1.0 - frac);
        out.push(TickMark {
            value: v,
            pixel: px,
            label: format_price(v, step),
            major: true,
        });
        v += step;
    }
    out
}

/// Compute time (X-axis) tick marks.
/// `chart_w` is the pane width in physical pixels.
pub fn compute_x_ticks(vp: &Viewport, bars: &[Bar], chart_w: f64, dpr: f64) -> Vec<TickMark> {
    let count = vp.end_bar - vp.start_bar;
    if count <= 0.0 || chart_w <= 0.0 { return vec![]; }

    // Target ~1 tick per 100 CSS px of width
    let target_count = (chart_w / (100.0 * dpr)).max(2.0);
    let step = nice_step(count / target_count).max(1.0);
    let first = (vp.start_bar / step).ceil() * step;

    let mut out = Vec::new();
    let mut v = first;
    while v <= vp.end_bar {
        let px = (v - vp.start_bar) / count * chart_w;
        let bar_i = v as usize;
        let (label, major) = if bar_i < bars.len() && bars[bar_i].timestamp > 0 {
            let lbl = format_timestamp(bars[bar_i].timestamp);
            // LWC: year and month labels are bold (major)
            let is_major = lbl.len() == 4 && lbl.chars().all(|c| c.is_ascii_digit())  // "2024"
                || lbl.len() == 3 && lbl.chars().all(|c| c.is_alphabetic());           // "Jan"
            (lbl, is_major)
        } else {
            (format!("{}", v as i64), false)
        };
        out.push(TickMark { value: v, pixel: px, label, major });
        v += step;
    }
    out
}
