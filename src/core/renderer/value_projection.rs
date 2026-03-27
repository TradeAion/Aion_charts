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
use crate::core::footprint::{FootprintBar, FootprintData, FootprintOptions};
use crate::core::formatters::{format_indexed, format_percent, format_price};
use crate::core::renderer::traits::ChartStyle;
use crate::core::renderer::transforms::price_to_y;
use crate::core::series::{SeriesCollection, SeriesType};
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

#[derive(Debug, Clone, Default)]
pub struct TimeScaleIndex {
    timestamps: Vec<u64>,
    main_bar_slots: Vec<usize>,
    main_bar_by_slot: Vec<Option<usize>>,
}

impl TimeScaleIndex {
    pub fn from_bars(bars: &BarArray) -> Self {
        let mut timestamps = Vec::with_capacity(bars.len());
        let mut main_bar_slots = Vec::with_capacity(bars.len());
        let mut main_bar_by_slot = Vec::with_capacity(bars.len());
        for index in 0..bars.len() {
            timestamps.push(bars.timestamp(index));
            main_bar_slots.push(index);
            main_bar_by_slot.push(Some(index));
        }
        Self {
            timestamps,
            main_bar_slots,
            main_bar_by_slot,
        }
    }

    pub fn from_bars_and_series(bars: &BarArray, series: &SeriesCollection) -> Self {
        let mut timestamps = Vec::with_capacity(
            bars.len()
                + series
                    .iter()
                    .map(|series| match series.series_type() {
                        SeriesType::Line | SeriesType::Area | SeriesType::Baseline => {
                            series.line_data.timestamps.len()
                        }
                        SeriesType::Histogram => series.histogram_data.timestamps.len(),
                        SeriesType::Bar => series.bar_data.timestamps.len(),
                        SeriesType::Candlestick => 0,
                    })
                    .sum::<usize>(),
        );

        for index in 0..bars.len() {
            let timestamp = bars.timestamp(index);
            if timestamp > 0 {
                timestamps.push(timestamp);
            }
        }

        for series in series.iter() {
            let source: &[u64] = match series.series_type() {
                SeriesType::Line | SeriesType::Area | SeriesType::Baseline => {
                    &series.line_data.timestamps
                }
                SeriesType::Histogram => &series.histogram_data.timestamps,
                SeriesType::Bar => &series.bar_data.timestamps,
                SeriesType::Candlestick => &[],
            };
            timestamps.extend(source.iter().copied().filter(|timestamp| *timestamp > 0));
        }

        timestamps.sort_unstable();
        timestamps.dedup();

        let mut main_bar_slots = Vec::with_capacity(bars.len());
        let mut main_bar_by_slot = vec![None; timestamps.len()];
        for index in 0..bars.len() {
            let timestamp = bars.timestamp(index);
            let slot = timestamps.binary_search(&timestamp).unwrap_or(0);
            main_bar_slots.push(slot);
            if let Some(entry) = main_bar_by_slot.get_mut(slot) {
                *entry = Some(index);
            }
        }

        Self {
            timestamps,
            main_bar_slots,
            main_bar_by_slot,
        }
    }

