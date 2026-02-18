//! OverlayRenderer — shared Canvas2D overlay for axes, crosshair, and watermark.
//!
//! This sits on the TOP canvas (z-index:2), above candles and grid.
//! It draws: Y-axis, X-axis, crosshair, watermark.
//! Grid lines are NOT here — they're on the background grid canvas (z-index:0)
//! so they appear BEHIND candles, matching LWC behavior.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{ChartStyle, CrosshairState, TickMark};
use crate::core::renderer::series::ChartLayout;
use crate::core::formatters::{format_price, format_timestamp};

pub struct OverlayRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    physical_width: u32,
    physical_height: u32,
    dpr: f64,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn snap(v: f64) -> f64 { v.floor() + 0.5 }

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
        Ok(Self { canvas, ctx, physical_width: pw, physical_height: ph, dpr })
    }

    pub fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        self.physical_width = pw;
        self.physical_height = ph;
        self.dpr = dpr;
        self.canvas.set_width(pw.max(1));
        self.canvas.set_height(ph.max(1));
        self.ctx.set_image_smoothing_enabled(false);
    }

    /// Render axes, crosshair, watermark. Receives pre-computed ticks from GridRenderer.
    pub fn render(
        &self,
        bars: &[Bar],
        vp: &Viewport,
        style: &ChartStyle,
        crosshair: &CrosshairState,
        layout: &ChartLayout,
        y_ticks: &[TickMark],
        x_ticks: &[TickMark],
    ) {
        // Clear the entire overlay (transparent)
        self.ctx.clear_rect(0.0, 0.0, self.physical_width as f64, self.physical_height as f64);

        self.draw_watermark(style, layout);
        self.draw_y_axis(style, layout, y_ticks);
        self.draw_x_axis(style, layout, x_ticks);
        self.draw_crosshair(crosshair, bars, vp, style, layout);
    }

    // ── Coordinate mapping ──────────────────────────────────────────────

    #[inline]
    fn x_to_bar(&self, x: f64, vp: &Viewport, chart_w: f64) -> f64 {
        vp.start_bar + (x / chart_w) * (vp.end_bar - vp.start_bar)
    }

    #[inline]
    fn y_to_price(&self, y: f64, vp: &Viewport, candle_h: f64) -> f64 {
        vp.price_min + (1.0 - y / candle_h) * (vp.price_max - vp.price_min)
    }

    // ── Y-Axis (price) ─────────────────────────────────────────────────

    fn draw_y_axis(
        &self,
        s: &ChartStyle,
        layout: &ChartLayout,
        ticks: &[TickMark],
    ) {
        let ax = layout.chart_w;
        let aw = s.y_axis_width as f64 * self.dpr;
        let fs = s.font_size_axis as f64 * self.dpr;
        let pad = 6.0 * self.dpr;

        // Axis background
        self.ctx.set_fill_style_str(&rgba(&s.axis_bg_color));
        self.ctx.fill_rect(ax, 0.0, aw, self.physical_height as f64);

        // Axis border line
        self.ctx.set_stroke_style_str(&rgba(&s.grid_color));
        self.ctx.set_line_width(1.0);
        self.ctx.begin_path();
        self.ctx.move_to(snap(ax), 0.0);
        self.ctx.line_to(snap(ax), self.physical_height as f64);
        self.ctx.stroke();

        // Tick labels
        self.ctx.set_fill_style_str(&rgba(&s.axis_text_color));
        self.ctx.set_font(&format!("{}px {}", fs, s.font_family));
        self.ctx.set_text_align("left");
        self.ctx.set_text_baseline("middle");
        for t in ticks {
            if t.pixel < 0.0 || t.pixel > layout.candle_h { continue; }
            let _ = self.ctx.fill_text(&t.label, ax + pad, t.pixel);
        }
    }

    // ── X-Axis (time/index) ─────────────────────────────────────────────

    fn draw_x_axis(
        &self,
        s: &ChartStyle,
        layout: &ChartLayout,
        ticks: &[TickMark],
    ) {
        let ay = layout.candle_h + layout.vol_h;
        let ah = s.x_axis_height as f64 * self.dpr;
        let fs = s.font_size_axis as f64 * self.dpr;
        let pad = 6.0 * self.dpr;

        // Axis background
        self.ctx.set_fill_style_str(&rgba(&s.axis_bg_color));
        self.ctx.fill_rect(0.0, ay, self.physical_width as f64, ah);

        // Axis border line
        self.ctx.set_stroke_style_str(&rgba(&s.grid_color));
        self.ctx.set_line_width(1.0);
        self.ctx.begin_path();
        self.ctx.move_to(0.0, snap(ay));
        self.ctx.line_to(self.physical_width as f64, snap(ay));
        self.ctx.stroke();

        // Tick labels
        self.ctx.set_fill_style_str(&rgba(&s.axis_text_color));
        self.ctx.set_font(&format!("{}px {}", fs, s.font_family));
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("top");
        for t in ticks {
            if t.pixel < 0.0 || t.pixel > layout.chart_w { continue; }
            let _ = self.ctx.fill_text(&t.label, t.pixel, ay + pad);
        }
    }

    // ── Crosshair ───────────────────────────────────────────────────────

    fn draw_crosshair(
        &self,
        ch: &CrosshairState,
        bars: &[Bar],
        vp: &Viewport,
        s: &ChartStyle,
        layout: &ChartLayout,
    ) {
        if !ch.active { return; }

        let mx = ch.x * self.dpr;
        let my = ch.y * self.dpr;
        let total_h = layout.candle_h + layout.vol_h;
        if mx < 0.0 || mx > layout.chart_w || my < 0.0 || my > total_h { return; }

        let fs = s.font_size_axis as f64 * self.dpr;

        // Dashed crosshair lines
        self.ctx.set_stroke_style_str(&rgba(&s.crosshair_color));
        self.ctx.set_line_width(1.0);
        let _ = self.ctx.set_line_dash(&js_sys::Array::of2(
            &JsValue::from(4.0 * self.dpr),
            &JsValue::from(3.0 * self.dpr),
        ));
        self.ctx.begin_path();
        self.ctx.move_to(0.0, snap(my));
        self.ctx.line_to(layout.chart_w, snap(my));
        self.ctx.stroke();
        self.ctx.begin_path();
        self.ctx.move_to(snap(mx), 0.0);
        self.ctx.line_to(snap(mx), total_h);
        self.ctx.stroke();
        let _ = self.ctx.set_line_dash(&js_sys::Array::new());

        self.ctx.set_font(&format!("{}px {}", fs, s.font_family));

        // Price label on Y-axis
        let price_lbl = format_price(self.y_to_price(my, vp, layout.candle_h), 0.01);
        let y_ax_x = layout.chart_w;
        let y_ax_w = s.y_axis_width as f64 * self.dpr;
        let lbl_h = fs + 8.0 * self.dpr;

        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_bg));
        self.ctx.fill_rect(y_ax_x, (my - lbl_h / 2.0).floor(), y_ax_w, lbl_h.ceil());
        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_text));
        self.ctx.set_text_align("left");
        self.ctx.set_text_baseline("middle");
        let _ = self.ctx.fill_text(&price_lbl, y_ax_x + 6.0 * self.dpr, my);

        // Time label on X-axis
        let bar_i = self.x_to_bar(mx, vp, layout.chart_w).round() as usize;
        let bar_lbl = if bar_i < bars.len() && bars[bar_i].timestamp > 0 {
            format_timestamp(bars[bar_i].timestamp)
        } else {
            format!("{}", bar_i)
        };
        let x_ax_y = layout.candle_h + layout.vol_h;
        let x_ax_h = s.x_axis_height as f64 * self.dpr;
        let lx_w = self.ctx.measure_text(&bar_lbl).map(|m| m.width()).unwrap_or(60.0)
            + 12.0 * self.dpr;

        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_bg));
        self.ctx.fill_rect((mx - lx_w / 2.0).floor(), x_ax_y, lx_w.ceil(), x_ax_h);
        self.ctx.set_fill_style_str(&rgba(&s.crosshair_label_text));
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");
        let _ = self.ctx.fill_text(&bar_lbl, mx, x_ax_y + x_ax_h / 2.0);
    }

    // ── Watermark ───────────────────────────────────────────────────────

    fn draw_watermark(&self, s: &ChartStyle, layout: &ChartLayout) {
        let fs = s.font_size_watermark as f64 * self.dpr;
        self.ctx.set_fill_style_str(&rgba(&s.watermark_color));
        self.ctx.set_font(&format!("bold {}px {}", fs, s.font_family));
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");
        let _ = self.ctx.fill_text("RayCharts", layout.chart_w / 2.0, layout.candle_h / 2.0);
    }
}
