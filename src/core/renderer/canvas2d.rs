//! Canvas2DRenderer — fallback renderer implementing ChartRenderer.
//!
//! Uses the existing DrawList/GeometryGenerator approach. Each `draw_*`
//! method generates its subset of geometry and draws it via fill_rect.
//! This preserves pixel-perfect parity with the old monolithic path.

#![cfg(target_arch = "wasm32")]

use crate::core::renderer::draw_list::ColoredRect;
use crate::core::renderer::geometry_generator;
use crate::core::renderer::traits::{ChartRenderer, RenderContext};
use crate::core::renderer::transforms::{bar_to_x, price_to_y};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct Canvas2DRenderer {
    #[allow(dead_code)]
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    physical_width: u32,
    physical_height: u32,
    dpr: f64,
}

/// Convert [R, G, B, A] (0.0-1.0) to CSS rgba() string.
fn rgba(c: &[f32; 4]) -> String {
    format!(
        "rgba({},{},{},{})",
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        c[3],
    )
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

        Ok(Self {
            canvas,
            ctx,
            physical_width: pw,
            physical_height: ph,
            dpr,
        })
    }

    /// Render a slice of ColoredRects via Canvas2D fill_rect.
    fn draw_rects(&self, rects: &[ColoredRect]) {
        for rect in rects {
            if rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            let color = format!(
                "rgba({},{},{},{})",
                (rect.r * 255.0) as u8,
                (rect.g * 255.0) as u8,
                (rect.b * 255.0) as u8,
                rect.a,
            );
            self.ctx.set_fill_style_str(&color);
            self.ctx
                .fill_rect(rect.x as f64, rect.y as f64, rect.w as f64, rect.h as f64);
        }
    }

    /// Draw a smooth anti-aliased line chart using Canvas2D native line primitives.
    fn draw_line_chart_native(&self, ctx: &RenderContext) {
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;
        let vol_h = pane_h * ctx.viewport.volume_height_ratio as f64;
        let candle_h = pane_h - vol_h;

        let start = (ctx.viewport.start_bar.floor() as usize)
            .saturating_sub(1)
            .min(ctx.bars.len());
        let end = ((ctx.viewport.end_bar.ceil() as usize) + 1).min(ctx.bars.len());

        if end <= start || end - start < 2 {
            return;
        }

        // Set line style
        let line_w = (ctx.main_chart_options.line_width * ctx.v_pixel_ratio as f32)
            .round()
            .max(1.0);
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        self.ctx
            .set_stroke_style_str(&rgba(&ctx.main_chart_options.line_color));
        self.ctx.set_line_width(line_w as f64);
        self.ctx.set_line_join("round");
        self.ctx.set_line_cap("round");

        // Build path
        self.ctx.begin_path();

        let first_bar = ctx.bars.get_unchecked(start);
        let first_x = bar_to_x(start as f64 + 0.5, ctx.viewport, pane_w).round() + correction;
        let first_y =
            price_to_y(first_bar.close as f64, ctx.viewport, candle_h).round() + correction;
        self.ctx.move_to(first_x, first_y);

        for i in (start + 1)..end {
            let b = ctx.bars.get_unchecked(i);
            let x = bar_to_x(i as f64 + 0.5, ctx.viewport, pane_w).round() + correction;
            let y = price_to_y(b.close as f64, ctx.viewport, candle_h).round() + correction;
            self.ctx.line_to(x, y);
        }

        self.ctx.stroke();
    }

    /// Draw a smooth anti-aliased area chart using Canvas2D native primitives.
    fn draw_area_chart_native(&self, ctx: &RenderContext) {
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;
        let vol_h = pane_h * ctx.viewport.volume_height_ratio as f64;
        let candle_h = pane_h - vol_h;

        let start = (ctx.viewport.start_bar.floor() as usize)
            .saturating_sub(1)
            .min(ctx.bars.len());
        let end = ((ctx.viewport.end_bar.ceil() as usize) + 1).min(ctx.bars.len());

        if end <= start || end - start < 2 {
            return;
        }

        // Step 1: Fill the area under the line
        self.ctx.begin_path();

        // Start at bottom-left
        let first_x = bar_to_x(start as f64 + 0.5, ctx.viewport, pane_w).round();
        self.ctx.move_to(first_x, candle_h);

        // Draw line along the top (close prices)
        for i in start..end {
            let b = ctx.bars.get_unchecked(i);
            let x = bar_to_x(i as f64 + 0.5, ctx.viewport, pane_w).round();
            let y = price_to_y(b.close as f64, ctx.viewport, candle_h).round();
            self.ctx.line_to(x, y);
        }

        // Close path along bottom
        let last_x = bar_to_x((end - 1) as f64 + 0.5, ctx.viewport, pane_w).round();
        self.ctx.line_to(last_x, candle_h);
        self.ctx.close_path();

        // Fill with gradient or solid color
        let fill_color = ctx.main_chart_options.area_top_color;
        self.ctx.set_fill_style_str(&rgba(&fill_color));
        self.ctx.fill();

        // Step 2: Draw the line on top
        let line_w = (ctx.main_chart_options.line_width * ctx.v_pixel_ratio as f32)
            .round()
            .max(1.0);
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        self.ctx
            .set_stroke_style_str(&rgba(&ctx.main_chart_options.line_color));
        self.ctx.set_line_width(line_w as f64);
        self.ctx.set_line_join("round");
        self.ctx.set_line_cap("round");

        self.ctx.begin_path();

        let first_bar = ctx.bars.get_unchecked(start);
        let first_y =
            price_to_y(first_bar.close as f64, ctx.viewport, candle_h).round() + correction;
        self.ctx.move_to(first_x + correction, first_y);

        for i in (start + 1)..end {
            let b = ctx.bars.get_unchecked(i);
            let x = bar_to_x(i as f64 + 0.5, ctx.viewport, pane_w).round() + correction;
            let y = price_to_y(b.close as f64, ctx.viewport, candle_h).round() + correction;
            self.ctx.line_to(x, y);
        }

        self.ctx.stroke();
    }
}

