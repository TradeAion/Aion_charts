//! ChartEngine — the top-level orchestrator that owns all subsystems.
//!
//! Renderer-agnostic: works with any backend that implements the Renderer trait.
//! Owns viewport, data, style, crosshair state, and delegates rendering to
//! the active RendererBackend.
//!
//! With the widget-based architecture, the engine only renders the PANE
//! (chart area). Axis rendering is handled by dedicated axis renderers
//! in the WASM layer.

use crate::core::data::{Bar, BarArray};
use crate::core::drawings::DrawingManager;
use crate::core::markers::MarkerManager;
use crate::core::price_line::{PriceLineId, PriceLineManager, PriceLineOptions};
use crate::core::renderer::traits::{
    ChartStyle, CrosshairState, RenderContext, Renderer, RendererBackend,
};
use crate::core::series::{
    AreaSeriesOptions, BarSeriesOptions, BaselineSeriesOptions, HistogramPoint,
    HistogramSeriesOptions, LinePoint, LineSeriesOptions, OhlcPoint, SeriesCollection, SeriesId,
};
use crate::core::studies::manager::{StudyId, StudyManager};
use crate::core::viewport::Viewport;

/// The main chart engine. Owns everything needed to render the pane.
pub struct ChartEngine {
    pub renderer: RendererBackend,
    pub viewport: Viewport,
    pub bars: BarArray,
    pub style: ChartStyle,
    pub crosshair: CrosshairState,
    pub drawings: DrawingManager,
    pub series: SeriesCollection,
    pub studies: StudyManager,
    pub price_lines: PriceLineManager,
    pub markers: MarkerManager,
    pub dpr: f64,
    /// Horizontal pixel ratio: exact `bitmapWidth / cssWidth`.
    /// Set from `device-pixel-content-box` ResizeObserver; falls back to `dpr`.
    pub h_pixel_ratio: f64,
    /// Vertical pixel ratio: exact `bitmapHeight / cssHeight`.
    pub v_pixel_ratio: f64,
}

impl ChartEngine {
    /// Create a new engine with a given renderer backend.
    /// `width` and `height` are the PANE physical pixel dimensions.
    pub fn new(renderer: RendererBackend, width: u32, height: u32, dpr: f64) -> Self {
        let viewport = Viewport::new(width, height);
        let bars = BarArray::new();
        let style = ChartStyle::default();
        let crosshair = CrosshairState::default();
        let drawings = DrawingManager::new();
        let series = SeriesCollection::new();
        let mut studies = StudyManager::new();
        let price_lines = PriceLineManager::new();
        let markers = MarkerManager::new();

        // Register built-in study calculators
        crate::core::studies::built_in::register_built_in_studies(&mut studies);

        Self {
            renderer,
            viewport,
            bars,
            style,
            crosshair,
            drawings,
            series,
            studies,
            price_lines,
            markers,
            dpr,
            h_pixel_ratio: dpr,
            v_pixel_ratio: dpr,
        }
    }

    /// Which renderer backend is active.
    pub fn renderer_name(&self) -> &str {
        self.renderer.name()
    }

