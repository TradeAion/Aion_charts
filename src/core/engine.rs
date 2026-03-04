//! ChartEngine — the top-level orchestrator that owns all subsystems.
//!
//! Renderer-agnostic: works with any backend that implements the Renderer trait.
//! Owns viewport, data, style, crosshair state, and delegates rendering to
//! the active RendererBackend.
//!
//! With the widget-based architecture, the engine only renders the PANE
//! (chart area). Axis rendering is handled by dedicated axis renderers
//! in the WASM layer.

use crate::core::chart_type::{MainChartOptions, MainChartType};
use crate::core::constants::{
    DEFAULT_BAR_SPACING_CSS, DEFAULT_INITIAL_VISIBLE_BARS, MIN_VISIBLE_BARS,
};
use crate::core::data::{Bar, BarArray};
use crate::core::drawings::types::DrawingGeometry;
use crate::core::drawings::DrawingManager;
use crate::core::events::EventBus;
use crate::core::footprint::{FootprintBar, FootprintData, FootprintDisplayMode, FootprintOptions};
use crate::core::indicators::IndicatorManager;
use crate::core::markers::MarkerManager;
use crate::core::price_line::PriceLineManager;
use crate::core::renderer::draw_list::DrawText;
use crate::core::renderer::traits::{
    ChartStyle, CrosshairState, RenderContext, Renderer, RendererBackend,
};
use crate::core::series::{
    AreaSeriesOptions, BarSeriesOptions, BaselineSeriesOptions, HistogramPoint,
    HistogramSeriesOptions, LinePoint, LineSeriesOptions, OhlcPoint, Series, SeriesCollection,
    SeriesId, SeriesType,
};
use crate::core::studies::manager::{StudyId, StudyManager};
use crate::core::viewport::Viewport;

#[inline]
fn ensure_strictly_increasing_timestamps(name: &str, timestamps: &[u64]) -> Result<(), String> {
    for i in 1..timestamps.len() {
        if timestamps[i] <= timestamps[i - 1] {
            return Err(format!(
                "{} timestamps must be strictly increasing at index {}: {} <= {}",
                name,
                i,
                timestamps[i],
                timestamps[i - 1]
            ));
        }
    }
    Ok(())
}

