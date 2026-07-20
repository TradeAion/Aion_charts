//! `ChartInner` rendering: WebGPU/Canvas2D pane execution, axis overlay painting, backend
//! failover, and browser text measurement.

use super::*;

impl ChartInner {
    /// Reports the active pane backend for diagnostics and runtime-matrix tests.
    pub fn backend_kind(&self) -> String {
        if self.gfx.is_some() {
            "webgpu".into()
        } else {
            "canvas2d".into()
        }
    }

    pub fn render(&mut self) -> Result<(), JsValue> {
        // ---- layout (price axis width negotiated against the price labels) ----
        self.recompute_layout(false);

        // time tick marks: built once (needs &mut), shared by GPU grid + 2D labels
        let pixels_per_character = (FONT_SIZE + 4.0) * 5.0 / 8.0;
        let max_label_width = pixels_per_character * TICK_MARK_MAX_CHARS;
        let axis_ctx = &self.axis_ctx;
        let dpr = self.dpr;
        self.axis_frame = self.engine.build_axis_frame(max_label_width, |text| {
            measure_text_ctx(axis_ctx, dpr, text)
        });

        // ---- GPU: one scissored draw group per stacked pane ----
        // The headless engine owns chart geometry. The WASM host only adds browser-adapter
        // concerns such as crosshair interaction and text labels.
        self.engine.build_frame_into(&mut self.frame);

        if self
            .gfx
            .as_ref()
            .is_some_and(|gfx| gfx.device_lost.load(Ordering::Acquire))
        {
            self.activate_canvas2d("WebGPU device was lost");
        }

        let bg = Color::parse_css(&self.opts().layout.background.color)
            .unwrap_or(Color::rgb(0xff, 0xff, 0xff));
        let pane_outcome = if self.gfx.is_some() {
            let engine_frame = &self.frame;
            self.gpu_groups
                .resize_with(engine_frame.panes.len(), DrawGroup::default);
            self.gpu_groups.truncate(engine_frame.panes.len());
            for (group, pane_frame) in self.gpu_groups.iter_mut().zip(&engine_frame.panes) {
                group.scissor = Some(pane_frame.scissor);
                group.under_quads.clear();
                group.fill_tris.clear();
                group.stroke_tris.clear();
                group.quads.clear();
                group.tex_quads.clear();
                // Convert the shared frame only at the WebGPU backend boundary.
                geom_prims_to_tris(
                    &pane_frame.main,
                    &pane_frame.points,
                    &mut group.fill_tris,
                    &mut group.stroke_tris,
                );
                prims_to_instances(&pane_frame.under, &mut group.under_quads);
                prims_to_instances(&pane_frame.main, &mut group.quads);
            }
            let groups = &self.gpu_groups[..];
            let Some(gfx) = self.gfx.as_mut() else {
                return Err(JsValue::from_str("WebGPU state disappeared mid-render"));
            };
            gfx.msaa.ensure(
                &gfx.device,
                gfx.config.format,
                gfx.config.width,
                gfx.config.height,
            );

            let acquired = match gfx.surface.get_current_texture() {
                Ok(frame) => Ok(Some(frame)),
                Err(error) => match surface_error_action(&error) {
                    SurfaceErrorAction::Reconfigure => {
                        // Resize and suspend/resume can invalidate only the swapchain. Reconfigure
                        // and retry once; if that fails, the warm Canvas2D pane takes over.
                        gfx.surface.configure(&gfx.device, &gfx.config);
                        match gfx.surface.get_current_texture() {
                            Ok(frame) => Ok(Some(frame)),
                            Err(retry_error)
                                if surface_error_action(&retry_error)
                                    == SurfaceErrorAction::SkipFrame =>
                            {
                                Ok(None)
                            }
                            Err(retry_error) => Err(retry_error),
                        }
                    }
                    SurfaceErrorAction::SkipFrame => Ok(None),
                    SurfaceErrorAction::Fallback => Err(error),
                },
            };

            match acquired {
                Ok(Some(frame)) => {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let bg_clear = wgpu::Color {
                        r: bg.r() as f64 / 255.0,
                        g: bg.g() as f64 / 255.0,
                        b: bg.b() as f64 / 255.0,
                        a: 1.0,
                    };
                    render_frame(
                        &gfx.device,
                        &gfx.queue,
                        gfx.msaa.view(),
                        &view,
                        gfx.config.width,
                        gfx.config.height,
                        bg_clear,
                        &gfx.quad_renderer,
                        &gfx.tex_renderer,
                        &gfx.tri_renderer,
                        groups,
                    );
                    frame.present();
                    PaneRenderOutcome::Presented
                }
                Ok(None) => PaneRenderOutcome::Timeout,
                Err(error) => PaneRenderOutcome::Fallback(format!(
                    "WebGPU surface acquisition failed after recovery: {error}"
                )),
            }
        } else {
            PaneRenderOutcome::Canvas2d
        };

        match pane_outcome {
            PaneRenderOutcome::Presented => {}
            PaneRenderOutcome::Timeout => {
                // Keep the last complete frame. The next animation/input repaint retries.
                return Ok(());
            }
            PaneRenderOutcome::Fallback(reason) => {
                self.activate_canvas2d(&reason);
                self.render_canvas2d()?;
            }
            PaneRenderOutcome::Canvas2d => self.render_canvas2d()?,
        }

        self.draw_axes_2d(&self.axis_frame)?;
        Ok(())
    }

    // --- data / scale bookkeeping ---

    pub(super) fn compute_price_axis_width(&mut self, target: PriceScaleTarget) -> f64 {
        let axis_ctx = self.axis_ctx.clone();
        let dpr = self.dpr;
        self.engine
            .optimal_price_axis_width_for(target, |text| measure_text_ctx(&axis_ctx, dpr, text))
    }

