//! Headless Aion chart engine.
//!
//! This crate owns chart state and behavior without depending on WASM, the DOM, WebGPU, or a
//! native windowing system. Hosts provide input and a viewport; rendering backends consume the
//! frame produced from this state. During the architecture recovery, frame construction is being
//! migrated here incrementally from `aion_wasm`.

mod frame;
pub use frame::{AxisFrame, AxisLabel, AxisTextAlign, AxisTextMidpoint, ChartFrame, FramePane};

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

#[derive(Clone, Debug, PartialEq)]
pub enum IndicatorKind {
    Sma { period: usize },
    Ema { period: usize },
    Bollinger { period: usize, deviation: f64 },
}

#[derive(Clone, Debug)]
struct IndicatorBinding {
    source: SeriesId,
    kind: IndicatorKind,
    outputs: Vec<SeriesId>,
    last_source_len: usize,
    last_source_time: Option<i64>,
}

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
}

impl SeriesEntry {
    pub fn new(id: SeriesId, kind: SeriesKind) -> Self {
        Self {
            id,
            kind,
            line_color: DEFAULT_LINE_COLOR,
            up_color: None,
            down_color: None,
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
        }
    }

    /// Add a series to the headless chart. The returned id is stable for the instance lifetime.
    pub fn add_series(&mut self, kind: SeriesKind) -> SeriesId {
        let id = self.data.add_series();
        self.series.push(SeriesEntry::new(id, kind));
        id
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

    /// Add a Rust-native simple moving-average producer. The returned line series is owned by the
    /// engine and is recomputed whenever its source series changes.
    pub fn add_sma(&mut self, source: SeriesId, period: usize) -> Option<SeriesId> {
        self.add_indicator(source, IndicatorKind::Sma { period }, 1)
            .into_iter()
            .next()
    }

    /// Add a Rust-native exponential moving-average producer.
    pub fn add_ema(&mut self, source: SeriesId, period: usize) -> Option<SeriesId> {
        self.add_indicator(source, IndicatorKind::Ema { period }, 1)
            .into_iter()
            .next()
    }

    /// Add upper, middle, and lower Bollinger-band line series in that order.
    pub fn add_bollinger(
        &mut self,
        source: SeriesId,
        period: usize,
        deviation: f64,
    ) -> Vec<SeriesId> {
        self.add_indicator(source, IndicatorKind::Bollinger { period, deviation }, 3)
    }

    fn add_indicator(
        &mut self,
        source: SeriesId,
        kind: IndicatorKind,
        outputs: usize,
    ) -> Vec<SeriesId> {
        if source >= self.series.len()
            || outputs == 0
            || matches!(
                &kind,
                IndicatorKind::Sma { period: 0 }
                    | IndicatorKind::Ema { period: 0 }
                    | IndicatorKind::Bollinger { period: 0, .. }
            )
        {
            return Vec::new();
        }
        let ids = (0..outputs)
            .map(|_| self.add_series(SeriesKind::Line))
            .collect::<Vec<_>>();
        self.indicators.push(IndicatorBinding {
            source,
            kind,
            outputs: ids.clone(),
            last_source_len: 0,
            last_source_time: None,
        });
        self.recompute_indicators();
        ids
    }

    /// Apply one streaming OHLC update after validating its time and values.
    pub fn update_series_bar(&mut self, id: SeriesId, time: f64, values: [f64; 4]) -> bool {
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

    pub fn price_scale_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<&PriceScaleCore> {
        let pane = self.panes.get(pane)?;
        Some(match target {
            PriceScaleTarget::Right => &pane.price_scale,
            PriceScaleTarget::Left => &pane.left_scale,
            PriceScaleTarget::Overlay => &pane.overlay_scale,
        })
    }

    pub fn price_scale_for_mut(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<&mut PriceScaleCore> {
        let pane = self.panes.get_mut(pane)?;
        Some(match target {
            PriceScaleTarget::Right => &mut pane.price_scale,
            PriceScaleTarget::Left => &mut pane.left_scale,
            PriceScaleTarget::Overlay => &mut pane.overlay_scale,
        })
    }

    /// Current visible raw-value range for a pane price scale.
    pub fn price_scale_visible_range(&self, pane: usize, overlay: bool) -> Option<(f64, f64)> {
        self.price_scale_visible_range_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_visible_range_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<(f64, f64)> {
        let range = self.price_scale_for(pane, target)?.price_range_for_api()?;
        Some((range.min_value(), range.max_value()))
    }

    /// Install a manual raw-value range and disable autoscale, matching LWC `setVisibleRange`.
    pub fn set_price_scale_visible_range(
        &mut self,
        pane: usize,
        overlay: bool,
        from: f64,
        to: f64,
    ) {
        self.set_price_scale_visible_range_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            from,
            to,
        );
    }

    pub fn set_price_scale_visible_range_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        from: f64,
        to: f64,
    ) {
        if !from.is_finite() || !to.is_finite() || from >= to {
            return;
        }
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_auto_scale(false);
            let range = scale.price_range_from_api(&PriceRange::new(from, to));
            scale.set_price_range(Some(range));
        }
    }

    pub fn price_scale_auto_scale(&self, pane: usize, overlay: bool) -> Option<bool> {
        self.price_scale_auto_scale_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_auto_scale_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<bool> {
        Some(self.price_scale_for(pane, target)?.is_auto_scale())
    }

    pub fn set_price_scale_auto_scale(&mut self, pane: usize, overlay: bool, enabled: bool) {
        self.set_price_scale_auto_scale_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            enabled,
        );
    }

    pub fn set_price_scale_auto_scale_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        enabled: bool,
    ) {
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_auto_scale(enabled);
        }
    }

    pub fn price_scale_inverted(&self, pane: usize, overlay: bool) -> Option<bool> {
        self.price_scale_inverted_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_inverted_for(&self, pane: usize, target: PriceScaleTarget) -> Option<bool> {
        Some(self.price_scale_for(pane, target)?.is_inverted())
    }

    pub fn set_price_scale_inverted(&mut self, pane: usize, overlay: bool, inverted: bool) {
        self.set_price_scale_inverted_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            inverted,
        );
    }

    pub fn set_price_scale_inverted_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        inverted: bool,
    ) {
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_invert_scale(inverted);
        }
    }

    pub fn price_scale_margins(&self, pane: usize, overlay: bool) -> Option<(f64, f64)> {
        self.price_scale_margins_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_margins_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<(f64, f64)> {
        let margins = self.price_scale_for(pane, target)?.options().scale_margins;
        Some((margins.top, margins.bottom))
    }

    pub fn set_price_scale_margins(&mut self, pane: usize, overlay: bool, top: f64, bottom: f64) {
        self.set_price_scale_margins_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            top,
            bottom,
        );
    }

    pub fn set_price_scale_margins_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        top: f64,
        bottom: f64,
    ) {
        if !top.is_finite()
            || !bottom.is_finite()
            || top < 0.0
            || bottom < 0.0
            || top > 1.0
            || bottom > 1.0
            || top + bottom > 1.0
        {
            return;
        }
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_scale_margins(top, bottom);
        }
    }

    pub fn price_scale_mode(&self, pane: usize, overlay: bool) -> Option<PriceScaleMode> {
        self.price_scale_mode_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
        )
    }

    pub fn price_scale_mode_for(
        &self,
        pane: usize,
        target: PriceScaleTarget,
    ) -> Option<PriceScaleMode> {
        Some(self.price_scale_for(pane, target)?.mode())
    }

    pub fn set_price_scale_mode(&mut self, pane: usize, overlay: bool, mode: PriceScaleMode) {
        self.set_price_scale_mode_for(
            pane,
            if overlay {
                PriceScaleTarget::Overlay
            } else {
                PriceScaleTarget::Right
            },
            mode,
        );
    }

    pub fn set_price_scale_mode_for(
        &mut self,
        pane: usize,
        target: PriceScaleTarget,
        mode: PriceScaleMode,
    ) {
        if let Some(scale) = self.price_scale_for_mut(pane, target) {
            scale.set_mode(mode);
        }
    }

    pub fn set_series_price_scale(&mut self, id: SeriesId, target: PriceScaleTarget) {
        if let Some(series) = self.series.iter_mut().find(|series| series.id == id) {
            series.overlay = target == PriceScaleTarget::Overlay;
            series.left_scale = target == PriceScaleTarget::Left;
        }
    }

    pub fn series_price_scale(&self, id: SeriesId) -> Option<(usize, PriceScaleTarget)> {
        self.series
            .iter()
            .find(|series| series.id == id)
            .map(|series| {
                let target = if series.overlay {
                    PriceScaleTarget::Overlay
                } else if series.left_scale {
                    PriceScaleTarget::Left
                } else {
                    PriceScaleTarget::Right
                };
                (series.pane_index, target)
            })
    }

    /// First close at or to the right of the visible left edge, matching LWC series first-value
    /// selection for percentage/indexed coordinate modes.
    pub(crate) fn series_base_value(&self, id: SeriesId, visible_from: i64) -> Option<f64> {
        let plot = self.data.plot(id);
        let row = plot.search(visible_from, MismatchDirection::NearestRight)?;
        let value = plot.value_at(row, PlotValueIndex::Close);
        value.is_finite().then_some(value)
    }

    pub(crate) fn visible_series_base_value(&self, id: SeriesId) -> Option<f64> {
        let from = self.time_scale.visible_strict_range()?.left();
        self.series_base_value(id, from)
    }

    pub fn series_price_to_coordinate(&self, id: SeriesId, price: f64) -> Option<f64> {
        if !price.is_finite() {
            return None;
        }
        let (pane, target) = self.series_price_scale(id)?;
        let scale = self.price_scale_for(pane, target)?;
        if scale.is_empty() {
            return None;
        }
        let base = self.visible_series_base_value(id)?;
        Some(scale.price_to_coordinate(price, base))
    }

    pub fn series_coordinate_to_price(&self, id: SeriesId, coordinate: f64) -> Option<f64> {
        if !coordinate.is_finite() {
            return None;
        }
        let (pane, target) = self.series_price_scale(id)?;
        let scale = self.price_scale_for(pane, target)?;
        if scale.is_empty() {
            return None;
        }
        let base = self.visible_series_base_value(id)?;
        Some(scale.coordinate_to_price(coordinate, base))
    }

    pub fn series_kind(&self, id: SeriesId) -> Option<SeriesKind> {
        self.series
            .iter()
            .find(|series| series.id == id)
            .map(|series| series.kind)
    }

    fn series_point_at_row(&self, id: SeriesId, row: usize) -> Option<SeriesDataPoint> {
        let plot = self.data.plot(id);
        let index = *plot.indices().get(row)?;
        let time = *self.data.merged_times().get(index as usize)?;
        Some(SeriesDataPoint {
            time,
            open: plot.value_at(row, PlotValueIndex::Open),
            high: plot.value_at(row, PlotValueIndex::High),
            low: plot.value_at(row, PlotValueIndex::Low),
            close: plot.value_at(row, PlotValueIndex::Close),
        })
    }

    pub fn series_data_by_index(
        &self,
        id: SeriesId,
        logical_index: i64,
        mismatch: MismatchDirection,
    ) -> Option<SeriesDataPoint> {
        let row = self.data.plot(id).search(logical_index, mismatch)?;
        self.series_point_at_row(id, row)
    }

    pub fn series_data(&self, id: SeriesId) -> Vec<SeriesDataPoint> {
        let size = self.data.plot(id).size();
        (0..size)
            .filter_map(|row| self.series_point_at_row(id, row))
            .collect()
    }

    /// LWC `barsInLogicalRange`, including its gap behavior and fractional bars-before/after
    /// results. Times are original UTC seconds of the first/last series bars inside the range.
    pub fn series_bars_in_logical_range(
        &self,
        id: SeriesId,
        from: f64,
        to: f64,
    ) -> Option<BarsInLogicalRange> {
        if !from.is_finite() || !to.is_finite() || from > to {
            return None;
        }
        let plot = self.data.plot(id);
        let data_first = plot.first_index()?;
        let data_last = plot.last_index()?;
        let strict = LogicalRange::new(from, to).to_strict();
        let first_row = plot.search(strict.left(), MismatchDirection::NearestRight);
        let last_row = plot.search(strict.right(), MismatchDirection::NearestLeft);
        let first_index = first_row.and_then(|row| plot.indices().get(row).copied());
        let last_index = last_row.and_then(|row| plot.indices().get(row).copied());

        if first_index
            .zip(last_index)
            .is_some_and(|(first, last)| first > last)
        {
            return Some(BarsInLogicalRange {
                bars_before: from - data_first as f64,
                bars_after: data_last as f64 - to,
                from: None,
                to: None,
            });
        }

        let bars_before = match first_index {
            None => from - data_first as f64,
            Some(index) if index == data_first => from - data_first as f64,
            Some(index) => (index - data_first) as f64,
        };
        let bars_after = match last_index {
            None => data_last as f64 - to,
            Some(index) if index == data_last => data_last as f64 - to,
            Some(index) => (data_last - index) as f64,
        };
        let times = first_index.zip(last_index).and_then(|(first, last)| {
            Some((
                *self.data.merged_times().get(first as usize)?,
                *self.data.merged_times().get(last as usize)?,
            ))
        });
        Some(BarsInLogicalRange {
            bars_before,
            bars_after,
            from: times.map(|times| times.0),
            to: times.map(|times| times.1),
        })
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

    fn recompute_indicators(&mut self) {
        for index in 0..self.indicators.len() {
            let binding = self.indicators[index].clone();
            let Some((times, values)) = self.data.series_data(binding.source) else {
                continue;
            };
            let times = times.to_vec();
            let close = values[3].to_vec();
            match binding.kind {
                IndicatorKind::Sma { period } => {
                    let values = aion_indicators::sma(&close, period);
                    self.install_indicator_output(binding.outputs[0], &times, &values);
                }
                IndicatorKind::Ema { period } => {
                    let values = aion_indicators::ema(&close, period);
                    self.install_indicator_output(binding.outputs[0], &times, &values);
                }
                IndicatorKind::Bollinger { period, deviation } => {
                    let values = aion_indicators::bollinger(&close, period, deviation);
                    let mut upper = Vec::with_capacity(values.len());
                    let mut middle = Vec::with_capacity(values.len());
                    let mut lower = Vec::with_capacity(values.len());
                    for point in values {
                        upper.push(point.upper);
                        middle.push(point.middle);
                        lower.push(point.lower);
                    }
                    self.install_indicator_output(binding.outputs[0], &times, &upper);
                    self.install_indicator_output(binding.outputs[1], &times, &middle);
                    self.install_indicator_output(binding.outputs[2], &times, &lower);
                }
            }
            self.indicators[index].last_source_len = times.len();
            self.indicators[index].last_source_time = times.last().copied();
        }
        self.sync_time_points();
    }

    fn update_indicators_after_source_update(&mut self, source: SeriesId, time: i64) {
        for index in 0..self.indicators.len() {
            if self.indicators[index].source != source {
                continue;
            }
            let binding = self.indicators[index].clone();
            let Some((times, values)) = self.data.series_data(source) else {
                continue;
            };
            let source_len = times.len();
            let source_last_time = times.last().copied();
            let close = values[3];
            let tail_update = binding.last_source_len > 0
                && binding
                    .last_source_time
                    .map(|last| time >= last)
                    .unwrap_or(false)
                && (source_len == binding.last_source_len
                    || source_len == binding.last_source_len + 1);
            if !tail_update {
                self.recompute_indicators();
                return;
            }
            let appended = source_len == binding.last_source_len + 1;
            match binding.kind {
                IndicatorKind::Sma { period } => {
                    if let Some(value) = rolling_mean(close, period) {
                        self.data.update(binding.outputs[0], time, [value; 4]);
                    }
                }
                IndicatorKind::Ema { period } => {
                    if let Some(value) =
                        rolling_ema_tail(close, period, &self.data, binding.outputs[0], appended)
                    {
                        self.data.update(binding.outputs[0], time, [value; 4]);
                    }
                }
                IndicatorKind::Bollinger { period, deviation } => {
                    if let Some((upper, middle, lower)) =
                        rolling_bollinger(close, period, deviation)
                    {
                        self.data.update(binding.outputs[0], time, [upper; 4]);
                        self.data.update(binding.outputs[1], time, [middle; 4]);
                        self.data.update(binding.outputs[2], time, [lower; 4]);
                    }
                }
            }
            self.indicators[index].last_source_len = source_len;
            self.indicators[index].last_source_time = source_last_time.or(Some(time));
        }
        self.sync_time_points();
    }

    fn install_indicator_output(&mut self, id: SeriesId, times: &[i64], values: &[Option<f64>]) {
        let mut out_times = Vec::new();
        let mut out_values = Vec::new();
        for (&time, value) in times.iter().zip(values) {
            if let Some(value) = value {
                out_times.push(time);
                out_values.push(*value);
            }
        }
        self.data.set_data(
            id,
            out_times,
            out_values.clone(),
            out_values.clone(),
            out_values.clone(),
            out_values,
        );
    }
}

