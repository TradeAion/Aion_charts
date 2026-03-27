//! Shared tick mark computation — single source of truth.
//!
//! Both the GridRenderer (for grid lines) and axis renderers (for labels)
//! consume the same tick marks, computed once per frame.
//!
//! Supports all PriceScaleMode variants: Normal, Logarithmic, Percentage, IndexedTo100.

use crate::core::constants::{X_TICK_MAX_COUNT, X_TICK_MIN_COUNT, X_TICK_TARGET_SPACING_CSS};
use crate::core::formatters::{format_time_axis_label, nice_step};
use crate::core::renderer::traits::{ChartStyle, TickMark};
use crate::core::renderer::value_projection::{
    format_scale_value, y_tick_step_internal, TimeScaleIndex,
};
use crate::core::viewport::Viewport;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisibleTimePoint {
    pub logical_index: f64,
    pub timestamp: u64,
}

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
    time_scale: &TimeScaleIndex,
    chart_w: f64,
    dpr: f64,
) -> Vec<TickMark> {
    let count = vp.end_bar - vp.start_bar;
    if count <= 0.0 || chart_w <= 0.0 || time_scale.is_empty() {
        return vec![];
    }

    let visible_time_points = collect_visible_time_points(vp, time_scale);

    // Target tick cadence based on axis CSS width, clamped for readability.
    let target_count =
        (chart_w / (X_TICK_TARGET_SPACING_CSS * dpr)).clamp(X_TICK_MIN_COUNT, X_TICK_MAX_COUNT);
    let step = nice_step(count / target_count).max(1.0).round();
    let first = (vp.start_bar / step).ceil() * step;
    let snap_distance = (step * 0.5).max(0.75);

    let mut out = Vec::new();
    let mut v = first;
    let mut last_labeled_timestamp = None;
    while v <= vp.end_bar {
        let px = (v + 0.5 - vp.start_bar) / count * chart_w;
        let snapped = nearest_visible_time_point(&visible_time_points, v)
            .filter(|point| (point.logical_index - v).abs() <= snap_distance)
            .map(|point| point.timestamp);
        let fallback = timestamp_for_logical_index(time_scale, v as i64);
        let (label, major, labeled_timestamp) = match snapped.or(fallback) {
            Some(ts) if ts > 0 => {
                let formatted = format_time_axis_label(ts);
                if last_labeled_timestamp == Some(ts) {
                    (String::new(), false, None)
                } else {
                    (formatted.text, formatted.major, Some(ts))
                }
            }
            _ => (String::new(), false, None),
        };
        if let Some(ts) = labeled_timestamp {
            last_labeled_timestamp = Some(ts);
        }
        out.push(TickMark {
            value: v,
            pixel: px,
            label,
            major,
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

pub fn collect_visible_time_points(
    vp: &Viewport,
    time_scale: &TimeScaleIndex,
) -> Vec<VisibleTimePoint> {
    let mut points = Vec::new();
    let visible_start = vp.start_bar - 2.0;
    let visible_end = vp.end_bar + 2.0;
    let start = visible_start.floor().max(0.0) as usize;
    let end = visible_end.ceil().max(0.0) as usize;

    for slot in start..end.min(time_scale.len()) {
        if let Some(timestamp) = time_scale.timestamp_at(slot) {
            if timestamp == 0 {
                continue;
            }
            points.push(VisibleTimePoint {
                logical_index: slot as f64,
                timestamp,
            });
        }
    }
    points
}

pub fn infer_time_scale_interval_ms(time_scale: &TimeScaleIndex) -> Option<i64> {
    let timestamps = time_scale.timestamps();
    let len = timestamps.len();
    if len < 2 {
        return None;
    }

    let start = len.saturating_sub(64);
    let mut diffs: Vec<i64> = Vec::with_capacity(len - start);
    for i in (start + 1)..len {
        let prev = timestamps[i - 1];
        let curr = timestamps[i];
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

pub(crate) fn timestamp_for_logical_index(
    time_scale: &TimeScaleIndex,
    logical_index: i64,
) -> Option<u64> {
    let len = time_scale.len() as i64;
    if len == 0 {
        return None;
    }

    if logical_index >= 0 && logical_index < len {
        return time_scale.timestamp_at(logical_index as usize);
    }

    let interval = infer_time_scale_interval_ms(time_scale)?;
    if interval <= 0 {
        return None;
    }

    let first = time_scale.timestamp_at(0)? as i128;
    let last = time_scale.timestamp_at((len - 1) as usize)? as i128;
    let ts = if logical_index < 0 {
        first + (logical_index as i128) * (interval as i128)
    } else {
        let delta = logical_index as i128 - (len as i128 - 1);
        last + delta * (interval as i128)
    };
    (ts > 0).then_some(ts as u64)
}

pub fn nearest_visible_time_point(
    points: &[VisibleTimePoint],
    logical_index: f64,
) -> Option<VisibleTimePoint> {
    if points.is_empty() {
        return None;
    }

    let mut lo = 0usize;
    let mut hi = points.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if points[mid].logical_index < logical_index {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    let candidate = points.get(lo).copied();
    let previous = lo
        .checked_sub(1)
        .and_then(|index| points.get(index).copied());
    match (previous, candidate) {
        (Some(prev), Some(next)) => {
            if (logical_index - prev.logical_index).abs()
                <= (next.logical_index - logical_index).abs()
            {
                Some(prev)
            } else {
                Some(next)
            }
        }
        (Some(prev), None) => Some(prev),
        (None, Some(next)) => Some(next),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_visible_time_points, nearest_visible_time_point};
    use crate::core::data::{Bar, BarArray};
    use crate::core::renderer::value_projection::TimeScaleIndex;
    use crate::core::series::{LinePoint, LineSeriesOptions, SeriesCollection};
    use crate::core::viewport::Viewport;

    fn sample_bars() -> BarArray {
        let mut bars = BarArray::new();
        bars.set(vec![
            Bar {
                timestamp: 1000,
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 2000,
                open: 11.0,
                high: 13.0,
                low: 10.0,
                close: 12.0,
                volume: 110.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 3000,
                open: 12.0,
                high: 14.0,
                low: 11.0,
                close: 13.0,
                volume: 120.0,
                _pad: 0.0,
            },
        ])
        .unwrap();
        bars
    }

    #[test]
    fn collect_visible_time_points_includes_overlay_timestamps() {
        let bars = sample_bars();
        let mut series = SeriesCollection::new();
        let id = series.add_line(LineSeriesOptions::default());
        series
            .get_mut(id)
            .unwrap()
            .line_data
            .set(vec![LinePoint {
                timestamp: 2500,
                value: 42.0,
            }])
            .unwrap();

        let mut vp = Viewport::new(100, 100);
        vp.start_bar = 0.0;
        vp.end_bar = 3.0;

        let time_scale = TimeScaleIndex::from_bars_and_series(&bars, &series);
        let points = collect_visible_time_points(&vp, &time_scale);
        assert!(points.iter().any(|point| point.timestamp == 2500));
    }

    #[test]
    fn nearest_visible_time_point_prefers_closest_logical_index() {
        let points = vec![
            super::VisibleTimePoint {
                logical_index: 0.0,
                timestamp: 1000,
            },
            super::VisibleTimePoint {
                logical_index: 1.5,
                timestamp: 2500,
            },
            super::VisibleTimePoint {
                logical_index: 2.0,
                timestamp: 3000,
            },
        ];

        let nearest = nearest_visible_time_point(&points, 1.4).unwrap();
        assert_eq!(nearest.timestamp, 2500);
    }
}
