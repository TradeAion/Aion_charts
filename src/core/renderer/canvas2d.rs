//! Canvas2DRenderer — HTML5 Canvas 2D backend.
//!
//! This renderer ONLY draws candles and volume using composable
//! PaneSeriesRenderer components. All shared UI elements (axes, grid,
//! crosshair, watermark) live in separate renderers (GridRenderer,
//! OverlayRenderer) on their own canvases.
//!
//! The Canvas2D backend clears with a transparent background so the
//! grid canvas behind it shows through.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use crate::core::renderer::traits::{Renderer, RenderContext};
use crate::core::renderer::series::{ChartLayout, PaneSeriesRenderer};
use crate::core::renderer::candle_series::CandleSeriesCanvas2D;
use crate::core::renderer::volume_series::VolumeSeriesCanvas2D;

pub struct Canvas2DRenderer {
    #[allow(dead_code)]
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    physical_width: u32,
    physical_height: u32,
    dpr: f64,
    /// Composable series renderers, drawn in order.
    series: Vec<Box<dyn PaneSeriesRenderer>>,
}

impl Canvas2DRenderer {
    pub fn new(canvas: HtmlCanvasElement, dpr: f64) -> Result<Self, String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("get_context('2d') failed: {:?}", e))?
            .ok_or("get_context('2d') returned None")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Context is not CanvasRenderingContext2d")?;

        ctx.set_image_smoothing_enabled(false);
        let pw = canvas.width();
        let ph = canvas.height();

        // Default series: volume first (behind), then candles on top
        let series: Vec<Box<dyn PaneSeriesRenderer>> = vec![
            Box::new(VolumeSeriesCanvas2D::new()),
            Box::new(CandleSeriesCanvas2D::new()),
        ];

        Ok(Self { canvas, ctx, physical_width: pw, physical_height: ph, dpr, series })
    }
}

impl Renderer for Canvas2DRenderer {
    fn name(&self) -> &str { "canvas2d" }

    fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        self.physical_width = pw;
        self.physical_height = ph;
        self.dpr = dpr;
        self.ctx.set_image_smoothing_enabled(false);
    }

    fn render_frame(&mut self, rc: &RenderContext) -> Result<(), String> {
        let layout = ChartLayout::from_physical(
            self.physical_width, self.physical_height, self.dpr, rc.style,
        );

        // Clear with transparent so the grid canvas behind shows through
        self.ctx.clear_rect(0.0, 0.0, self.physical_width as f64, self.physical_height as f64);

        // Draw each series in order (volume behind, candles on top)
        for s in &self.series {
            s.draw(&self.ctx, rc.bars, rc.viewport, rc.style, &layout);
        }

        Ok(())
    }

    fn is_valid(&self) -> bool { true }
}
