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
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{Renderer, RendererBackend, RenderContext, ChartStyle, CrosshairState};
use crate::core::drawings::DrawingManager;
use crate::core::series::{SeriesId, SeriesCollection, LineSeriesOptions, LinePoint};

/// The main chart engine. Owns everything needed to render the pane.
pub struct ChartEngine {
    pub renderer: RendererBackend,
    pub viewport: Viewport,
    pub bars: BarArray,
    pub style: ChartStyle,
    pub crosshair: CrosshairState,
    pub drawings: DrawingManager,
    pub series: SeriesCollection,
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

        Self {
            renderer,
            viewport,
            bars,
            style,
            crosshair,
            drawings,
            series,
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

    /// Set data points for a line series.
    pub fn set_series_data(&mut self, id: SeriesId, data: Vec<LinePoint>) {
        if let Some(s) = self.series.get_mut(id) {
            s.line_data.set(data);
        }
    }

    /// Remove a series by ID.
    pub fn remove_series(&mut self, id: SeriesId) -> bool {
        self.series.remove(id)
    }

    /// Set visibility of a series.
    pub fn set_series_visible(&mut self, id: SeriesId, visible: bool) {
        if let Some(s) = self.series.get_mut(id) {
            s.line_options.visible = visible;
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
