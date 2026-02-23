//! Canvas2DRenderer — fallback renderer implementing ChartRenderer.
//!
//! Uses the existing DrawList/GeometryGenerator approach. Each `draw_*`
//! method generates its subset of geometry and draws it via fill_rect.
//! This preserves pixel-perfect parity with the old monolithic path.

#![cfg(target_arch = "wasm32")]

use crate::core::renderer::draw_list::{ColoredRect, LineSegment};
use crate::core::renderer::geometry_generator;
use crate::core::renderer::traits::{ChartRenderer, RenderContext};
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

    /// Draw line segments as native Canvas2D strokes for smooth anti-aliased lines.
    /// Coordinates are already pixel-aligned by the generator, so we just draw them directly.
    fn draw_line_segments(&self, segments: &[LineSegment]) {
        if segments.is_empty() {
            return;
        }

        // Set line cap and join for clean segment connections
        self.ctx.set_line_cap("round");
        self.ctx.set_line_join("round");

        // Group segments by color and width for efficient batching
        let mut prev_color: Option<[f32; 4]> = None;
        let mut prev_width: Option<f32> = None;

        for seg in segments {
            let color = [seg.r, seg.g, seg.b, seg.a];
            let width = seg.width;

            // Update stroke style only when color changes
            if prev_color != Some(color) {
                self.ctx.set_stroke_style_str(&rgba(&color));
                prev_color = Some(color);
            }

            // Update line width only when it changes
            if prev_width != Some(width) {
                self.ctx.set_line_width(width as f64);
                prev_width = Some(width);
            }

            // Coordinates are already pixel-aligned by generate_line_segments
            self.ctx.begin_path();
            self.ctx.move_to(seg.x1 as f64, seg.y1 as f64);
            self.ctx.line_to(seg.x2 as f64, seg.y2 as f64);
            self.ctx.stroke();
        }
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
                let line_width = ctx.main_chart_options.line_width * ctx.v_pixel_ratio as f32;
                let segments = geometry_generator::generate_line_segments(
                    ctx.bars,
                    ctx.viewport,
                    ctx.main_chart_options.line_color,
                    line_width,
                    pane_w,
                    pane_h,
                );
                self.draw_line_segments(&segments);
            }
            MainChartType::Area => {
                let line_width = ctx.main_chart_options.line_width * ctx.v_pixel_ratio as f32;
                let fill_rects = geometry_generator::generate_main_area_fill_rects(
                    ctx.bars,
                    ctx.viewport,
                    ctx.main_chart_options.area_top_color,
                    ctx.main_chart_options.area_bottom_color,
                    pane_w,
                    pane_h,
                );
                let line_segments = geometry_generator::generate_line_segments(
                    ctx.bars,
                    ctx.viewport,
                    ctx.main_chart_options.line_color,
                    line_width,
                    pane_w,
                    pane_h,
                );
                self.draw_rects(&fill_rects);
                self.draw_line_segments(&line_segments);
            }
            MainChartType::Baseline => {
                let fill_rects = geometry_generator::generate_main_baseline_fill_rects(
                    ctx.bars,
                    ctx.viewport,
                    ctx.main_chart_options.baseline_value,
                    ctx.main_chart_options.baseline_top_fill_color,
                    ctx.main_chart_options.baseline_bottom_fill_color,
                    pane_w,
                    pane_h,
                );
                let line_segments = geometry_generator::generate_main_baseline_line_segments(
                    ctx.bars,
                    ctx.viewport,
                    ctx.main_chart_options.baseline_value,
                    ctx.main_chart_options.baseline_top_line_color,
                    ctx.main_chart_options.baseline_bottom_line_color,
                    ctx.main_chart_options.line_width,
                    pane_w,
                    pane_h,
                    ctx.v_pixel_ratio,
                );
                self.draw_rects(&fill_rects);
                self.draw_line_segments(&line_segments);
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
        let ts: Vec<u64> = (0..ctx.bars.len()).map(|i| ctx.bars.timestamp(i)).collect();

        // Generate smooth line segments + fill rects for overlays
        let (line_segments, fill_rects) =
            crate::core::renderer::line_generator::generate_all_overlay_geometry(
                ctx.series,
                ctx.viewport,
                &ts,
                pane_w,
                pane_h,
                ctx.h_pixel_ratio,
                ctx.v_pixel_ratio,
            );

        // Draw fill rects first (area/baseline fills behind lines)
        self.draw_rects(&fill_rects);

        // Draw smooth line segments on top using native Canvas2D strokes
        self.draw_line_segments(&line_segments);

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
