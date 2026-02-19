//! OverlayRenderer — pane top canvas: crosshair lines + watermark only.
//!
//! Sits on the pane's top canvas (z-index:2).
//! No longer draws axes or crosshair labels — those are on their own widget canvases.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use crate::core::renderer::traits::{ChartStyle, CrosshairState};

pub struct OverlayRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
}

#[inline]
fn rgba(c: &[f32; 4]) -> String {
    format!(
        "rgba({},{},{},{})",
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        c[3]
    )
}

impl OverlayRenderer {
    pub fn new(canvas: HtmlCanvasElement, dpr: f64) -> Result<Self, String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("overlay get_context('2d') failed: {:?}", e))?
            .ok_or("overlay get_context('2d') returned None")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "overlay context is not CanvasRenderingContext2d")?;

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

    /// Render crosshair lines + watermark on the pane's top canvas.
    /// The canvas is sized to the pane (chart area) only — no axis regions.
    pub fn render(
        &self,
        crosshair: &CrosshairState,
        style: &ChartStyle,
    ) {
        let pw = self.pw as f64;
        let ph = self.ph as f64;
        self.ctx.clear_rect(0.0, 0.0, pw, ph);
        self.draw_crosshair(crosshair, style, pw, ph);
    }

    fn draw_crosshair(
        &self,
        ch: &CrosshairState,
        style: &ChartStyle,
        pane_w: f64,
        pane_h: f64,
    ) {
        if !ch.active { return; }

        let dpr = self.dpr;
        let mx = ch.x * dpr;
        let my = ch.y * dpr;

        if mx < 0.0 || mx > pane_w || my < 0.0 || my > pane_h { return; }

        // Dashed crosshair lines (LWC: LargeDashed = 6*lineWidth, 6*lineWidth)
        let line_w = (1.0 * dpr).floor().max(1.0);
        let dash_len = 6.0 * line_w;
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        self.ctx.set_stroke_style_str(&rgba(&style.crosshair_color));
        self.ctx.set_line_width(line_w);
        self.ctx.set_line_cap("butt");
        let _ = self.ctx.set_line_dash(&js_sys::Array::of2(
            &JsValue::from(dash_len),
            &JsValue::from(dash_len),
        ));

        // Horizontal line (full pane width)
        let hy = my.round() + correction;
        self.ctx.begin_path();
        self.ctx.move_to(0.0, hy);
        self.ctx.line_to(pane_w, hy);
        self.ctx.stroke();

        // Vertical line (full pane height)
        let vx = mx.round() + correction;
        self.ctx.begin_path();
        self.ctx.move_to(vx, 0.0);
        self.ctx.line_to(vx, pane_h);
        self.ctx.stroke();

        // Reset dash
        let _ = self.ctx.set_line_dash(&js_sys::Array::new());
    }
}
