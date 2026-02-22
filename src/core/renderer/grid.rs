//! GridRenderer — draws grid lines on the pane's base (grid) canvas.
//!
//! Uses pre-computed tick marks from tick_marks.rs (single source of truth).
//! Canvas is sized to the pane (chart area) only — no axis regions.

#![cfg(target_arch = "wasm32")]

use crate::core::renderer::rgba_str as rgba;
use crate::core::renderer::traits::{ChartStyle, TickMark};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct GridRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
}

#[inline]
fn snap(v: f64) -> f64 {
    v.floor() + 0.5
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
        self.pw = pw;
        self.ph = ph;
        self.dpr = dpr;
        self.canvas.set_width(pw.max(1));
        self.canvas.set_height(ph.max(1));
        self.ctx.set_image_smoothing_enabled(false);
    }

    /// Render grid lines using pre-computed ticks.
    /// The canvas is sized to the pane only (chart_w x chart_h in physical px).
    /// Grid lines are currently disabled - only clears background.
    pub fn render(&self, style: &ChartStyle, _y_ticks: &[TickMark], _x_ticks: &[TickMark]) {
        let w = self.pw as f64;
        let h = self.ph as f64;

        // Clear with chart background only - grid lines disabled
        self.ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.ctx.fill_rect(0.0, 0.0, w, h);
    }
}