    #[inline]
    pub fn timestamps(&self) -> &[u64] {
        &self.timestamps
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.timestamps.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    #[inline]
    pub fn main_bar_len(&self) -> usize {
        self.main_bar_slots.len()
    }

    #[inline]
    pub fn logical_index_for_timestamp(&self, ts: u64) -> Option<f64> {
        timestamp_to_bar_index_in_slice(ts, &self.timestamps)
    }

    #[inline]
    pub fn timestamp_at(&self, index: usize) -> Option<u64> {
        self.timestamps.get(index).copied()
    }

    #[inline]
    pub fn logical_index_for_main_bar(&self, bar_index: usize) -> Option<f64> {
        self.main_bar_slots.get(bar_index).map(|slot| *slot as f64)
    }

    #[inline]
    pub fn main_bar_index_at_slot(&self, slot: usize) -> Option<usize> {
        self.main_bar_by_slot.get(slot).copied().flatten()
    }

    #[inline]
    pub fn main_bar_index_for_logical(&self, logical_index: f64) -> Option<usize> {
        if !logical_index.is_finite() || logical_index < 0.0 {
            return None;
        }
        self.main_bar_index_at_slot(logical_index.floor() as usize)
    }

    pub fn nearest_main_bar(&self, logical_index: f64) -> Option<(usize, usize)> {
        if self.main_bar_slots.is_empty() || !logical_index.is_finite() {
            return None;
        }

        let target = logical_index.clamp(0.0, self.timestamps.len().saturating_sub(1) as f64);
        let target_slot = target.floor() as usize;

        let nearest_main_bar_index = match self.main_bar_slots.binary_search(&target_slot) {
            Ok(pos) => pos,
            Err(0) => 0,
            Err(pos) if pos >= self.main_bar_slots.len() => self.main_bar_slots.len() - 1,
            Err(pos) => {
                let prev = self.main_bar_slots[pos - 1];
                let next = self.main_bar_slots[pos];
                let prev_dist = (target - prev as f64).abs();
                let next_dist = (next as f64 - target).abs();
                if prev_dist <= next_dist {
                    pos - 1
                } else {
                    pos
                }
            }
        };

        Some((
            nearest_main_bar_index,
            self.main_bar_slots[nearest_main_bar_index],
        ))
    }

    #[inline]
    pub fn nearest_main_bar_index_for_logical(&self, logical_index: f64) -> Option<usize> {
        self.nearest_main_bar(logical_index)
            .map(|(main_bar_index, _)| main_bar_index)
    }

    #[inline]
    pub fn nearest_main_bar_slot_for_logical(&self, logical_index: f64) -> Option<usize> {
        self.nearest_main_bar(logical_index)
            .map(|(_, logical_slot)| logical_slot)
    }

    pub fn visible_main_bar_range(
        &self,
        start_logical: f64,
        end_logical: f64,
    ) -> Option<(usize, usize)> {
        if self.main_bar_slots.is_empty()
            || !start_logical.is_finite()
            || !end_logical.is_finite()
            || end_logical <= start_logical
        {
            return None;
        }

        let start_slot = start_logical.floor().max(0.0) as usize;
        let end_slot = end_logical.ceil().max(0.0) as usize;
        let first = self
            .main_bar_slots
            .partition_point(|slot| *slot < start_slot);
        let last = self.main_bar_slots.partition_point(|slot| *slot < end_slot);
        (first < last).then_some((first, last))
    }

    pub fn resolve_rounded_timestamp(&self, logical_index: f64) -> Option<u64> {
        let len = self.timestamps.len();
        if len == 0 || !logical_index.is_finite() {
            return None;
        }

        let idx = logical_index.round() as isize;
        if idx >= 0 && idx < len as isize {
            return self.timestamps.get(idx as usize).copied();
        }

        if len >= 2 && idx < 0 {
            let dt = self.timestamps[1] as f64 - self.timestamps[0] as f64;
            if dt > 0.0 {
                let extra = logical_index * dt;
                return Some((self.timestamps[0] as f64 + extra).round() as u64);
            }
        }

        if len >= 2 && idx >= len as isize {
            let last = len - 1;
            let dt = self.timestamps[last] as f64 - self.timestamps[last - 1] as f64;
            if dt > 0.0 {
                let extra = (logical_index - last as f64) * dt;
                return Some((self.timestamps[last] as f64 + extra).round() as u64);
            }
        }

        None
    }
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
    let min_movement = inferred_min_movement(range);

    let s1 = lwc_tick_span(range, max_tick_span, min_movement, &[2.0, 2.5, 2.0]);
    let s2 = lwc_tick_span(range, max_tick_span, min_movement, &[2.0, 2.0, 2.5]);
    let s3 = lwc_tick_span(range, max_tick_span, min_movement, &[2.5, 2.0, 2.0]);

    s1.min(s2).min(s3).max(0.0001)
}

const TICK_SPAN_EPS: f64 = 1e-14;
const FRACTIONAL_DIVIDERS: [f64; 3] = [2.0, 2.5, 2.0];

/// Port of LWC's `PriceTickSpanCalculator.tickSpan()` for base-decimal prices
/// with an adaptive minimum movement derived from the current range.
fn lwc_tick_span(
    range: f64,
    max_tick_span: f64,
    min_movement: f64,
    integral_dividers: &[f64],
) -> f64 {
    let min_movement = min_movement.max(1e-8);

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

#[inline]
fn inferred_min_movement(range: f64) -> f64 {
    let magnitude = range.abs().max(1e-12);
    let exponent = magnitude.log10().floor() as i32 - 2;
    10.0_f64.powi(exponent.clamp(-8, 8))
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
    footprint_data: &FootprintData,
    footprint_opts: &FootprintOptions,
    vp: &Viewport,
    style: &ChartStyle,
    pane_ph: f64,
    v_pixel_ratio: f64,
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
    if let Some(projected) = project_main_last_value(
        bars,
        main_chart_type,
        footprint_data,
        footprint_opts,
        vp,
        style,
        pane_ph,
        v_pixel_ratio,
        dpr,
    ) {
        out.push(projected);
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

/// Resolve the rendered main-series last price + color for the active chart type.
///
/// This differs from `main_series_last_price_and_color()` for footprint mode:
/// the price is snapped to the rendered footprint close edge so all live-price
/// consumers stay attached to the same painted candle body.
pub fn main_series_rendered_last_price_and_color(
    bars: &BarArray,
    main_chart_type: MainChartType,
    footprint_data: &FootprintData,
    footprint_opts: &FootprintOptions,
    vp: &Viewport,
    style: &ChartStyle,
    pane_ph: f64,
    v_pixel_ratio: f64,
) -> Option<(f64, [f32; 4])> {
    let (base_price, color) = main_series_last_price_and_color(bars, main_chart_type, style)?;
    if main_chart_type != MainChartType::Footprint {
        return Some((base_price, color));
    }

    let last_idx = bars.len().checked_sub(1)?;
    let bar = bars.get(last_idx)?;
    let fp_bar = footprint_data.get_bar(last_idx)?;
    let snapped = snapped_footprint_close_price(
        bar.open as f64,
        bar.close as f64,
        fp_bar,
        footprint_opts,
        vp,
        pane_ph,
        v_pixel_ratio,
    )
    .unwrap_or(base_price);
    Some((snapped, color))
}

/// Project the main-series last value using the exact chart-type-aware rendered price.
pub fn project_main_last_value(
    bars: &BarArray,
    main_chart_type: MainChartType,
    footprint_data: &FootprintData,
    footprint_opts: &FootprintOptions,
    vp: &Viewport,
    style: &ChartStyle,
    pane_ph: f64,
    v_pixel_ratio: f64,
    dpr: f64,
) -> Option<ProjectedLastValue> {
    let candle_h = candle_area_height_ph(vp, pane_ph);
    if candle_h <= 0.0 {
        return None;
    }
    let step = y_tick_step_internal(vp, pane_ph, dpr, style);
    let (price, color) = main_series_rendered_last_price_and_color(
        bars,
        main_chart_type,
        footprint_data,
        footprint_opts,
        vp,
        style,
        pane_ph,
        v_pixel_ratio,
    )?;
    Some(ProjectedLastValue {
        price,
        y_phys: price_to_y(price, vp, candle_h),
        color,
        label: format_scale_value(vp, price, step),
        countdown: None,
    })
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

/// Compute the rendered close-edge price for a footprint candle.
///
/// Footprint candles are snapped to an effective tick (base tick x aggregation
/// factor), so raw close may lie inside the body. This returns the snapped close
/// edge used by rendering, ensuring the live line, label, and chip align to the
/// same footprint body boundary.
pub fn snapped_footprint_close_price(
    open: f64,
    close: f64,
    fp_bar: &FootprintBar,
    fp_opts: &FootprintOptions,
    viewport: &Viewport,
    pane_ph: f64,
    v_ratio: f64,
) -> Option<f64> {
    if fp_bar.levels.is_empty() {
        return None;
    }
    let base_tick = if fp_opts.tick_size > 0.0 {
        fp_opts.tick_size as f64
    } else {
        fp_bar.inferred_tick_size() as f64
    };
    if !base_tick.is_finite() || base_tick <= 0.0 {
        return None;
    }

    let first_price = fp_bar.levels[0].price as f64;
    let y0 = price_to_pane_y_phys(first_price, viewport, pane_ph);
    let y1 = price_to_pane_y_phys(first_price + base_tick, viewport, pane_ph);
    let natural_cell_h = (y0 - y1).abs();
    let min_cell_px = fp_opts.aggregation_min_cell_height_css() * v_ratio.max(1.0);
    let agg_factor = if min_cell_px > 0.0 && natural_cell_h > 0.0 && natural_cell_h < min_cell_px {
        (min_cell_px / natural_cell_h).ceil().max(1.0)
    } else {
        1.0
    };
    let effective_tick = base_tick * agg_factor;
    if !effective_tick.is_finite() || effective_tick <= 0.0 {
        return None;
    }

    let is_bull = close >= open;
    let snapped_close = if is_bull {
        (close / effective_tick).ceil() * effective_tick
    } else {
        (close / effective_tick).floor() * effective_tick
    };
    Some(snapped_close)
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

fn timestamp_to_bar_index_in_slice_impl(
    ts: u64,
    len: usize,
    timestamp_at: impl Fn(usize) -> u64,
) -> Option<f64> {
    if len == 0 {
        return None;
    }

    // lower_bound binary search on the timestamp source.
    let mut lo = 0usize;
    let mut hi = len;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if timestamp_at(mid) < ts {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    if lo < len && timestamp_at(lo) == ts {
        return Some(lo as f64);
    }

    // insertion point = lo
    if lo == 0 {
        if len >= 2 {
            let t0 = timestamp_at(0) as f64;
            let t1 = timestamp_at(1) as f64;
            let dt = t1 - t0;
            if dt > 0.0 {
                return Some(-((t0 - ts as f64) / dt));
            }
        }
        return None;
    }

    if lo >= len {
        if len >= 2 {
            let tn1 = timestamp_at(len - 1) as f64;
            let tn2 = timestamp_at(len - 2) as f64;
            let dt = tn1 - tn2;
            if dt > 0.0 {
                return Some((len - 1) as f64 + ((ts as f64 - tn1) / dt));
            }
        }
        return None;
    }

    let t0 = timestamp_at(lo - 1) as f64;
    let t1 = timestamp_at(lo) as f64;
    let dt = t1 - t0;
    if dt <= 0.0 {
        return Some(lo as f64);
    }

    let frac = (ts as f64 - t0) / dt;
    Some((lo - 1) as f64 + frac)
}

/// Map a timestamp to a fractional bar index using a timestamp slice.
pub fn timestamp_to_bar_index_in_slice(ts: u64, bar_timestamps: &[u64]) -> Option<f64> {
    timestamp_to_bar_index_in_slice_impl(ts, bar_timestamps.len(), |index| bar_timestamps[index])
}

/// Map a timestamp to a fractional bar index using `BarArray` timestamps.
///
/// Returns:
/// - exact index for an exact timestamp match
/// - interpolated fractional index between surrounding bars
/// - extrapolated fractional index before/after the data range
pub fn timestamp_to_bar_index_in_bars(ts: u64, bars: &BarArray) -> Option<f64> {
    timestamp_to_bar_index_in_slice_impl(ts, bars.len(), |index| bars.timestamp(index))
}

#[cfg(test)]
mod tests {
    use super::{main_series_last_price_and_color, main_series_rendered_last_price_and_color};
    use crate::core::chart_type::MainChartType;
    use crate::core::data::{Bar, BarArray};
    use crate::core::footprint::{FootprintBar, FootprintData, FootprintLevel, FootprintOptions};
    use crate::core::renderer::traits::ChartStyle;
    use crate::core::viewport::Viewport;

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
        ])
        .unwrap();
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

    #[test]
    fn rendered_footprint_last_price_uses_snapped_close() {
        let mut bars = BarArray::new();
        bars.set(vec![Bar {
            timestamp: 1,
            open: 100.0,
            high: 100.5,
            low: 99.75,
            close: 100.1,
            volume: 100.0,
            _pad: 0.0,
        }])
        .unwrap();

        let mut footprint = FootprintData::new();
        footprint.set_bar(
            0,
            FootprintBar {
                levels: vec![
                    FootprintLevel {
                        price: 99.75,
                        bid_volume: 10.0,
                        ask_volume: 12.0,
                    },
                    FootprintLevel {
                        price: 100.0,
                        bid_volume: 8.0,
                        ask_volume: 15.0,
                    },
                ],
            },
        );

        let mut vp = Viewport::new(800, 400);
        vp.volume_height_ratio = 0.0;
        vp.price_min = 99.0;
        vp.price_max = 101.0;

        let style = ChartStyle::default();
        let mut opts = FootprintOptions::default();
        opts.tick_size = 0.25;

        let (price, color) = main_series_rendered_last_price_and_color(
            &bars,
            MainChartType::Footprint,
            &footprint,
            &opts,
            &vp,
            &style,
            400.0,
            1.0,
        )
        .expect("rendered main last value");

        assert!((price - 100.25).abs() < 1e-9);
        assert_eq!(color, style.bullish_color);
    }
}
