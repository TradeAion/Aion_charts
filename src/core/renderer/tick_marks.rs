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
///
/// Faithfully ports LWC's `PriceTickMarkBuilder._updateMarks()`:
/// - Tick step is computed from the CANDLE AREA height (the same coordinate
///   space where ticks are positioned). LWC uses a single `priceScale.height()`
///   for both step and coordinate mapping.
/// - Iteration runs HIGH→LOW (top of chart to bottom), matching LWC exactly.
///   This gives priority to higher-value ticks when spacing is tight.
/// - Tick alignment uses `high - (high % span)` (LWC's modulo alignment).
///
/// Supports all PriceScaleMode variants: Normal, Logarithmic, Percentage, IndexedTo100.
pub fn compute_y_ticks(
    vp: &Viewport,
    pane_h: f64,
    candle_h: f64,
    dpr: f64,
    style: &ChartStyle,
) -> Vec<TickMark> {
    let range = vp.price_max - vp.price_min;
    if range <= 0.0 || candle_h <= 0.0 || pane_h <= 0.0 {
        return vec![];
    }

    // ── Step computation uses CANDLE AREA height (matches LWC) ──
    // LWC: maxTickSpan = (high-low) * markHeight / priceScale.height()
    // We use candle_h so that step transitions match the coordinate space.
    let step = y_tick_step_internal(vp, candle_h, dpr, style).max(0.0001);
    let min_gap_px = (style.price_scale_tick_mark_spacing_css() * dpr).max(1.0);

    // ── Compute high/low: extend range to cover the full pane ──
    // LWC: high = coordinateToLogical(0), low = coordinateToLogical(height-1)
    // This extends the price range into margin/volume areas so ticks can
    // appear outside the data region.
    let price_at_pane_bottom = vp.price_min - (range * (pane_h - candle_h) / candle_h);
    let high = vp.price_max + step; // slightly above top to catch edge ticks
    let low = price_at_pane_bottom;

    // ── LWC modulo alignment: start at largest multiple of step ≤ high ──
    // LWC: mod = high % span; mod += (mod < 0) ? span : 0; start = high - mod
    let mut modulo = high % step;
    if modulo < 0.0 {
        modulo += step;
    }
    let start = high - modulo;

    // ── Iterate HIGH → LOW (LWC's _updateMarks direction) ──
    let mut out: Vec<TickMark> = Vec::new();
    let mut prev_px: Option<f64> = None;
    let mut v = start;

    while v > low {
        // Map price to pixel using the candle area coordinate system
        let frac = (v - vp.price_min) / range;
        let px = candle_h * (1.0 - frac);

        // Skip ticks outside the full pane bounds (with small margin)
        if px < -min_gap_px || px > pane_h + min_gap_px {
            v -= step;
            continue;
        }

        // Skip ticks too close to previous (LWC: abs(coord - prevCoord) < tickMarkHeight)
        if let Some(prev) = prev_px {
            if (prev - px).abs() < min_gap_px {
                v -= step;
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

        prev_px = Some(px);
        v -= step;
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

pub fn infer_bar_interval_ms(bars: &crate::core::data::BarArray) -> Option<i64> {
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
