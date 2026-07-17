//! Headless Aion chart engine.
//!
//! This crate owns chart state and behavior without depending on WASM, the DOM, WebGPU, or a
//! native windowing system. Hosts provide input and a viewport; rendering backends consume the
//! frame produced from this state. During the architecture recovery, frame construction is being
//! migrated here incrementally from `aion_wasm`.

mod frame;
pub use frame::{AxisFrame, AxisLabel, AxisTextAlign, ChartFrame, FramePane};

use aion_core::format::price_formatter::PriceFormatter;
use aion_core::model::data_layer::{DataLayer, SeriesId};
use aion_core::model::data_validation::{sanitize_ohlc, sanitize_point, ValidationError, ValidationReport};
use aion_core::model::magnet::CrosshairMode;
use aion_core::options::ChartOptionsStore;
use aion_core::scale::price_scale_core::{
    PriceScaleCore, PriceScaleCoreOptions, PriceScaleMargins,
};
use aion_core::scale::time_scale_core::{TimeScaleCore, TimeScaleOptions};
use aion_core::scale::time_tick_marks::TimeTickMarks;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesKind {
    Candlestick,
    Bar,
    Line,
    Area,
    Histogram,
    Baseline,
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
    pub pane_index: usize,
    pub line_type: LineType,
    pub point_markers: bool,
    pub visible: bool,
    pub baseline: Option<f64>,
    pub last_price_animation: bool,
    pub price_lines: Vec<PriceLine>,
    pub markers: Vec<Marker>,
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
            pane_index: 0,
            line_type: LineType::Simple,
            point_markers: false,
            visible: true,
            baseline: None,
            last_price_animation: false,
            price_lines: Vec::new(),
            markers: Vec::new(),
        }
    }
}

const PANE_PAD_TOP: f64 = 0.2;
const PANE_PAD_BOTTOM: f64 = 0.1;
pub const PANE_SEPARATOR: f64 = 1.0;

pub struct Pane {
    pub price_scale: PriceScaleCore,
    pub overlay_scale: PriceScaleCore,
    pub stretch_factor: f64,
    pub overlay_top: f64,
    pub overlay_bottom: f64,
    pub top: f64,
    pub height: f64,
}

impl Pane {
    pub fn new() -> Self {
        let scale = || {
            PriceScaleCore::new(PriceScaleCoreOptions {
                scale_margins: PriceScaleMargins {
                    top: 0.0,
                    bottom: 0.0,
                },
                ..PriceScaleCoreOptions::default()
            })
        };
        Self {
            price_scale: scale(),
            overlay_scale: scale(),
            stretch_factor: 1.0,
            overlay_top: 0.8,
            overlay_bottom: 0.0,
            top: 0.0,
            height: 0.0,
        }
    }

