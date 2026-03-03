//! Shared value projection utilities.
//!
//! Centralizes:
//! - Price -> pane Y projection for the main candle area
//! - Price-scale-aware label formatting
//! - Last-value extraction for main bars + overlay series
//!
//! This module ensures pane overlays and price-axis labels are derived from
//! the exact same data and coordinate math.

use crate::core::data::BarArray;
use crate::core::formatters::{format_indexed, format_percent, format_price, nice_step_ceiling};
use crate::core::renderer::traits::ChartStyle;
use crate::core::renderer::transforms::price_to_y;
use crate::core::series::SeriesCollection;
use crate::core::viewport::{PriceScaleMode, Viewport};

/// A projected last-value item (used by both pane overlays and price-axis labels).
#[derive(Debug, Clone)]
pub struct ProjectedLastValue {
    /// Raw price value (series close/value).
    pub price: f64,
    /// Y coordinate in physical pixels, in main candle-area space.
    pub y_phys: f64,
    /// Series color used by last-price line/label.
    pub color: [f32; 4],
    /// Formatted label text for the current price-scale mode.
    pub label: String,
}

/// Candle-area height in physical pixels.
#[inline]
pub fn candle_area_height_ph(vp: &Viewport, pane_ph: f64) -> f64 {
    pane_ph * vp.candle_height_frac()
}

/// Project a raw price to physical Y within the candle area.
#[inline]
pub fn price_to_pane_y_phys(price: f64, vp: &Viewport, pane_ph: f64) -> f64 {
    price_to_y(price, vp, candle_area_height_ph(vp, pane_ph))
}

/// Shared Y tick-step in internal price-scale space (same policy as tick_marks.rs).
/// `axis_ph` is the full visible price-axis height in physical pixels.
#[inline]
pub fn y_tick_step_internal(vp: &Viewport, axis_ph: f64, dpr: f64, style: &ChartStyle) -> f64 {
    let range = vp.price_max - vp.price_min;
    if range <= 0.0 || axis_ph <= 0.0 || dpr <= 0.0 {
        return 0.0001;
    }
    // LWC policy: row spacing is typography-driven, not a hardcoded pixel constant.
    // tickMarkHeight = ceil(fontSize * tickMarkDensity)
    let row_spacing_css = style.price_scale_tick_mark_spacing_css();
    let target_count = (axis_ph / (row_spacing_css * dpr)).max(1.0);
    nice_step_ceiling(range / target_count).max(0.0001)
}

/// Format a raw price according to the active price-scale mode.
#[inline]
pub fn format_scale_value(vp: &Viewport, raw_price: f64, step_internal: f64) -> String {
    match vp.price_scale_mode {
        PriceScaleMode::Normal | PriceScaleMode::Logarithmic => {
            format_price(raw_price, step_internal)
        }
        PriceScaleMode::Percentage => {
            let internal = vp.price_to_internal(raw_price);
            format_percent(internal, step_internal)
        }
        PriceScaleMode::IndexedTo100 => {
            let internal = vp.price_to_internal(raw_price);
            format_indexed(internal, step_internal)
        }
    }
}

/// Collect projected last values for main bars + visible overlay series.
///
/// Returned coordinates and labels are shared across pane/axis renderers to
/// keep the live line and axis label perfectly synchronized.
pub fn collect_last_values(
    series: &SeriesCollection,
    bars: &BarArray,
    vp: &Viewport,
    style: &ChartStyle,
    pane_ph: f64,
    dpr: f64,
) -> Vec<ProjectedLastValue> {
    let mut out = Vec::new();
    let candle_h = candle_area_height_ph(vp, pane_ph);
    if candle_h <= 0.0 {
        return out;
    }

    let step = y_tick_step_internal(vp, pane_ph, dpr, style);

    // Main candlestick last value (pending-aware read via BarArray::get).
    //
    // LWC behaviour: the last-price label is ALWAYS included regardless of
    // whether the price is currently inside the visible candle area.  The
    // rendering side clamps the label to the top/bottom edge of the axis
    // (via `compute_right_axis_label_geometry`), keeping it visible even
    // when the user has scaled the price axis so the last price is off-screen.
    if bars.len() > 0 {
        if let Some(last) = bars.get(bars.len() - 1) {
            let y_phys = price_to_y(last.close as f64, vp, candle_h);
            let color = if last.close >= last.open {
                style.bullish_color
            } else {
                style.bearish_color
            };
            out.push(ProjectedLastValue {
                price: last.close as f64,
                y_phys,
                color,
                label: format_scale_value(vp, last.close as f64, step),
            });
        }
    }

    // Overlay series last values.
    for s in series.iter() {
        if !s.is_visible() {
            continue;
        }
        let last_val = match s.last_value() {
            Some(v) => v,
            None => continue,
        };
        let y_phys = price_to_y(last_val, vp, candle_h);
        out.push(ProjectedLastValue {
            price: last_val,
            y_phys,
            color: s.series_color(),
            label: format_scale_value(vp, last_val, step),
        });
    }

    out
}

/// Map a timestamp to a fractional bar index using `BarArray` timestamps.
///
/// Returns:
/// - exact index for an exact timestamp match
/// - interpolated fractional index between surrounding bars
/// - extrapolated fractional index before/after the data range
pub fn timestamp_to_bar_index_in_bars(ts: u64, bars: &BarArray) -> Option<f64> {
    let len = bars.len();
    if len == 0 {
        return None;
    }

    // lower_bound binary search on BarArray::timestamp(i)
    let mut lo = 0usize;
    let mut hi = len;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if bars.timestamp(mid) < ts {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    if lo < len && bars.timestamp(lo) == ts {
        return Some(lo as f64);
    }

    // insertion point = lo
    if lo == 0 {
        if len >= 2 {
            let t0 = bars.timestamp(0) as f64;
            let t1 = bars.timestamp(1) as f64;
            let dt = t1 - t0;
            if dt > 0.0 {
                return Some(-((t0 - ts as f64) / dt));
            }
        }
        return None;
    }

    if lo >= len {
        if len >= 2 {
            let tn1 = bars.timestamp(len - 1) as f64;
            let tn2 = bars.timestamp(len - 2) as f64;
            let dt = tn1 - tn2;
            if dt > 0.0 {
                return Some((len - 1) as f64 + ((ts as f64 - tn1) / dt));
            }
        }
        return None;
    }

    let t0 = bars.timestamp(lo - 1) as f64;
    let t1 = bars.timestamp(lo) as f64;
    let dt = t1 - t0;
    if dt <= 0.0 {
        return Some(lo as f64);
    }

    let frac = (ts as f64 - t0) / dt;
    Some((lo - 1) as f64 + frac)
}
