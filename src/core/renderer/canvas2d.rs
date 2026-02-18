//! Canvas2DRenderer — dumb DrawList consumer.
//!
//! Receives a pre-computed DrawList (from GeometryGenerator) and draws every
//! ColoredRect via CanvasRenderingContext2d.fill_rect(). No candle logic here.
//! This guarantees pixel-perfect consistency with the WebGPU path.

#![cfg(target_arch = "wasm32")]

use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use wasm_bindgen::prelude::*;
use crate::core::renderer::traits::{Renderer, RenderContext};
use crate::core::renderer::series::ChartLayout;
use crate::core::renderer::draw_list::DrawList;
use crate::core::renderer::geometry_generator;

pub struct Canvas2DRenderer {
    #[allow(dead_code)]
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    physical_width: u32,
    physical_height: u32,
    dpr: f64,
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

        Ok(Self { canvas, ctx, physical_width: pw, physical_height: ph, dpr })
    }

    /// Render a DrawList — simple loop over rects.
    fn draw_list(&self, dl: &DrawList) {
        for rect in &dl.rects {
            if rect.w <= 0.0 || rect.h <= 0.0 { continue; }
            let color = format!(
                "rgba({},{},{},{})",
                (rect.r * 255.0) as u8,
                (rect.g * 255.0) as u8,
                (rect.b * 255.0) as u8,
                rect.a,
            );
            self.ctx.set_fill_style_str(&color);
            self.ctx.fill_rect(
                rect.x as f64,
                rect.y as f64,
                rect.w as f64,
                rect.h as f64,
            );
        }
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
            self.physical_width, self.physical_height, self.dpr, rc.style, rc.y_axis_css_w,
        );

        // Generate geometry — single source of truth
        let (dl, _, _) = geometry_generator::generate(rc.bars, rc.viewport, rc.style, &layout);

        // No clear needed — DrawList starts with opaque bg rect
        self.draw_list(&dl);

        Ok(())
    }

    fn is_valid(&self) -> bool { true }
}
