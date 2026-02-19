//! GridRenderer — draws grid lines on a dedicated background canvas.
//!
//! This is always Canvas2D on a canvas that sits BELOW the main rendering
//! canvas (z-index:0). Grid lines are drawn behind candles/volume, matching
//! LWC's behavior where grid is the bottommost layer in a pane.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::traits::{ChartStyle, TickMark};
use crate::core::renderer::series::ChartLayout;
use crate::core::formatters::{format_price, format_timestamp, nice_step};

pub struct GridRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    physical_width: u32,
    physical_height: u32,
    dpr: f64,
}

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

    /// Render the grid. Also returns computed ticks for use by the overlay (axes).
    pub fn render(
        &self,
        bars: &[Bar],
        vp: &Viewport,
        style: &ChartStyle,
        layout: &ChartLayout,
    ) -> (Vec<TickMark>, Vec<TickMark>) {
        // Clear with chart background
        self.ctx.set_fill_style_str(&rgba(&style.bg_color));
        self.ctx.fill_rect(0.0, 0.0, self.physical_width as f64, self.physical_height as f64);

        let y_ticks = self.compute_y_ticks(vp, layout);
        let x_ticks = self.compute_x_ticks(vp, bars, layout);

        self.draw_grid_lines(vp, style, layout, &y_ticks, &x_ticks);

        (y_ticks, x_ticks)
    }

    // ── Coordinate helpers ───────────────────────────────────────────────

    #[inline]
    fn bar_to_x(&self, bar_idx: f64, vp: &Viewport, chart_w: f64) -> f64 {
        (bar_idx - vp.start_bar) / (vp.end_bar - vp.start_bar) * chart_w
    }

    #[inline]
    fn price_to_y(&self, price: f64, vp: &Viewport, candle_h: f64) -> f64 {
        let frac = (price - vp.price_min) / (vp.price_max - vp.price_min);
        candle_h * (1.0 - frac)
    }

    // ── Tick computation ─────────────────────────────────────────────────

    fn compute_y_ticks(&self, vp: &Viewport, layout: &ChartLayout) -> Vec<TickMark> {
        let range = vp.price_max - vp.price_min;
        if range <= 0.0 { return vec![]; }
        let step = nice_step(range / (layout.candle_h / (40.0 * layout.dpr)).max(3.0).min(15.0));
        let first = (vp.price_min / step).ceil() * step;
        let mut out = Vec::new();
        let mut v = first;
        while v <= vp.price_max {
            let px = self.price_to_y(v, vp, layout.candle_h);
            out.push(TickMark {
                value: v,
                pixel: px,
                label: format_price(v, step),
                major: true,
            });
            v += step;
        }
        out
    }

    fn compute_x_ticks(&self, vp: &Viewport, bars: &[Bar], layout: &ChartLayout) -> Vec<TickMark> {
        let count = vp.end_bar - vp.start_bar;
        if count <= 0.0 { return vec![]; }
        let step = nice_step(count / (layout.chart_w / (100.0 * layout.dpr)).max(2.0)).max(1.0);
        let first = (vp.start_bar / step).ceil() * step;
        let mut out = Vec::new();
        let mut v = first;
        while v <= vp.end_bar {
            let px = self.bar_to_x(v, vp, layout.chart_w);
            let bar_i = v as usize;
            let label = if bar_i < bars.len() && bars[bar_i].timestamp > 0 {
                format_timestamp(bars[bar_i].timestamp)
            } else {
                format!("{}", v as i64)
            };
            out.push(TickMark { value: v, pixel: px, label, major: true });
            v += step;
        }
        out
    }

    // ── Grid line drawing ────────────────────────────────────────────────

    fn draw_grid_lines(
        &self,
        vp: &Viewport,
        style: &ChartStyle,
        layout: &ChartLayout,
        y_ticks: &[TickMark],
        x_ticks: &[TickMark],
    ) {
        self.ctx.set_stroke_style_str(&rgba(&style.grid_color));
        self.ctx.set_line_width(1.0);
        let total_h = layout.candle_h + layout.vol_h;
        self.ctx.begin_path();

        // Horizontal grid lines (at price ticks)
        for t in y_ticks {
            if !t.major { continue; }
            let y = snap(self.price_to_y(t.value, vp, layout.candle_h));
            if y > 0.0 && y < total_h {
                self.ctx.move_to(0.0, y);
                self.ctx.line_to(layout.chart_w, y);
            }
        }

        // Vertical grid lines (at time ticks)
        for t in x_ticks {
            if !t.major { continue; }
            let x = snap(self.bar_to_x(t.value, vp, layout.chart_w));
            if x > 0.0 && x < layout.chart_w {
                self.ctx.move_to(x, 0.0);
                self.ctx.line_to(x, total_h);
            }
        }

        self.ctx.stroke();
    }
}
