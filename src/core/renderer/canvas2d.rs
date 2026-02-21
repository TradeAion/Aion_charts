//! Canvas2DRenderer — fallback renderer implementing ChartRenderer.
//!
//! Uses the existing DrawList/GeometryGenerator approach. Each `draw_*`
//! method generates its subset of geometry and draws it via fill_rect.
//! This preserves pixel-perfect parity with the old monolithic path.

#![cfg(target_arch = "wasm32")]

use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use wasm_bindgen::prelude::*;
use crate::core::renderer::traits::{ChartRenderer, RenderContext};
use crate::core::renderer::draw_list::ColoredRect;
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

    /// Render a slice of ColoredRects via Canvas2D fill_rect.
    fn draw_rects(&self, rects: &[ColoredRect]) {
        for rect in rects {
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

impl ChartRenderer for Canvas2DRenderer {
    fn name(&self) -> &str { "canvas2d" }

    fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        self.physical_width = pw;
        self.physical_height = ph;
        self.dpr = dpr;
        self.ctx.set_image_smoothing_enabled(false);
    }

    fn is_valid(&self) -> bool { true }

    fn begin_frame(&mut self, _ctx: &RenderContext) -> Result<(), String> {
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;
        self.ctx.clear_rect(0.0, 0.0, pane_w, pane_h);
        Ok(())
    }

    fn draw_grid(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;
        // Background fill
        let bg = &ctx.style.bg_color;
        let bg_rect = ColoredRect {
            x: 0.0, y: 0.0, w: pane_w as f32, h: pane_h as f32,
            r: bg[0], g: bg[1], b: bg[2], a: bg[3],
        };
        self.draw_rects(&[bg_rect]);
        // Grid lines (as thin rects)
        let grid_rects = geometry_generator::generate_grid_rects(
            ctx.style, ctx.y_ticks, ctx.x_ticks, pane_w, pane_h,
        );
        self.draw_rects(&grid_rects);
        Ok(())
    }

    fn draw_candles(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;
        let candle_rects = geometry_generator::generate_candle_rects(
            ctx.bars, ctx.viewport, ctx.style, pane_w, pane_h,
            ctx.h_pixel_ratio, ctx.v_pixel_ratio,
        );
        self.draw_rects(&candle_rects);
        Ok(())
    }

    fn draw_volume(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;
        let vol_rects = geometry_generator::generate_volume_rects(
            ctx.bars, ctx.viewport, ctx.style, pane_w, pane_h,
            ctx.h_pixel_ratio, ctx.v_pixel_ratio,
        );
        self.draw_rects(&vol_rects);
        Ok(())
    }

    fn draw_lines(&mut self, _ctx: &RenderContext) -> Result<(), String> {
        // TODO: indicator lines (SMA, EMA) — not yet implemented
        Ok(())
    }

    fn draw_text(&mut self, _ctx: &RenderContext) -> Result<(), String> {
        // Text is handled by separate axis renderers in the WASM layer
        Ok(())
    }

    fn draw_crosshair(&mut self, _ctx: &RenderContext) -> Result<(), String> {
        // Crosshair is handled by the OverlayRenderer in the WASM layer
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), String> {
        // Canvas2D: nothing to submit
        Ok(())
    }
}