    // ---- Canvas2D axis overlay ----

    fn draw_axes_2d(&self, axis_frame: &AxisFrame) -> Result<(), JsValue> {
        let ctx = &self.axis_ctx;
        let dpr = self.dpr;
        let bitmap_w = self.bitmap_w as f64;
        let bitmap_h = self.bitmap_h as f64;
        let pane_left = self.pane_left;
        let pane_w = self.pane_w;
        let pane_h = self.pane_h;

        ctx.clear_rect(0.0, 0.0, bitmap_w, bitmap_h);
        let border_w = 1f64.max(dpr.floor());

        ctx.set_fill_style_str(BORDER_CSS);
        if self.left_axis_w > 0.0 {
            ctx.fill_rect(
                (pane_left * dpr).round() - border_w,
                0.0,
                border_w,
                (pane_h * dpr).round(),
            );
        }
        if self.axis_w > 0.0 {
            ctx.fill_rect(
                ((pane_left + pane_w) * dpr).round(),
                0.0,
                border_w,
                (pane_h * dpr).round(),
            );
        }
        ctx.fill_rect(0.0, (pane_h * dpr).round(), bitmap_w, border_w);

        // separators between stacked panes (roadmap Phase B1): a border line at each pane boundary
        for separator in &axis_frame.separators {
            let y = (separator * dpr).round();
            ctx.fill_rect(
                (pane_left * dpr).round(),
                y,
                (pane_w * dpr).round(),
                (PANE_SEPARATOR * dpr).max(border_w),
            );
        }

        self.draw_axis_labels(axis_frame, dpr)?;
        Ok(())
    }

    fn draw_axis_labels(&self, axis_frame: &AxisFrame, dpr: f64) -> Result<(), JsValue> {
        let ctx = &self.axis_ctx;
        // Label backgrounds are bitmap-aligned geometry, matching LWC's bitmap-coordinate pass.
        for label in &axis_frame.labels {
            if let Some((x, y, w, h, color)) = label.background {
                ctx.set_fill_style_str(&color.to_hex());
                ctx.fill_rect(
                    (x * dpr).round(),
                    (y * dpr).round(),
                    (w * dpr).round(),
                    (h * dpr).round(),
                );
            }
        }

        // LWC draws glyphs in media-coordinate space: the context is scaled by DPR while the font
        // remains 12 CSS px. Using an independently hinted 12*dpr bitmap font is observably
        // different at fractional DPR even when every logical coordinate is identical.
        ctx.save();
        if let Err(error) = ctx.scale(dpr, dpr) {
            ctx.restore();
            return Err(error);
        }
        ctx.set_text_baseline("middle");
        let mut draw_result = Ok(());
        for label in &axis_frame.labels {
            ctx.set_font(&if label.bold {
                format!("bold {FONT_SIZE}px {FONT_FAMILY}")
            } else {
                format!("{FONT_SIZE}px {FONT_FAMILY}")
            });
            ctx.set_text_align(match label.align {
                AxisTextAlign::Left => "left",
                AxisTextAlign::Right => "right",
                AxisTextAlign::Center => "center",
            });
            ctx.set_fill_style_str(&label.color.to_hex());
            let metrics_text = match label.midpoint {
                AxisTextMidpoint::None => None,
                AxisTextMidpoint::Label => Some(label.text.as_str()),
                AxisTextMidpoint::StableTime => Some("Apr0"),
            };
            let y_mid_correction = metrics_text
                .and_then(|text| ctx.measure_text(text).ok())
                .map(|metrics| {
                    (metrics.actual_bounding_box_ascent() - metrics.actual_bounding_box_descent())
                        / 2.0
                })
                .unwrap_or(0.0);
            if let Err(error) = ctx.fill_text(&label.text, label.x, label.y + y_mid_correction) {
                draw_result = Err(error);
                break;
            }
        }
        ctx.restore();
        draw_result
    }

    /// Permanently switch this chart instance to its already-initialized Canvas2D pane.
    fn activate_canvas2d(&mut self, reason: &str) {
        if self.gfx.take().is_some() {
            set_backend_visibility(&self.gpu_pane, &self.fallback_pane, false);
            web_sys::console::warn_1(&format!("aion: {reason}; continuing with Canvas2D").into());
        }
    }

    /// Execute the exact same retained frame consumed by WebGPU through Canvas2D.
    pub(super) fn render_canvas2d(&self) -> Result<(), JsValue> {
        let ctx = &self.pane_ctx;
        let width = self.bitmap_w as f64;
        let height = self.bitmap_h as f64;
        ctx.clear_rect(0.0, 0.0, width, height);
        let bg = self.opts().layout.background.color;
        ctx.set_fill_style_str(&bg);
        ctx.fill_rect(0.0, 0.0, width, height);
        let mut target = crate::canvas2d_target::WasmCanvas2d::new(ctx);
        let viewport = CanvasViewport {
            width: width as f32,
            height: height as f32,
        };
        for pane in &self.frame.panes {
            target.save();
            let [x, y, w, h] = pane.scissor;
            target.clip_rect(x as f32, y as f32, w as f32, h as f32);
            execute_canvas2d(&pane.under, &pane.points, &mut target, viewport);
            execute_canvas2d(&pane.main, &pane.points, &mut target, viewport);
            target.restore();
        }
        Ok(())
    }
}

fn measure_text_ctx(ctx: &CanvasRenderingContext2d, dpr: f64, text: &str) -> f64 {
    ctx.set_font(&format!("{}px {FONT_FAMILY}", FONT_SIZE * dpr));
    ctx.measure_text(text).map(|m| m.width()).unwrap_or(0.0) / dpr
}
