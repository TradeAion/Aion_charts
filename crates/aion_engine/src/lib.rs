//! Headless Aion chart engine.
//!
//! This crate owns chart state and behavior without depending on WASM, the DOM, WebGPU, or a
//! native windowing system. Hosts provide input and a viewport; rendering backends consume the
//! frame produced from this state. During the architecture recovery, frame construction is being
//! migrated here incrementally from `aion_wasm`.

mod frame;
mod indicators;
mod price_scale_api;
mod series_query_api;
#[cfg(test)]
mod tests;

pub use frame::{AxisFrame, AxisLabel, AxisTextAlign, AxisTextMidpoint, ChartFrame, FramePane};
pub(crate) use indicators::IndicatorBinding;
pub use indicators::IndicatorKind;

use aion_core::format::price_formatter::PriceFormatter;
use aion_core::model::data_layer::{DataLayer, SeriesId};
use aion_core::model::data_validation::{
    sanitize_ohlc, sanitize_point, ValidationError, ValidationReport,
};
use aion_core::model::magnet::CrosshairMode;
use aion_core::model::plot_list::{MismatchDirection, PlotValueIndex};
use aion_core::model::price_range::PriceRange;
use aion_core::model::range::{LogicalRange, StrictRange};
use aion_core::options::ChartOptionsStore;
use aion_core::scale::price_scale_core::{
    PriceScaleCore, PriceScaleCoreOptions, PriceScaleMargins, PriceScaleMode,
};
use aion_core::scale::time_scale_core::{TimeScaleCore, TimeScaleOptions};
use aion_core::scale::time_tick_marks::TimeTickMarks;
use aion_core::TimePointIndex;
use aion_render::color::Color;
use aion_render::draw_list::{LineStyle, LineType};

/// Host formatting callbacks (LWC localization / `tickMarkFormatter`). Each returns `None` to fall
/// back to the built-in formatter. Boxed so the headless engine carries them without a js dependency.
pub type PriceFormatterFn = Box<dyn Fn(f64) -> Option<String>>;
pub type TickMarkFormatterFn = Box<dyn Fn(i64, u8) -> Option<String>>;
pub type TimeFormatterFn = Box<dyn Fn(i64) -> Option<String>>;

const DEFAULT_LINE_COLOR: Color = Color::rgb(0x21, 0x96, 0xf3);
/// Default media-coordinate height of the horizontal axis. Hosts use this value during layout;
/// it is engine policy rather than a browser/demo constant.
pub const TIME_AXIS_HEIGHT: f64 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesKind {
    Candlestick,
    Bar,
    Line,
    Area,
    Histogram,
    Baseline,
}

