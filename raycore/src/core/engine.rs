//! ChartEngine — the top-level orchestrator that owns all subsystems.
//!
//! Renderer-agnostic: works with any backend that implements the Renderer trait.
//! Owns viewport, data, style, crosshair state, and delegates rendering to
//! the active RendererBackend.

use crate::core::data::{Bar, BarArray};
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{Renderer, RendererBackend, RenderContext, ChartStyle, CrosshairState};

/// The main chart engine. Owns everything needed to render a chart.
pub struct ChartEngine {
    pub renderer: RendererBackend,
    pub viewport: Viewport,
    pub bars: BarArray,
    pub style: ChartStyle,
    pub crosshair: CrosshairState,
    pub dpr: f64,
    /// Dynamic Y-axis width in CSS px (set by the WASM layer after measuring text).
    pub y_axis_css_w: f64,
}

impl ChartEngine {
    /// Create a new engine with a given renderer backend.
    pub fn new(renderer: RendererBackend, width: u32, height: u32, dpr: f64) -> Self {
        let viewport = Viewport::new(width, height);
        let bars = BarArray::new();
        let style = ChartStyle::default();
        let crosshair = CrosshairState::default();

        Self {
            renderer,
            viewport,
            bars,
            style,
            crosshair,
            dpr,
            y_axis_css_w: 0.0,
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
            self.viewport.auto_fit_price(self.bars.as_slice());
        }
    }

    /// Resize the canvas / surface.
    pub fn resize(&mut self, width: u32, height: u32, dpr: f64) {
        self.dpr = dpr;
        self.renderer.resize(width, height, dpr);
        self.viewport.resize(width, height);
    }

    /// Set visible bar range.
    pub fn zoom_to_range(&mut self, start: u64, end: u64) {
        self.viewport.set_range(start as f64, end as f64);
        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(self.bars.as_slice());
        }
    }

    /// Update crosshair position (CSS pixels relative to canvas).
    pub fn set_crosshair(&mut self, x: f64, y: f64, active: bool) {
        self.crosshair.active = active;
        self.crosshair.x = x;
        self.crosshair.y = y;

        if active && !self.bars.is_empty() {
            let layout = crate::core::renderer::series::ChartLayout::from_physical(
                self.viewport.width, self.viewport.height, self.dpr, &self.style,
                if self.y_axis_css_w > 0.0 { self.y_axis_css_w } else { 34.0 },
            );
            let bar_idx = self.viewport.pixel_to_bar(x * self.dpr, layout.chart_w);
            let idx = bar_idx.round() as usize;
            self.crosshair.bar_index = if idx < self.bars.len() { Some(idx) } else { None };
            self.crosshair.price = self.viewport.pixel_to_price(y * self.dpr, layout.candle_h);
        }
    }

    /// Main render — called once per frame.
    pub fn render(&mut self) -> Result<(), String> {
        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(self.bars.as_slice());
        }

        let logical_w = self.viewport.width as f64 / self.dpr;
        let logical_h = self.viewport.height as f64 / self.dpr;

        let ctx = RenderContext {
            bars: self.bars.as_slice(),
            viewport: &self.viewport,
            style: &self.style,
            crosshair: &self.crosshair,
            dpr: self.dpr,
            logical_width: logical_w,
            logical_height: logical_h,
            y_axis_css_w: if self.y_axis_css_w > 0.0 { self.y_axis_css_w } else { 34.0 },
        };

        self.renderer.render_frame(&ctx)
    }
}