#[inline]
fn ensure_strictly_increasing_bar_timestamps(bars: &[Bar]) -> Result<(), String> {
    for i in 1..bars.len() {
        if bars[i].timestamp <= bars[i - 1].timestamp {
            return Err(format!(
                "bars timestamps must be strictly increasing at index {}: {} <= {}",
                i,
                bars[i].timestamp,
                bars[i - 1].timestamp
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpsertAction {
    Append,
    UpdateLast,
}

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
    pub indicators: IndicatorManager,
    pub dpr: f64,
    /// Horizontal pixel ratio: exact `bitmapWidth / cssWidth`.
    /// Set from `device-pixel-content-box` ResizeObserver; falls back to `dpr`.
    pub h_pixel_ratio: f64,
    /// Vertical pixel ratio: exact `bitmapHeight / cssHeight`.
    pub v_pixel_ratio: f64,
    /// Main chart type (candlestick, OHLC bars, line, area, footprint, etc.).
    pub main_chart_type: MainChartType,
    /// Options for the main chart rendering.
    pub main_chart_options: MainChartOptions,
    /// Footprint (order-flow) data for the Footprint chart type.
    pub footprint_data: FootprintData,
    /// Footprint text labels from the last render frame.
    /// Populated during draw_candles so the overlay Canvas2D layer can render them
    /// (WebGPU cannot render text natively).
    pub footprint_texts: Vec<DrawText>,
    /// Event bus — collects events for the platform layer to drain and forward.
    pub event_bus: EventBus,
}

impl ChartEngine {
    fn series_type_name(series_type: SeriesType) -> &'static str {
        match series_type {
            SeriesType::Candlestick => "candlestick",
            SeriesType::Line => "line",
            SeriesType::Area => "area",
            SeriesType::Histogram => "histogram",
            SeriesType::Bar => "bar",
            SeriesType::Baseline => "baseline",
        }
    }

    fn get_series_mut_checked(
        &mut self,
        id: SeriesId,
        accepted: &[SeriesType],
    ) -> Result<&mut Series, String> {
        let s = self
            .series
            .get_mut(id)
            .ok_or_else(|| format!("series id {} not found", id.0))?;

        let actual = s.series_type();
        if accepted.iter().any(|t| *t == actual) {
            return Ok(s);
        }

        let expected = accepted
            .iter()
            .map(|t| Self::series_type_name(*t))
            .collect::<Vec<_>>()
            .join("|");
        Err(format!(
            "series id {} has type {}, expected {}",
            id.0,
            Self::series_type_name(actual),
            expected
        ))
    }

    #[inline]
    fn validate_append_timestamp(
        op: &str,
        last_ts: Option<u64>,
        incoming_ts: u64,
    ) -> Result<(), String> {
        if let Some(last) = last_ts {
            if incoming_ts <= last {
                return Err(format!(
                    "{} requires timestamp > last timestamp ({} <= {})",
                    op, incoming_ts, last
                ));
            }
        }
        Ok(())
    }

    #[inline]
    fn validate_update_timestamp(
        op: &str,
        last_ts: Option<u64>,
        incoming_ts: u64,
    ) -> Result<(), String> {
        let last = last_ts.ok_or_else(|| format!("{} cannot update an empty series", op))?;
        if incoming_ts != last {
            return Err(format!(
                "{} requires timestamp == last timestamp ({} != {})",
                op, incoming_ts, last
            ));
        }
        Ok(())
    }

    #[inline]
    fn resolve_upsert_action(
        op: &str,
        last_ts: Option<u64>,
        incoming_ts: u64,
    ) -> Result<UpsertAction, String> {
        match last_ts {
            None => Ok(UpsertAction::Append),
            Some(last) if incoming_ts == last => Ok(UpsertAction::UpdateLast),
            Some(last) if incoming_ts > last => Ok(UpsertAction::Append),
            Some(last) => Err(format!(
                "{} requires timestamp >= last timestamp ({} < {})",
                op, incoming_ts, last
            )),
        }
    }

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
        let indicators = IndicatorManager::default();

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
            indicators,
            dpr,
            h_pixel_ratio: dpr,
            v_pixel_ratio: dpr,
            main_chart_type: MainChartType::default(),
            main_chart_options: MainChartOptions::default(),
            footprint_data: FootprintData::new(),
            footprint_texts: Vec::new(),
            event_bus: EventBus::new(),
        }
    }

    /// Which renderer backend is active.
    pub fn renderer_name(&self) -> &str {
        self.renderer.name()
    }

    /// Get the current main chart type.
    pub fn main_chart_type(&self) -> MainChartType {
        self.main_chart_type
    }

    /// Set the main chart type (candlestick, OHLC bars, line, area, footprint, etc.).
    pub fn set_main_chart_type(&mut self, chart_type: MainChartType) {
        self.main_chart_type = chart_type;
        self.main_chart_options.chart_type = chart_type;

        // Auto-generate footprint data from OHLCV bars when switching to Footprint
        // and no footprint data is loaded yet.
        if chart_type == MainChartType::Footprint {
            self.ensure_footprint_data();
        }
    }

    /// Get the main chart options.
    pub fn main_chart_options(&self) -> &MainChartOptions {
        &self.main_chart_options
    }

    /// Set main chart options.
    pub fn set_main_chart_options(&mut self, options: MainChartOptions) {
        self.main_chart_type = options.chart_type;
        self.main_chart_options = options;
        if self.main_chart_type == MainChartType::Footprint {
            self.ensure_footprint_data();
        }
    }

    /// Replace all bar data.
    pub fn set_data(&mut self, bars: Vec<Bar>) -> Result<(), String> {
        ensure_strictly_increasing_bar_timestamps(&bars)?;
        let len = bars.len();
        self.bars.set(bars);

        // Update studies with new data
        self.studies.update_studies(&self.bars);
        self.indicators.on_set_data(&self.bars);

        // Clear stale footprint data — if in footprint mode, re-generate from new bars.
        self.footprint_data.clear();
        if self.main_chart_type == MainChartType::Footprint {
            self.ensure_footprint_data();
        }

        // LWC-like initial zoom: derive visible bars from default bar spacing (6 CSS px).
        // Fallback to legacy constant if dimensions are not ready yet.
        let visible = {
            let by_spacing = if self.viewport.width > 0 && self.h_pixel_ratio > 0.0 {
                self.viewport.width as f64 / (DEFAULT_BAR_SPACING_CSS * self.h_pixel_ratio)
            } else {
                DEFAULT_INITIAL_VISIBLE_BARS
            };
            (len as f64).min(by_spacing.max(MIN_VISIBLE_BARS))
        };
        self.viewport.set_range((len as f64) - visible, len as f64);

        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(&self.bars);
        }
        Ok(())
    }

    /// Recompute user indicator runtime instances against current in-memory bars.
    pub fn recompute_indicators(&mut self) {
        self.indicators.on_set_data(&self.bars);
    }

    /// Resize the pane canvas / surface.
    pub fn resize(&mut self, width: u32, height: u32, dpr: f64) {
        let width = width.max(1);
        let height = height.max(1);
        if self.viewport.width == width
            && self.viewport.height == height
            && (self.dpr - dpr).abs() < 1e-6
        {
            return;
        }

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
    pub fn set_series_data(&mut self, id: SeriesId, data: Vec<LinePoint>) -> Result<(), String> {
        for i in 1..data.len() {
            if data[i].timestamp <= data[i - 1].timestamp {
                return Err(format!(
                    "line timestamps must be strictly increasing at index {}: {} <= {}",
                    i,
                    data[i].timestamp,
                    data[i - 1].timestamp
                ));
            }
        }
        let s = self.get_series_mut_checked(
            id,
            &[SeriesType::Line, SeriesType::Area, SeriesType::Baseline],
        )?;
        s.line_data.set(data);
        Ok(())
    }

    /// Set data points for a histogram series.
    pub fn set_histogram_data(
        &mut self,
        id: SeriesId,
        data: Vec<HistogramPoint>,
    ) -> Result<(), String> {
        for i in 1..data.len() {
            if data[i].timestamp <= data[i - 1].timestamp {
                return Err(format!(
                    "histogram timestamps must be strictly increasing at index {}: {} <= {}",
                    i,
                    data[i].timestamp,
                    data[i - 1].timestamp
                ));
            }
        }
        let s = self.get_series_mut_checked(id, &[SeriesType::Histogram])?;
        s.histogram_data.set_data(data);
        Ok(())
    }

    /// Set histogram data from parallel arrays (no per-bar color).
    pub fn set_histogram_data_arrays(
        &mut self,
        id: SeriesId,
        timestamps: &[u64],
        values: &[f32],
    ) -> Result<(), String> {
        if timestamps.len() != values.len() {
            return Err(format!(
                "histogram arrays length mismatch: timestamps={} values={}",
                timestamps.len(),
                values.len()
            ));
        }
        ensure_strictly_increasing_timestamps("histogram", timestamps)?;
        let s = self.get_series_mut_checked(id, &[SeriesType::Histogram])?;
        s.histogram_data.set_from_arrays(timestamps, values)?;
        Ok(())
    }

    /// Set data points for a bar (OHLC) series.
    pub fn set_bar_data(&mut self, id: SeriesId, data: Vec<OhlcPoint>) -> Result<(), String> {
        for i in 1..data.len() {
            if data[i].timestamp <= data[i - 1].timestamp {
                return Err(format!(
                    "bar timestamps must be strictly increasing at index {}: {} <= {}",
                    i,
                    data[i].timestamp,
                    data[i - 1].timestamp
                ));
            }
        }
        let s = self.get_series_mut_checked(id, &[SeriesType::Bar])?;
        s.bar_data.set_data(data);
        Ok(())
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
    ) -> Result<(), String> {
        let len = timestamps.len();
        if open.len() != len || high.len() != len || low.len() != len || close.len() != len {
            return Err(format!(
                "bar arrays length mismatch: timestamps={} open={} high={} low={} close={}",
                len,
                open.len(),
                high.len(),
                low.len(),
                close.len()
            ));
        }
        ensure_strictly_increasing_timestamps("bar", timestamps)?;
        let s = self.get_series_mut_checked(id, &[SeriesType::Bar])?;
        s.bar_data
            .set_from_arrays(timestamps, open, high, low, close)?;
        Ok(())
    }

    /// Append a point to a line/area/baseline overlay series.
    pub fn append_series_point(&mut self, id: SeriesId, point: LinePoint) -> Result<(), String> {
        let s = self.get_series_mut_checked(
            id,
            &[SeriesType::Line, SeriesType::Area, SeriesType::Baseline],
        )?;
        Self::validate_append_timestamp(
            "append_series_point",
            s.line_data.last_timestamp(),
            point.timestamp,
        )?;
        s.line_data.push(point);
        Ok(())
    }

    /// Update the last point in a line/area/baseline overlay series.
    pub fn update_last_series_point(
        &mut self,
        id: SeriesId,
        point: LinePoint,
    ) -> Result<(), String> {
        let s = self.get_series_mut_checked(
            id,
            &[SeriesType::Line, SeriesType::Area, SeriesType::Baseline],
        )?;
        Self::validate_update_timestamp(
            "update_last_series_point",
            s.line_data.last_timestamp(),
            point.timestamp,
        )?;
        s.line_data.update_last(point);
        Ok(())
    }

    /// LWC-style update semantics for a line/area/baseline series:
    /// update last when timestamp matches, append when newer.
    pub fn upsert_series_point(&mut self, id: SeriesId, point: LinePoint) -> Result<(), String> {
        let last_ts = self
            .get_series_mut_checked(
                id,
                &[SeriesType::Line, SeriesType::Area, SeriesType::Baseline],
            )?
            .line_data
            .last_timestamp();
        match Self::resolve_upsert_action("upsert_series_point", last_ts, point.timestamp)? {
            UpsertAction::Append => self.append_series_point(id, point),
            UpsertAction::UpdateLast => self.update_last_series_point(id, point),
        }
    }

    /// Append a point to a histogram overlay series.
    pub fn append_histogram_point(
        &mut self,
        id: SeriesId,
        point: HistogramPoint,
    ) -> Result<(), String> {
        let s = self.get_series_mut_checked(id, &[SeriesType::Histogram])?;
        Self::validate_append_timestamp(
            "append_histogram_point",
            s.histogram_data.last_timestamp(),
            point.timestamp,
        )?;
        s.histogram_data.push(point);
        Ok(())
    }

    /// Update the last point in a histogram overlay series.
    pub fn update_last_histogram_point(
        &mut self,
        id: SeriesId,
        point: HistogramPoint,
    ) -> Result<(), String> {
        let s = self.get_series_mut_checked(id, &[SeriesType::Histogram])?;
        Self::validate_update_timestamp(
            "update_last_histogram_point",
            s.histogram_data.last_timestamp(),
            point.timestamp,
        )?;
        s.histogram_data.update_last(point);
        Ok(())
    }

    /// LWC-style update semantics for a histogram series:
    /// update last when timestamp matches, append when newer.
    pub fn upsert_histogram_point(
        &mut self,
        id: SeriesId,
        point: HistogramPoint,
    ) -> Result<(), String> {
        let last_ts = self
            .get_series_mut_checked(id, &[SeriesType::Histogram])?
            .histogram_data
            .last_timestamp();
        match Self::resolve_upsert_action("upsert_histogram_point", last_ts, point.timestamp)? {
            UpsertAction::Append => self.append_histogram_point(id, point),
            UpsertAction::UpdateLast => self.update_last_histogram_point(id, point),
        }
    }

    /// Append a point to a bar (OHLC) overlay series.
    pub fn append_bar_series_point(
        &mut self,
        id: SeriesId,
        point: OhlcPoint,
    ) -> Result<(), String> {
        let s = self.get_series_mut_checked(id, &[SeriesType::Bar])?;
        Self::validate_append_timestamp(
            "append_bar_series_point",
            s.bar_data.last_timestamp(),
            point.timestamp,
        )?;
        s.bar_data.push(point);
        Ok(())
    }

    /// Update the last point in a bar (OHLC) overlay series.
    pub fn update_last_bar_series_point(
        &mut self,
        id: SeriesId,
        point: OhlcPoint,
    ) -> Result<(), String> {
        let s = self.get_series_mut_checked(id, &[SeriesType::Bar])?;
        Self::validate_update_timestamp(
            "update_last_bar_series_point",
            s.bar_data.last_timestamp(),
            point.timestamp,
        )?;
        s.bar_data.update_last(point);
        Ok(())
    }

    /// LWC-style update semantics for an OHLC bar overlay series:
    /// update last when timestamp matches, append when newer.
    pub fn upsert_bar_series_point(
        &mut self,
        id: SeriesId,
        point: OhlcPoint,
    ) -> Result<(), String> {
        let last_ts = self
            .get_series_mut_checked(id, &[SeriesType::Bar])?
            .bar_data
            .last_timestamp();
        match Self::resolve_upsert_action("upsert_bar_series_point", last_ts, point.timestamp)? {
            UpsertAction::Append => self.append_bar_series_point(id, point),
            UpsertAction::UpdateLast => self.update_last_bar_series_point(id, point),
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

    // ── Footprint data management ────────────────────────────────────────

    /// Ensure footprint data exists. If empty, auto-generates synthetic data
    /// from the current OHLCV bars so the chart has something to display.
    pub fn ensure_footprint_data(&mut self) {
        if self.footprint_data.is_empty() && !self.bars.is_empty() {
            let tick_size = self.main_chart_options.footprint.tick_size;
            // Collect bars into a Vec for the generator
            let bar_vec: Vec<Bar> = (0..self.bars.len())
                .map(|i| self.bars.get_unchecked(i))
                .collect();
            self.footprint_data =
                crate::core::demo_data::generate_footprint_from_bars(&bar_vec, tick_size);
        }
    }

    /// Set footprint data for a specific bar index.
    /// Levels should be sorted by price ascending.
    pub fn set_footprint_bar(&mut self, bar_idx: usize, bar: FootprintBar) {
        self.footprint_data.set_bar(bar_idx, bar);
    }

    /// Set footprint data for multiple bars at once (bulk load).
    pub fn set_footprint_bars(&mut self, bars: Vec<(usize, FootprintBar)>) {
        self.footprint_data.set_bars(bars);
    }

    /// Clear all footprint data.
    pub fn clear_footprint_data(&mut self) {
        self.footprint_data.clear();
    }

    /// Get footprint options (mutable) for configuration.
    pub fn footprint_options_mut(&mut self) -> &mut FootprintOptions {
        &mut self.main_chart_options.footprint
    }

    /// Set footprint display mode.
    pub fn set_footprint_display_mode(&mut self, mode: FootprintDisplayMode) {
        self.main_chart_options.footprint.display_mode = mode;
    }

    /// Set footprint tick size (price granularity per row).
    /// Pass 0.0 for auto-detection.
    pub fn set_footprint_tick_size(&mut self, tick_size: f32) {
        self.main_chart_options.footprint.tick_size = tick_size;
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

    /// Recalculate all studies. Call this after changing parameters or when
    /// data is updated from the WASM layer (as opposed to via append_bar/update_bar).
    pub fn recalculate_studies(&mut self) {
        self.studies.update_studies(&self.bars);
    }

    /// Auto-fit price axis to visible data if not locked.
    /// Call this after panning/zooming when price_locked is false.
    pub fn auto_fit_price_if_unlocked(&mut self) {
        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(&self.bars);
        }
    }

    /// Get study count.
    pub fn study_count(&self) -> usize {
        self.studies.study_count()
    }

    // ── Real-time data updates ──────────────────────────────────────────

    #[inline]
    fn last_main_timestamp(&self) -> Option<u64> {
        if self.bars.is_empty() {
            None
        } else {
            Some(self.bars.timestamp(self.bars.len() - 1))
        }
    }

    /// Append a single bar to the end of the data array.
    /// Updates studies incrementally and adjusts viewport to keep the latest bar visible.
    pub fn append_bar(&mut self, bar: Bar) -> Result<(), String> {
        Self::validate_append_timestamp("append_bar", self.last_main_timestamp(), bar.timestamp)?;
        self.bars.append(bar);

        // Update studies with new data
        self.studies.update_studies(&self.bars);
        self.indicators.on_incremental_update(&self.bars);

        // LWC-style viewport advance: if auto_scroll is enabled AND the previous
        // last bar was inside the visible range, shift the viewport right by
        // exactly 1 bar so the new bar comes into view at the same position the
        // old last bar occupied.
        // When auto_scroll is disabled the viewport is never touched here —
        // giving the user a fully static view during live streaming.
        // When auto_scroll is enabled but the user has panned away, the new bar
        // accumulates off-screen to the right and the viewport is left untouched.
        let len = self.bars.len() as f64;
        let old_last_bar = len - 2.0; // index of bar that was last before this append
        if self.viewport.auto_scroll && self.viewport.end_bar > old_last_bar {
            self.viewport.start_bar += 1.0;
            self.viewport.end_bar += 1.0;
            // price_invalidated = true tells the render loop to call auto_fit_price
            // for the new visible range; no explicit call needed here.
            self.viewport.price_invalidated = true;
        }

        // Y-drift guard: only refit price when the new bar's high/low exits
        // the current viewport bounds.  Same pattern as update_bar — prevents
        // the price axis from jittering on every new-bar event.
        if !self.viewport.price_locked {
            let h = bar.high as f64;
            let l = bar.low as f64;
            if h > self.viewport.price_max || l < self.viewport.price_min {
                self.viewport.auto_fit_price(&self.bars);
            }
        }
        Ok(())
    }

    /// Update the last bar in the data array (e.g., for real-time tick updates).
    pub fn update_bar(&mut self, bar: Bar) -> Result<(), String> {
        Self::validate_update_timestamp("update_bar", self.last_main_timestamp(), bar.timestamp)?;
        let len = self.bars.len();

        self.bars.update_last(bar);

        // Recalculate studies for the last bar only
        // Reset last_calculated_index to len-1 so only the last bar is recalculated
        for study in self.studies.studies_iter_mut() {
            if study.last_calculated_index > 0 {
                study.last_calculated_index = len - 1;
            }
        }
        self.studies.update_studies(&self.bars);
        self.indicators.on_incremental_update(&self.bars);

        if !self.viewport.price_locked {
            // Only rescale when the live bar's price actually exits the current
            // viewport bounds.  Calling auto_fit_price on every tick was causing
            // the entire price scale to shift every 200 ms (Y-drift) because
            // auto_fit_price scans all visible bars and adjusts price_min/price_max
            // with scale margins.  Since price_max already includes the 20 % top
            // margin and price_min the 10 % bottom margin, this guard fires only
            // when the bar genuinely moves outside the displayed range.
            let h = bar.high as f64;
            let l = bar.low as f64;
            if h > self.viewport.price_max || l < self.viewport.price_min {
                self.viewport.auto_fit_price(&self.bars);
            }
        }
        Ok(())
    }

    /// LWC-style update semantics for the main bar series:
    /// update last when timestamp matches, append when newer.
    pub fn upsert_bar(&mut self, bar: Bar) -> Result<(), String> {
        match Self::resolve_upsert_action("upsert_bar", self.last_main_timestamp(), bar.timestamp)?
        {
            UpsertAction::Append => self.append_bar(bar),
            UpsertAction::UpdateLast => self.update_bar(bar),
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
        bottom_drawings: &[DrawingGeometry],
    ) -> Result<(), String> {
        if self.viewport.price_invalidated && !self.viewport.price_locked {
            self.viewport.auto_fit_price(&self.bars);
            self.viewport.price_invalidated = false;
        }

        // Pre-generate footprint geometry so both backends get identical output
        // and text labels are available for the overlay Canvas2D layer.
        let footprint_rects = if self.main_chart_type == MainChartType::Footprint
            && !self.footprint_data.is_empty()
        {
            let pane_w = self.viewport.width as f64;
            let pane_h = self.viewport.height as f64;
            let geom = crate::core::renderer::footprint_generator::generate_footprint_geometry(
                &self.bars,
                &self.viewport,
                &self.style,
                &self.footprint_data,
                &self.main_chart_options.footprint,
                pane_w,
                pane_h,
                self.h_pixel_ratio,
                self.v_pixel_ratio,
            );
            self.footprint_texts = geom.texts;
            geom.rects
        } else {
            self.footprint_texts.clear();
            Vec::new()
        };

        let indicator_draw_instructions = self.indicators.collect_sorted_draw_instructions();
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
            indicator_draw_instructions: &indicator_draw_instructions,
            main_chart_type: self.main_chart_type,
            main_chart_options: &self.main_chart_options,
            bottom_drawings,
            footprint_data: &self.footprint_data,
            footprint_rects: &footprint_rects,
            footprint_texts: &self.footprint_texts,
        };

        self.renderer.render_frame(&ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_strictly_increasing_bar_timestamps, ensure_strictly_increasing_timestamps,
        ChartEngine, UpsertAction,
    };
    use crate::core::data::Bar;

    fn mk_bar(ts: u64) -> Bar {
        Bar {
            timestamp: ts,
            open: 1.0,
            high: 2.0,
            low: 0.5,
            close: 1.5,
            volume: 1.0,
            _pad: 0.0,
        }
    }

    #[test]
    fn increasing_timestamps_pass() {
        assert!(ensure_strictly_increasing_timestamps("line", &[1, 2, 3, 4]).is_ok());
    }

    #[test]
    fn duplicate_timestamps_fail() {
        assert!(ensure_strictly_increasing_timestamps("line", &[1, 2, 2, 3]).is_err());
    }

    #[test]
    fn descending_timestamps_fail() {
        assert!(ensure_strictly_increasing_timestamps("line", &[1, 3, 2]).is_err());
    }

    #[test]
    fn increasing_bar_timestamps_pass() {
        let bars = vec![mk_bar(1000), mk_bar(2000), mk_bar(3000)];
        assert!(ensure_strictly_increasing_bar_timestamps(&bars).is_ok());
    }

    #[test]
    fn duplicate_bar_timestamps_fail() {
        let bars = vec![mk_bar(1000), mk_bar(2000), mk_bar(2000)];
        assert!(ensure_strictly_increasing_bar_timestamps(&bars).is_err());
    }

    #[test]
    fn upsert_action_for_empty_is_append() {
        let action = ChartEngine::resolve_upsert_action("x", None, 1000).unwrap();
        assert_eq!(action, UpsertAction::Append);
    }

    #[test]
    fn upsert_action_for_equal_timestamp_is_update() {
        let action = ChartEngine::resolve_upsert_action("x", Some(1000), 1000).unwrap();
        assert_eq!(action, UpsertAction::UpdateLast);
    }

    #[test]
    fn upsert_action_for_newer_timestamp_is_append() {
        let action = ChartEngine::resolve_upsert_action("x", Some(1000), 1001).unwrap();
        assert_eq!(action, UpsertAction::Append);
    }

    #[test]
    fn upsert_action_for_older_timestamp_is_error() {
        assert!(ChartEngine::resolve_upsert_action("x", Some(1000), 999).is_err());
    }
}
