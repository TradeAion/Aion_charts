//! Canvas2DRenderer — fallback renderer implementing ChartRenderer.
//!
//! Uses the existing DrawList/GeometryGenerator approach. Each `draw_*`
//! method generates its subset of geometry and draws it via fill_rect.
//! This preserves pixel-perfect parity with the old monolithic path.

#![cfg(target_arch = "wasm32")]

use crate::core::drawings::types::DrawingGeometry;
use crate::core::renderer::draw_list::{
    AreaSegment, ColoredRect, HorizontalGradientRect, LineSegment,
};
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

    fn draw_horizontal_gradient_rects(&self, rects: &[HorizontalGradientRect]) {
        for rect in rects {
            if rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            let left = [rect.left_r, rect.left_g, rect.left_b, rect.left_a];
            let right = [rect.right_r, rect.right_g, rect.right_b, rect.right_a];
            let gradient = self.ctx.create_linear_gradient(
                rect.x as f64,
                rect.y as f64,
                (rect.x + rect.w) as f64,
                rect.y as f64,
            );
            let _ = gradient.add_color_stop(0.0, &rgba(&left));
            let _ = gradient.add_color_stop(1.0, &rgba(&right));
            self.ctx.set_fill_style_canvas_gradient(&gradient);
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

    /// Draw a smooth filled area from contiguous area segments.
    /// Uses a single polygon path to avoid strip/band artifacts.
    fn draw_area_segments(&self, segments: &[AreaSegment]) {
        if segments.is_empty() {
            return;
        }

        let first = segments.first().unwrap();
        let last = segments.last().unwrap();

        let mut min_top = first.y1.min(first.y2);
        for seg in segments {
            min_top = min_top.min(seg.y1.min(seg.y2));
        }

        let top = [first.top_r, first.top_g, first.top_b, first.top_a];
        let bottom = [
            first.bottom_r,
            first.bottom_g,
            first.bottom_b,
            first.bottom_a,
        ];

        let gradient =
            self.ctx
                .create_linear_gradient(0.0, min_top as f64, 0.0, first.bottom as f64);
        let _ = gradient.add_color_stop(0.0, &rgba(&top));
        let _ = gradient.add_color_stop(1.0, &rgba(&bottom));

        self.ctx.begin_path();
        self.ctx.move_to(first.x1 as f64, first.y1 as f64);
        for seg in segments {
            self.ctx.line_to(seg.x2 as f64, seg.y2 as f64);
        }
        self.ctx.line_to(last.x2 as f64, first.bottom as f64);
        self.ctx.line_to(first.x1 as f64, first.bottom as f64);
        self.ctx.close_path();
        self.ctx.set_fill_style_canvas_gradient(&gradient);
        self.ctx.fill();
    }

    /// Draw footprint volume text labels using Canvas2D fillText.
    /// Kept as a utility — primary text rendering goes through the overlay layer.
    #[allow(dead_code)]
    fn draw_footprint_texts(&self, texts: &[crate::core::renderer::draw_list::DrawText]) {
        if texts.is_empty() {
            return;
        }

        let font_family = crate::core::renderer::theme::FONT_FAMILY;
        self.ctx.set_text_baseline("middle");

        let mut prev_font: Option<(f32, u16, bool)> = None;
        let mut prev_color: Option<[f32; 4]> = None;

        for t in texts {
            let font_key = (t.font_size, t.font_weight, t.italic);
            if prev_font != Some(font_key) {
                let font = if t.italic {
                    format!("italic {} {}px {}", t.font_weight, t.font_size, font_family)
                } else {
                    format!("{} {}px {}", t.font_weight, t.font_size, font_family)
                };
                self.ctx.set_font(&font);
                prev_font = Some(font_key);
            }

            // Update color when it changes
            let color = [t.r, t.g, t.b, t.a];
            if prev_color != Some(color) {
                self.ctx.set_fill_style_str(&rgba(&color));
                prev_color = Some(color);
            }

            self.ctx.set_text_align("center");
            if t.rotation_rad.abs() > f32::EPSILON {
                self.ctx.save();
                let _ = self.ctx.translate(t.x as f64, t.y as f64);
                let _ = self.ctx.rotate(t.rotation_rad as f64);
                let _ = self.ctx.fill_text(&t.text, 0.0, 0.0);
                self.ctx.restore();
            } else {
                let _ = self.ctx.fill_text(&t.text, t.x as f64, t.y as f64);
            }
        }
    }

    fn draw_drawing_geometry_with_font(&self, geom: &DrawingGeometry, font_family: &str) {
        for r in &geom.rects {
            if r.w <= 0.0 || r.h <= 0.0 {
                continue;
            }
            self.ctx.set_fill_style_str(&rgba(&[r.r, r.g, r.b, r.a]));
            self.ctx
                .fill_rect(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
        }

        for l in &geom.lines {
            self.ctx.set_stroke_style_str(&rgba(&[l.r, l.g, l.b, l.a]));
            self.ctx.set_line_width(l.width as f64);
            self.ctx.set_line_cap("round");
            self.ctx.set_line_join("round");

            if l.dash > 0.0 && l.gap > 0.0 {
                let _ = self.ctx.set_line_dash(&js_sys::Array::of2(
                    &JsValue::from(l.dash as f64),
                    &JsValue::from(l.gap as f64),
                ));
            } else {
                let _ = self.ctx.set_line_dash(&js_sys::Array::new());
            }

            let correction = if (l.width as i32) % 2 == 1 { 0.5 } else { 0.0 };
            self.ctx.begin_path();
            self.ctx
                .move_to(l.x0 as f64 + correction, l.y0 as f64 + correction);
            self.ctx
                .line_to(l.x1 as f64 + correction, l.y1 as f64 + correction);
            self.ctx.stroke();
        }
        let _ = self.ctx.set_line_dash(&js_sys::Array::new());

        for t in &geom.texts {
            let font = if t.italic {
                format!("italic {} {}px {}", t.font_weight, t.font_size, font_family)
            } else {
                format!("{} {}px {}", t.font_weight, t.font_size, font_family)
            };
            self.ctx.save();
            self.ctx.set_font(&font);
            self.ctx.set_fill_style_str(&rgba(&[t.r, t.g, t.b, t.a]));
            self.ctx.set_text_align(t.align.as_canvas_str());
            self.ctx.set_text_baseline(t.vertical_align.as_canvas_str());
            if t.rotation_rad.abs() > f32::EPSILON {
                let _ = self.ctx.translate(t.x as f64, t.y as f64);
                let _ = self.ctx.rotate(t.rotation_rad as f64);
                let _ = self.ctx.fill_text(&t.text, 0.0, 0.0);
            } else {
                let _ = self.ctx.fill_text(&t.text, t.x as f64, t.y as f64);
            }
            self.ctx.restore();
        }

        for a in &geom.anchors {
            self.ctx.set_fill_style_str(&rgba(&a.fill));
            self.ctx.begin_path();
            let _ = self
                .ctx
                .arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            self.ctx.fill();

            self.ctx.set_stroke_style_str(&rgba(&a.border));
            self.ctx.set_line_width(a.border_width);
            self.ctx.begin_path();
            let _ = self
                .ctx
                .arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
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

    fn draw_bottom_drawings(&mut self, ctx: &RenderContext) -> Result<(), String> {
        for geom in ctx.bottom_drawings {
            self.draw_drawing_geometry_with_font(geom, &ctx.style.font_family);
        }
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
                    ctx.time_scale,
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
                let area_segments = geometry_generator::generate_area_segments(
                    ctx.bars,
                    ctx.time_scale,
                    ctx.viewport,
                    ctx.main_chart_options.area_top_color,
                    ctx.main_chart_options.area_bottom_color,
                    pane_w,
                    pane_h,
                );
                let line_segments = geometry_generator::generate_main_area_line_segments(
                    ctx.bars,
                    ctx.time_scale,
                    ctx.viewport,
                    ctx.main_chart_options.line_color,
                    line_width,
                    pane_w,
                    pane_h,
                );
                self.draw_area_segments(&area_segments);
                self.draw_line_segments(&line_segments);
            }
            MainChartType::Candlestick => {
                let bullish_border = ctx
                    .main_chart_options
                    .up_border_color
                    .unwrap_or(ctx.style.wick_bullish_color);
                let bearish_border = ctx
                    .main_chart_options
                    .down_border_color
                    .unwrap_or(ctx.style.wick_bearish_color);
                let rects = geometry_generator::generate_candle_rects(
                    ctx.bars,
                    ctx.time_scale,
                    ctx.viewport,
                    ctx.style,
                    bullish_border,
                    bearish_border,
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
                    ctx.time_scale,
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
                    ctx.time_scale,
                    ctx.viewport,
                    ctx.style,
                    pane_w,
                    pane_h,
                    ctx.h_pixel_ratio,
                    ctx.v_pixel_ratio,
                );
                self.draw_rects(&rects);
            }
            MainChartType::Footprint => {
                // Rects are pre-computed by ChartEngine::render().
                // Texts are rendered by the overlay layer (render_footprint_texts).
                //
                // If footprint data is not loaded yet, fall back to candles so
                // switching chart type never blanks the pane.
                if ctx.footprint_base_rects.is_empty()
                    && ctx.footprint_gradient_rects.is_empty()
                    && ctx.footprint_overlay_rects.is_empty()
                {
                    let bullish_border = ctx
                        .main_chart_options
                        .up_border_color
                        .unwrap_or(ctx.style.wick_bullish_color);
                    let bearish_border = ctx
                        .main_chart_options
                        .down_border_color
                        .unwrap_or(ctx.style.wick_bearish_color);
                    let rects = geometry_generator::generate_candle_rects(
                        ctx.bars,
                        ctx.time_scale,
                        ctx.viewport,
                        ctx.style,
                        bullish_border,
                        bearish_border,
                        pane_w,
                        pane_h,
                        ctx.h_pixel_ratio,
                        ctx.v_pixel_ratio,
                    );
                    self.draw_rects(&rects);
                } else {
                    self.draw_rects(ctx.footprint_base_rects);
                    self.draw_horizontal_gradient_rects(ctx.footprint_gradient_rects);
                    self.draw_rects(ctx.footprint_overlay_rects);
                }
            }
        }

        Ok(())
    }

    fn draw_volume(&mut self, ctx: &RenderContext) -> Result<(), String> {
        // Footprint chart integrates volume directly into the cells — skip separate volume bars.
        if ctx.main_chart_type == crate::core::chart_type::MainChartType::Footprint {
            return Ok(());
        }
        let pane_w = self.physical_width as f64;
        let pane_h = self.physical_height as f64;
        let vol_rects = geometry_generator::generate_volume_rects(
            ctx.bars,
            ctx.time_scale,
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

        // Generate smooth line segments + fill rects for overlays
        let (line_segments, fill_rects) =
            crate::core::renderer::line_generator::generate_all_overlay_geometry(
                ctx.series,
                ctx.viewport,
                ctx.time_scale.timestamps(),
                pane_w,
                pane_h,
                ctx.h_pixel_ratio,
                ctx.v_pixel_ratio,
            );

        // Overlay series keep legacy fill->line order.
        self.draw_rects(&fill_rects);
        self.draw_line_segments(&line_segments);

        let indicator_commands =
            crate::core::renderer::line_generator::generate_indicator_instruction_commands(
                ctx.indicator_draw_instructions,
                ctx.viewport,
                ctx.time_scale.timestamps(),
                pane_w,
                pane_h,
                ctx.h_pixel_ratio,
                ctx.v_pixel_ratio,
            );
        let mut pending_rects = Vec::new();
        let mut pending_lines = Vec::new();
        for command in indicator_commands {
            match command.primitive {
                crate::core::renderer::line_generator::IndicatorGeometryPrimitive::Rect(rect) => {
                    if !pending_lines.is_empty() {
                        self.draw_line_segments(&pending_lines);
                        pending_lines.clear();
                    }
                    pending_rects.push(rect);
                }
                crate::core::renderer::line_generator::IndicatorGeometryPrimitive::Line(
                    segment,
                ) => {
                    if !pending_rects.is_empty() {
                        self.draw_rects(&pending_rects);
                        pending_rects.clear();
                    }
                    pending_lines.push(segment);
                }
            }
        }
        if !pending_rects.is_empty() {
            self.draw_rects(&pending_rects);
        }
        if !pending_lines.is_empty() {
            self.draw_line_segments(&pending_lines);
        }

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