fn rolling_mean(values: &[f64], period: usize) -> Option<f64> {
    (period > 0 && values.len() >= period)
        .then(|| values[values.len() - period..].iter().sum::<f64>() / period as f64)
}

fn rolling_bollinger(values: &[f64], period: usize, deviation: f64) -> Option<(f64, f64, f64)> {
    let window =
        (period > 0 && values.len() >= period).then(|| &values[values.len() - period..])?;
    let middle = window.iter().sum::<f64>() / period as f64;
    let spread = (window.iter().map(|v| (v - middle).powi(2)).sum::<f64>() / period as f64).sqrt()
        * deviation.max(0.0);
    Some((middle + spread, middle, middle - spread))
}

fn rolling_ema_tail(
    values: &[f64],
    period: usize,
    data: &DataLayer,
    output: SeriesId,
    appended: bool,
) -> Option<f64> {
    if period == 0 || values.len() < period {
        return None;
    }
    if values.len() == period {
        return rolling_mean(values, period);
    }
    let previous = data.series_data(output)?;
    let output_values = previous.1[3];
    let previous_ema = if appended {
        output_values.last().copied()?
    } else if output_values.len() >= 2 {
        output_values[output_values.len() - 2]
    } else {
        return rolling_mean(&values[..values.len() - 1], period);
    };
    let alpha = 2.0 / (period as f64 + 1.0);
    Some(alpha * values[values.len() - 1] + (1.0 - alpha) * previous_ema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aion_render::canvas2d::{execute, Canvas2d, Viewport};
    use aion_render::color::Color;

    #[derive(Default)]
    struct CountingCanvas {
        calls: usize,
    }

    impl Canvas2d for CountingCanvas {
        fn set_fill_solid(&mut self, _color: Color) {
            self.calls += 1;
        }
        fn set_fill_vgradient(&mut self, _y_top: f32, _y_bottom: f32, _top: Color, _bottom: Color) {
            self.calls += 1;
        }
        fn set_stroke(&mut self, _color: Color) {
            self.calls += 1;
        }
        fn set_line_width(&mut self, _width: f32) {
            self.calls += 1;
        }
        fn set_line_dash(&mut self, _pattern: &[f32]) {
            self.calls += 1;
        }
        fn fill_rect(&mut self, _x: f32, _y: f32, _w: f32, _h: f32) {
            self.calls += 1;
        }
        fn begin_path(&mut self) {
            self.calls += 1;
        }
        fn move_to(&mut self, _x: f32, _y: f32) {
            self.calls += 1;
        }
        fn line_to(&mut self, _x: f32, _y: f32) {
            self.calls += 1;
        }
        fn close_path(&mut self) {
            self.calls += 1;
        }
        fn arc(&mut self, _cx: f32, _cy: f32, _r: f32, _start: f32, _end: f32) {
            self.calls += 1;
        }
        fn stroke(&mut self) {
            self.calls += 1;
        }
        fn fill(&mut self) {
            self.calls += 1;
        }
    }

    #[test]
    fn constructs_without_a_browser_or_gpu() {
        let chart = ChartEngine::new(800.0, 500.0, 2.0);
        assert_eq!(chart.series.len(), 1);
        assert_eq!(chart.panes.len(), 1);
        assert_eq!(chart.css_width, 800.0);
        assert_eq!(chart.dpr, 2.0);
    }

    #[test]
    fn pane_layout_is_host_independent() {
        let mut pane = Pane::new();
        pane.top = 100.0;
        pane.height = 200.0;
        pane.layout(500.0);
        pane.price_scale.apply_autoscale_range(
            Some(aion_core::model::price_range::PriceRange::new(0.0, 2.0)),
            0.01,
        );
        let y = pane.price_scale.price_to_coordinate(1.0, 1.0);
        assert!(y.is_finite() && (100.0..=300.0).contains(&y));
    }

    #[test]
    fn ingests_data_without_a_host_runtime() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        let report = chart
            .set_series_data(
                0,
                &[3.0, 1.0, 2.0],
                &[12.0, 10.0, 11.0],
                &[13.0, 11.0, 12.0],
                &[9.0, 8.0, 10.0],
                &[11.0, 10.0, 11.5],
            )
            .unwrap();
        assert!(report.reordered);
        assert_eq!(chart.data.merged_times(), &[1, 2, 3]);
        chart.time_scale.set_width(800.0);
        chart.fit_content();
        assert!(chart.time_scale.visible_logical_range().is_some());
        let frame = chart.build_frame();
        assert_eq!(frame.panes.len(), 1);
        assert!(!frame.panes[0].main.is_empty());
    }

    #[test]
    fn hidden_series_do_not_expand_autoscale() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart
            .set_series_data(
                0,
                &[1.0, 2.0],
                &[5.0, 6.0],
                &[10.0, 9.0],
                &[0.0, 1.0],
                &[7.0, 8.0],
            )
            .unwrap();
        let hidden = chart.add_series(SeriesKind::Line);
        chart
            .set_series_data(
                hidden,
                &[1.0, 2.0],
                &[1000.0, 1001.0],
                &[1000.0, 1001.0],
                &[1000.0, 1001.0],
                &[1000.0, 1001.0],
            )
            .unwrap();
        chart.set_series_visible(hidden, false);
        chart.time_scale.set_width(800.0);
        chart.fit_content();
        chart.autoscale_visible();
        assert_eq!(
            chart.panes[0]
                .price_scale
                .price_range()
                .unwrap()
                .max_value(),
            10.0
        );

        chart.set_series_visible(hidden, true);
        chart.autoscale_visible();
        assert_eq!(
            chart.panes[0]
                .price_scale
                .price_range()
                .unwrap()
                .max_value(),
            1001.0
        );
    }

    #[test]
    fn marker_autoscale_margins_are_headless_and_can_be_disabled() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart
            .set_series_data(
                0,
                &[1.0, 2.0],
                &[100.0, 101.0],
                &[102.0, 103.0],
                &[99.0, 100.0],
                &[101.0, 102.0],
            )
            .unwrap();
        chart.set_series_markers(
            0,
            vec![Marker {
                time: 2,
                position: marker_pos::ABOVE,
                shape: marker_shape::CIRCLE,
                color: Color::rgb(0x21, 0x96, 0xf3),
                text: String::new(),
            }],
        );
        chart.time_scale.set_width(800.0);
        chart.fit_content();
        chart.build_frame();
        // Two fitted bars clamp marker geometry to LWC's maximum spacing bucket.
        assert_eq!(chart.panes[0].marker_margin_above, 48.0);
        assert_eq!(chart.panes[0].marker_margin_below, 0.0);

        chart.set_series_markers_auto_scale(0, false);
        chart.build_frame();
        assert_eq!(chart.panes[0].marker_margin_above, 0.0);
        assert_eq!(chart.panes[0].marker_margin_below, 0.0);
    }

    #[test]
    fn public_time_scale_options_are_validated_and_headless() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart.time_scale.set_width(800.0);
        chart.set_bar_spacing(50.0);
        chart.set_right_offset(3.5);
        assert_eq!(chart.bar_spacing(), 50.0);
        assert_eq!(chart.right_offset(), 3.5);
        chart.set_bar_spacing(f64::NAN);
        chart.set_right_offset(f64::INFINITY);
        assert_eq!(chart.bar_spacing(), 50.0);
        assert_eq!(chart.right_offset(), 3.5);
    }

    #[test]
    fn richer_time_scale_queries_and_mutations_are_headless() {
        let mut chart = ChartEngine::new(300.0, 200.0, 1.0);
        chart
            .set_series_data(
                0,
                &[10.0, 20.0, 30.0],
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
            )
            .unwrap();
        chart.time_scale.set_width(300.0);
        chart.fit_content();

        assert_eq!(chart.time_to_index(20.0, false), Some(1));
        assert_eq!(chart.time_to_index(15.0, false), None);
        assert_eq!(chart.time_to_index(15.0, true), Some(1));
        assert_eq!(chart.time_to_index(35.0, true), Some(2));
        let x = chart.logical_to_coordinate(1.0).unwrap();
        assert_eq!(chart.coordinate_to_logical(x), Some(1.0));
        assert_eq!(chart.logical_to_coordinate(1.25), Some(0.0));
        assert_eq!(
            chart.time_to_coordinate(20.0),
            chart.logical_to_coordinate(1.0)
        );
        assert_eq!(
            chart.coordinate_to_time(chart.logical_to_coordinate(2.0).unwrap()),
            Some(30.0)
        );

        chart.scroll_to_position(4.0);
        // The core clamps excessive future whitespace for a three-point data set.
        assert_eq!(chart.scroll_position(), 1.0);
        chart.scroll_to_real_time();
        assert_eq!(chart.scroll_position(), 0.0);
        chart.set_bar_spacing(20.0);
        chart.set_right_offset(2.0);
        chart.reset_time_scale();
        assert_eq!(chart.bar_spacing(), 6.0);
        assert_eq!(chart.right_offset(), 0.0);

        chart.set_visible_time_range(10.0, 20.0);
        assert_eq!(chart.visible_time_range(), Some((10.0, 20.0)));
    }

    #[test]
    fn public_price_scale_state_is_headless_and_manual_ranges_survive_rendering() {
        let mut chart = ChartEngine::new(300.0, 200.0, 1.0);
        chart
            .set_series_data(
                0,
                &[10.0, 20.0, 30.0],
                &[100.0, 101.0, 102.0],
                &[101.0, 102.0, 103.0],
                &[99.0, 100.0, 101.0],
                &[100.5, 101.5, 102.5],
            )
            .unwrap();
        chart.time_scale.set_width(300.0);
        chart.layout_panes(172.0);
        chart.fit_content();
        chart.build_frame();
        assert_eq!(chart.price_scale_margins(0, false), Some((0.2, 0.1)));

        chart.set_price_scale_visible_range(0, false, 90.0, 110.0);
        assert_eq!(chart.price_scale_auto_scale(0, false), Some(false));
        chart.build_frame();
        assert_eq!(
            chart.price_scale_visible_range(0, false),
            Some((90.0, 110.0))
        );

        chart.set_price_scale_inverted(0, false, true);
        chart.set_price_scale_margins(0, false, 0.25, 0.15);
        assert_eq!(chart.price_scale_inverted(0, false), Some(true));
        assert_eq!(chart.price_scale_margins(0, false), Some((0.25, 0.15)));

        chart.set_price_scale_auto_scale(0, false, true);
        chart.build_frame();
        assert_eq!(chart.price_scale_auto_scale(0, false), Some(true));
        assert_ne!(
            chart.price_scale_visible_range(0, false),
            Some((90.0, 110.0))
        );
        assert_eq!(
            chart.series_price_scale(0),
            Some((0, PriceScaleTarget::Right))
        );

        chart.set_price_scale_mode(0, false, PriceScaleMode::Percentage);
        chart.build_frame();
        assert_eq!(
            chart.price_scale_mode(0, false),
            Some(PriceScaleMode::Percentage)
        );
        assert_eq!(chart.price_scale_auto_scale(0, false), Some(true));
        let coordinate = chart.series_price_to_coordinate(0, 101.5).unwrap();
        assert!((chart.series_coordinate_to_price(0, coordinate).unwrap() - 101.5).abs() < 1e-9);
        let axis = chart.build_axis_frame(80.0, |text| text.len() as f64 * 7.0);
        assert!(axis.labels.iter().any(|label| label.text.ends_with('%')));

        chart.set_price_scale_mode(0, false, PriceScaleMode::Logarithmic);
        chart.build_frame();
        let coordinate = chart.series_price_to_coordinate(0, 102.5).unwrap();
        assert!((chart.series_coordinate_to_price(0, coordinate).unwrap() - 102.5).abs() < 1e-8);
    }

    #[test]
    fn left_price_scale_owns_range_axis_labels_and_pane_origin() {
        let mut chart = ChartEngine::new(300.0, 200.0, 1.0);
        chart
            .set_series_data(
                0,
                &[10.0, 20.0, 30.0],
                &[100.0, 101.0, 102.0],
                &[101.0, 102.0, 103.0],
                &[99.0, 100.0, 101.0],
                &[100.5, 101.5, 102.5],
            )
            .unwrap();
        chart.set_series_price_scale(0, PriceScaleTarget::Left);
        chart
            .options
            .apply_str(r#"{"leftPriceScale":{"visible":true},"rightPriceScale":{"visible":false}}"#)
            .unwrap();
        chart.pane_left = 58.0;
        chart.left_axis_w = 58.0;
        chart.pane_w = 242.0;
        chart.time_scale.set_width(242.0);
        chart.layout_panes(172.0);
        chart.fit_content();

        let frame = chart.build_frame();
        assert_eq!(
            chart.series_price_scale(0),
            Some((0, PriceScaleTarget::Left))
        );
        assert!(chart
            .price_scale_visible_range_for(0, PriceScaleTarget::Left)
            .is_some());
        assert!(chart
            .price_scale_visible_range_for(0, PriceScaleTarget::Right)
            .is_none());
        assert_eq!(frame.width, 300.0);
        assert_eq!(frame.panes[0].scissor[0], 58);
        assert!(frame.panes[0].main.iter().any(|prim| matches!(
            prim,
            aion_render::draw_list::Prim::Rect { rect, .. } if rect.x >= 58
        )));

        let axis = chart.build_axis_frame(80.0, |text| text.len() as f64 * 7.0);
        assert!(axis
            .labels
            .iter()
            .any(|label| label.align == AxisTextAlign::Right));
        assert!(!axis
            .labels
            .iter()
            .any(|label| label.align == AxisTextAlign::Left));
        let coordinate = chart.series_price_to_coordinate(0, 101.5).unwrap();
        assert!((chart.series_coordinate_to_price(0, coordinate).unwrap() - 101.5).abs() < 1e-9);
    }

    #[test]
    fn series_data_and_logical_range_queries_match_lwc_gap_semantics() {
        let mut chart = ChartEngine::new(300.0, 200.0, 1.0);
        let times = (0..=10).map(|time| time as f64 * 10.0).collect::<Vec<_>>();
        let values = (0..=10).map(|value| value as f64).collect::<Vec<_>>();
        chart
            .set_series_data(0, &times, &values, &values, &values, &values)
            .unwrap();
        let sparse = chart.add_series(SeriesKind::Line);
        chart
            .set_series_data(
                sparse,
                &[0.0, 100.0],
                &[5.0, 15.0],
                &[5.0, 15.0],
                &[5.0, 15.0],
                &[5.0, 15.0],
            )
            .unwrap();

        assert_eq!(chart.series_kind(sparse), Some(SeriesKind::Line));
        assert_eq!(chart.series_data(sparse).len(), 2);
        assert_eq!(
            chart.series_data_by_index(sparse, 5, MismatchDirection::NearestLeft),
            Some(SeriesDataPoint {
                time: 0,
                open: 5.0,
                high: 5.0,
                low: 5.0,
                close: 5.0,
            })
        );
        assert_eq!(
            chart
                .series_data_by_index(sparse, 5, MismatchDirection::NearestRight)
                .map(|point| point.time),
            Some(100)
        );
        assert_eq!(
            chart.series_bars_in_logical_range(sparse, 3.0, 7.0),
            Some(BarsInLogicalRange {
                bars_before: 3.0,
                bars_after: 3.0,
                from: None,
                to: None,
            })
        );
        assert_eq!(
            chart.series_bars_in_logical_range(sparse, -1.5, 5.25),
            Some(BarsInLogicalRange {
                bars_before: -1.5,
                bars_after: 10.0,
                from: Some(0),
                to: Some(0),
            })
        );
    }

    #[test]
    fn crosshair_geometry_is_host_independent() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart.series[0].kind = SeriesKind::Line;
        chart
            .set_series_data(
                0,
                &[1.0, 2.0, 3.0],
                &[10.0, 11.0, 12.0],
                &[11.0, 12.0, 13.0],
                &[9.0, 10.0, 11.0],
                &[10.5, 11.5, 12.5],
            )
            .unwrap();
        chart.time_scale.set_width(800.0);
        chart.fit_content();
        chart.crosshair = Some((200.0, 120.0));
        let frame = chart.build_frame();
        assert!(frame.panes[0]
            .main
            .iter()
            .any(|p| matches!(p, aion_render::draw_list::Prim::VLine { .. })));
        assert!(frame.panes[0]
            .main
            .iter()
            .any(|p| matches!(p, aion_render::draw_list::Prim::HLine { .. })));
        assert!(frame.panes[0]
            .main
            .iter()
            .any(|p| matches!(p, aion_render::draw_list::Prim::Circle { .. })));

        let mut canvas = CountingCanvas::default();
        for pane in &frame.panes {
            execute(
                &pane.under,
                &pane.points,
                &mut canvas,
                Viewport {
                    width: 800.0,
                    height: 500.0,
                },
            );
            execute(
                &pane.main,
                &pane.points,
                &mut canvas,
                Viewport {
                    width: 800.0,
                    height: 500.0,
                },
            );
        }
        assert!(
            canvas.calls > 0,
            "the shared frame must be executable by a Canvas2D backend"
        );
    }

    #[test]
    fn indicators_are_engine_owned_series() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart
            .set_series_data(
                0,
                &[1.0, 2.0, 3.0, 4.0],
                &[1.0, 2.0, 3.0, 4.0],
                &[1.0, 2.0, 3.0, 4.0],
                &[1.0, 2.0, 3.0, 4.0],
                &[1.0, 2.0, 3.0, 4.0],
            )
            .unwrap();
        let sma = chart.add_sma(0, 2).expect("valid indicator");
        let rows = chart.data.series_data(sma).unwrap();
        assert_eq!(rows.0, &[2, 3, 4]);
        assert_eq!(rows.1[3], &[1.5, 2.5, 3.5]);

        chart.update_series_bar(0, 4.0, [4.0, 5.0, 3.0, 5.0]);
        let rows = chart.data.series_data(sma).unwrap();
        assert_eq!(rows.1[3], &[1.5, 2.5, 4.0]);

        let ema = chart.add_ema(0, 2).expect("valid indicator");
        let initial_ema = chart.data.series_data(ema).unwrap().1[3];
        assert_eq!(initial_ema.len(), 3);
        assert!((initial_ema[2] - 4.166666666666667).abs() < 1e-12);
        chart.update_series_bar(0, 5.0, [5.0, 6.0, 4.0, 6.0]);
        let ema_rows = chart.data.series_data(ema).unwrap();
        assert!((ema_rows.1[3].last().copied().unwrap() - 5.388888888888889).abs() < 1e-12);
    }

    #[test]
    fn bollinger_creates_three_output_series() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart
            .set_series_data(
                0,
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
            )
            .unwrap();
        let ids = chart.add_bollinger(0, 3, 2.0);
        assert_eq!(ids.len(), 3);
        assert!(chart.data.series_data(ids[0]).unwrap().1[3][0] > 3.0);
        assert_eq!(chart.data.series_data(ids[1]).unwrap().1[3], &[2.0]);
    }

    #[test]
    fn retained_frame_reuses_pane_buffers() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart
            .set_series_data(
                0,
                &[1.0, 2.0, 3.0],
                &[1.0, 2.0, 3.0],
                &[2.0, 3.0, 4.0],
                &[0.0, 1.0, 2.0],
                &[1.5, 2.5, 3.5],
            )
            .unwrap();
        chart.time_scale.set_width(800.0);
        chart.fit_content();
        let mut frame = ChartFrame::default();
        chart.build_frame_into(&mut frame);
        let first_capacity = frame.panes[0].main.capacity();
        chart.crosshair = Some((300.0, 100.0));
        chart.build_frame_into(&mut frame);
        assert!(frame.panes[0].main.capacity() >= first_capacity);
    }

    #[test]
    fn axis_frame_owns_label_content_and_positions() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart
            .set_series_data(
                0,
                &[1.0, 2.0],
                &[10.0, 11.0],
                &[11.0, 12.0],
                &[9.0, 10.0],
                &[10.0, 11.0],
            )
            .unwrap();
        chart.time_scale.set_width(760.0);
        chart.fit_content();
        let axes = chart.build_axis_frame(80.0, |text| text.len() as f64);
        assert!(!axes.labels.is_empty());
        assert!(axes.labels.iter().any(|label| label.text.contains("11")));
    }
}
