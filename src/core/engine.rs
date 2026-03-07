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
    DEFAULT_BAR_SPACING_CSS, DEFAULT_INITIAL_VISIBLE_BARS, DEGENERATE_PRICE_RANGE_FALLBACK,
    MIN_FOOTPRINT_BAR_CSS, MIN_VISIBLE_BARS,
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

/// Preset strategies for restoring the main chart viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainViewportPreset {
    /// Restore the default recent-bars window with a small right-side gap.
    DefaultRecent,
    /// Fit the full dataset with a small right-side gap.
    FitAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FootprintTimeRangeAnchor {
    LatestWithGap,
    CenterPreserving,
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
    /// Saved volume_height_ratio from before entering Footprint mode.
    /// Footprint sets the ratio to 0 so the candle area fills the full pane,
    /// and ALL coordinate projections (candle, live line, price label, asset
    /// chip) go through the standard `candle_area_height_ph()` path with no
    /// special-casing.  Restored when leaving Footprint mode.
    saved_volume_height_ratio: Option<f32>,
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
            saved_volume_height_ratio: None,
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

    fn default_recent_visible_span(&self) -> f64 {
        let data_len = self.bars.len() as f64;
        if data_len <= 0.0 {
            return DEFAULT_INITIAL_VISIBLE_BARS.max(MIN_VISIBLE_BARS);
        }
        let by_spacing = if self.viewport.width > 0 && self.h_pixel_ratio > 0.0 {
            self.viewport.width as f64 / (DEFAULT_BAR_SPACING_CSS * self.h_pixel_ratio)
        } else {
            DEFAULT_INITIAL_VISIBLE_BARS
        };
        data_len.min(by_spacing.max(MIN_VISIBLE_BARS))
    }

    fn footprint_max_visible_bars(&self) -> Option<f64> {
        let pane_css_w = self.viewport.width as f64 / self.h_pixel_ratio.max(1.0);
        if !pane_css_w.is_finite() || pane_css_w <= 0.0 {
            return None;
        }
        let max_bars = (pane_css_w / MIN_FOOTPRINT_BAR_CSS).floor().max(1.0);
        if max_bars.is_finite() && max_bars > 0.0 {
            Some(max_bars)
        } else {
            None
        }
    }

    fn clamp_footprint_visible_span(&self, requested_span: f64) -> f64 {
        let span = if requested_span.is_finite() && requested_span > 0.0 {
            requested_span.max(1.0)
        } else {
            1.0
        };
        self.footprint_max_visible_bars()
            .map_or(span, |max_bars| span.min(max_bars))
    }

    fn default_recent_right_gap(span: f64) -> f64 {
        if !span.is_finite() || span <= 1.0 {
            return 0.0;
        }
        (span * 0.03).clamp(2.0, 6.0).min((span - 1.0).max(0.0))
    }

    fn current_time_range_overlaps_loaded_data(&self) -> bool {
        if self.bars.is_empty() {
            return false;
        }
        let data_len = self.bars.len() as f64;
        self.viewport.end_bar > 0.0 && self.viewport.start_bar < data_len
    }

    fn footprint_time_range_anchor(&self) -> FootprintTimeRangeAnchor {
        if !self.current_time_range_overlaps_loaded_data() {
            return FootprintTimeRangeAnchor::LatestWithGap;
        }
        let data_len = self.bars.len() as f64;
        if self.viewport.end_bar >= data_len - 1.0 && self.viewport.start_bar > 0.0 {
            FootprintTimeRangeAnchor::LatestWithGap
        } else {
            FootprintTimeRangeAnchor::CenterPreserving
        }
    }

    fn apply_footprint_time_range(
        &mut self,
        requested_span: f64,
        anchor: FootprintTimeRangeAnchor,
    ) {
        let data_len = self.bars.len() as f64;
        if data_len <= 0.0 {
            self.viewport.price_invalidated = true;
            return;
        }

        let span = self.clamp_footprint_visible_span(requested_span);
        match anchor {
            FootprintTimeRangeAnchor::LatestWithGap => {
                let gap = Self::default_recent_right_gap(span);
                let end = data_len + gap;
                self.viewport.set_range(end - span, end);
            }
            FootprintTimeRangeAnchor::CenterPreserving => {
                let mid = (self.viewport.start_bar + self.viewport.end_bar) * 0.5;
                let half = span * 0.5;
                self.viewport.set_range(mid - half, mid + half);
            }
        }
    }

    fn normalize_footprint_time_range(&mut self, anchor: FootprintTimeRangeAnchor) {
        self.apply_footprint_time_range(self.viewport.visible_bar_count(), anchor);
    }

    /// Reset the main chart viewport to a preset time range and auto-scaled price view.
    pub fn reset_main_viewport(&mut self, preset: MainViewportPreset) {
        let data_len = self.bars.len() as f64;
        self.viewport.price_locked = false;

        if data_len <= 0.0 {
            self.viewport.price_invalidated = true;
            return;
        }

        match preset {
            MainViewportPreset::DefaultRecent => {
                let span = self.default_recent_visible_span();
                if self.main_chart_type == MainChartType::Footprint {
                    self.apply_footprint_time_range(
                        span,
                        FootprintTimeRangeAnchor::LatestWithGap,
                    );
                } else {
                    let gap = Self::default_recent_right_gap(span);
                    let visible_data = (span - gap).max(1.0).min(data_len);
                    let start = data_len - visible_data;
                    self.viewport.set_range(start, start + span);
                }
            }
            MainViewportPreset::FitAll => {
                let gap = (data_len * 0.05).max(2.0);
                self.viewport.set_range(0.0, data_len + gap);
            }
        }

        self.auto_fit_price_for_reset();
    }

    /// Set the main chart type (candlestick, OHLC bars, line, area, footprint, etc.).
    pub fn set_main_chart_type(&mut self, chart_type: MainChartType) {
        let prev_chart_type = self.main_chart_type;
        self.main_chart_type = chart_type;
        self.main_chart_options.chart_type = chart_type;

        // ── Volume height ratio management ──
        // Footprint integrates volume into the ladder cells and needs the full
        // pane height for candles.  Instead of special-casing every Y-projection
        // call site, we set volume_height_ratio = 0 so the standard
        // `candle_area_height_ph()` path returns `pane_h` naturally.
        // The original ratio is saved and restored when leaving Footprint mode.
        if chart_type == MainChartType::Footprint && prev_chart_type != MainChartType::Footprint {
            self.saved_volume_height_ratio = Some(self.viewport.volume_height_ratio);
            self.viewport.volume_height_ratio = 0.0;
        } else if chart_type != MainChartType::Footprint
            && prev_chart_type == MainChartType::Footprint
        {
            if let Some(saved) = self.saved_volume_height_ratio.take() {
                self.viewport.volume_height_ratio = saved;
            }
        }

        if prev_chart_type != chart_type && !self.bars.is_empty() {
            if chart_type == MainChartType::Footprint && prev_chart_type != MainChartType::Footprint
            {
                if !self.current_time_range_overlaps_loaded_data() {
                    self.reset_main_viewport(MainViewportPreset::DefaultRecent);
                } else {
                    let anchor = self.footprint_time_range_anchor();
                    let current_span = self.viewport.visible_bar_count();
                    let needs_clamp = self
                        .footprint_max_visible_bars()
                        .is_some_and(|max_bars| current_span > max_bars + 1e-9);
                    if anchor == FootprintTimeRangeAnchor::LatestWithGap {
                        self.normalize_footprint_time_range(anchor);
                    } else if needs_clamp {
                        self.normalize_footprint_time_range(anchor);
                    }

                    if !self.viewport.price_locked {
                        self.auto_fit_price_for_current_chart();
                    } else {
                        self.viewport.price_invalidated = true;
                    }
                }
            } else if !self.current_time_range_overlaps_loaded_data() {
                self.reset_main_viewport(MainViewportPreset::DefaultRecent);
            } else if !self.viewport.price_locked {
                self.auto_fit_price_for_current_chart();
            } else {
                self.viewport.price_invalidated = true;
            }
        } else {
            self.viewport.price_invalidated = true;
        }
    }

    /// Get the main chart options.
    pub fn main_chart_options(&self) -> &MainChartOptions {
        &self.main_chart_options
    }

    /// Set main chart options.
    pub fn set_main_chart_options(&mut self, options: MainChartOptions) {
        self.main_chart_options = options;
        self.set_main_chart_type(self.main_chart_options.chart_type);
    }

    /// Stamp timestamps on all drawing anchor points from the current bar data.
    /// Call after any drawing creation, modification, or drag ends.
    pub fn stamp_drawing_timestamps(&mut self) {
        self.drawings.stamp_timestamps(&self.bars);
    }

    fn replace_main_bars(&mut self, bars: Vec<Bar>) -> Result<(), String> {
        ensure_strictly_increasing_bar_timestamps(&bars)?;

        // Stamp timestamps on existing drawings from the OLD bar data so they
        // can be remapped after the data swap.
        self.drawings.stamp_timestamps(&self.bars);

        self.bars.set(bars);

        // Remap drawing positions to the new bar data using stored timestamps.
        self.drawings.remap_to_new_data(&self.bars);

        // Update studies with new data
        self.studies.update_studies(&self.bars);
        self.indicators.on_set_data(&self.bars);
        Ok(())
    }

    /// Replace all bar data.
    pub fn set_data(&mut self, bars: Vec<Bar>) -> Result<(), String> {
        self.replace_main_bars(bars)?;

        // Clear stale footprint data when main OHLCV data is replaced.
        self.footprint_data.clear();

        self.reset_main_viewport(MainViewportPreset::DefaultRecent);
        Ok(())
    }

    /// Replace all bar data and footprint data in one operation.
    ///
    /// Footprint levels are expected to be indexed against the provided `bars`.
    pub fn set_data_with_footprint(
        &mut self,
        bars: Vec<Bar>,
        footprint: FootprintData,
    ) -> Result<(), String> {
        self.replace_main_bars(bars)?;
        self.footprint_data = footprint;
        self.reset_main_viewport(MainViewportPreset::DefaultRecent);
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
            self.auto_fit_price_for_current_chart();
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

    /// Set footprint data for a specific bar index.
    /// Levels should be sorted by price ascending.
    pub fn set_footprint_bar(&mut self, bar_idx: usize, bar: FootprintBar) {
        self.footprint_data.set_bar(bar_idx, bar);
        if self.main_chart_type == MainChartType::Footprint && !self.viewport.price_locked {
            self.auto_fit_price_for_current_chart();
        }
    }

    /// Set footprint data for multiple bars at once (bulk load).
    pub fn set_footprint_bars(&mut self, bars: Vec<(usize, FootprintBar)>) {
        self.footprint_data.set_bars(bars);
        if self.main_chart_type == MainChartType::Footprint && !self.viewport.price_locked {
            self.auto_fit_price_for_current_chart();
        }
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

    /// Enable/disable coupled time+price zoom behavior for footprint pane wheel/pinch.
    pub fn set_footprint_zoom_price_with_time(&mut self, enabled: bool) {
        self.main_chart_options.footprint.zoom_price_with_time = enabled;
    }

    /// Returns whether footprint pane wheel/pinch uses coupled time+price zoom.
    pub fn footprint_zoom_price_with_time(&self) -> bool {
        self.main_chart_options.footprint.zoom_price_with_time
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

    fn auto_fit_price_for_reset(&mut self) {
        self.auto_fit_price_for_current_chart_impl(true);
    }

    fn auto_fit_price_for_current_chart(&mut self) {
        self.auto_fit_price_for_current_chart_impl(false);
    }

    fn auto_fit_price_for_current_chart_impl(&mut self, apply_soft_default_compactness: bool) {
        let fitted_footprint = if self.main_chart_type == MainChartType::Footprint
            && !self.footprint_data.is_empty()
        {
            self.fit_price_to_visible_footprint_bounds(apply_soft_default_compactness)
        } else {
            false
        };
        if !fitted_footprint {
            self.viewport.auto_fit_price(&self.bars);
        }
        self.enforce_footprint_min_cell_height();
    }

    fn visible_footprint_internal_bounds(&self) -> Option<(f64, f64)> {
        if self.main_chart_type != MainChartType::Footprint
            || self.footprint_data.is_empty()
            || self.bars.is_empty()
        {
            return None;
        }
        let start = (self.viewport.start_bar.floor() as usize)
            .saturating_sub(1)
            .min(self.bars.len());
        let end = ((self.viewport.end_bar.ceil() as usize) + 1).min(self.bars.len());
        if start >= end {
            return None;
        }

        let fixed_tick = (self.main_chart_options.footprint.tick_size > 0.0)
            .then_some(self.main_chart_options.footprint.tick_size as f64);
        let mut min_internal = f64::INFINITY;
        let mut max_internal = f64::NEG_INFINITY;
        let mut any = false;

        for i in start..end {
            let Some(fp_bar) = self.footprint_data.get_bar(i) else {
                continue;
            };
            if fp_bar.levels.is_empty() {
                continue;
            }
            let tick = fixed_tick.unwrap_or_else(|| fp_bar.inferred_tick_size() as f64);
            if !tick.is_finite() || tick <= 0.0 {
                continue;
            }
            for level in &fp_bar.levels {
                let lo = self.viewport.price_to_internal(level.price as f64);
                let hi = self.viewport.price_to_internal(level.price as f64 + tick);
                if !lo.is_finite() || !hi.is_finite() {
                    continue;
                }
                let lvl_min = lo.min(hi);
                let lvl_max = lo.max(hi);
                min_internal = min_internal.min(lvl_min);
                max_internal = max_internal.max(lvl_max);
                any = true;
            }
        }

        any.then_some((min_internal, max_internal))
    }

    fn footprint_required_range_with_margins(&self) -> Option<f64> {
        let (data_min, data_max) = self.visible_footprint_internal_bounds()?;
        let internal_frac =
            1.0 - self.viewport.scale_margin_top - self.viewport.scale_margin_bottom;
        if internal_frac <= 0.0 {
            return None;
        }
        let raw_range = data_max - data_min;
        Some(if raw_range > 0.0 {
            raw_range / internal_frac
        } else {
            DEGENERATE_PRICE_RANGE_FALLBACK / internal_frac
        })
    }

    fn fit_price_to_visible_footprint_bounds(
        &mut self,
        apply_soft_default_compactness: bool,
    ) -> bool {
        // Prime scale-mode conversion state (first visible value/log formula)
        // before projecting footprint prices into internal coordinates.
        self.viewport.auto_fit_price(&self.bars);
        let (data_min, data_max) = match self.visible_footprint_internal_bounds() {
            Some(v) => v,
            None => return false,
        };
        let internal_frac =
            1.0 - self.viewport.scale_margin_top - self.viewport.scale_margin_bottom;
        if internal_frac <= 0.0 {
            return false;
        }
        let raw_range = data_max - data_min;
        let raw_full_range = if raw_range > 0.0 {
            raw_range / internal_frac
        } else {
            DEGENERATE_PRICE_RANGE_FALLBACK / internal_frac
        };
        let full_range = if apply_soft_default_compactness {
            self.footprint_soft_default_full_range(raw_full_range)
        } else {
            raw_full_range
        };
        let required_min = data_min - full_range * self.viewport.scale_margin_bottom;
        let required_max = required_min + full_range;
        if !required_min.is_finite() || !required_max.is_finite() || required_max <= required_min {
            return false;
        }
        self.viewport.price_min = required_min;
        self.viewport.price_max = required_max;
        true
    }

    fn representative_visible_footprint_tick_size(&self) -> Option<f64> {
        if self.main_chart_options.footprint.tick_size > 0.0 {
            return Some(self.main_chart_options.footprint.tick_size as f64);
        }
        if self.footprint_data.is_empty() || self.bars.is_empty() {
            return None;
        }
        let start = (self.viewport.start_bar.floor() as usize)
            .saturating_sub(1)
            .min(self.bars.len());
        let end = ((self.viewport.end_bar.ceil() as usize) + 1).min(self.bars.len());
        if start >= end {
            return None;
        }
        let mut ticks = Vec::with_capacity(end - start);
        for i in start..end {
            if let Some(fp_bar) = self.footprint_data.get_bar(i) {
                let tick = fp_bar.inferred_tick_size() as f64;
                if tick.is_finite() && tick > 0.0 {
                    ticks.push(tick);
                }
            }
        }
        if ticks.is_empty() {
            return None;
        }
        ticks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = ticks.len() / 2;
        Some(if ticks.len() % 2 == 0 {
            (ticks[mid - 1] + ticks[mid]) * 0.5
        } else {
            ticks[mid]
        })
    }

    fn representative_visible_footprint_tick_internal(&self) -> Option<f64> {
        let tick_size = self.representative_visible_footprint_tick_size()?;
        let (data_min, data_max) = self.visible_footprint_internal_bounds()?;
        let mid_internal = (data_min + data_max) * 0.5;
        if !mid_internal.is_finite() {
            return None;
        }
        let mid_price = self.viewport.internal_to_price(mid_internal);
        let up_price = mid_price + tick_size;
        if !mid_price.is_finite() || !up_price.is_finite() {
            return None;
        }
        let tick_internal = (self.viewport.price_to_internal(up_price)
            - self.viewport.price_to_internal(mid_price))
        .abs();
        if tick_internal.is_finite() && tick_internal > 0.0 {
            Some(tick_internal)
        } else {
            None
        }
    }

    /// Target full-range that keeps default footprint rows below a max cell height.
    fn footprint_target_full_range_for_max_cell_height(
        &self,
        target_cell_css: f64,
    ) -> Option<f64> {
        let pane_h = self.viewport.height as f64;
        if pane_h <= 1.0 {
            return None;
        }
        let tick_internal = self.representative_visible_footprint_tick_internal()?;
        let target_cell_ph = target_cell_css * self.v_pixel_ratio.max(1.0);
        if !target_cell_ph.is_finite() || target_cell_ph <= 0.0 {
            return None;
        }
        let range = tick_internal * pane_h / target_cell_ph;
        if range.is_finite() && range > 0.0 {
            Some(range)
        } else {
            None
        }
    }

    fn footprint_soft_default_full_range(&self, raw_full_range: f64) -> f64 {
        const TARGET_MAX_DEFAULT_CELL_CSS: f64 = 12.0;
        const MAX_DEFAULT_RANGE_EXPANSION: f64 = 1.5;

        if !raw_full_range.is_finite() || raw_full_range <= 0.0 {
            return raw_full_range;
        }

        let Some(compact_target_range) =
            self.footprint_target_full_range_for_max_cell_height(TARGET_MAX_DEFAULT_CELL_CSS)
        else {
            return raw_full_range;
        };
        if !compact_target_range.is_finite() || compact_target_range <= raw_full_range {
            return raw_full_range;
        }

        compact_target_range.min(raw_full_range * MAX_DEFAULT_RANGE_EXPANSION)
    }

    /// Enforce a minimum cell height for footprint mode.
    ///
    /// Clamps the viewport's price range so each footprint row occupies at
    /// least `min_cell_height` CSS pixels.  This runs **every frame** —
    /// regardless of whether the price axis is locked — so the user can
    /// never zoom out past the point where cells become unreadable.
    fn enforce_footprint_min_cell_height(&mut self) {
        if self.main_chart_type != MainChartType::Footprint {
            return;
        }
        let pane_h = self.viewport.height as f64;
        if pane_h <= 1.0 {
            return;
        }
        let tick_size = match self.representative_visible_footprint_tick_size() {
            Some(t) => t,
            None => return,
        };

        let current_range = self.viewport.price_max - self.viewport.price_min;
        if !current_range.is_finite() || current_range <= 0.0 {
            return;
        }
        let mid_internal = (self.viewport.price_min + self.viewport.price_max) * 0.5;
        if !mid_internal.is_finite() {
            return;
        }
        let mid_price = self.viewport.internal_to_price(mid_internal);
        let up_price = mid_price + tick_size;
        if !mid_price.is_finite() || !up_price.is_finite() {
            return;
        }
        let tick_internal = (self.viewport.price_to_internal(up_price)
            - self.viewport.price_to_internal(mid_price))
        .abs();
        if !tick_internal.is_finite() || tick_internal <= 0.0 {
            return;
        }

        // With dynamic aggregation the renderer will merge N adjacent price
        // levels to keep cells readable.  We only hard-clamp when even extreme
        // aggregation (MAX_AGG_FACTOR ticks merged) would produce sub-pixel
        // cells — at that point the footprint is truly unusable.
        const MAX_AGG_FACTOR: f64 = 50.0;
        let hard_min_cell_px = 2.0 * self.v_pixel_ratio.max(1.0);
        let max_range_for_hard_clamp =
            tick_internal * MAX_AGG_FACTOR * pane_h / hard_min_cell_px;
        if !max_range_for_hard_clamp.is_finite() || max_range_for_hard_clamp <= 0.0 {
            return;
        }
        let min_safe_range = self.footprint_required_range_with_margins().unwrap_or(0.0);
        let target_range = max_range_for_hard_clamp.max(min_safe_range);
        if current_range > target_range {
            let half = target_range * 0.5;
            self.viewport.price_min = mid_internal - half;
            self.viewport.price_max = mid_internal + half;
        }
    }

    /// Enforce a minimum bar width for footprint mode.
    ///
    /// Prevents zooming out so far that bars become too narrow for footprint
    /// cells to be useful. This is only a safety fallback; reset/switch paths
    /// should already normalize the footprint time range before render.
    fn enforce_footprint_min_bar_width(&mut self) {
        if self.main_chart_type != MainChartType::Footprint {
            return;
        }
        let visible_bars = self.viewport.end_bar - self.viewport.start_bar;
        if visible_bars <= 0.0 {
            return;
        }
        let Some(max_bars) = self.footprint_max_visible_bars() else {
            return;
        };
        if visible_bars <= max_bars + 1e-9 {
            return;
        }
        self.normalize_footprint_time_range(self.footprint_time_range_anchor());
    }

    /// Auto-fit price axis to visible data if not locked.
    /// Call this after panning/zooming when price_locked is false.
    pub fn auto_fit_price_if_unlocked(&mut self) {
        if !self.viewport.price_locked {
            self.auto_fit_price_for_current_chart();
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
                self.auto_fit_price_for_current_chart();
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
                self.auto_fit_price_for_current_chart();
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
    /// `y_ticks` and `x_ticks` are provided by the WASM layer. The current
    /// WASM pipeline passes empty slices to disable axis tick generation.
    pub fn render(
        &mut self,
        y_ticks: &[crate::core::renderer::traits::TickMark],
        x_ticks: &[crate::core::renderer::traits::TickMark],
        bottom_drawings: &[DrawingGeometry],
    ) -> Result<(), String> {
        if self.viewport.price_invalidated && !self.viewport.price_locked {
            self.auto_fit_price_for_current_chart();
            self.viewport.price_invalidated = false;
        }
        self.enforce_footprint_min_cell_height();
        self.enforce_footprint_min_bar_width();

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
        ChartEngine, MainViewportPreset, UpsertAction,
    };
    use crate::core::chart_type::MainChartType;
    use crate::core::data::Bar;
    use crate::core::footprint::{FootprintBar, FootprintData, FootprintLevel};
    use crate::core::renderer::traits::RendererBackend;
    use crate::core::renderer::transforms::price_to_y;

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

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "{label}: actual={actual}, expected={expected}"
        );
    }

    fn assert_default_recent_range(engine: &ChartEngine) {
        let data_len = engine.bars.len() as f64;
        let span = engine.default_recent_visible_span();
        let gap = ChartEngine::default_recent_right_gap(span);
        let visible_data = (span - gap).max(1.0).min(data_len);
        let expected_start = data_len - visible_data;
        let expected_end = expected_start + span;

        assert_close(engine.viewport.start_bar, expected_start, "default recent start");
        assert_close(engine.viewport.end_bar, expected_end, "default recent end");
    }

    fn assert_footprint_latest_range(engine: &ChartEngine, requested_span: f64) {
        let span = engine.clamp_footprint_visible_span(requested_span);
        let gap = ChartEngine::default_recent_right_gap(span);
        let expected_end = engine.bars.len() as f64 + gap;
        let expected_start = expected_end - span;

        assert_close(engine.viewport.start_bar, expected_start, "footprint latest start");
        assert_close(engine.viewport.end_bar, expected_end, "footprint latest end");
    }

    fn assert_footprint_span_is_readable(engine: &ChartEngine) {
        if let Some(max_bars) = engine.footprint_max_visible_bars() {
            assert!(
                engine.viewport.visible_bar_count() <= max_bars + 1e-9,
                "footprint visible span {} should be <= {}",
                engine.viewport.visible_bar_count(),
                max_bars
            );
        }
    }

    fn assert_fit_all_range(engine: &ChartEngine) {
        let data_len = engine.bars.len() as f64;
        let expected_end = data_len + (data_len * 0.05).max(2.0);
        assert_close(engine.viewport.start_bar, 0.0, "fit all start");
        assert_close(engine.viewport.end_bar, expected_end, "fit all end");
    }

    fn visible_footprint_pixel_height(engine: &ChartEngine) -> Option<f64> {
        let (data_min, data_max) = engine.visible_footprint_internal_bounds()?;
        let pane_h = engine.viewport.height as f64;
        let top_price = engine.viewport.internal_to_price(data_max);
        let bottom_price = engine.viewport.internal_to_price(data_min);
        let y_top = price_to_y(top_price, &engine.viewport, pane_h);
        let y_bottom = price_to_y(bottom_price, &engine.viewport, pane_h);
        Some((y_bottom - y_top).abs())
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

    #[test]
    fn footprint_min_cell_height_guard_shrinks_unlocked_price_range() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        engine.set_main_chart_type(MainChartType::Footprint);
        engine.main_chart_options.footprint.tick_size = 0.25;
        engine.main_chart_options.footprint.min_cell_height = 20.0;

        let bars: Vec<Bar> = (0..50)
            .map(|i| Bar {
                timestamp: 1_000 + i,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 100.0,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();

        let mut fp = FootprintData::new();
        for i in 0..50usize {
            let mut levels = Vec::new();
            for j in 0..8usize {
                levels.push(FootprintLevel {
                    price: 99.0 + (j as f32) * 0.25,
                    bid_volume: 10.0,
                    ask_volume: 10.0,
                });
            }
            fp.set_bar(i, FootprintBar { levels });
        }

        engine.set_data_with_footprint(bars, fp).unwrap();

        // With dynamic aggregation the hard clamp now allows up to
        // MAX_AGG_FACTOR (50) ticks merged, only clamping when even that
        // would produce sub-pixel cells (2px hard min).  Verify the range
        // stays within that outer bound.
        let range = engine.viewport.price_max - engine.viewport.price_min;
        let max_agg_factor = 50.0;
        let hard_min_cell_px = 2.0; // matches engine constant
        let max_allowed_range = 0.25 * max_agg_factor * (engine.viewport.height as f64) / hard_min_cell_px;
        assert!(
            range <= max_allowed_range + 1e-6,
            "range {range} should be <= {max_allowed_range} (hard outer bound with aggregation)"
        );
    }

    #[test]
    fn footprint_init_fits_visible_ladder_bounds_instead_of_ohlc_extremes() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        engine.main_chart_options.footprint.tick_size = 0.25;
        engine.set_main_chart_type(MainChartType::Footprint);

        let bars: Vec<Bar> = (0..60)
            .map(|i| Bar {
                timestamp: 10_000 + i,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 100.0,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();

        let mut fp = FootprintData::new();
        for i in 0..60usize {
            let levels = (0..8usize)
                .map(|j| FootprintLevel {
                    price: 99.0 + (j as f32) * 0.25,
                    bid_volume: 10.0,
                    ask_volume: 12.0,
                })
                .collect();
            fp.set_bar(i, FootprintBar { levels });
        }

        engine.set_data_with_footprint(bars, fp).unwrap();

        let raw_required = engine.footprint_required_range_with_margins().unwrap();
        let actual_range = engine.viewport.price_max - engine.viewport.price_min;
        let ohlc_full_range =
            (110.0_f64 - 90.0_f64) / (1.0 - engine.viewport.scale_margin_top - engine.viewport.scale_margin_bottom);

        assert!(
            actual_range <= raw_required * 1.5 + 1e-6,
            "footprint init range {actual_range} should stay within soft compactness cap over raw {raw_required}"
        );
        assert!(
            actual_range < ohlc_full_range * 0.5,
            "footprint init should fit ladder bounds, not OHLC extremes (actual={actual_range}, ohlc={ohlc_full_range})"
        );

        let occupied = visible_footprint_pixel_height(&engine).unwrap();
        assert!(
            occupied >= engine.viewport.height as f64 * 0.45,
            "footprint ladder should occupy a meaningful portion of the pane (occupied={occupied})"
        );
    }

    #[test]
    fn footprint_demo_like_init_stays_within_soft_compactness_cap() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 1200, 700, 1.0);
        engine.set_main_chart_type(MainChartType::Footprint);

        let (bars, fp) =
            crate::core::demo_data::generate_footprint_sample_data(600, 1_000, 60_000, 0.0);
        engine.set_data_with_footprint(bars, fp).unwrap();

        let raw_required = engine.footprint_required_range_with_margins().unwrap();
        let actual_range = engine.viewport.price_max - engine.viewport.price_min;
        assert!(
            actual_range <= raw_required * 1.5 + 1e-6,
            "demo-like footprint init range {actual_range} should stay within soft compactness cap over raw {raw_required}"
        );

        let occupied = visible_footprint_pixel_height(&engine).unwrap();
        assert!(
            occupied >= engine.viewport.height as f64 * 0.45,
            "demo-like footprint init should not compress the ladder into the bottom band (occupied={occupied})"
        );

        assert_footprint_latest_range(&engine, engine.default_recent_visible_span());
        assert_footprint_span_is_readable(&engine);

        let before_start = engine.viewport.start_bar;
        let before_end = engine.viewport.end_bar;
        engine.render(&[], &[], &[]).unwrap();
        assert_close(
            engine.viewport.start_bar,
            before_start,
            "footprint init start should remain stable after render",
        );
        assert_close(
            engine.viewport.end_bar,
            before_end,
            "footprint init end should remain stable after render",
        );
    }

    #[test]
    fn set_data_resets_to_default_recent_viewport_with_right_gap() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        let bars: Vec<Bar> = (0..120)
            .map(|i| Bar {
                timestamp: 1_000 + i,
                open: 100.0 + i as f32 * 0.1,
                high: 101.0 + i as f32 * 0.1,
                low: 99.0 + i as f32 * 0.1,
                close: 100.5 + i as f32 * 0.1,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        engine.viewport.price_locked = true;
        engine.set_data(bars).unwrap();

        assert_default_recent_range(&engine);
        let data_len = engine.bars.len() as f64;
        assert!(
            engine.viewport.end_bar > data_len,
            "default recent init should leave a right gap past the latest bar"
        );
        assert!(
            !engine.viewport.price_locked,
            "set_data should unlock stale price locks during viewport reset"
        );
    }

    #[test]
    fn reset_main_viewport_default_recent_reanchors_far_left_range() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        let bars: Vec<Bar> = (0..120)
            .map(|i| Bar {
                timestamp: 2_000 + i,
                open: 200.0 + i as f32 * 0.1,
                high: 201.0 + i as f32 * 0.1,
                low: 199.0 + i as f32 * 0.1,
                close: 200.5 + i as f32 * 0.1,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        engine.set_data(bars).unwrap();

        engine.viewport.set_range(-240.0, -120.0);
        engine.viewport.price_locked = true;

        engine.reset_main_viewport(MainViewportPreset::DefaultRecent);

        assert_default_recent_range(&engine);
        assert!(
            !engine.viewport.price_locked,
            "default recent reset should unlock stale price locks"
        );
    }

    #[test]
    fn reset_main_viewport_default_recent_reanchors_far_right_range() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        let bars: Vec<Bar> = (0..120)
            .map(|i| Bar {
                timestamp: 3_000 + i,
                open: 300.0 + i as f32 * 0.1,
                high: 301.0 + i as f32 * 0.1,
                low: 299.0 + i as f32 * 0.1,
                close: 300.5 + i as f32 * 0.1,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        engine.set_data(bars).unwrap();

        engine.viewport.set_range(10_000.0, 10_120.0);
        engine.viewport.price_locked = true;

        engine.reset_main_viewport(MainViewportPreset::DefaultRecent);

        assert_default_recent_range(&engine);
        assert!(
            !engine.viewport.price_locked,
            "default recent reset should unlock stale price locks"
        );
    }

    #[test]
    fn footprint_reset_main_viewport_default_recent_clamps_to_latest_readable_range() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        engine.set_main_chart_type(MainChartType::Footprint);

        let (bars, fp) =
            crate::core::demo_data::generate_footprint_sample_data(500, 1_000, 60_000, 0.0);
        engine.set_data_with_footprint(bars, fp).unwrap();

        engine.viewport.set_range(0.0, 240.0);
        engine.reset_main_viewport(MainViewportPreset::DefaultRecent);

        assert_footprint_latest_range(&engine, engine.default_recent_visible_span());
        assert_footprint_span_is_readable(&engine);
    }

    #[test]
    fn reset_main_viewport_fit_all_shows_full_dataset_plus_gap() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        let bars: Vec<Bar> = (0..120)
            .map(|i| Bar {
                timestamp: 4_000 + i,
                open: 400.0 + i as f32 * 0.1,
                high: 401.0 + i as f32 * 0.1,
                low: 399.0 + i as f32 * 0.1,
                close: 400.5 + i as f32 * 0.1,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        engine.set_data(bars).unwrap();

        engine.viewport.set_range(10_000.0, 10_120.0);
        engine.viewport.price_locked = true;

        engine.reset_main_viewport(MainViewportPreset::FitAll);

        assert_fit_all_range(&engine);
        assert!(
            !engine.viewport.price_locked,
            "fit-all reset should unlock stale price locks"
        );
    }

    #[test]
    fn switch_chart_type_reanchors_detached_range_to_default_recent() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        let bars: Vec<Bar> = (0..120)
            .map(|i| Bar {
                timestamp: 5_000 + i,
                open: 500.0 + i as f32 * 0.1,
                high: 501.0 + i as f32 * 0.1,
                low: 499.0 + i as f32 * 0.1,
                close: 500.5 + i as f32 * 0.1,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        engine.set_data(bars).unwrap();

        engine.viewport.set_range(10_000.0, 10_120.0);
        engine.viewport.price_locked = true;

        engine.set_main_chart_type(MainChartType::Footprint);

        assert_footprint_latest_range(&engine, engine.default_recent_visible_span());
        assert_footprint_span_is_readable(&engine);
        assert!(
            !engine.viewport.price_locked,
            "chart-type switch reset should unlock stale price locks"
        );
    }

    #[test]
    fn switch_chart_type_reanchors_latest_view_to_readable_footprint_latest_range() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        let (bars, _fp) =
            crate::core::demo_data::generate_footprint_sample_data(500, 1_000, 60_000, 0.0);
        engine.set_data(bars).unwrap();

        let before_span = engine.viewport.visible_bar_count();
        assert!(
            engine
                .footprint_max_visible_bars()
                .is_some_and(|max_bars| before_span > max_bars),
            "test setup requires a candlestick latest view wider than footprint allows"
        );

        engine.set_main_chart_type(MainChartType::Footprint);

        assert_footprint_latest_range(&engine, before_span);
        assert_footprint_span_is_readable(&engine);

        let before_start = engine.viewport.start_bar;
        let before_end = engine.viewport.end_bar;
        engine.render(&[], &[], &[]).unwrap();
        assert_close(
            engine.viewport.start_bar,
            before_start,
            "latest footprint start should remain stable after render",
        );
        assert_close(
            engine.viewport.end_bar,
            before_end,
            "latest footprint end should remain stable after render",
        );
    }

    #[test]
    fn switch_chart_type_preserves_mid_history_time_range_when_already_readable() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        let bars: Vec<Bar> = (0..200)
            .map(|i| Bar {
                timestamp: 1_000 + i,
                open: 100.0,
                high: 102.0,
                low: 99.0,
                close: 101.0,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        engine.set_data(bars).unwrap();

        engine.viewport.set_range(40.0, 60.0);
        let before_start = engine.viewport.start_bar;
        let before_end = engine.viewport.end_bar;
        engine.viewport.price_locked = true;

        engine.set_main_chart_type(MainChartType::Footprint);

        assert!(
            (engine.viewport.start_bar - before_start).abs() < 1e-9,
            "should keep overlapping time range start"
        );
        assert!(
            (engine.viewport.end_bar - before_end).abs() < 1e-9,
            "should keep overlapping time range end"
        );
        assert!(
            engine.viewport.price_locked,
            "overlapping user range should preserve explicit price locks"
        );
    }

    #[test]
    fn switch_chart_type_shrinks_overwide_mid_history_range_around_same_midpoint() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        let bars: Vec<Bar> = (0..200)
            .map(|i| Bar {
                timestamp: 6_000 + i,
                open: 100.0,
                high: 102.0,
                low: 99.0,
                close: 101.0,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        engine.set_data(bars).unwrap();

        engine.viewport.set_range(40.0, 140.0);
        let before_mid = (engine.viewport.start_bar + engine.viewport.end_bar) * 0.5;
        engine.viewport.price_locked = true;

        engine.set_main_chart_type(MainChartType::Footprint);

        let after_mid = (engine.viewport.start_bar + engine.viewport.end_bar) * 0.5;
        assert_close(after_mid, before_mid, "mid-history midpoint should be preserved");
        assert_footprint_span_is_readable(&engine);
        assert!(
            engine.viewport.end_bar < engine.bars.len() as f64 - 1.0,
            "over-wide mid-history switch should not snap to latest bars"
        );
        assert!(
            engine.viewport.price_locked,
            "mid-history readability clamp should preserve explicit price locks"
        );
    }

    #[test]
    fn switch_back_to_footprint_with_existing_data_uses_footprint_price_fit() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        engine.main_chart_options.footprint.tick_size = 0.25;
        engine.set_main_chart_type(MainChartType::Footprint);

        let bars: Vec<Bar> = (0..80)
            .map(|i| Bar {
                timestamp: 20_000 + i,
                open: 100.0,
                high: 110.0,
                low: 90.0,
                close: 100.0,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        let mut fp = FootprintData::new();
        for i in 0..80usize {
            let levels = (0..8usize)
                .map(|j| FootprintLevel {
                    price: 99.0 + (j as f32) * 0.25,
                    bid_volume: 8.0,
                    ask_volume: 11.0,
                })
                .collect();
            fp.set_bar(i, FootprintBar { levels });
        }

        engine.set_data_with_footprint(bars, fp).unwrap();
        let raw_required = engine.footprint_required_range_with_margins().unwrap();

        engine.set_main_chart_type(MainChartType::Candlestick);
        engine.set_main_chart_type(MainChartType::Footprint);

        let actual_range = engine.viewport.price_max - engine.viewport.price_min;
        assert!(
            actual_range <= raw_required * 1.5 + 1e-6,
            "switching back to footprint should use ladder bounds for price fit (actual={actual_range}, raw={raw_required})"
        );

        let occupied = visible_footprint_pixel_height(&engine).unwrap();
        assert!(
            occupied >= engine.viewport.height as f64 * 0.45,
            "switching back to footprint should restore a full-height ladder view (occupied={occupied})"
        );
    }

    #[test]
    fn footprint_autofit_includes_top_tick_extension() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        engine.main_chart_options.footprint.tick_size = 0.5;
        engine.set_main_chart_type(MainChartType::Footprint);

        let bars: Vec<Bar> = (0..20)
            .map(|i| Bar {
                timestamp: 1_000 + i,
                open: 100.0,
                high: 100.0,
                low: 99.0,
                close: 99.5,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect();
        let mut fp = FootprintData::new();
        for i in 0..20usize {
            fp.set_bar(
                i,
                FootprintBar {
                    levels: vec![
                        FootprintLevel {
                            price: 99.0,
                            bid_volume: 10.0,
                            ask_volume: 12.0,
                        },
                        FootprintLevel {
                            price: 100.0,
                            bid_volume: 14.0,
                            ask_volume: 8.0,
                        },
                    ],
                },
            );
        }

        engine.set_data_with_footprint(bars, fp).unwrap();

        // Top cell boundary is level.price + tick => 100.5.
        // It must be inside pane coordinates (not clipped at y <= 0).
        let y_top = price_to_y(100.5, &engine.viewport, engine.viewport.height as f64);
        assert!(
            y_top > 0.0,
            "footprint top extension should not initialize clipped at the pane top"
        );
    }
}
