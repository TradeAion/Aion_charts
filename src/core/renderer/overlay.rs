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
    /// Reference to the base chart canvas for rendering base-layer drawings.
    base_canvas: Option<HtmlCanvasElement>,
    base_ctx: Option<CanvasRenderingContext2d>,
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
        Ok(Self { canvas, ctx, base_canvas: None, base_ctx: None, pw, ph, dpr })
    }

    /// Set the base chart canvas for rendering base-layer drawings.
    /// Call this after construction to enable drawing on the chart canvas.
    pub fn set_base_canvas(&mut self, canvas: HtmlCanvasElement) -> Result<(), String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|e| format!("base canvas get_context('2d') failed: {:?}", e))?
            .ok_or("base canvas get_context('2d') returned None")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "base canvas context is not CanvasRenderingContext2d")?;
        ctx.set_image_smoothing_enabled(false);
        self.base_canvas = Some(canvas);
        self.base_ctx = Some(ctx);
        Ok(())
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

    /// Draw a DrawingGeometry on the overlay canvas.
    fn draw_geometry(&self, geom: &DrawingGeometry) {
        Self::draw_geometry_on(&self.ctx, geom);
    }

    /// Draw a DrawingGeometry (lines, rects, text, anchor circles) on any 2D context.
    fn draw_geometry_on(ctx: &CanvasRenderingContext2d, geom: &DrawingGeometry) {
        // Filled rects
        for r in &geom.rects {
            if r.w <= 0.0 || r.h <= 0.0 { continue; }
            ctx.set_fill_style_str(&rgba(&[r.r, r.g, r.b, r.a]));
            ctx.fill_rect(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
        }

        // Lines
        for l in &geom.lines {
            ctx.set_stroke_style_str(&rgba(&[l.r, l.g, l.b, l.a]));
            ctx.set_line_width(l.width as f64);
            ctx.set_line_cap("round");

            if l.dash > 0.0 && l.gap > 0.0 {
                let _ = ctx.set_line_dash(&js_sys::Array::of2(
                    &JsValue::from(l.dash as f64),
                    &JsValue::from(l.gap as f64),
                ));
            } else {
                let _ = ctx.set_line_dash(&js_sys::Array::new());
            }

            // LWC strokeInPixel: add 0.5px offset for odd-width lines
            // to snap to pixel center and prevent blurry sub-pixel rendering
            let correction = if (l.width as i32) % 2 == 1 { 0.5 } else { 0.0 };

            ctx.begin_path();
            ctx.move_to(l.x0 as f64 + correction, l.y0 as f64 + correction);
            ctx.line_to(l.x1 as f64 + correction, l.y1 as f64 + correction);
            ctx.stroke();
        }
        let _ = ctx.set_line_dash(&js_sys::Array::new());

        // Text labels (in physical pixel coords)
        for t in &geom.texts {
            let font = format!("{}px {}", t.font_size, "-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif");
            ctx.set_font(&font);
            ctx.set_fill_style_str(&rgba(&[t.r, t.g, t.b, t.a]));
            ctx.set_text_align("center");
            ctx.set_text_baseline("middle");
            let _ = ctx.fill_text(&t.text, t.x as f64, t.y as f64);
        }

        // Anchor circles
        for a in &geom.anchors {
            // Fill
            ctx.set_fill_style_str(&rgba(&a.fill));
            ctx.begin_path();
            let _ = ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            ctx.fill();
            // Border
            ctx.set_stroke_style_str(&rgba(&a.border));
            ctx.set_line_width(a.border_width);
            ctx.begin_path();
            let _ = ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            ctx.stroke();
        }
    }

    /// Render base-layer drawings on the chart (pane base) canvas.
    /// These sit above candles but below the crosshair/top canvas.
    /// Does NOT clear the canvas — call after engine.render() which already drew candles.
    pub fn render_base_drawings(&self, drawings: &[DrawingGeometry]) {
        let ctx = match &self.base_ctx {
            Some(c) => c,
            None => return, // base canvas not set up
        };
        for geom in drawings {
            Self::draw_geometry_on(ctx, geom);
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
