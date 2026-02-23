//! GridRenderer — draws grid lines on the pane's base (grid) canvas.
//!
//! NOW USES CENTRALIZED generate_grid_rects() from geometry_generator.rs.
//! This ensures consistent grid rendering across all renderers.

#![cfg(target_arch = "wasm32")]

use crate::core::renderer::geometry_generator;
use crate::core::renderer::rgba_str as rgba;
use crate::core::renderer::traits::{ChartStyle, TickMark};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct GridRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    #[allow(dead_code)]
    dpr: f64,
}

impl GridRenderer {
    pub fn new(canvas: HtmlCanvasElement, dpr: f64) -> Result<Self, String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("grid get_context('2d') failed: {:?}", e))?
            .ok_or("grid get_context('2d') returned None")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "grid context is not CanvasRenderingContext2d")?;

        ctx.set_image_smoothing_enabled(false);
        let pw = canvas.width();
        let ph = canvas.height();
        Ok(Self {
            canvas,
            ctx,
            pw,
            ph,
            dpr,
        })
    }

    pub fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        let pw = pw.max(1);
        let ph = ph.max(1);
        if self.pw == pw && self.ph == ph && (self.dpr - dpr).abs() < 1e-6 {
            return;
        }

        self.pw = pw;
        self.ph = ph;
        self.dpr = dpr;
        if self.canvas.width() != pw {
            self.canvas.set_width(pw);
        }
        if self.canvas.height() != ph {
            self.canvas.set_height(ph);
        }
        self.ctx.set_image_smoothing_enabled(false);
    }

    /// Render grid lines using the CENTRALIZED generate_grid_rects().
    /// This ensures consistent grid appearance across all renderers.
    pub fn render(&self, style: &ChartStyle, y_ticks: &[TickMark], x_ticks: &[TickMark]) {
        let w = self.pw as f64;
        let h = self.ph as f64;

        // Clear with chart background
        self.ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.ctx.fill_rect(0.0, 0.0, w, h);

        // Use centralized grid rect generation
        let grid_rects = geometry_generator::generate_grid_rects(style, y_ticks, x_ticks, w, h);

        // Draw grid rects
        for rect in &grid_rects {
            let color = format!(
                "rgba({},{},{},{})",
                (rect.r * 255.0) as u8,
                (rect.g * 255.0) as u8,
                (rect.b * 255.0) as u8,
                rect.a
            );
            self.ctx.set_fill_style_str(&color);
            self.ctx
                .fill_rect(rect.x as f64, rect.y as f64, rect.w as f64, rect.h as f64);
        }
    }
}
