//! Shared value projection utilities.
//!
//! Centralizes:
//! - Price -> pane Y projection for the main candle area
//! - Price-scale-aware label formatting
//! - Last-value extraction for main bars + overlay series
//!
//! This module ensures pane overlays and price-axis labels are derived from
//! the exact same data and coordinate math.

use crate::core::chart_type::MainChartType;
use crate::core::data::BarArray;
use crate::core::formatters::{format_indexed, format_percent, format_price};
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
    /// Optional countdown string (e.g. "00:39") — rendered on a second line
    /// below the price label, TradingView-style.
    pub countdown: Option<String>,
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

/// Shared Y tick-step in internal price-scale space.
///
/// Ported from LWC's `PriceTickMarkBuilder.tickSpan()` which runs three
/// `PriceTickSpanCalculator` instances with divider sequences
/// `[2, 2.5, 2]`, `[2, 2, 2.5]`, `[2.5, 2, 2]` and takes the minimum.
///
/// `axis_ph` must be the CANDLE AREA height in physical pixels — the same
/// coordinate space where tick positions are mapped. LWC uses a single
/// `priceScale.height()` for both step and coordinate mapping; we must do
/// the same to avoid step transitions at wrong zoom levels.
pub fn y_tick_step_internal(vp: &Viewport, axis_ph: f64, dpr: f64, style: &ChartStyle) -> f64 {
    let range = vp.price_max - vp.price_min;
    if range <= 0.0 || axis_ph <= 0.0 || dpr <= 0.0 {
        return 0.0001;
    }
    let mark_height = style.price_scale_tick_mark_spacing_css();
    let scale_height = axis_ph / dpr;
    let max_tick_span = range * mark_height / scale_height;

    let s1 = lwc_tick_span(range, max_tick_span, &[2.0, 2.5, 2.0]);
    let s2 = lwc_tick_span(range, max_tick_span, &[2.0, 2.0, 2.5]);
    let s3 = lwc_tick_span(range, max_tick_span, &[2.5, 2.0, 2.0]);

    s1.min(s2).min(s3).max(0.0001)
}

const TICK_SPAN_EPS: f64 = 1e-14;
const FRACTIONAL_DIVIDERS: [f64; 3] = [2.0, 2.5, 2.0];

/// LWC default base for `PriceTickSpanCalculator` (100 = 2 decimal places).
/// This sets `minMovement = 1/BASE = 0.01`, preventing ticks finer than 1 cent.
const LWC_DEFAULT_BASE: f64 = 100.0;

/// Port of LWC's `PriceTickSpanCalculator.tickSpan()` for base-decimal prices
/// (base = 100, matching LWC's default PriceScale formatter).
fn lwc_tick_span(range: f64, max_tick_span: f64, integral_dividers: &[f64]) -> f64 {
    let min_movement = 1.0 / LWC_DEFAULT_BASE; // 0.01

    let mut span = 10.0_f64.powf(0.0_f64.max(range.log10().ceil()));

    let mut idx = 0usize;
    let mut c = integral_dividers[0];

    // LWC integral loop — three conditions must ALL be true to continue:
    // 1. span >= minMovement  (and span > minMovement + eps)
    // 2. span >= maxTickSpan * c
    // 3. span >= 1
    loop {
        let larger_min_movement =
            (span - min_movement) >= -TICK_SPAN_EPS && span > (min_movement + TICK_SPAN_EPS);
        let larger_max = (max_tick_span * c - span) <= TICK_SPAN_EPS;
        let larger_one = (1.0 - span) <= TICK_SPAN_EPS;
        if !(larger_min_movement && larger_max && larger_one) {
            break;
        }
        span /= c;
        idx += 1;
        c = integral_dividers[idx % integral_dividers.len()];
    }

    // Clamp to minMovement if we got close
    if span <= min_movement + TICK_SPAN_EPS {
        span = min_movement;
    }

    span = span.max(1.0);

    // Fractional loop — only enters when span ≈ 1 and base is decimal
    if (span - 1.0).abs() < TICK_SPAN_EPS {
        idx = 0;
        c = FRACTIONAL_DIVIDERS[0];
        while (max_tick_span * c - span) <= TICK_SPAN_EPS && span > (min_movement + TICK_SPAN_EPS) {
            span /= c;
            idx += 1;
            c = FRACTIONAL_DIVIDERS[idx % FRACTIONAL_DIVIDERS.len()];
        }
    }

    span
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
    main_chart_type: MainChartType,
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
    if let Some((last_price, color)) =
        main_series_last_price_and_color(bars, main_chart_type, style)
    {
        let y_phys = price_to_y(last_price, vp, candle_h);
        out.push(ProjectedLastValue {
            price: last_price,
            y_phys,
            color,
            label: format_scale_value(vp, last_price, step),
            countdown: None,
        });
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
            countdown: None,
        });
    }

    out
}

