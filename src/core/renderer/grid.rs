//! GridRenderer — draws grid lines on the pane's base (grid) canvas.
//!
//! Uses pre-computed tick marks from tick_marks.rs (single source of truth).
//! Canvas is sized to the pane (chart area) only — no axis regions.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use crate::core::renderer::traits::{ChartStyle, TickMark};
use crate::core::renderer::rgba_str as rgba;

pub struct GridRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
}

#[inline]
fn snap(v: f64) -> f64 { v.floor() + 0.5 }

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
        Ok(Self { canvas, ctx, pw, ph, dpr })
    }

    pub fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        self.pw = pw;
        self.ph = ph;
        self.dpr = dpr;
        self.canvas.set_width(pw.max(1));
        self.canvas.set_height(ph.max(1));
        self.ctx.set_image_smoothing_enabled(false);
    }

    /// Render grid lines using pre-computed ticks.
    /// The canvas is sized to the pane only (chart_w x chart_h in physical px).
    pub fn render(
        &self,
        style: &ChartStyle,
        y_ticks: &[TickMark],
        x_ticks: &[TickMark],
    ) {
        let w = self.pw as f64;
        let h = self.ph as f64;

        // Clear with chart background
        self.ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.ctx.fill_rect(0.0, 0.0, w, h);

        // Grid lines (1 physical pixel wide, major ticks only)
        self.ctx.set_fill_style_str(&rgba(&style.grid_color));

        // Horizontal grid lines (at price ticks)
        for t in y_ticks {
            if !t.major { continue; }
            let y = snap(t.pixel);
            if y > 0.0 && y < h {
                self.ctx.fill_rect(0.0, y, w, 1.0);
            }
        }

        // Vertical grid lines (at time ticks)
        for t in x_ticks {
            if !t.major { continue; }
            let x = snap(t.pixel);
            if x > 0.0 && x < w {
                self.ctx.fill_rect(x, 0.0, 1.0, h);
            }
        }
    }
}
