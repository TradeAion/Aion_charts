//! Extracted render pipeline — a free function callable from both `AxiusCharts::render()`
//! and the auto-render RAF loop.
//!
//! This module exists because the RAF closure cannot call `&mut self` methods,
//! but needs access to the full render pipeline. By extracting the render body
//! into a free function that takes `SharedInner` + `Rc<Cell<bool>>` +
//! `Rc<RefCell<EventEmitter>>`, both the public `render()` method and the
//! RAF closure can call the same code without duplication.

use std::rc::Rc;

use crate::chart_inner::{ChartInner, SharedInner};
use crate::event_emitter::chart_event_to_js;
use crate::subpane::IndicatorConfig;
use crate::{get_dpr, sync_widget_sizes, RenderInvalidation};
use axiuscharts::tick_marks;

/// Execute the full render pipeline.
///
/// This is the single source of truth for rendering. Called from:
/// - `AxiusCharts::render()` (public API, manual mode)
/// - Auto-render RAF closure (automatic mode)
///
/// Uses `try_borrow_mut()` to gracefully skip a frame if `inner` is already
/// borrowed (e.g., during an event handler callback).
pub(crate) fn do_render_frame(inner: &SharedInner, dirty: &Rc<RenderInvalidation>) -> bool {
    let Ok(mut s) = inner.try_borrow_mut() else {
        return false;
    };

    // Detect DPR changes (browser zoom) that may not trigger ResizeObserver
    let current_dpr = get_dpr();
    if (current_dpr - s.engine.dpr).abs() > 0.001 {
        s.engine.dpr = current_dpr;
        sync_widget_sizes(&mut *s, current_dpr, true);
        for subpane in s.subpanes.iter_mut() {
            subpane.resize(current_dpr);
        }
    }

    let subpane_animation_active = s.subpanes.iter().any(|subpane| {
        let scroll = subpane.scroll_state.borrow();
        scroll.dragging || scroll.animation.is_active()
    });
    let needs_continuous_render = s.interaction.is_gliding
        || subpane_animation_active
        || (s.replay_active && s.replay_playing);
    if !dirty.get() && !needs_continuous_render {
        return false;
    }

    let dpr = s.engine.dpr;
    let anim_time = js_sys::Date::now(); // For pulsing animations

    if let Err(err) = s.replay_tick(anim_time) {
        log::warn!("replay tick failed: {}", err);
    }

    let mut replay_crosshair_style_override: Option<axiuscharts::ChartStyle> = None;
    if s.replay_active && s.replay_trim_edit_mode {
        let mut replay_style = s.engine.style.clone();

        // Trim-edit mode only: vertical trim guide, hide horizontal guide.
        replay_style.crosshair_horz_line.visible = false;
        replay_style.crosshair_horz_line.label_visible = false;

        replay_style.crosshair_vert_line.style = axiuscharts::LineStyle::Solid;
        replay_style.crosshair_vert_line.color = [0.18, 0.72, 0.30, 0.95];

        if s.replay_crosshair_over_empty_area() {
            replay_style.crosshair_vert_line.color[3] *= 0.35;
        }
        replay_crosshair_style_override = Some(replay_style);
    }

    let (pane_css_w_pre, pane_css_h_pre) = s.layout.pane_css_size();
    let before_start = s.engine.viewport.start_bar;
    let before_end = s.engine.viewport.end_bar;

    // Update main chart kinetic scrolling
    {
        let ChartInner {
            ref mut interaction,
            ref mut engine,
            ..
        } = *s;
        interaction.update_gliding(
            pane_css_w_pre,
            pane_css_h_pre,
            &mut engine.viewport,
            &engine.bars,
            engine.time_scale.len(),
        );
    }

    // Update subpane kinetic scrolling (shares time axis with main chart)
    {
        let bar_len = s.engine.bars.len();
        let pane_css_w_for_kinetic = pane_css_w_pre;

        let mut total_kinetic_delta_px = 0.0;
        for subpane in s.subpanes.iter() {
            if let Some(delta_px) = subpane.update_kinetic(anim_time) {
                total_kinetic_delta_px += delta_px;
            }
        }

        if total_kinetic_delta_px.abs() > 0.001 && pane_css_w_for_kinetic > 0.0 {
            let bar_range = s.engine.viewport.end_bar - s.engine.viewport.start_bar;
            let delta_bars = -total_kinetic_delta_px * bar_range / pane_css_w_for_kinetic;
            s.engine.viewport.pan_clamped(delta_bars, bar_len);
            s.engine.auto_fit_price_if_unlocked();
        }
    }

    s.engine
        .emit_visible_range_change_if_changed(before_start, before_end);

    // 1. Provisional ticks from current pane size (for axis-width estimation).
    let mut pane_pw = s.engine.viewport.width as f64;
    let mut pane_ph = s.engine.viewport.height as f64;
    if pane_pw <= 0.0 || pane_ph <= 0.0 {
        return false;
    }
    let candle_ph = pane_ph * s.engine.viewport.candle_height_frac();
    let provisional_y_ticks =
        tick_marks::compute_y_ticks(&s.engine.viewport, pane_ph, candle_ph, dpr, &s.engine.style);

    // 2. Measure price axis width using full label set, then update grid layout.
    {
        let ChartInner {
            ref mut price_axis_renderer,
            ref engine,
            ref mut layout,
            ref mut subpanes,
            ..
        } = *s;
        let mut max_text_w_phys = price_axis_renderer.measure_optimal_width(
            &engine.style,
            &provisional_y_ticks,
            &engine.series,
            &engine.bars,
            engine.main_chart_options.chart_type,
            &engine.footprint_data,
            &engine.main_chart_options.footprint,
            &engine.price_lines,
            &engine.viewport,
            pane_ph,
            engine.v_pixel_ratio,
        );

        // Include subpane tick labels in width measurement so they don't clip
        for subpane in subpanes.iter_mut() {
            // Auto-scale subpane to visible range before measuring
            if subpane.config.auto_scale {
                subpane
                    .auto_scale_price_visible(engine.viewport.start_bar, engine.viewport.end_bar);
            }
            let sp_w = subpane.measure_axis_label_width(&engine.style);
            if sp_w > max_text_w_phys {
                max_text_w_phys = sp_w;
            }
        }

        let max_text_w_css = max_text_w_phys / dpr;
        let price_axis_css_w = engine.style.price_axis_width(max_text_w_css);
        let time_axis_css_h = engine.style.time_axis_height();

        // 3. Update CSS grid layout (subpane-aware — keeps time axis visible)
        if subpanes.is_empty() {
            layout.update_axis_sizes(price_axis_css_w, time_axis_css_h);
        } else {
            let heights: Vec<f64> = subpanes.iter().map(|sp| sp.get_height()).collect();
            layout.update_axis_sizes_with_subpanes(price_axis_css_w, time_axis_css_h, &heights);
        }
    }

    // 3. Immediately synchronize canvas/renderer sizes with the new layout.
    sync_widget_sizes(&mut *s, dpr, true);

    // 4. Recompute pane dimensions + final ticks after layout sync.
    let (pane_css_w, pane_css_h) = s.layout.pane_css_size();
    pane_pw = s.engine.viewport.width as f64;
    pane_ph = s.engine.viewport.height as f64;
    if pane_pw <= 0.0 || pane_ph <= 0.0 {
        return false;
    }

    let y_ticks = {
        let candle_ph = pane_ph * s.engine.viewport.candle_height_frac();
        tick_marks::compute_y_ticks(&s.engine.viewport, pane_ph, candle_ph, dpr, &s.engine.style)
    };
    let x_ticks =
        tick_marks::compute_x_ticks(&s.engine.viewport, &s.engine.time_scale, pane_pw, dpr);
    let time_scale = s.engine.time_scale.clone();

    // 5. Generate drawing geometry (bottom = idle/non-hovered, top = hovered/active).
    let (mut base_drawings, mut top_drawings) = s.engine.drawings.generate_all_geometry(
        &s.engine.viewport,
        pane_css_w,
        pane_css_h,
        dpr,
        s.engine.h_pixel_ratio,
        s.engine.v_pixel_ratio,
    );

    let webgpu_backend = s.engine.renderer_name() == "webgpu";
    if webgpu_backend {
        // WebGPU idle drawings: promote fills and texts to the overlay so
        // they render above candles.  Lines stay in the bottom layer for
        // z-order behaviour.  Fill alpha is kept as-is so it matches the
        // hovered/active state (no visible opacity pop on hover).
        for geom in &mut base_drawings {
            if !geom.rects.is_empty() {
                let mut fill_only = geom.clone();
                fill_only.lines.clear();
                fill_only.texts.clear();
                fill_only.anchors.clear();
                top_drawings.push(fill_only);
                geom.rects.clear();
            }

            if geom.texts.is_empty() {
                continue;
            }
            let mut text_only = geom.clone();
            text_only.lines.clear();
            text_only.rects.clear();
            text_only.anchors.clear();
            top_drawings.push(text_only);
            geom.texts.clear();
        }
    }

    // 5b. Engine render — grid + bottom drawings + data series on pane base canvas.
    if let Err(e) = s
        .engine
        .render(&time_scale, &y_ticks, &x_ticks, &base_drawings)
    {
        log::warn!("render error: {}", e);
    }

    // 6. Render overlay, dashed series, price lines, last price lines, drawings, crosshair, markers
    {
        let ChartInner {
            ref mut overlay,
            ref engine,
            ref active_subpane_id,
            ref mut execution_mark_hit_areas,
            ref hovered_execution_mark_id,
            ref symbol,
            ref selected_execution_mark_id,
            ..
        } = *s;
        let dashed_on_overlay = engine.renderer_name() == "webgpu";
        let indicator_draw_instructions = engine.indicators.collect_sorted_draw_instructions();
        let main_crosshair = if active_subpane_id.is_some() && engine.crosshair.active {
            let mut ch = engine.crosshair;
            ch.y = -1.0;
            ch
        } else {
            engine.crosshair
        };
        let crosshair_style = replay_crosshair_style_override
            .as_ref()
            .unwrap_or(&engine.style);

        // Canvas2D path: base-layer drawings on base canvas, top-layer on overlay
        overlay.render_with_drawings(
            &main_crosshair,
            crosshair_style,
            &top_drawings,
            Some((&engine.bars, &engine.viewport, &time_scale)),
        );
        // Footprint text labels — rendered on overlay for both WebGPU and Canvas2D
        // so text always appears on top of the chart data.
        overlay.render_footprint_texts(&engine.footprint_texts, &engine.style);
        overlay.render_dashed_series(
            &engine.series,
            &engine.viewport,
            &time_scale,
            pane_pw,
            pane_ph,
            engine.v_pixel_ratio,
            dashed_on_overlay,
        );
        overlay.render_price_lines(
            &engine.price_lines,
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
        );
        overlay.render_last_price_lines(
            &engine.series,
            &engine.bars,
            &time_scale,
            engine.main_chart_options.chart_type,
            &engine.footprint_data,
            &engine.main_chart_options.footprint,
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
            engine.h_pixel_ratio,
            engine.v_pixel_ratio,
            anim_time,
        );
        overlay.render_asset_name_chip(
            symbol,
            &engine.bars,
            engine.main_chart_options.chart_type,
            &engine.footprint_data,
            &engine.main_chart_options.footprint,
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
            engine.v_pixel_ratio,
        );
        overlay.render_markers(
            &engine.markers,
            &engine.bars,
            &time_scale,
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
        );
        overlay.render_indicator_labels(
            &indicator_draw_instructions,
            &time_scale,
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
        );
        overlay.render_crosshair_markers(
            &main_crosshair,
            &engine.series,
            &engine.bars,
            &time_scale,
            &engine.viewport,
            crosshair_style,
            pane_css_w,
            pane_css_h,
        );
        // Render execution marks after the crosshair so hover/click locators
        // stay visible at the exact execution point instead of being masked by
        // the dashed crosshair overlay.
        *execution_mark_hit_areas = overlay.render_execution_marks(
            &engine.execution_marks,
            &engine.bars,
            &time_scale,
            &engine.viewport,
            &engine.style,
            engine.execution_mark_text_visible(),
            hovered_execution_mark_id.as_deref(),
            selected_execution_mark_id.as_deref(),
            pane_css_w,
            pane_css_h,
        );
        if let Some(ref selected_id) = selected_execution_mark_id {
            overlay.render_selected_execution_locators(
                &engine.execution_marks,
                Some(selected_id.as_str()),
                &engine.viewport,
                &engine.style,
                pane_css_w,
                pane_css_h,
            );
        }
    }

    // 7. Price axis — base (ticks + labels) + last price labels + price line labels + top (crosshair label)
    {
        let ChartInner {
            ref mut price_axis_renderer,
            ref engine,
            ref active_subpane_id,
            ..
        } = *s;
        price_axis_renderer.render_base(&engine.style, &y_ticks);
        price_axis_renderer.render_last_price_labels(
            &engine.series,
            &engine.bars,
            engine.main_chart_options.chart_type,
            &engine.footprint_data,
            &engine.main_chart_options.footprint,
            &engine.viewport,
            &engine.style,
            pane_ph,
            engine.v_pixel_ratio,
        );
        price_axis_renderer.render_price_line_labels(
            &engine.price_lines,
            &engine.viewport,
            &engine.style,
            pane_ph,
        );
        let main_ch = if active_subpane_id.is_some() && engine.crosshair.active {
            let mut ch = engine.crosshair;
            ch.y = -1.0;
            ch
        } else {
            engine.crosshair
        };
        let crosshair_style = replay_crosshair_style_override
            .as_ref()
            .unwrap_or(&engine.style);
        price_axis_renderer.render_top(&main_ch, &engine.viewport, crosshair_style, pane_ph);
    }

    // 8. Time axis — base (ticks + labels) + top (crosshair label)
    {
        let ChartInner {
            ref mut time_axis_renderer,
            ref engine,
            ..
        } = *s;
        let crosshair_style = replay_crosshair_style_override
            .as_ref()
            .unwrap_or(&engine.style);
        time_axis_renderer.render_base(&engine.style, &x_ticks, pane_pw);
        time_axis_renderer.render_top(
            &engine.crosshair,
            &engine.bars,
            &engine.series,
            &engine.time_scale,
            &engine.viewport,
            crosshair_style,
            pane_css_w,
        );
    }

    // 9. Indicator sub-panes — update data from studies and render
    {
        let study_updates: Vec<(u32, Vec<axiuscharts::core::series::LineDataArray>, String)> = {
            let ChartInner {
                ref subpanes,
                ref engine,
                ..
            } = *s;
            subpanes
                .iter()
                .filter_map(|subpane| {
                    if let Some(study) = engine
                        .studies
                        .get_study(axiuscharts::StudyId(subpane.study_id))
                    {
                        let mut data = Vec::new();
                        for i in 0..study.outputs.len() {
                            if let Some(output) = study.get_output(i) {
                                data.push(output.data.clone());
                            }
                        }
                        Some((subpane.id, data, subpane.indicator_type.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        };

        for (pane_id, data, indicator_type) in study_updates {
            if let Some(subpane) = s.subpanes.iter_mut().find(|sp| sp.id == pane_id) {
                if !data.is_empty() {
                    let config = IndicatorConfig::for_type(&indicator_type);
                    let colors: Vec<[f32; 4]> = data
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            config.colors.get(i).copied().unwrap_or(
                                axiuscharts::ThemeConfig::default()
                                    .indicator_palette
                                    .fallback,
                            )
                        })
                        .collect();
                    subpane.set_data(data, colors);
                }
            }
        }

        let ChartInner {
            ref mut subpanes,
            ref engine,
            active_subpane_id: _,
            ..
        } = *s;
        let crosshair_x = if engine.crosshair.active {
            Some(engine.crosshair.x)
        } else {
            None
        };
        let crosshair_style = replay_crosshair_style_override
            .as_ref()
            .unwrap_or(&engine.style);
        for subpane in subpanes.iter_mut() {
            subpane.resize(dpr);
            let top_drawings =
                subpane.render(&engine.viewport, &time_scale, &engine.style, &x_ticks);

            subpane.clear_crosshair_overlay();
            subpane.render_top_drawings(&top_drawings);

            if let Some(x) = crosshair_x {
                subpane.render_crosshair_vert(x, crosshair_style);
            }
            subpane.render_crosshair_horiz(crosshair_style);
        }
    }

    // 10. Clear dirty flag + release borrow
    let keep_animating = s.interaction.is_gliding
        || s.subpanes.iter().any(|subpane| {
            let scroll = subpane.scroll_state.borrow();
            scroll.dragging || scroll.animation.is_active()
        })
        || (s.replay_active && s.replay_playing);
    drop(s);
    dirty.set(false);

    // 11. Flush events from core EventBus to JS callbacks
    if let Ok(mut s) = inner.try_borrow_mut() {
        let events: Vec<axiuscharts::ChartEvent> = s.engine.event_bus.drain().collect();
        drop(s);
        if !events.is_empty() {
            let mut emitter = dirty.event_emitter().borrow_mut();
            for event in &events {
                let js_event = chart_event_to_js(event);
                emitter.emit(event.name(), &js_event);
            }
        }
    }

    keep_animating
}
