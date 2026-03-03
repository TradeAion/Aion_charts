//! Extracted render pipeline — a free function callable from both `RayCore::render()`
//! and the auto-render RAF loop.
//!
//! This module exists because the RAF closure cannot call `&mut self` methods,
//! but needs access to the full render pipeline. By extracting the render body
//! into a free function that takes `SharedInner` + `Rc<Cell<bool>>` +
//! `Rc<RefCell<EventEmitter>>`, both the public `render()` method and the
//! RAF closure can call the same code without duplication.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::chart_inner::{ChartInner, SharedInner};
use crate::event_emitter::{chart_event_to_js, EventEmitter};
use crate::subpane::IndicatorConfig;
use crate::{get_dpr, sync_widget_sizes};
use raycore::tick_marks;

/// Execute the full render pipeline.
///
/// This is the single source of truth for rendering. Called from:
/// - `RayCore::render()` (public API, manual mode)
/// - Auto-render RAF closure (automatic mode)
///
/// Uses `try_borrow_mut()` to gracefully skip a frame if `inner` is already
/// borrowed (e.g., during an event handler callback).
pub(crate) fn do_render_frame(
    inner: &SharedInner,
    dirty: &Rc<Cell<bool>>,
    event_emitter: &Rc<RefCell<EventEmitter>>,
) {
    let Ok(mut s) = inner.try_borrow_mut() else {
        return;
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

    let dpr = s.engine.dpr;
    let anim_time = js_sys::Date::now(); // For pulsing animations

    let (pane_css_w_pre, pane_css_h_pre) = s.layout.pane_css_size();

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

    // 1. Provisional ticks from current pane size (for axis-width estimation).
    let mut pane_pw = s.engine.viewport.width as f64;
    let mut pane_ph = s.engine.viewport.height as f64;
    if pane_pw <= 0.0 || pane_ph <= 0.0 {
        return;
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
            &engine.price_lines,
            &engine.viewport,
            pane_ph,
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
    sync_widget_sizes(&mut *s, dpr, false);

    // 4. Recompute pane dimensions + final ticks after layout sync.
    let (pane_css_w, pane_css_h) = s.layout.pane_css_size();
    pane_pw = s.engine.viewport.width as f64;
    pane_ph = s.engine.viewport.height as f64;
    if pane_pw <= 0.0 || pane_ph <= 0.0 {
        return;
    }

    let y_ticks = {
        let candle_ph = pane_ph * s.engine.viewport.candle_height_frac();
        tick_marks::compute_y_ticks(&s.engine.viewport, pane_ph, candle_ph, dpr, &s.engine.style)
    };
    let x_ticks = tick_marks::compute_x_ticks(&s.engine.viewport, &s.engine.bars, pane_pw, dpr);

    // 5. Engine render — candles + volume on pane chart canvas
    if let Err(e) = s.engine.render(&y_ticks, &x_ticks) {
        log::warn!("render error: {}", e);
    }

    // 5b. Dashed line series — rendered via Canvas2D strokePath (not rects).
    let bar_ts: Vec<u64> = (0..s.engine.bars.len())
        .map(|i| s.engine.bars.timestamp(i))
        .collect();

    // 5c. Generate drawing geometry (base = Idle/Selected, top = Creating/Dragging)
    let (base_drawings, top_drawings) = s.engine.drawings.generate_all_geometry(
        &s.engine.viewport,
        pane_css_w,
        pane_css_h,
        dpr,
        s.engine.h_pixel_ratio,
        s.engine.v_pixel_ratio,
    );

    // 6. Render overlay, dashed series, price lines, last price lines, drawings, crosshair, markers
    {
        let ChartInner {
            ref mut overlay,
            ref engine,
            ref active_subpane_id,
            ..
        } = *s;
        let indicator_draw_instructions = engine.indicators.collect_sorted_draw_instructions();
        let main_crosshair = if active_subpane_id.is_some() && engine.crosshair.active {
            let mut ch = engine.crosshair;
            ch.y = -1.0;
            ch
        } else {
            engine.crosshair
        };
        // Canvas2D path: base-layer drawings on base canvas, top-layer on overlay
        overlay.render_with_drawings(
            &main_crosshair,
            &engine.style,
            &top_drawings,
            Some(&engine.bars),
        );
        overlay.render_dashed_series(
            &engine.series,
            &engine.viewport,
            &bar_ts,
            pane_pw,
            pane_ph,
            engine.v_pixel_ratio,
            false,
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
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
            anim_time,
        );
        overlay.render_markers(
            &engine.markers,
            &engine.bars,
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
        );
        overlay.render_base_drawings(&base_drawings);
        overlay.render_indicator_labels(
            &indicator_draw_instructions,
            &engine.bars,
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
        );
        overlay.render_crosshair_markers(
            &main_crosshair,
            &engine.series,
            &engine.bars,
            &bar_ts,
            &engine.viewport,
            &engine.style,
            pane_css_w,
            pane_css_h,
        );
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
            &engine.viewport,
            &engine.style,
            pane_ph,
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
        price_axis_renderer.render_top(&main_ch, &engine.viewport, &engine.style, pane_ph);
    }

    // 8. Time axis — base (ticks + labels) + top (crosshair label)
    {
        let ChartInner {
            ref mut time_axis_renderer,
            ref engine,
            ..
        } = *s;
        time_axis_renderer.render_base(&engine.style, &x_ticks, pane_pw);
        time_axis_renderer.render_top(
            &engine.crosshair,
            &engine.bars,
            &engine.viewport,
            &engine.style,
            pane_css_w,
        );
    }

    // 9. Indicator sub-panes — update data from studies and render
    {
        let study_updates: Vec<(u32, Vec<raycore::core::series::LineDataArray>, String)> = {
            let ChartInner {
                ref subpanes,
                ref engine,
                ..
            } = *s;
            subpanes
                .iter()
                .filter_map(|subpane| {
                    if let Some(study) =
                        engine.studies.get_study(raycore::StudyId(subpane.study_id))
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
                                raycore::ThemeConfig::default().indicator_palette.fallback,
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
        for subpane in subpanes.iter_mut() {
            subpane.resize(dpr);
            subpane.render(&engine.viewport, &engine.style, &x_ticks);

            let (base_drawings, top_drawings) = subpane.generate_drawing_geometry(&engine.viewport);
            let mut all_drawings = base_drawings;
            all_drawings.extend(top_drawings);
            subpane.render_drawings(&all_drawings);

            subpane.clear_crosshair_overlay();

            if let Some(x) = crosshair_x {
                subpane.render_crosshair_vert(x, &engine.style);
            }
            subpane.render_crosshair_horiz(&engine.style);
        }
    }

    // 10. Clear dirty flag + release borrow
    drop(s);
    dirty.set(false);

    // 11. Flush events from core EventBus to JS callbacks
    if let Ok(mut s) = inner.try_borrow_mut() {
        let events: Vec<raycore::ChartEvent> = s.engine.event_bus.drain().collect();
        drop(s);
        if !events.is_empty() {
            let mut emitter = event_emitter.borrow_mut();
            for event in &events {
                let js_event = chart_event_to_js(event);
                emitter.emit(event.name(), &js_event);
            }
        }
    }
}