impl ChartRenderer for Canvas2DRenderer {
    fn name(&self) -> &str {
        "canvas2d"
    }

    fn resize(&mut self, pw: u32, ph: u32, dpr: f64) {
        self.physical_width = pw;
        self.physical_height = ph;
        self.dpr = dpr;
        self.ctx.set_image_smoothing_enabled(false);
    }

    fn is_valid(&self) -> bool {
        true
    }

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
            x: 0.0,
            y: 0.0,
            w: pane_w as f32,
            h: pane_h as f32,
            r: bg[0],
            g: bg[1],
            b: bg[2],
            a: bg[3],
        };
        self.draw_rects(&[bg_rect]);
        // Grid lines (as thin rects)
        let grid_rects = geometry_generator::generate_grid_rects(
            ctx.style,
            ctx.y_ticks,
            ctx.x_ticks,
            pane_w,
            pane_h,
        );
        self.draw_rects(&grid_rects);
        Ok(())
    }

    fn draw_candles(&mut self, ctx: &RenderContext) -> Result<(), String> {
        use crate::core::chart_type::MainChartType;
        use crate::core::renderer::geometry_generator;

        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;

        match ctx.main_chart_type {
            MainChartType::Line => {
                // Use native Canvas2D line drawing for smooth anti-aliased lines
                self.draw_line_chart_native(ctx);
            }
            MainChartType::Area | MainChartType::Baseline => {
                // Use native Canvas2D for smooth area fill and line
                self.draw_area_chart_native(ctx);
            }
            MainChartType::Candlestick => {
                let rects = geometry_generator::generate_candle_rects(
                    ctx.bars,
                    ctx.viewport,
                    ctx.style,
                    pane_w,
                    pane_h,
                    ctx.h_pixel_ratio,
                    ctx.v_pixel_ratio,
                );
                self.draw_rects(&rects);
            }
            MainChartType::OhlcBars => {
                let rects = geometry_generator::generate_ohlc_bar_rects(
                    ctx.bars,
                    ctx.viewport,
                    ctx.style,
                    pane_w,
                    pane_h,
                    ctx.h_pixel_ratio,
                    ctx.v_pixel_ratio,
                );
                self.draw_rects(&rects);
            }
            MainChartType::HeikinAshi => {
                let rects = geometry_generator::generate_heikin_ashi_rects(
                    ctx.bars,
                    ctx.viewport,
                    ctx.style,
                    pane_w,
                    pane_h,
                    ctx.h_pixel_ratio,
                    ctx.v_pixel_ratio,
                );
                self.draw_rects(&rects);
            }
        }

        Ok(())
    }

    fn draw_volume(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;
        let vol_rects = geometry_generator::generate_volume_rects(
            ctx.bars,
            ctx.viewport,
            ctx.style,
            pane_w,
            pane_h,
            ctx.h_pixel_ratio,
            ctx.v_pixel_ratio,
        );
        self.draw_rects(&vol_rects);
        Ok(())
    }

    fn draw_lines(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;

        // Build timestamps slice for bar-index lookup
        let ts: Vec<u64> = (0..ctx.bars.len())
            .map(|i| ctx.bars.timestamps.value(i))
            .collect();

        let line_rects = crate::core::renderer::line_generator::generate_all_line_rects(
            ctx.series,
            ctx.viewport,
            &ts,
            pane_w,
            pane_h,
            ctx.h_pixel_ratio,
            ctx.v_pixel_ratio,
        );
        self.draw_rects(&line_rects);
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