/// Resolve the main-series last price + color for the active chart type.
///
/// For Heikin-Ashi, this returns the transformed `ha_close` and candle direction
/// color computed from `(ha_open, ha_close)` so the last-price line/label
/// stays attached to the rendered HA body edge.
pub fn main_series_last_price_and_color(
    bars: &BarArray,
    main_chart_type: MainChartType,
    style: &ChartStyle,
) -> Option<(f64, [f32; 4])> {
    if bars.len() == 0 {
        return None;
    }

    match main_chart_type {
        MainChartType::HeikinAshi => {
            let (ha_open, ha_close) = heikin_ashi_last_open_close(bars)?;
            let color = if ha_close >= ha_open {
                style.bullish_color
            } else {
                style.bearish_color
            };
            Some((ha_close, color))
        }
        _ => {
            let last = bars.get(bars.len() - 1)?;
            let color = if last.close >= last.open {
                style.bullish_color
            } else {
                style.bearish_color
            };
            Some((last.close as f64, color))
        }
    }
}

fn heikin_ashi_last_open_close(bars: &BarArray) -> Option<(f64, f64)> {
    let len = bars.len();
    if len == 0 {
        return None;
    }

    let first = bars.get(0)?;
    let mut prev_ha_open = (first.open as f64 + first.close as f64) * 0.5;
    let mut prev_ha_close =
        (first.open as f64 + first.high as f64 + first.low as f64 + first.close as f64) * 0.25;

    if len == 1 {
        return Some((prev_ha_open, prev_ha_close));
    }

    for i in 1..len {
        let b = bars.get(i)?;
        let ha_close = (b.open as f64 + b.high as f64 + b.low as f64 + b.close as f64) * 0.25;
        let ha_open = (prev_ha_open + prev_ha_close) * 0.5;
        prev_ha_open = ha_open;
        prev_ha_close = ha_close;
    }

    Some((prev_ha_open, prev_ha_close))
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

#[cfg(test)]
mod tests {
    use super::main_series_last_price_and_color;
    use crate::core::chart_type::MainChartType;
    use crate::core::data::{Bar, BarArray};
    use crate::core::renderer::traits::ChartStyle;

    fn sample_bars() -> BarArray {
        let mut bars = BarArray::new();
        bars.set(vec![
            Bar {
                timestamp: 1,
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                volume: 100.0,
                _pad: 0.0,
            },
            Bar {
                timestamp: 2,
                open: 11.0,
                high: 13.0,
                low: 10.0,
                close: 12.0,
                volume: 120.0,
                _pad: 0.0,
            },
        ]);
        bars
    }

    #[test]
    fn main_last_price_uses_raw_close_for_candlestick() {
        let bars = sample_bars();
        let style = ChartStyle::default();
        let (price, color) =
            main_series_last_price_and_color(&bars, MainChartType::Candlestick, &style)
                .expect("main last value");
        assert!((price - 12.0).abs() < 1e-9);
        assert_eq!(color, style.bullish_color);
    }

    #[test]
    fn main_last_price_uses_heikin_ashi_close_for_heikin_ashi() {
        let bars = sample_bars();
        let style = ChartStyle::default();
        let (price, color) =
            main_series_last_price_and_color(&bars, MainChartType::HeikinAshi, &style)
                .expect("main last value");

        // For the sample bars:
        // bar0: ha_open=10.5, ha_close=10.5
        // bar1: ha_open=(10.5+10.5)/2=10.5, ha_close=(11+13+10+12)/4=11.5
        assert!((price - 11.5).abs() < 1e-9);
        assert_eq!(color, style.bullish_color);
    }
}