    /// Replace all bar data.
    pub fn set_data(&mut self, bars: Vec<Bar>) {
        let len = bars.len();
        self.bars.set(bars);

        // Update studies with new data
        self.studies.update_studies(&self.bars);

        // Auto-fit viewport to show last N bars
        let visible = (len as f64).min(200.0);
        self.viewport.set_range((len as f64) - visible, len as f64);

        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(&self.bars);
        }
    }

    /// Resize the pane canvas / surface.
    pub fn resize(&mut self, width: u32, height: u32, dpr: f64) {
        self.dpr = dpr;
        self.renderer.resize(width, height, dpr);
        self.viewport.resize(width, height);
    }

    /// Set visible bar range.
    pub fn zoom_to_range(&mut self, start: u64, end: u64) {
        self.viewport.set_range(start as f64, end as f64);
        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(&self.bars);
        }
    }

    // ── Series management ────────────────────────────────────────────────

    /// Add a new line series overlay. Returns its unique ID.
    pub fn add_line_series(&mut self, options: LineSeriesOptions) -> SeriesId {
        self.series.add_line(options)
    }

    /// Add a new area series overlay. Returns its unique ID.
    pub fn add_area_series(&mut self, options: AreaSeriesOptions) -> SeriesId {
        self.series.add_area(options)
    }

    /// Add a new histogram series overlay. Returns its unique ID.
    pub fn add_histogram_series(&mut self, options: HistogramSeriesOptions) -> SeriesId {
        self.series.add_histogram(options)
    }

    /// Add a new bar (OHLC) series overlay. Returns its unique ID.
    pub fn add_bar_series(&mut self, options: BarSeriesOptions) -> SeriesId {
        self.series.add_bar(options)
    }

    /// Add a new baseline series overlay. Returns its unique ID.
    pub fn add_baseline_series(&mut self, options: BaselineSeriesOptions) -> SeriesId {
        self.series.add_baseline(options)
    }

    /// Set data points for a line or area series.
    pub fn set_series_data(&mut self, id: SeriesId, data: Vec<LinePoint>) {
        if let Some(s) = self.series.get_mut(id) {
            s.line_data.set(data);
        }
    }

    /// Set data points for a histogram series.
    pub fn set_histogram_data(&mut self, id: SeriesId, data: Vec<HistogramPoint>) {
        if let Some(s) = self.series.get_mut(id) {
            s.histogram_data.set_data(data);
        }
    }

    /// Set histogram data from parallel arrays (no per-bar color).
    pub fn set_histogram_data_arrays(&mut self, id: SeriesId, timestamps: &[u64], values: &[f32]) {
        if let Some(s) = self.series.get_mut(id) {
            s.histogram_data.set_from_arrays(timestamps, values);
        }
    }

    /// Set data points for a bar (OHLC) series.
    pub fn set_bar_data(&mut self, id: SeriesId, data: Vec<OhlcPoint>) {
        if let Some(s) = self.series.get_mut(id) {
            s.bar_data.set_data(data);
        }
    }

    /// Set bar (OHLC) data from parallel arrays.
    pub fn set_bar_data_arrays(
        &mut self,
        id: SeriesId,
        timestamps: &[u64],
        open: &[f32],
        high: &[f32],
        low: &[f32],
        close: &[f32],
    ) {
        if let Some(s) = self.series.get_mut(id) {
            s.bar_data
                .set_from_arrays(timestamps, open, high, low, close);
        }
    }

    /// Remove a series by ID.
    pub fn remove_series(&mut self, id: SeriesId) -> bool {
        self.series.remove(id)
    }

    /// Set visibility of a series.
    pub fn set_series_visible(&mut self, id: SeriesId, visible: bool) {
        if let Some(s) = self.series.get_mut(id) {
            match s.series_type() {
                crate::core::series::SeriesType::Line => s.line_options.visible = visible,
                crate::core::series::SeriesType::Area => s.area_options.visible = visible,
                crate::core::series::SeriesType::Histogram => s.histogram_options.visible = visible,
                crate::core::series::SeriesType::Bar => s.bar_options.visible = visible,
                crate::core::series::SeriesType::Baseline => s.baseline_options.visible = visible,
                _ => {}
            }
        }
    }

    // ── Study management ────────────────────────────────────────────────

    /// Create a new study instance.
    pub fn create_study(&mut self, study_type: &str) -> Option<StudyId> {
        self.studies.create_study(study_type)
    }

    /// Remove a study by ID.
    pub fn remove_study(&mut self, id: StudyId) -> bool {
        self.studies.remove_study(id)
    }

    /// Set a study parameter.
    pub fn set_study_parameter(&mut self, id: StudyId, key: &str, value: f64) {
        if let Some(study) = self.studies.get_study_mut(id) {
            study.set_parameter(key.to_string(), value);
            // Reset calculation index so it recalculates from scratch
            study.last_calculated_index = 0;
        }
    }

    /// Get study count.
    pub fn study_count(&self) -> usize {
        self.studies.study_count()
    }

    // ── Real-time data updates ──────────────────────────────────────────

    /// Append a single bar to the end of the data array.
    /// Updates studies incrementally and adjusts viewport to keep the latest bar visible.
    pub fn append_bar(&mut self, bar: Bar) {
        self.bars.append(bar);

        // Update studies with new data
        self.studies.update_studies(&self.bars);

        // Scroll viewport to keep latest bar visible (if near the right edge)
        let len = self.bars.len() as f64;
        let visible = self.viewport.end_bar - self.viewport.start_bar;
        if self.viewport.end_bar >= len - visible * 0.1 - 1.0 {
            // User is near the right edge — auto-scroll
            self.viewport.set_range(len - visible, len);
        }

        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(&self.bars);
        }
    }

    /// Update the last bar in the data array (e.g., for real-time tick updates).
    pub fn update_bar(&mut self, bar: Bar) {
        let len = self.bars.len();
        if len == 0 {
            return;
        }

        self.bars.update_last(bar);

        // Recalculate studies for the last bar only
        // Reset last_calculated_index to len-1 so only the last bar is recalculated
        for study in self.studies.studies_iter_mut() {
            if study.last_calculated_index > 0 {
                study.last_calculated_index = len - 1;
            }
        }
        self.studies.update_studies(&self.bars);

        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(&self.bars);
        }
    }

    /// Main render — called once per frame.
    /// Only renders the pane (candles + volume). Axes are rendered separately.
    /// `y_ticks` and `x_ticks` are pre-computed by the WASM layer so
    /// both the grid and axis renderers share the same tick marks.
    pub fn render(
        &mut self,
        y_ticks: &[crate::core::renderer::traits::TickMark],
        x_ticks: &[crate::core::renderer::traits::TickMark],
    ) -> Result<(), String> {
        if self.viewport.price_invalidated && !self.viewport.price_locked {
            self.viewport.auto_fit_price(&self.bars);
            self.viewport.price_invalidated = false;
        }

        let ctx = RenderContext {
            bars: &self.bars,
            viewport: &self.viewport,
            style: &self.style,
            crosshair: &self.crosshair,
            dpr: self.dpr,
            h_pixel_ratio: self.h_pixel_ratio,
            v_pixel_ratio: self.v_pixel_ratio,
            y_ticks,
            x_ticks,
            series: &self.series,
        };

        self.renderer.render_frame(&ctx)
    }
}
