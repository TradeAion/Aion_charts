//! OverlayRenderer — pane top canvas: crosshair lines + watermark only.
//!
//! Sits on the pane's top canvas (z-index:2).
//! No longer draws axes or crosshair labels — those are on their own widget canvases.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use crate::core::renderer::traits::{ChartStyle, CrosshairState};
use crate::core::renderer::rgba_str as rgba;
use crate::core::drawings::types::DrawingGeometry;

pub struct OverlayRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    pw: u32,
    ph: u32,
    dpr: f64,
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

    /// Render crosshair lines + active drawings on the pane's top canvas.
    pub fn render(
        &self,
        crosshair: &CrosshairState,
        style: &ChartStyle,
    ) {
        self.render_with_drawings(crosshair, style, &[]);
    }

    /// Render crosshair + top-layer drawing geometry on the overlay canvas.
    pub fn render_with_drawings(
        &self,
        crosshair: &CrosshairState,
        style: &ChartStyle,
        top_drawings: &[DrawingGeometry],
    ) {
        let pw = self.pw as f64;
        let ph = self.ph as f64;
        self.ctx.clear_rect(0.0, 0.0, pw, ph);

        // Draw active/hovered drawings BELOW crosshair
        for geom in top_drawings {
            self.draw_geometry(geom);
        }

        self.draw_crosshair(crosshair, style, pw, ph);
    }

    /// Draw a DrawingGeometry (lines, rects, text, anchor circles) on the overlay.
    fn draw_geometry(&self, geom: &DrawingGeometry) {
        // Filled rects
        for r in &geom.rects {
            if r.w <= 0.0 || r.h <= 0.0 { continue; }
            self.ctx.set_fill_style_str(&rgba(&[r.r, r.g, r.b, r.a]));
            self.ctx.fill_rect(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
        }

        // Lines
        for l in &geom.lines {
            self.ctx.set_stroke_style_str(&rgba(&[l.r, l.g, l.b, l.a]));
            self.ctx.set_line_width(l.width as f64);
            self.ctx.set_line_cap("round");

            if l.dash > 0.0 && l.gap > 0.0 {
                let _ = self.ctx.set_line_dash(&js_sys::Array::of2(
                    &JsValue::from(l.dash as f64),
                    &JsValue::from(l.gap as f64),
                ));
            } else {
                let _ = self.ctx.set_line_dash(&js_sys::Array::new());
            }

            self.ctx.begin_path();
            self.ctx.move_to(l.x0 as f64, l.y0 as f64);
            self.ctx.line_to(l.x1 as f64, l.y1 as f64);
            self.ctx.stroke();
        }
        let _ = self.ctx.set_line_dash(&js_sys::Array::new());

        // Text labels (in physical pixel coords)
        for t in &geom.texts {
            let font = format!("{}px {}", t.font_size, "-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif");
            self.ctx.set_font(&font);
            self.ctx.set_fill_style_str(&rgba(&[t.r, t.g, t.b, t.a]));
            self.ctx.set_text_align("center");
            self.ctx.set_text_baseline("middle");
            let _ = self.ctx.fill_text(&t.text, t.x as f64, t.y as f64);
        }

        // Anchor circles
        for a in &geom.anchors {
            // Fill
            self.ctx.set_fill_style_str(&rgba(&a.fill));
            self.ctx.begin_path();
            let _ = self.ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            self.ctx.fill();
            // Border
            self.ctx.set_stroke_style_str(&rgba(&a.border));
            self.ctx.set_line_width(a.border_width);
            self.ctx.begin_path();
            let _ = self.ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            self.ctx.stroke();
        }
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