    pub fn layout(&mut self, content_h: f64) {
        self.price_scale.set_height(content_h);
        self.overlay_scale.set_height(content_h);
        let below = (content_h - self.top - self.height).max(0.0);
        self.price_scale.set_internal_margins(
            self.top + PANE_PAD_TOP * self.height,
            below + PANE_PAD_BOTTOM * self.height,
        );
        self.overlay_scale.set_internal_margins(
            self.top + self.overlay_top * self.height,
            below + self.overlay_bottom * self.height,
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
            crosshair_mode: CrosshairMode::Magnet,
            animation_time: 0.0,
            next_price_line_id: 1,
            time_visible: true,
            css_width,
            css_height,
            dpr,
            crosshair: None,
            pane_w: css_width,
            pane_h: css_height,
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

    /// Add a Rust-native simple moving-average producer. The returned line series is owned by the
    /// engine and is recomputed whenever its source series changes.
    pub fn add_sma(&mut self, source: SeriesId, period: usize) -> Option<SeriesId> {
        self.add_indicator(source, IndicatorKind::Sma { period }, 1).into_iter().next()
    }

    /// Add a Rust-native exponential moving-average producer.
    pub fn add_ema(&mut self, source: SeriesId, period: usize) -> Option<SeriesId> {
        self.add_indicator(source, IndicatorKind::Ema { period }, 1).into_iter().next()
    }

    /// Add upper, middle, and lower Bollinger-band line series in that order.
    pub fn add_bollinger(&mut self, source: SeriesId, period: usize, deviation: f64) -> Vec<SeriesId> {
        self.add_indicator(source, IndicatorKind::Bollinger { period, deviation }, 3)
    }

    fn add_indicator(&mut self, source: SeriesId, kind: IndicatorKind, outputs: usize) -> Vec<SeriesId> {
        if source >= self.series.len() || outputs == 0 || matches!(&kind, IndicatorKind::Sma { period: 0 } | IndicatorKind::Ema { period: 0 } | IndicatorKind::Bollinger { period: 0, .. }) {
            return Vec::new();
        }
        let ids = (0..outputs).map(|_| self.add_series(SeriesKind::Line)).collect::<Vec<_>>();
        self.indicators.push(IndicatorBinding { source, kind, outputs: ids.clone(), last_source_len: 0, last_source_time: None });
        self.recompute_indicators();
        ids
    }

    /// Apply one streaming OHLC update after validating its time and values.
    pub fn update_series_bar(&mut self, id: SeriesId, time: f64, values: [f64; 4]) -> bool {
        let Some((time, values)) = sanitize_point(time, values) else { return false };
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

    /// Lay out stacked panes inside the chart content area. This is shared by hosts that need
    /// pane bounds before frame submission (for example, to draw axis separators).
    pub fn layout_panes(&mut self, content_h: f64) {
        let usable = (content_h - PANE_SEPARATOR * self.panes.len().saturating_sub(1) as f64).max(1.0);
        let total: f64 = self.panes.iter().map(|p| p.stretch_factor.max(0.01)).sum();
        let mut top = 0.0;
        let pane_count = self.panes.len();
        for (i, pane) in self.panes.iter_mut().enumerate() {
            pane.top = top;
            pane.height = usable * pane.stretch_factor.max(0.01) / total;
            pane.layout(content_h);
            top += pane.height;
            if i + 1 < pane_count { top += PANE_SEPARATOR; }
        }
    }

    fn sync_time_points(&mut self) {
        let times = self.data.merged_times();
        let appended = times.len() == self.synced_points_len + 1
            && !times.is_empty()
            && times.last().copied() > self.synced_last_time;
        if appended {
            let mut weights = vec![0u8; times.len()];
            aion_core::scale::time_tick_marks::fill_weights_for_points(times, &mut weights, self.synced_points_len);
            self.tick_marks.append_weights(self.synced_points_len, &weights);
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
            let Some((times, values)) = self.data.series_data(binding.source) else { continue };
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
            if self.indicators[index].source != source { continue; }
            let binding = self.indicators[index].clone();
            let Some((times, values)) = self.data.series_data(source) else { continue };
            let times = times.to_vec();
            let close = values[3].to_vec();
            let tail_update = binding.last_source_len > 0
                && binding.last_source_time.map(|last| time >= last).unwrap_or(false)
                && (times.len() == binding.last_source_len || times.len() == binding.last_source_len + 1);
            if !tail_update {
                self.recompute_indicators();
                return;
            }
            match binding.kind {
                IndicatorKind::Sma { period } => {
                    if let Some(value) = rolling_mean(&close, period) {
                        self.data.update(binding.outputs[0], time, [value; 4]);
                    }
                }
                IndicatorKind::Ema { period } => {
                    if let Some(value) = rolling_ema_tail(&close, period, &self.data, binding.outputs[0], times.len() == binding.last_source_len + 1) {
                        self.data.update(binding.outputs[0], time, [value; 4]);
                    }
                }
                IndicatorKind::Bollinger { period, deviation } => {
                    if let Some((upper, middle, lower)) = rolling_bollinger(&close, period, deviation) {
                        self.data.update(binding.outputs[0], time, [upper; 4]);
                        self.data.update(binding.outputs[1], time, [middle; 4]);
                        self.data.update(binding.outputs[2], time, [lower; 4]);
                    }
                }
            }
            self.indicators[index].last_source_len = times.len();
            self.indicators[index].last_source_time = Some(time);
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
    (period > 0 && values.len() >= period).then(|| values[values.len() - period..].iter().sum::<f64>() / period as f64)
}

fn rolling_bollinger(values: &[f64], period: usize, deviation: f64) -> Option<(f64, f64, f64)> {
    let window = (period > 0 && values.len() >= period).then(|| &values[values.len() - period..])?;
    let middle = window.iter().sum::<f64>() / period as f64;
    let spread = (window.iter().map(|v| (v - middle).powi(2)).sum::<f64>() / period as f64).sqrt() * deviation.max(0.0);
    Some((middle + spread, middle, middle - spread))
}

fn rolling_ema_tail(values: &[f64], period: usize, data: &DataLayer, output: SeriesId, appended: bool) -> Option<f64> {
    if period == 0 || values.len() < period { return None; }
    if values.len() == period { return rolling_mean(values, period); }
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
        fn set_fill_solid(&mut self, _color: Color) { self.calls += 1; }
        fn set_fill_vgradient(&mut self, _y_top: f32, _y_bottom: f32, _top: Color, _bottom: Color) { self.calls += 1; }
        fn set_stroke(&mut self, _color: Color) { self.calls += 1; }
        fn set_line_width(&mut self, _width: f32) { self.calls += 1; }
        fn set_line_dash(&mut self, _pattern: &[f32]) { self.calls += 1; }
        fn fill_rect(&mut self, _x: f32, _y: f32, _w: f32, _h: f32) { self.calls += 1; }
        fn begin_path(&mut self) { self.calls += 1; }
        fn move_to(&mut self, _x: f32, _y: f32) { self.calls += 1; }
        fn line_to(&mut self, _x: f32, _y: f32) { self.calls += 1; }
        fn close_path(&mut self) { self.calls += 1; }
        fn arc(&mut self, _cx: f32, _cy: f32, _r: f32, _start: f32, _end: f32) { self.calls += 1; }
        fn stroke(&mut self) { self.calls += 1; }
        fn fill(&mut self) { self.calls += 1; }
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
        assert!(frame.panes[0].main.iter().any(|p| matches!(p, aion_render::draw_list::Prim::VLine { .. })));
        assert!(frame.panes[0].main.iter().any(|p| matches!(p, aion_render::draw_list::Prim::HLine { .. })));
        assert!(frame.panes[0].main.iter().any(|p| matches!(p, aion_render::draw_list::Prim::Circle { .. })));

        let mut canvas = CountingCanvas::default();
        for pane in &frame.panes {
            execute(&pane.under, &pane.points, &mut canvas, Viewport { width: 800.0, height: 500.0 });
            execute(&pane.main, &pane.points, &mut canvas, Viewport { width: 800.0, height: 500.0 });
        }
        assert!(canvas.calls > 0, "the shared frame must be executable by a Canvas2D backend");
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
            .set_series_data(0, &[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0])
            .unwrap();
        let ids = chart.add_bollinger(0, 3, 2.0);
        assert_eq!(ids.len(), 3);
        assert!(chart.data.series_data(ids[0]).unwrap().1[3][0] > 3.0);
        assert_eq!(chart.data.series_data(ids[1]).unwrap().1[3], &[2.0]);
    }

    #[test]
    fn retained_frame_reuses_pane_buffers() {
        let mut chart = ChartEngine::new(800.0, 500.0, 1.0);
        chart.set_series_data(0, &[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0], &[2.0, 3.0, 4.0], &[0.0, 1.0, 2.0], &[1.5, 2.5, 3.5]).unwrap();
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
        chart.set_series_data(0, &[1.0, 2.0], &[10.0, 11.0], &[11.0, 12.0], &[9.0, 10.0], &[10.0, 11.0]).unwrap();
        chart.time_scale.set_width(760.0);
        chart.fit_content();
        let axes = chart.build_axis_frame(80.0, |text| text.len() as f64);
        assert!(!axes.labels.is_empty());
        assert!(axes.labels.iter().any(|label| label.text.contains("11")));
    }
}
