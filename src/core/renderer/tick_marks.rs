//! Shared tick mark computation — single source of truth.
//!
//! Both the GridRenderer (for grid lines) and axis renderers (for labels)
//! consume the same tick marks, computed once per frame.
//!
//! Supports all PriceScaleMode variants: Normal, Logarithmic, Percentage, IndexedTo100.

use crate::core::constants::{X_TICK_MAX_COUNT, X_TICK_MIN_COUNT, X_TICK_TARGET_SPACING_CSS};
use crate::core::formatters::{format_timestamp, nice_step};
use crate::core::renderer::traits::{ChartStyle, TickMark};
use crate::core::renderer::value_projection::{format_scale_value, y_tick_step_internal};
use crate::core::viewport::Viewport;

/// Compute price (Y-axis) tick marks.
/// `chart_h` is the pane height in physical pixels.
/// Handles all price scale modes (Normal, Log, Percentage, IndexedTo100).
pub fn compute_y_ticks(vp: &Viewport, chart_h: f64, dpr: f64, style: &ChartStyle) -> Vec<TickMark> {
    let range = vp.price_max - vp.price_min;
    if range <= 0.0 || chart_h <= 0.0 {
        return vec![];
    }

    // Price axis ticks should span the full axis height, not only candle area.
    // Spacing follows LWC's typography-driven density model.
    let step = y_tick_step_internal(vp, chart_h, dpr, style).max(0.0001);
    let min_gap_px = (style.price_scale_tick_mark_spacing_css() * dpr).max(1.0);
    let first = (vp.price_min / step).ceil() * step;

    let mut out: Vec<TickMark> = Vec::new();
    let mut v = first;
    while v <= vp.price_max {
        let frac = (v - vp.price_min) / range;
        let px = chart_h * (1.0 - frac);
        if let Some(prev) = out.last() {
            if (prev.pixel - px).abs() < min_gap_px {
                v += step;
                continue;
            }
        }
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

    // Target tick cadence based on axis CSS width, clamped for readability.
    let target_count =
        (chart_w / (X_TICK_TARGET_SPACING_CSS * dpr)).clamp(X_TICK_MIN_COUNT, X_TICK_MAX_COUNT);
    let step = nice_step(count / target_count).max(1.0).round();
    let first = (vp.start_bar / step).ceil() * step;
    let interval_ms = infer_bar_interval_ms(bars);

    let mut out = Vec::new();
    let mut v = first;
    while v <= vp.end_bar {
        let px = (v + 0.5 - vp.start_bar) / count * chart_w;
        let bar_i = v as i64;
        let label = match timestamp_for_bar_index(bars, bar_i, interval_ms) {
            Some(ts) if ts > 0 => format_timestamp(ts),
            _ => format!("{}", bar_i.max(0)),
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

fn infer_bar_interval_ms(bars: &crate::core::data::BarArray) -> Option<i64> {
    let len = bars.len();
    if len < 2 {
        return None;
    }

    // Use a robust median interval from recent bars to avoid outlier gaps.
    let start = len.saturating_sub(64);
    let mut diffs: Vec<i64> = Vec::with_capacity(len - start);
    for i in (start + 1)..len {
        let prev = bars.timestamp(i - 1);
        let curr = bars.timestamp(i);
        if curr > prev {
            let diff = (curr - prev) as i64;
            if diff > 0 {
                diffs.push(diff);
            }
        }
    }
    if diffs.is_empty() {
        return None;
    }
    diffs.sort_unstable();
    Some(diffs[diffs.len() / 2])
}

fn timestamp_for_bar_index(
    bars: &crate::core::data::BarArray,
    bar_index: i64,
    interval_ms: Option<i64>,
) -> Option<u64> {
    let len = bars.len() as i64;
    if len == 0 {
        return None;
    }

    if bar_index >= 0 && bar_index < len {
        let ts = bars.timestamp(bar_index as usize);
        if ts > 0 {
            return Some(ts);
        }
    }

    let interval = interval_ms?;
    if interval <= 0 {
        return None;
    }

    let first = bars.timestamp(0) as i128;
    let last = bars.timestamp((len - 1) as usize) as i128;
    let ts = if bar_index < 0 {
        first + (bar_index as i128) * (interval as i128)
    } else {
        let delta = bar_index as i128 - (len as i128 - 1);
        last + delta * (interval as i128)
    };
    if ts <= 0 {
        None
    } else {
        Some(ts as u64)
    }
}
