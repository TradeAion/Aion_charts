//! Headless Aion chart engine.
//!
//! This crate owns chart state and behavior without depending on WASM, the DOM, WebGPU, or a
//! native windowing system. Hosts provide input and a viewport; rendering backends consume the
//! frame produced from this state. During the architecture recovery, frame construction is being
//! migrated here incrementally from `aion_wasm`.

mod frame;
pub use frame::{ChartFrame, FramePane};

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
        }
    }

    /// Add a series to the headless chart. The returned id is stable for the instance lifetime.
    pub fn add_series(&mut self, kind: SeriesKind) -> SeriesId {
        let id = self.data.add_series();
        self.series.push(SeriesEntry::new(id, kind));
        id
    }

    /// Apply one streaming OHLC update after validating its time and values.
    pub fn update_series_bar(&mut self, id: SeriesId, time: f64, values: [f64; 4]) -> bool {
        let Some((time, values)) = sanitize_point(time, values) else { return false };
        self.data.update(id, time, values);
        self.sync_time_points();
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
        let mut weights = vec![0u8; times.len()];
        aion_core::scale::time_tick_marks::fill_weights_for_points(times, &mut weights, 0);
        self.tick_marks.set_weights(&weights);
        self.time_scale.set_points_len(times.len());
        self.time_scale.set_base_index(self.data.base_index());
    }
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
}