/// The price scale that owns a series. Left and right are visible pane axes; overlay is the
/// axis-less independent scale used by volume and other pane overlays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceScaleTarget {
    Right,
    Left,
    Overlay,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesDataPoint {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarsInLogicalRange {
    pub bars_before: f64,
    pub bars_after: f64,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

impl SeriesKind {
    pub fn from_u8(kind: u8) -> Self {
        match kind {
            1 => Self::Bar,
            2 => Self::Line,
            3 => Self::Area,
            4 => Self::Histogram,
            5 => Self::Baseline,
            _ => Self::Candlestick,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Self::Candlestick => 0,
            Self::Bar => 1,
            Self::Line => 2,
            Self::Area => 3,
            Self::Histogram => 4,
            Self::Baseline => 5,
        }
    }
}

pub fn line_style_from_u8(style: u8) -> LineStyle {
    match style {
        1 => LineStyle::Dotted,
        2 => LineStyle::Dashed,
        3 => LineStyle::LargeDashed,
        4 => LineStyle::SparseDotted,
        _ => LineStyle::Solid,
    }
}

pub mod marker_pos {
    pub const ABOVE: u8 = 0;
    pub const BELOW: u8 = 1;
    pub const IN_BAR: u8 = 2;
}

pub mod marker_shape {
    pub const CIRCLE: u8 = 0;
    pub const SQUARE: u8 = 1;
    pub const ARROW_UP: u8 = 2;
    pub const ARROW_DOWN: u8 = 3;
}

#[derive(Clone)]
pub struct Marker {
    pub time: i64,
    pub position: u8,
    pub shape: u8,
    pub color: Color,
    pub text: String,
}

#[derive(Clone)]
pub struct PriceLine {
    pub id: u32,
    pub price: f64,
    pub color: Color,
    pub width: i32,
    pub style: LineStyle,
    pub title: String,
}

pub struct SeriesEntry {
    pub id: SeriesId,
    pub kind: SeriesKind,
    pub line_color: Color,
    pub up_color: Option<Color>,
    pub down_color: Option<Color>,
    /// Candlestick wick colors per direction; `None` falls back to the body color (LWC parity).
    pub wick_up_color: Option<Color>,
    pub wick_down_color: Option<Color>,
    /// Candlestick border colors per direction; `None` falls back to the body color (LWC parity).
    pub border_up_color: Option<Color>,
    pub border_down_color: Option<Color>,
    /// Candlestick part visibility; `None` = visible (LWC parity).
    pub wick_visible: Option<bool>,
    pub border_visible: Option<bool>,
    pub line_width: Option<f64>,
    pub area_top_color: Option<Color>,
    pub area_bottom_color: Option<Color>,
    pub histogram_updown: bool,
    pub overlay: bool,
    pub left_scale: bool,
    pub pane_index: usize,
    pub line_type: LineType,
    pub point_markers: bool,
    pub visible: bool,
    pub baseline: Option<f64>,
    pub last_price_animation: bool,
    pub price_lines: Vec<PriceLine>,
    pub markers: Vec<Marker>,
    pub markers_auto_scale: bool,
    /// Tombstone flag (LWC `removeSeries`). `SeriesId` is a positional index into the data layer
    /// and this vector, so a removed series keeps its slot (data emptied, hidden) rather than being
    /// compacted; every other series keeps its id. Removed slots are inert in every draw/scale path
    /// because they carry no data and are not visible.
    pub removed: bool,
}

impl SeriesEntry {
    pub fn new(id: SeriesId, kind: SeriesKind) -> Self {
        Self {
            id,
            kind,
            line_color: DEFAULT_LINE_COLOR,
            up_color: None,
            down_color: None,
            wick_up_color: None,
            wick_down_color: None,
            border_up_color: None,
            border_down_color: None,
            wick_visible: None,
            border_visible: None,
            line_width: None,
            area_top_color: None,
            area_bottom_color: None,
            histogram_updown: false,
            overlay: false,
            left_scale: false,
            pane_index: 0,
            line_type: LineType::Simple,
            point_markers: false,
            visible: true,
            baseline: None,
            last_price_animation: false,
            price_lines: Vec::new(),
            markers: Vec::new(),
            markers_auto_scale: true,
            removed: false,
        }
    }
}

pub const PANE_SEPARATOR: f64 = 1.0;

pub struct Pane {
    pub price_scale: PriceScaleCore,
    pub left_scale: PriceScaleCore,
    pub overlay_scale: PriceScaleCore,
    pub stretch_factor: f64,
    pub overlay_top: f64,
    pub overlay_bottom: f64,
    pub marker_margin_above: f64,
    pub marker_margin_below: f64,
    pub left_marker_margin_above: f64,
    pub left_marker_margin_below: f64,
    pub overlay_marker_margin_above: f64,
    pub overlay_marker_margin_below: f64,
    pub top: f64,
    pub height: f64,
}

impl Pane {
    pub fn new() -> Self {
        let main_scale = PriceScaleCore::new(PriceScaleCoreOptions::default());
        let overlay_scale = PriceScaleCore::new(PriceScaleCoreOptions {
            scale_margins: PriceScaleMargins {
                top: 0.8,
                bottom: 0.0,
            },
            ..PriceScaleCoreOptions::default()
        });
        Self {
            price_scale: main_scale,
            left_scale: PriceScaleCore::new(PriceScaleCoreOptions::default()),
            overlay_scale,
            stretch_factor: 1.0,
            overlay_top: 0.8,
            overlay_bottom: 0.0,
            marker_margin_above: 0.0,
            marker_margin_below: 0.0,
            left_marker_margin_above: 0.0,
            left_marker_margin_below: 0.0,
            overlay_marker_margin_above: 0.0,
            overlay_marker_margin_below: 0.0,
            top: 0.0,
            height: 0.0,
        }
    }

    pub fn layout(&mut self, content_h: f64) {
        self.price_scale.set_height(content_h);
        self.left_scale.set_height(content_h);
        self.overlay_scale.set_height(content_h);
        self.refresh_internal_margins();
    }

    pub fn refresh_internal_margins(&mut self) {
        let content_h = self.price_scale.height();
        let below = (content_h - self.top - self.height).max(0.0);
        self.price_scale.set_internal_margins(
            self.top + self.marker_margin_above,
            below + self.marker_margin_below,
        );
        self.left_scale.set_internal_margins(
            self.top + self.left_marker_margin_above,
            below + self.left_marker_margin_below,
        );
        self.overlay_scale.set_internal_margins(
            self.top + self.overlay_marker_margin_above,
            below + self.overlay_marker_margin_below,
        );
    }
}

impl Default for Pane {
    fn default() -> Self {
        Self::new()
    }
}

/// Platform-independent state for one chart instance.
pub struct ChartEngine {
    pub time_scale: TimeScaleCore,
    pub panes: Vec<Pane>,
    pub price_formatter: PriceFormatter,
    pub data: DataLayer,
    pub series: Vec<SeriesEntry>,
    pub tick_marks: TimeTickMarks,
    pub options: ChartOptionsStore,
    pub crosshair_mode: CrosshairMode,
    pub animation_time: f64,
    pub next_price_line_id: u32,
    pub time_visible: bool,
    /// LWC `timeScale.secondsVisible` — include seconds in axis/crosshair time labels when
    /// `time_visible` is set. Defaults to false (LWC default).
    pub seconds_visible: bool,
    pub css_width: f64,
    pub css_height: f64,
    pub dpr: f64,
    pub crosshair: Option<(f64, f64)>,
    pub pane_w: f64,
    pub pane_h: f64,
    /// Media-coordinate x origin of the pane after reserving a visible left axis.
    pub pane_left: f64,
    pub left_axis_w: f64,
    pub axis_w: f64,
    indicators: Vec<IndicatorBinding>,
    synced_points_len: usize,
    synced_last_time: Option<i64>,
    /// Optional host formatting callbacks (LWC `localization.priceFormatter`/`timeFormatter` and
    /// `timeScale.tickMarkFormatter`). The engine stays headless — the host supplies plain boxed
    /// closures; each returns `None` to fall back to the built-in formatter (e.g. the callback
    /// threw at the boundary). Kept as trait objects, so `ChartEngine` is intentionally not
    /// `Clone`/`Debug`/`Send`.
    price_formatter_fn: Option<PriceFormatterFn>,
    tick_mark_formatter_fn: Option<TickMarkFormatterFn>,
    time_formatter_fn: Option<TimeFormatterFn>,
}

impl ChartEngine {
    pub fn new(css_width: f64, css_height: f64, dpr: f64) -> Self {
        let mut data = DataLayer::new();
        let main = data.add_series();
        Self {
            time_scale: TimeScaleCore::new(TimeScaleOptions::default()),
            panes: vec![Pane::new()],
            price_formatter: PriceFormatter::default(),
            data,
            series: vec![SeriesEntry::new(main, SeriesKind::Candlestick)],
            tick_marks: TimeTickMarks::new(),
            options: ChartOptionsStore::new(),
            crosshair_mode: CrosshairMode::Normal,
            animation_time: 0.0,
            next_price_line_id: 1,
            time_visible: true,
            seconds_visible: false,
            css_width,
            css_height,
            dpr,
            crosshair: None,
            pane_w: css_width,
            pane_h: css_height,
            pane_left: 0.0,
            left_axis_w: 0.0,
            axis_w: 0.0,
            indicators: Vec::new(),
            synced_points_len: 0,
            synced_last_time: None,
            price_formatter_fn: None,
            tick_mark_formatter_fn: None,
            time_formatter_fn: None,
        }
    }

    /// Install (or clear with `None`) the host price formatter (LWC `localization.priceFormatter`).
    /// Applied to non-percentage price labels; a `None` return from the callback falls back to the
    /// built-in formatter.
    pub fn set_price_formatter(&mut self, f: Option<PriceFormatterFn>) {
        self.price_formatter_fn = f;
    }

    /// Install (or clear) the host time-axis tick formatter (LWC `timeScale.tickMarkFormatter`).
    /// The callback receives the UTC-second timestamp and the tick-mark type (0 Year, 1 Month,
    /// 2 DayOfMonth, 3 Time, 4 TimeWithSeconds).
    pub fn set_tick_mark_formatter(&mut self, f: Option<TickMarkFormatterFn>) {
        self.tick_mark_formatter_fn = f;
    }

    /// Install (or clear) the host crosshair time formatter (LWC `localization.timeFormatter`).
    pub fn set_time_formatter(&mut self, f: Option<TimeFormatterFn>) {
        self.time_formatter_fn = f;
    }

    /// Add a series to the headless chart. The returned id is stable for the instance lifetime.
    pub fn add_series(&mut self, kind: SeriesKind) -> SeriesId {
        let id = self.data.add_series();
        self.series.push(SeriesEntry::new(id, kind));
        id
    }

    /// Remove a series (LWC `removeSeries`). The primary series (id 0) anchors the crosshair,
    /// last-value badge, and pulse, so it cannot be removed — `remove_series(0)` returns false, as
    /// does an unknown or already-removed id. Any indicators bound to (or derived from) the series
    /// are dropped with it. Returns true if a live series was removed.
    ///
    /// The slot is tombstoned rather than compacted: `SeriesId` is a positional index into the
    /// data layer and the series list (`series[rs.id]` is used directly), so compaction would
    /// invalidate every other id. The emptied, hidden slot is inert in all draw/scale paths.
    pub fn remove_series(&mut self, id: SeriesId) -> bool {
        if id == 0 || !self.series.iter().any(|s| s.id == id && !s.removed) {
            return false;
        }
        // Drop indicator bindings touching this series and collect their output series to tombstone
        // alongside it (a removed source leaves no derived data behind).
        let mut tombstones = self.drop_indicators_touching(id);
        tombstones.push(id);
        for rid in tombstones {
            if let Some(entry) = self.series.iter_mut().find(|s| s.id == rid) {
                entry.removed = true;
                entry.visible = false;
                entry.price_lines.clear();
                entry.markers.clear();
            }
            // Empty the data slot; this rebuilds the merged time points so the removed series'
            // timestamps leave the shared time axis.
            self.data.set_data(
                rid,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
        }
        self.sync_time_points();
        true
    }

    /// Whether `id` names a tombstoned (removed) series. Data mutations on such a slot are ignored
    /// so a removed series can never be silently revived.
    pub fn is_series_removed(&self, id: SeriesId) -> bool {
        self.series.iter().any(|s| s.id == id && s.removed)
    }

    /// Toggle a series without destroying its data or indicator binding.
    pub fn set_series_visible(&mut self, id: SeriesId, visible: bool) {
        if let Some(series) = self.series.iter_mut().find(|series| series.id == id) {
            series.visible = visible;
        }
    }

    pub fn set_series_markers(&mut self, id: SeriesId, markers: Vec<Marker>) {
        if let Some(series) = self.series.iter_mut().find(|series| series.id == id) {
            series.markers = markers;
        }
    }

    pub fn set_series_markers_auto_scale(&mut self, id: SeriesId, enabled: bool) {
        if let Some(series) = self.series.iter_mut().find(|series| series.id == id) {
            series.markers_auto_scale = enabled;
        }
    }

    /// Apply one streaming OHLC update after validating its time and values.
    pub fn update_series_bar(&mut self, id: SeriesId, time: f64, values: [f64; 4]) -> bool {
        if self.is_series_removed(id) {
            return false;
        }
        let Some((time, values)) = sanitize_point(time, values) else {
            return false;
        };
        self.data.update(id, time, values);
        self.sync_time_points();
        self.update_indicators_after_source_update(id, time);
        true
    }

    /// Validate and install one series' parallel OHLC columns without involving a host runtime.
    /// The returned report lets browser, native, and server callers expose identical diagnostics.
    pub fn set_series_data(
        &mut self,
        id: SeriesId,
        times: &[f64],
        open: &[f64],
        high: &[f64],
        low: &[f64],
        close: &[f64],
    ) -> Result<ValidationReport, ValidationError> {
        // A removed slot must stay empty; ignore the data (the TS series handle rejects the call
        // before it reaches here, so this is defense-in-depth) and report a clean no-op.
        if self.is_series_removed(id) {
            return Ok(ValidationReport::default());
        }
        let sanitized = sanitize_ohlc(times, open, high, low, close)?;
        let report = sanitized.report.clone();
        self.data.set_data(
            id,
            sanitized.times,
            sanitized.open,
            sanitized.high,
            sanitized.low,
            sanitized.close,
        );
        self.sync_time_points();
        self.recompute_indicators();
        Ok(report)
    }

    /// Install columns that have already crossed the validation boundary (used by adapters that
    /// need to report the sanitization details before handing ownership to the engine).
    pub fn install_series_data(
        &mut self,
        id: SeriesId,
        times: Vec<i64>,
        open: Vec<f64>,
        high: Vec<f64>,
        low: Vec<f64>,
        close: Vec<f64>,
    ) {
        if self.is_series_removed(id) {
            return;
        }
        self.data.set_data(id, times, open, high, low, close);
        self.sync_time_points();
        self.recompute_indicators();
    }

    /// Fit the horizontal scale to the current union of series timestamps.
    pub fn fit_content(&mut self) {
        self.time_scale.fit_content();
    }

    /// Apply the public horizontal-scale spacing while keeping ownership in the headless model.
    pub fn set_bar_spacing(&mut self, spacing: f64) {
        if spacing.is_finite() && spacing > 0.0 {
            self.time_scale.set_bar_spacing(spacing);
        }
    }

    /// Apply the public horizontal-scale right offset in logical bars.
    pub fn set_right_offset(&mut self, offset: f64) {
        if offset.is_finite() {
            self.time_scale.set_right_offset(offset);
        }
    }

    /// LWC `timeScale.timeVisible`: show the time of day in axis/crosshair labels.
    pub fn set_time_visible(&mut self, visible: bool) {
        self.time_visible = visible;
    }

    /// LWC `timeScale.secondsVisible`: include seconds when `time_visible` is set.
    pub fn set_seconds_visible(&mut self, visible: bool) {
        self.seconds_visible = visible;
    }

    /// LWC `timeScale.minBarSpacing`.
    pub fn set_min_bar_spacing(&mut self, spacing: f64) {
        self.time_scale.set_min_bar_spacing(spacing);
    }

    /// LWC `timeScale.fixLeftEdge`.
    pub fn set_fix_left_edge(&mut self, fix: bool) {
        self.time_scale.set_fix_left_edge(fix);
    }

    /// LWC `timeScale.fixRightEdge`.
    pub fn set_fix_right_edge(&mut self, fix: bool) {
        self.time_scale.set_fix_right_edge(fix);
    }

    /// LWC `timeScale.lockVisibleTimeRangeOnResize`.
    pub fn set_lock_visible_time_range_on_resize(&mut self, lock: bool) {
        self.time_scale.set_lock_visible_time_range_on_resize(lock);
    }

    /// LWC `timeScale.rightBarStaysOnScroll`.
    pub fn set_right_bar_stays_on_scroll(&mut self, stays: bool) {
        self.time_scale.set_right_bar_stays_on_scroll(stays);
    }

    /// Host-pushed "all scaling and scrolling disabled" aggregate (LWC
    /// `_isAllScalingAndScrollingDisabled`): forces fix-edge semantics on the time scale.
    pub fn set_interaction_disabled(&mut self, disabled: bool) {
        self.time_scale.set_interaction_disabled(disabled);
    }

    pub fn bar_spacing(&self) -> f64 {
        self.time_scale.bar_spacing()
    }

    pub fn right_offset(&self) -> f64 {
        self.time_scale.right_offset()
    }

    /// Current distance, in logical bars, from the latest data point to the right edge.
    pub fn scroll_position(&self) -> f64 {
        self.time_scale.right_offset()
    }

    /// Move the latest data point to `position` logical bars from the right edge. Animation is a
    /// host scheduling concern; this headless operation applies the target state immediately.
    pub fn scroll_to_position(&mut self, position: f64) {
        self.set_right_offset(position);
    }

    /// Restore the real-time edge. This intentionally targets zero rather than the configured
    /// default offset, matching Lightweight Charts' `scrollToRealTime` contract.
    pub fn scroll_to_real_time(&mut self) {
        self.time_scale.set_right_offset(0.0);
    }

    /// Restore the configured default bar spacing and right offset.
    pub fn reset_time_scale(&mut self) {
        self.time_scale.restore_default();
    }

    /// X coordinate for an integer logical index, or `None` when the scale has no points.
    pub fn logical_to_coordinate(&self, logical: f64) -> Option<f64> {
        if !logical.is_finite() || self.data.merged_times().is_empty() {
            return None;
        }
        // LWC's internal indexToCoordinate returns zero for non-integer runtime input. The public
        // Logical nominal type normally prevents this, but preserving it makes the JS boundary
        // deterministic for untyped callers too.
        if logical.fract() != 0.0 {
            return Some(0.0);
        }
        Some(
            self.time_scale
                .index_to_coordinate(logical as TimePointIndex),
        )
    }

    /// Integer logical bar owning an X coordinate. Values may extend outside the data.
    pub fn coordinate_to_logical(&self, x: f64) -> Option<f64> {
        if !x.is_finite() || self.data.merged_times().is_empty() {
            return None;
        }
        Some(self.time_scale.coordinate_to_index(x) as f64)
    }

    /// Logical index for a UTC-seconds timestamp. With `find_nearest`, select the first point at
    /// or after the timestamp and clamp timestamps beyond the last point to that final point,
    /// matching LWC's lower-bound behavior.
    pub fn time_to_index(&self, time: f64, find_nearest: bool) -> Option<TimePointIndex> {
        if !time.is_finite() {
            return None;
        }
        let times = self.data.merged_times();
        if times.is_empty() {
            return None;
        }
        let time = time as i64;
        let index = times.partition_point(|&point| point < time);
        if index < times.len() && times[index] == time {
            return Some(index as TimePointIndex);
        }
        if !find_nearest {
            return None;
        }
        Some(index.min(times.len() - 1) as TimePointIndex)
    }

    /// X coordinate for an exact UTC-seconds timestamp.
    pub fn time_to_coordinate(&self, time: f64) -> Option<f64> {
        let index = self.time_to_index(time, false)?;
        Some(self.time_scale.index_to_coordinate(index))
    }

    /// UTC-seconds timestamp at the rounded logical index under X.
    pub fn coordinate_to_time(&self, x: f64) -> Option<f64> {
        if !x.is_finite() {
            return None;
        }
        let times = self.data.merged_times();
        let index = self.time_scale.coordinate_to_index(x);
        if index < 0 || index as usize >= times.len() {
            return None;
        }
        Some(times[index as usize] as f64)
    }

    pub fn visible_logical_range(&self) -> Option<(f64, f64)> {
        self.time_scale
            .visible_logical_range()
            .map(|range| (range.left(), range.right()))
    }

    pub fn set_visible_logical_range(&mut self, from: f64, to: f64) {
        if from.is_finite() && to.is_finite() && from <= to {
            self.time_scale
                .set_logical_range(LogicalRange::new(from, to));
        }
    }

    /// Visible data timestamps nearest the logical window edges.
    pub fn visible_time_range(&self) -> Option<(f64, f64)> {
        let times = self.data.merged_times();
        let range = self.time_scale.visible_strict_range()?;
        if times.is_empty() {
            return None;
        }
        let last = times.len() as i64 - 1;
        let left = range.left().clamp(0, last) as usize;
        let right = range.right().clamp(0, last) as usize;
        Some((times[left] as f64, times[right] as f64))
    }

    /// Set the visible window to the points bracketing a UTC-seconds range.
    pub fn set_visible_time_range(&mut self, from: f64, to: f64) {
        if !from.is_finite() || !to.is_finite() || from > to {
            return;
        }
        let times = self.data.merged_times();
        if times.is_empty() {
            return;
        }
        let left = times.partition_point(|&time| (time as f64) < from);
        let right = times.partition_point(|&time| (time as f64) <= to);
        if right == 0 || left >= times.len() {
            return;
        }
        let last = times.len() - 1;
        let left = left.min(last) as i64;
        let right = (right - 1).min(last) as i64;
        if left <= right {
            self.time_scale
                .set_visible_range(StrictRange::new(left, right), false);
        }
    }

    /// Lay out stacked panes inside the chart content area. This is shared by hosts that need
    /// pane bounds before frame submission (for example, to draw axis separators).
    pub fn layout_panes(&mut self, content_h: f64) {
        let usable =
            (content_h - PANE_SEPARATOR * self.panes.len().saturating_sub(1) as f64).max(1.0);
        let total: f64 = self.panes.iter().map(|p| p.stretch_factor.max(0.01)).sum();
        let mut top = 0.0;
        let pane_count = self.panes.len();
        for (i, pane) in self.panes.iter_mut().enumerate() {
            pane.top = top;
            pane.height = usable * pane.stretch_factor.max(0.01) / total;
            pane.layout(content_h);
            top += pane.height;
            if i + 1 < pane_count {
                top += PANE_SEPARATOR;
            }
        }
    }

    fn sync_time_points(&mut self) {
        let times = self.data.merged_times();
        let appended = times.len() == self.synced_points_len + 1
            && !times.is_empty()
            && times.last().copied() > self.synced_last_time;
        if appended {
            let mut weights = vec![0u8; times.len()];
            aion_core::scale::time_tick_marks::fill_weights_for_points(
                times,
                &mut weights,
                self.synced_points_len,
            );
            self.tick_marks
                .append_weights(self.synced_points_len, &weights);
        } else if times.len() != self.synced_points_len {
            let mut weights = vec![0u8; times.len()];
            aion_core::scale::time_tick_marks::fill_weights_for_points(times, &mut weights, 0);
            self.tick_marks.set_weights(&weights);
        }
        self.synced_points_len = times.len();
        self.synced_last_time = times.last().copied();
        self.time_scale.set_points_len(times.len());
        self.time_scale.set_base_index(self.data.base_index());
    }
}
