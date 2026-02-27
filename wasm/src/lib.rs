//! RayCore WASM bindings — LWC-style widget-based chart library.
//!
//! Architecture (matches LWC):
//!   - WidgetLayout creates CSS-grid DOM: [pane|price_axis] / [time_axis]
//!   - Each widget has its own DOM container + canvases + event handlers
//!   - InteractionHandler processes per-widget events (zone from DOM, not pixel math)
//!   - ChartEngine renders the pane only; axis renderers are separate
//!
//! Public WASM API:
//!   RayCore.create_chart(container, options)  → sets up everything, attaches all events
//!   core.set_data_arrays(open, high, ...)     → load bar data
//!   core.render()                             → draw one frame (call from RAF)
//!   core.dispose()                            → detach events, cleanup
//!
//! Module structure:
//!   - chart_inner: Internal state (ChartInner) and helper methods
//!   - canvas_manager: DOM layout and canvas management (WidgetLayout)
//!   - subpane: Indicator subpane management

use raycore::{
    generate_sample_data, AreaSeriesOptions, Bar, BarSeriesOptions,
    BaselineSeriesOptions, Canvas2DRenderer, ChartEngine, ChartGroup as NativeChartGroup,
    ChartPaneId, ChartStyle, CrosshairMagnetMode, CrosshairSnapshot, DataRange, GpuContext,
    HistogramPoint, HistogramSeriesOptions, HitZone, InteractionHandler, LinePoint,
    LineSeriesOptions, LineStyle, MarkerPosition, MarkerShape, OhlcPoint, OverlayRenderer,
    PriceAxisRenderer, PriceLineOptions, RendererBackend, SeriesId, SeriesMarker, TimeAxisRenderer,
    TimeRange, Viewport, WgpuRenderer,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod canvas_manager;
mod chart_inner;
mod event_emitter;
mod render_frame;
mod subpane;
mod utils;
mod workspace;

use canvas_manager::WidgetLayout;
use chart_inner::{
    event_css_pos, wheel_css_pos, ChartInner, EventListenerRegistry, ExactPixelSizes, SharedInner,
};
use event_emitter::EventEmitter;
use subpane::{IndicatorConfig, PaneHeightCoordinator, SubPane, SubPaneSeparatorStyle};

fn init_logging() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
}

fn webgpu_available() -> bool {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return false,
    };
    js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("gpu"))
        .map(|v| !v.is_undefined() && !v.is_null())
        .unwrap_or(false)
}

fn get_dpr() -> f64 {
    web_sys::window()
        .map(|w| w.device_pixel_ratio())
        .unwrap_or(1.0)
        .max(1.0)
}

fn has_exact_widget_sizes(s: &ChartInner) -> bool {
    let es = s.exact_sizes;
    es.available
        && es.pane_pw > 0
        && es.pane_ph > 0
        && es.price_axis_pw > 0
        && es.price_axis_ph > 0
        && es.time_axis_pw > 0
        && es.time_axis_ph > 0
        && es.corner_stub_pw > 0
        && es.corner_stub_ph > 0
}

/// Synchronize all widget canvases + renderers from current layout sizes.
///
/// If `prefer_exact` is true and device-pixel-content-box sizes are available,
/// exact bitmap sizes are used; otherwise it falls back to `round(css * dpr)`.
fn sync_widget_sizes(s: &mut ChartInner, dpr: f64, prefer_exact: bool) {
    if prefer_exact && has_exact_widget_sizes(s) {
        let es = s.exact_sizes;

        let (pcw, pch) = s.layout.pane_css_size();
        let (acw, ach) = s.layout.price_axis_css_size();
        let (tcw, tch) = s.layout.time_axis_css_size();
        let (scw, sch) = s.layout.corner_stub_css_size();

        s.layout.resize_pane_exact(es.pane_pw, es.pane_ph, pcw, pch);
        s.layout
            .resize_price_axis_exact(es.price_axis_pw, es.price_axis_ph, acw, ach);
        s.layout
            .resize_time_axis_exact(es.time_axis_pw, es.time_axis_ph, tcw, tch);
        s.layout
            .resize_corner_stub_exact(es.corner_stub_pw, es.corner_stub_ph, scw, sch);

        let h_ratio = if pcw > 0.0 {
            es.pane_pw as f64 / pcw
        } else {
            dpr
        };
        let v_ratio = if pch > 0.0 {
            es.pane_ph as f64 / pch
        } else {
            dpr
        };
        s.engine.h_pixel_ratio = h_ratio;
        s.engine.v_pixel_ratio = v_ratio;

        s.engine.resize(es.pane_pw.max(1), es.pane_ph.max(1), dpr);
        s.overlay.resize(es.pane_pw.max(1), es.pane_ph.max(1), dpr);
        s.price_axis_renderer
            .resize(es.price_axis_pw.max(1), es.price_axis_ph.max(1), dpr);
        s.time_axis_renderer
            .resize(es.time_axis_pw.max(1), es.time_axis_ph.max(1), dpr);
        return;
    }

    s.layout.resize_all_canvases(dpr);
    s.engine.h_pixel_ratio = dpr;
    s.engine.v_pixel_ratio = dpr;

    let (pw, ph) = s.layout.pane_css_size();
    let ppw = (pw * dpr).round() as u32;
    let pph = (ph * dpr).round() as u32;
    s.engine.resize(ppw.max(1), pph.max(1), dpr);
    s.overlay.resize(ppw.max(1), pph.max(1), dpr);

    let (aw, ah) = s.layout.price_axis_css_size();
    s.price_axis_renderer
        .resize((aw * dpr).round() as u32, (ah * dpr).round() as u32, dpr);
    let (tw, th) = s.layout.time_axis_css_size();
    s.time_axis_renderer
        .resize((tw * dpr).round() as u32, (th * dpr).round() as u32, dpr);
}

fn with_crosshair_lines_mut<F>(style: &mut ChartStyle, target: &str, mut f: F)
where
    F: FnMut(&mut raycore::core::renderer::traits::CrosshairLineStyle),
{
    match target {
        "vert" | "vertical" => f(&mut style.crosshair_vert_line),
        "horz" | "horizontal" => f(&mut style.crosshair_horz_line),
        _ => {
            f(&mut style.crosshair_vert_line);
            f(&mut style.crosshair_horz_line);
        }
    }
}

fn parse_crosshair_mode(mode: &str) -> raycore::CrosshairMode {
    match mode {
        "normal" => raycore::CrosshairMode::Normal,
        "magnet_ohlc" | "ohlc" => raycore::CrosshairMode::MagnetOHLC,
        "magnet" => raycore::CrosshairMode::Magnet,
        _ => raycore::CrosshairMode::Normal,
    }
}

fn crosshair_mode_key(mode: raycore::CrosshairMode) -> &'static str {
    match mode {
        raycore::CrosshairMode::Normal => "normal",
        raycore::CrosshairMode::Magnet => "magnet",
        raycore::CrosshairMode::MagnetOHLC => "magnet_ohlc",
    }
}

fn js_err(message: impl Into<String>) -> JsValue {
    JsValue::from_str(&message.into())
}

/// Get a property from a JS object, returning None if undefined/null or if the object is not valid.
fn js_get(obj: &JsValue, key: &str) -> Option<JsValue> {
    if obj.is_undefined() || obj.is_null() {
        return None;
    }
    js_sys::Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
}

/// Get a string property from a JS object.
fn js_get_str(obj: &JsValue, key: &str) -> Option<String> {
    js_get(obj, key).and_then(|v| v.as_string())
}

/// Get a f64 property from a JS object.
fn js_get_f64(obj: &JsValue, key: &str) -> Option<f64> {
    js_get(obj, key).and_then(|v| v.as_f64())
}

/// Get a bool property from a JS object.
fn js_get_bool(obj: &JsValue, key: &str) -> Option<bool> {
    js_get(obj, key).and_then(|v| v.as_bool())
}

fn ensure_equal_len(name_a: &str, len_a: usize, name_b: &str, len_b: usize) -> Result<(), JsValue> {
    if len_a != len_b {
        Err(js_err(format!(
            "{} and {} length mismatch: {} != {}",
            name_a, name_b, len_a, len_b
        )))
    } else {
        Ok(())
    }
}

fn ensure_finite_slice(name: &str, values: &[f32]) -> Result<(), JsValue> {
    if let Some((idx, value)) = values.iter().enumerate().find(|(_, v)| !v.is_finite()) {
        return Err(js_err(format!(
            "{} contains non-finite value at index {}: {}",
            name, idx, value
        )));
    }
    Ok(())
}

fn ensure_finite_fields(ctx: &str, fields: &[(&str, f32)]) -> Result<(), JsValue> {
    if let Some((name, _)) = fields.iter().find(|(_, v)| !v.is_finite()) {
        return Err(js_err(format!("{}: {} must be finite", ctx, name)));
    }
    Ok(())
}

#[wasm_bindgen]
pub struct RayCore {
    inner: SharedInner,
    symbol: String,
    interval: String,
    _closures: Vec<Closure<dyn FnMut(web_sys::Event)>>,
    _wheel_closures: Vec<Closure<dyn FnMut(web_sys::WheelEvent)>>,
    _touch_closures: Vec<Closure<dyn FnMut(web_sys::TouchEvent)>>,
    _resize_closure: Option<Closure<dyn FnMut(js_sys::Array)>>,
    _resize_observer: Option<web_sys::ResizeObserver>,
    /// Long-press timer ID (from setTimeout), shared with closures.
    _long_press_timer: Rc<RefCell<Option<i32>>>,
    /// Last touch-tap time for double-tap detection.
    _last_tap_time: Rc<RefCell<f64>>,
    /// Registry for tracking event listeners for cleanup.
    _event_registry: EventListenerRegistry,
    /// JS event emitter for on/off/once callbacks (Rc for RAF closure sharing).
    event_emitter: Rc<RefCell<EventEmitter>>,
    /// Active theme configuration.
    theme_config: raycore::ThemeConfig,
    /// Whether auto-render (internal RAF loop) is active.
    auto_render: bool,
    /// RAF closure slot for auto-render mode. Stored as Rc so the closure
    /// can reference the same slot when rescheduling itself each frame.
    _raf_closure: Option<Rc<RefCell<Option<Closure<dyn FnMut()>>>>>,
    /// Current RAF ID for cancellation.
    _raf_id: Rc<Cell<i32>>,
    /// Dirty flag — set on any mutation, cleared after render.
    dirty: Rc<Cell<bool>>,
}

#[wasm_bindgen]
impl RayCore {
    /// Create with a specific renderer backend ("webgpu" or "canvas2d").
    pub async fn create_with(container_id: &str, renderer: &str) -> Result<RayCore, JsValue> {
        init_logging();

        let layout = WidgetLayout::new(container_id)?;
        let dpr = get_dpr();

        // Set initial axis sizes so CSS grid can compute pane dimensions
        let style = raycore::ChartStyle::default();
        let time_axis_h = style.time_axis_height();
        let initial_price_w = 50.0; // will be recalculated on first render
        layout.update_axis_sizes(initial_price_w, time_axis_h);

        // Force layout reflow so pane dimensions are available
        let _ = layout.pane_css_size();

        // Resize canvases to computed sizes
        layout.resize_all_canvases(dpr);

        let (pane_css_w, pane_css_h) = layout.pane_css_size();
        let pane_pw = (pane_css_w * dpr).round() as u32;
        let pane_ph = (pane_css_h * dpr).round() as u32;

        log::info!(
            "RayCore: creating '{}' — pane CSS {}x{}, physical {}x{}, dpr={}",
            renderer,
            pane_css_w,
            pane_css_h,
            pane_pw,
            pane_ph,
            dpr
        );

        // Create pane renderer backend (only for the pane/chart canvas)
        let backend = match renderer {
            "webgpu" => {
                match GpuContext::new(
                    wgpu::SurfaceTarget::Canvas(layout.pane.chart.clone()),
                    pane_pw.max(1),
                    pane_ph.max(1),
                )
                .await
                {
                    Ok(gpu) => {
                        log::info!("WebGPU adapter: {:?}", gpu.format);
                        RendererBackend::Wgpu(WgpuRenderer::new(gpu))
                    }
                    Err(e) => {
                        log::warn!("WebGPU unavailable: {}. Falling back to Canvas2D.", e);
                        let r = Canvas2DRenderer::new(layout.pane.chart.clone(), dpr)
                            .map_err(|e| JsValue::from_str(&e))?;
                        RendererBackend::Canvas2D(r)
                    }
                }
            }
            _ => {
                let r = Canvas2DRenderer::new(layout.pane.chart.clone(), dpr)
                    .map_err(|e| JsValue::from_str(&e))?;
                RendererBackend::Canvas2D(r)
            }
        };

        // Pane overlay renderer (also gets reference to base chart canvas for base-layer drawings)
        let mut overlay = OverlayRenderer::new(layout.pane.top.clone(), dpr)
            .map_err(|e| JsValue::from_str(&e))?;
        let _ = overlay.set_base_canvas(layout.pane.chart.clone());

        // Axis renderers (each with base + top canvas pair)
        let price_axis_renderer = PriceAxisRenderer::new(
            layout.price_axis.base.clone(),
            layout.price_axis.top.clone(),
            dpr,
        )
        .map_err(|e| JsValue::from_str(&e))?;

        let time_axis_renderer = TimeAxisRenderer::new(
            layout.time_axis.base.clone(),
            layout.time_axis.top.clone(),
            dpr,
        )
        .map_err(|e| JsValue::from_str(&e))?;

        // Engine only manages the pane
        let engine = ChartEngine::new(backend, pane_pw.max(1), pane_ph.max(1), dpr);
        let interaction = InteractionHandler::new();

        log::info!("RayCore initialized: {}", engine.renderer_name());

        // Initialize pane height coordinator with main pane height
        let pane_coordinator = PaneHeightCoordinator::new(pane_css_h);

        let inner = Rc::new(RefCell::new(ChartInner {
            engine,
            overlay,
            price_axis_renderer,
            time_axis_renderer,
            layout,
            interaction,
            exact_sizes: ExactPixelSizes::default(),
            subpanes: Vec::new(),
            next_subpane_id: 1,
            active_subpane_id: None,
            pane_coordinator,
            subpane_separator_style: SubPaneSeparatorStyle::from_chart_style(&style),
        }));

        let mut closures: Vec<Closure<dyn FnMut(web_sys::Event)>> = Vec::new();
        let mut wheel_closures: Vec<Closure<dyn FnMut(web_sys::WheelEvent)>> = Vec::new();
        let mut touch_closures: Vec<Closure<dyn FnMut(web_sys::TouchEvent)>> = Vec::new();
        let long_press_timer: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
        let last_tap_time: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.0));
        // Shared closure handle for long-press timeout callback
        let long_press_cb_handle: Rc<RefCell<Option<Closure<dyn FnMut()>>>> =
            Rc::new(RefCell::new(None));

        // ── PANE events (on container div — canvases have pointer-events:none) ──
        let pane_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.pane_container.clone().unchecked_into()
        };
        let pane_container_el: web_sys::Element = pane_el.clone();
        let grid_c: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.grid_wrapper.clone().unchecked_into()
        };

        // pane: pointerenter
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let mut s = inner.borrow_mut();
                    s.interaction.set_touch(pe.pointer_type() == "touch");
                    s.on_pointer_enter(HitZone::Chart);
                    // Clear subpane focus — cursor is now in the main pane
                    s.active_subpane_id = None;
                    let cursor = s.cursor_css();
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);
                }));
            pane_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointerleave
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let mut s = inner.borrow_mut();
                    s.on_pointer_leave(HitZone::Chart);
                    // Clear the override to let CSS default take over (crosshair)
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", "");
                }));
            pane_el
                .add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointermove
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let grid_move = grid_c.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, y) = event_css_pos(&pe, &pane_c);
                    let shift_pressed = pe.shift_key();
                    let ctrl_pressed = pe.ctrl_key() || pe.meta_key(); // meta for Mac Cmd
                    let mut s = inner.borrow_mut();

                    // Detect touch on every move (not just pointerdown)
                    s.interaction.set_touch(pe.pointer_type() == "touch");

                    // Ensure zone is set (fixes missing pointerenter on page load)
                    s.on_pointer_enter(HitZone::Chart);
                    s.on_pane_pointer_move(x, y, shift_pressed, ctrl_pressed);

                    let cursor = s.cursor_css();
                    let is_dragging =
                        s.interaction.is_dragging() || s.interaction.drawing_drag_active;

                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);

                    let grid_el: &web_sys::HtmlElement = grid_move.unchecked_ref();
                    if is_dragging {
                        let _ = grid_el.style().set_property("cursor", cursor);
                    } else {
                        let _ = grid_el.style().set_property("cursor", "");
                    }
                }));
            pane_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointerdown
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let lp_timer = Rc::clone(&long_press_timer);
            let lp_cb = Rc::clone(&long_press_cb_handle);
            let last_tap = Rc::clone(&last_tap_time);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();

                    // Ignore right-click (button 2) — handled by contextmenu
                    if pe.button() == 2 {
                        return;
                    }

                    let (x, y) = event_css_pos(&pe, &pane_c);
                    let mut s = inner.borrow_mut();

                    // Detect touch input
                    let is_touch = pe.pointer_type() == "touch";
                    s.interaction.is_touch = is_touch;

                    s.on_pointer_enter(HitZone::Chart);
                    s.on_pointer_down(x, y, HitZone::Chart);
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.set_pointer_capture(pe.pointer_id());

                    // Touch-specific: cancel any existing long-press timer
                    if let Some(tid) = lp_timer.borrow_mut().take() {
                        let _ = web_sys::window().unwrap().clear_timeout_with_handle(tid);
                    }
                    *lp_cb.borrow_mut() = None;

                    if is_touch {
                        // Double-tap detection (500ms)
                        let now = js_sys::Date::now();
                        let last = *last_tap.borrow();
                        if now - last < 500.0 {
                            // Double tap
                            *last_tap.borrow_mut() = 0.0;
                            s.on_touch_double_tap();
                            return;
                        }
                        *last_tap.borrow_mut() = now;

                        // Start long-press timer (240ms like LWC)
                        let inner_lp = Rc::clone(&inner);
                        let lp_timer_inner = Rc::clone(&lp_timer);
                        let lp_x = x;
                        let lp_y = y;

                        drop(s); // release borrow before setTimeout callback

                        let timeout_cb = Closure::<dyn FnMut()>::wrap(Box::new(move || {
                            let mut s = inner_lp.borrow_mut();
                            if s.interaction.pressed
                                && !s.interaction.drag_active
                                && !s.interaction.pinch_active
                            {
                                s.on_long_press(lp_x, lp_y);
                            }
                            *lp_timer_inner.borrow_mut() = None;
                        }));

                        let window = web_sys::window().unwrap();
                        let tid = window
                            .set_timeout_with_callback_and_timeout_and_arguments_0(
                                timeout_cb.as_ref().unchecked_ref(),
                                240,
                            )
                            .unwrap_or(0);
                        *lp_timer.borrow_mut() = Some(tid);
                        *lp_cb.borrow_mut() = Some(timeout_cb);
                    }
                }));
            pane_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointerup
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let grid_up = grid_c.clone();
            let lp_timer = Rc::clone(&long_press_timer);
            let lp_cb = Rc::clone(&long_press_cb_handle);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();

                    // Cancel long-press timer
                    if let Some(tid) = lp_timer.borrow_mut().take() {
                        let _ = web_sys::window().unwrap().clear_timeout_with_handle(tid);
                    }
                    *lp_cb.borrow_mut() = None;

                    let mut s = inner.borrow_mut();
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);

                    // Clear grid wrapper override
                    let grid_el: &web_sys::HtmlElement = grid_up.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                }));
            pane_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: wheel
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let (x, _y) = wheel_css_pos(&e, &pane_c);
                    let mut s = inner.borrow_mut();
                    s.on_pointer_enter(HitZone::Chart);
                    s.on_pane_wheel(x, e.delta_x(), e.delta_y(), e.delta_mode());
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            pane_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel",
                cb.as_ref().unchecked_ref(),
                &opts,
            )?;
            wheel_closures.push(cb);
        }
        // pane: contextmenu — remove all scale drawings and exit scale mode on right-click
        {
            let inner = Rc::clone(&inner);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    e.prevent_default();
                    let mut s = inner.borrow_mut();
                    s.engine.drawings.remove_all_scale();
                    // Also exit scale drawing mode if active
                    if s.engine.drawings.active_tool == raycore::DrawingTool::Scale {
                        s.engine.drawings.cancel_creation();
                        s.engine.drawings.active_tool = raycore::DrawingTool::None;
                    }
                }));
            pane_el.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointercancel
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let grid_can = grid_c.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let mut s = inner.borrow_mut();
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);
                    let grid_el: &web_sys::HtmlElement = grid_can.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                }));
            pane_el
                .add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }

        // ── PANE TOUCH events (for pinch zoom — needs raw TouchEvent for multi-touch) ──
        // touchstart: detect 2-finger pinch start; cancel long-press if multi-touch
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let lp_timer = Rc::clone(&long_press_timer);
            let lp_cb = Rc::clone(&long_press_cb_handle);
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(
                move |e: web_sys::TouchEvent| {
                    e.prevent_default();
                    let touches = e.touches();
                    if touches.length() >= 2 {
                        // Cancel long-press timer on multi-touch
                        if let Some(tid) = lp_timer.borrow_mut().take() {
                            let _ = web_sys::window().unwrap().clear_timeout_with_handle(tid);
                        }
                        *lp_cb.borrow_mut() = None;

                        let t0 = touches.get(0).unwrap();
                        let t1 = touches.get(1).unwrap();
                        let rect = pane_c.get_bounding_client_rect();
                        let x0 = t0.client_x() as f64 - rect.left();
                        let y0 = t0.client_y() as f64 - rect.top();
                        let x1 = t1.client_x() as f64 - rect.left();
                        let y1 = t1.client_y() as f64 - rect.top();
                        let cx = (x0 + x1) / 2.0;
                        let cy = (y0 + y1) / 2.0;
                        let dx = x1 - x0;
                        let dy = y1 - y0;
                        let distance = (dx * dx + dy * dy).sqrt();
                        let mut s = inner.borrow_mut();
                        s.on_pinch_start(cx, cy, distance);
                    }
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            pane_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchstart",
                cb.as_ref().unchecked_ref(),
                &opts,
            )?;
            touch_closures.push(cb);
        }
        // touchmove: update pinch scale
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(
                move |e: web_sys::TouchEvent| {
                    e.prevent_default();
                    let touches = e.touches();
                    let s_ref = inner.borrow();
                    let pinch_active = s_ref.interaction.pinch_active;
                    let pinch_start_dist = s_ref.interaction.pinch_start_distance;
                    drop(s_ref);

                    if touches.length() >= 2 && pinch_active {
                        let t0 = touches.get(0).unwrap();
                        let t1 = touches.get(1).unwrap();
                        let dx = (t1.client_x() - t0.client_x()) as f64;
                        let dy = (t1.client_y() - t0.client_y()) as f64;
                        let distance = (dx * dx + dy * dy).sqrt();
                        let scale = distance / pinch_start_dist.max(1.0);
                        let mut s = inner.borrow_mut();
                        s.on_pinch_update(scale);
                    }
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            pane_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchmove",
                cb.as_ref().unchecked_ref(),
                &opts,
            )?;
            touch_closures.push(cb);
        }
        // touchend: end pinch when fingers lift
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(
                move |e: web_sys::TouchEvent| {
                    let touches = e.touches();
                    if touches.length() < 2 {
                        let mut s = inner.borrow_mut();
                        if s.interaction.pinch_active {
                            s.on_pinch_end();
                        }
                    }
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            pane_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchend",
                cb.as_ref().unchecked_ref(),
                &opts,
            )?;
            touch_closures.push(cb);
        }

        // ── PRICE AXIS events (on container div) ──
        let price_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.price_axis_container.clone().unchecked_into()
        };
        let price_container_el: web_sys::Element = price_el.clone();

        // price axis: pointerenter
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let mut s = inner.borrow_mut();
                    s.on_pointer_enter(HitZone::PriceAxis);
                    let cursor = s.cursor_css();
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);
                }));
            price_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerleave
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let mut s = inner.borrow_mut();
                    s.on_pointer_leave(HitZone::PriceAxis);
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", "");
                }));
            price_el
                .add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointermove
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let grid_move_p = grid_c.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (_x, y) = event_css_pos(&pe, &price_c);
                    let mut s = inner.borrow_mut();
                    s.on_pointer_enter(HitZone::PriceAxis);
                    s.on_price_axis_move(y);
                    let cursor = s.cursor_css();
                    let is_dragging = s.interaction.is_dragging();

                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);

                    let grid_el: &web_sys::HtmlElement = grid_move_p.unchecked_ref();
                    if is_dragging {
                        let _ = grid_el.style().set_property("cursor", cursor);
                    } else {
                        let _ = grid_el.style().set_property("cursor", "");
                    }
                }));
            price_el
                .add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerdown
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (_x, y) = event_css_pos(&pe, &price_c);
                    let mut s = inner.borrow_mut();
                    s.interaction.is_touch = pe.pointer_type() == "touch";
                    s.on_pointer_enter(HitZone::PriceAxis);
                    s.on_pointer_down(0.0, y, HitZone::PriceAxis);
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.set_pointer_capture(pe.pointer_id());
                }));
            price_el
                .add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerup
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let grid_up_p = grid_c.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let mut s = inner.borrow_mut();
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);

                    // Clear grid wrapper override
                    let grid_el: &web_sys::HtmlElement = grid_up_p.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                }));
            price_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: wheel
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let mut s = inner.borrow_mut();
                    s.on_pointer_enter(HitZone::PriceAxis);
                    s.on_price_axis_wheel(e.delta_y(), e.delta_mode());
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            price_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel",
                cb.as_ref().unchecked_ref(),
                &opts,
            )?;
            wheel_closures.push(cb);
        }
        // price axis: contextmenu
        {
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    e.prevent_default();
                }));
            price_el
                .add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointercancel
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let grid_can_p = grid_c.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let mut s = inner.borrow_mut();
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);
                    let grid_el: &web_sys::HtmlElement = grid_can_p.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                }));
            price_el
                .add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }

        // ── TIME AXIS events (on container div) ──
        let time_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.time_axis_container.clone().unchecked_into()
        };
        let time_container_el: web_sys::Element = time_el.clone();

        // time axis: pointerenter
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let mut s = inner.borrow_mut();
                    s.on_pointer_enter(HitZone::TimeAxis);
                    let cursor = s.cursor_css();
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);
                }));
            time_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerleave
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let mut s = inner.borrow_mut();
                    s.on_pointer_leave(HitZone::TimeAxis);
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", "");
                }));
            time_el
                .add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointermove
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let grid_move_t = grid_c.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, _y) = event_css_pos(&pe, &time_c);
                    let mut s = inner.borrow_mut();
                    s.on_pointer_enter(HitZone::TimeAxis);
                    s.on_time_axis_move(x);
                    let cursor = s.cursor_css();
                    let is_dragging = s.interaction.is_dragging();

                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);

                    let grid_el: &web_sys::HtmlElement = grid_move_t.unchecked_ref();
                    if is_dragging {
                        let _ = grid_el.style().set_property("cursor", cursor);
                    } else {
                        let _ = grid_el.style().set_property("cursor", "");
                    }
                }));
            time_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerdown
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, _y) = event_css_pos(&pe, &time_c);
                    let mut s = inner.borrow_mut();
                    s.interaction.is_touch = pe.pointer_type() == "touch";
                    s.on_pointer_enter(HitZone::TimeAxis);
                    s.on_pointer_down(x, 0.0, HitZone::TimeAxis);
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.set_pointer_capture(pe.pointer_id());
                }));
            time_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerup
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let grid_up_t = grid_c.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let mut s = inner.borrow_mut();
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);

                    // Clear grid wrapper override
                    let grid_el: &web_sys::HtmlElement = grid_up_t.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                }));
            time_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: wheel
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let (x, _y) = wheel_css_pos(&e, &time_c);
                    let mut s = inner.borrow_mut();
                    s.on_pointer_enter(HitZone::TimeAxis);
                    s.on_time_axis_wheel(x, e.delta_y(), e.delta_mode());
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            time_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel",
                cb.as_ref().unchecked_ref(),
                &opts,
            )?;
            wheel_closures.push(cb);
        }
        // time axis: contextmenu
        {
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    e.prevent_default();
                }));
            time_el.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointercancel
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let grid_can_t = grid_c.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let mut s = inner.borrow_mut();
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);
                    let grid_el: &web_sys::HtmlElement = grid_can_t.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                }));
            time_el
                .add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }

        // ── CORNER STUB events (on container div) ──
        let corner_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.corner_stub_container.clone().unchecked_into()
        };
        // corner stub: pointerenter
        {
            let inner = Rc::clone(&inner);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let mut s = inner.borrow_mut();
                    s.on_pointer_enter(HitZone::None);
                }));
            corner_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }

        // ── ResizeObserver on widget containers (device-pixel-content-box) ──
        //
        // LWC (fancy-canvas) uses ResizeObserver with `device-pixel-content-box`
        // to get the exact integer device-pixel size of each canvas element.
        // This eliminates the ±1px rounding error from `round(css * dpr)`
        // that causes blur at non-integer zoom levels.
        //
        // We observe all four widget containers and store their exact sizes.
        // On each callback we also resize canvases and renderers.

        let container_el: web_sys::HtmlElement = {
            let borrow = inner.borrow();
            borrow.layout.container().clone()
        };

        // Grab references to each widget container for the observer
        let pane_container_for_ro: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.pane_container.clone().unchecked_into()
        };
        let price_container_for_ro: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.price_axis_container.clone().unchecked_into()
        };
        let time_container_for_ro: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.time_axis_container.clone().unchecked_into()
        };
        let corner_container_for_ro: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.corner_stub_container.clone().unchecked_into()
        };

        let (resize_closure, resize_observer) = {
            let inner = Rc::clone(&inner);
            let pane_ref = pane_container_for_ro.clone();
            let price_ref = price_container_for_ro.clone();
            let time_ref = time_container_for_ro.clone();
            let corner_ref = corner_container_for_ro.clone();

            let cb = Closure::<dyn FnMut(js_sys::Array)>::wrap(Box::new(
                move |entries: js_sys::Array| {
                    let mut s = inner.borrow_mut();
                    let dpr = get_dpr();
                    s.engine.dpr = dpr;

                    // Try to extract exact device-pixel sizes from entries
                    let mut got_exact = false;
                    for i in 0..entries.length() {
                        let entry: web_sys::ResizeObserverEntry = entries.get(i).unchecked_into();
                        let target = entry.target();

                        // Try device-pixel-content-box (returns exact integer device pixels)
                        let dpsize = js_sys::Reflect::get(
                            &entry,
                            &JsValue::from_str("devicePixelContentBoxSize"),
                        )
                        .ok();
                        let (exact_w, exact_h) = if let Some(ref dp) = dpsize {
                            if !dp.is_undefined() && !dp.is_null() {
                                let arr: &js_sys::Array = dp.unchecked_ref();
                                if arr.length() > 0 {
                                    let item = arr.get(0);
                                    let iw = js_sys::Reflect::get(
                                        &item,
                                        &JsValue::from_str("inlineSize"),
                                    )
                                    .ok()
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                    let ih = js_sys::Reflect::get(
                                        &item,
                                        &JsValue::from_str("blockSize"),
                                    )
                                    .ok()
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                    got_exact = true;
                                    (iw as u32, ih as u32)
                                } else {
                                    (0, 0)
                                }
                            } else {
                                (0, 0)
                            }
                        } else {
                            (0, 0)
                        };

                        if got_exact && exact_w > 0 && exact_h > 0 {
                            if target == pane_ref {
                                s.exact_sizes.pane_pw = exact_w;
                                s.exact_sizes.pane_ph = exact_h;
                            } else if target == price_ref {
                                s.exact_sizes.price_axis_pw = exact_w;
                                s.exact_sizes.price_axis_ph = exact_h;
                            } else if target == time_ref {
                                s.exact_sizes.time_axis_pw = exact_w;
                                s.exact_sizes.time_axis_ph = exact_h;
                            } else if target == corner_ref {
                                s.exact_sizes.corner_stub_pw = exact_w;
                                s.exact_sizes.corner_stub_ph = exact_h;
                            }
                            s.exact_sizes.available = true;
                        }
                    }

                    sync_widget_sizes(&mut *s, dpr, true);

                    // Emit resize event
                    let (cw, ch) = s.layout.container_css_size();
                    s.engine.event_bus.emit(raycore::ChartEvent::Resize {
                        width: cw,
                        height: ch,
                    });
                },
            ));
            let observer = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref())?;

            // Try to observe with device-pixel-content-box; fall back to content-box
            let observe_with_dpcb = js_sys::Function::new_with_args(
                "observer,element",
                "try { observer.observe(element, { box: 'device-pixel-content-box' }); return true; } catch(e) { observer.observe(element); return false; }"
            );
            let _ =
                observe_with_dpcb.call2(&JsValue::NULL, observer.as_ref(), &pane_container_for_ro);
            let _ =
                observe_with_dpcb.call2(&JsValue::NULL, observer.as_ref(), &price_container_for_ro);
            let _ =
                observe_with_dpcb.call2(&JsValue::NULL, observer.as_ref(), &time_container_for_ro);
            let _ = observe_with_dpcb.call2(
                &JsValue::NULL,
                observer.as_ref(),
                &corner_container_for_ro,
            );

            // Also observe the outer container for general layout changes
            observer.observe(&container_el.clone().unchecked_into());

            (cb, observer)
        };

        Ok(RayCore {
            inner,
            symbol: "DEMO".to_string(),
            interval: "1m".to_string(),
            _closures: closures,
            _wheel_closures: wheel_closures,
            _touch_closures: touch_closures,
            _resize_closure: Some(resize_closure),
            _resize_observer: Some(resize_observer),
            _long_press_timer: long_press_timer,
            _last_tap_time: last_tap_time,
            _event_registry: EventListenerRegistry::new(),
            event_emitter: Rc::new(RefCell::new(EventEmitter::new())),
            theme_config: raycore::ThemeConfig::default(),
            auto_render: false,
            _raf_closure: None,
            _raf_id: Rc::new(Cell::new(0)),
            dirty: Rc::new(Cell::new(false)),
        })
    }

    // ── Public API ───────────────────────────────────────────────────────────

    pub fn renderer_name(&self) -> String {
        self.inner.borrow().engine.renderer_name().to_string()
    }

    pub fn get_supported_renderers() -> js_sys::Array {
        let arr = js_sys::Array::new();
        arr.push(&JsValue::from_str("canvas2d"));
        if webgpu_available() {
            arr.push(&JsValue::from_str("webgpu"));
        }
        arr
    }

    // ── Modern API ───────────────────────────────────────────────────────────

    /// Create a new RayCore instance with a full options object.
    ///
    /// `container` can be an `HTMLElement` reference or a string container ID.
    /// `options` is an optional JS object:
    /// ```js
    /// {
    ///   theme: "dark" | "light" | { colors: {...}, crosshair: {...}, ... },
    ///   renderer: "auto" | "webgpu" | "canvas2d",
    ///   autoRender: true,
    ///   symbol: "BTCUSD",
    ///   interval: "1D",
    ///   crosshair: { mode: "normal" | "magnet_ohlc" },
    ///   priceScale: { mode: "normal", margins: { top: 0.1, bottom: 0.1 } },
    /// }
    /// ```
    pub async fn create_chart(
        container: JsValue,
        options: JsValue,
    ) -> Result<RayCore, JsValue> {
        // Resolve container: HTMLElement or string ID
        let container_id = if container.is_string() {
            container.as_string().unwrap_or_default()
        } else if let Some(el) = container.dyn_ref::<web_sys::HtmlElement>() {
            // If element has an ID, use it. Otherwise assign a temporary one.
            let id = el.id();
            if id.is_empty() {
                let generated = format!("raycore-{}", js_sys::Date::now() as u64);
                el.set_id(&generated);
                generated
            } else {
                id
            }
        } else {
            return Err(js_err("container must be an HTMLElement or a string ID"));
        };

        // Parse renderer option
        let renderer = js_get_str(&options, "renderer").unwrap_or_else(|| "auto".to_string());
        let preferred = match renderer.as_str() {
            "webgpu" => "webgpu",
            "canvas2d" => "canvas2d",
            _ => {
                if webgpu_available() {
                    "webgpu"
                } else {
                    "canvas2d"
                }
            }
        };

        // Create via existing path
        let mut chart = Self::create_with(&container_id, preferred).await?;

        // Apply theme
        if let Some(theme_val) = js_get(&options, "theme") {
            if let Some(theme_str) = theme_val.as_string() {
                match theme_str.as_str() {
                    "light" => {
                        chart.theme_config = raycore::ThemeConfig::light();
                        let style = chart.theme_config.to_chart_style();
                        chart.inner.borrow_mut().engine.style = style;
                    }
                    _ => {} // "dark" is already default
                }
            }
        }

        // Apply auto-render
        let auto_render = js_get_bool(&options, "autoRender").unwrap_or(true);
        chart.auto_render = auto_render;
        if auto_render {
            chart.start_auto_render_internal();
        }

        // Apply symbol
        if let Some(symbol) = js_get_str(&options, "symbol") {
            chart.symbol = symbol;
        }

        // Apply interval
        if let Some(interval) = js_get_str(&options, "interval") {
            chart.interval = interval;
        }

        // Apply crosshair mode
        if let Some(crosshair_obj) = js_get(&options, "crosshair") {
            if let Some(mode) = js_get_str(&crosshair_obj, "mode") {
                let mode = parse_crosshair_mode(&mode);
                chart.inner.borrow_mut().engine.crosshair.mode = mode;
            }
        }

        // Apply price scale options
        if let Some(ps_obj) = js_get(&options, "priceScale") {
            if let Some(mode_str) = js_get_str(&ps_obj, "mode") {
                let mode = match mode_str.as_str() {
                    "logarithmic" | "log" => raycore::PriceScaleMode::Logarithmic,
                    "percentage" | "percent" => raycore::PriceScaleMode::Percentage,
                    "indexedTo100" | "indexed" => raycore::PriceScaleMode::IndexedTo100,
                    _ => raycore::PriceScaleMode::Normal,
                };
                chart.inner.borrow_mut().engine.viewport.set_price_scale_mode(mode);
            }
            if let Some(margins_obj) = js_get(&ps_obj, "margins") {
                let top = js_get_f64(&margins_obj, "top").unwrap_or(0.1);
                let bottom = js_get_f64(&margins_obj, "bottom").unwrap_or(0.1);
                chart.inner.borrow_mut().engine.viewport.scale_margin_top = top;
                chart.inner.borrow_mut().engine.viewport.scale_margin_bottom = bottom;
            }
        }

        // Apply CSS variables from theme
        chart.apply_css_variables();

        Ok(chart)
    }

    /// Apply partial options update at runtime.
    ///
    /// Accepts the same options shape as `create_chart()`. Only provided
    /// fields are updated; omitted fields keep their current values.
    pub fn apply_options(&mut self, options: JsValue) {
        if options.is_undefined() || options.is_null() {
            return;
        }

        // Theme
        if let Some(theme_val) = js_get(&options, "theme") {
            if let Some(theme_str) = theme_val.as_string() {
                match theme_str.as_str() {
                    "dark" => {
                        self.theme_config = raycore::ThemeConfig::dark();
                    }
                    "light" => {
                        self.theme_config = raycore::ThemeConfig::light();
                    }
                    _ => {}
                }
                let style = self.theme_config.to_chart_style();
                self.inner.borrow_mut().engine.style = style;
                self.apply_css_variables();
            }
        }



        // Symbol
        if let Some(symbol) = js_get_str(&options, "symbol") {
            self.symbol = symbol.clone();
            self.inner.borrow_mut().engine.event_bus.emit(
                raycore::ChartEvent::SymbolChange { symbol },
            );
        }

        // Interval
        if let Some(interval) = js_get_str(&options, "interval") {
            self.interval = interval.clone();
            self.inner.borrow_mut().engine.event_bus.emit(
                raycore::ChartEvent::IntervalChange { interval },
            );
        }

        // Crosshair
        if let Some(crosshair_obj) = js_get(&options, "crosshair") {
            if let Some(mode) = js_get_str(&crosshair_obj, "mode") {
                let mode = parse_crosshair_mode(&mode);
                self.inner.borrow_mut().engine.crosshair.mode = mode;
            }
        }

        // Auto render
        if let Some(auto) = js_get_bool(&options, "autoRender") {
            if auto && !self.auto_render {
                self.auto_render = true;
                self.start_auto_render_internal();
            } else if !auto && self.auto_render {
                self.auto_render = false;
                self.stop_auto_render_internal();
            }
        }

        self.mark_dirty();
    }

    // ── Event System ─────────────────────────────────────────────────────────

    /// Register an event callback.
    ///
    /// ```js
    /// chart.on("crosshairMove", (event) => {
    ///   console.log(event.x, event.y, event.price);
    /// });
    /// ```
    ///
    /// Valid event names: crosshairMove, visibleRangeChange, click,
    /// drawingCreated, drawingSelected, symbolChange, intervalChange,
    /// priceScaleChange, chartTypeChange, resize, error.
    pub fn on(&mut self, event: &str, callback: js_sys::Function) {
        self.event_emitter.borrow_mut().on(event, callback);
    }

    /// Remove a specific event callback.
    pub fn off(&mut self, event: &str, callback: js_sys::Function) {
        self.event_emitter.borrow_mut().off(event, &callback);
    }

    /// Register a one-shot event callback (auto-removes after first call).
    pub fn once(&mut self, event: &str, callback: js_sys::Function) {
        self.event_emitter.borrow_mut().once(event, callback);
    }

    // ── Auto-Render ──────────────────────────────────────────────────────────

    /// Start the auto-render RAF loop.
    pub fn start_auto_render(&mut self) {
        if !self.auto_render {
            self.auto_render = true;
            self.start_auto_render_internal();
        }
    }

    /// Stop the auto-render RAF loop. Caller must manually call render().
    pub fn stop_auto_render(&mut self) {
        if self.auto_render {
            self.auto_render = false;
            self.stop_auto_render_internal();
        }
    }

    /// Returns whether auto-render is currently active.
    pub fn is_auto_render(&self) -> bool {
        self.auto_render
    }

    /// Get the current theme preset name ("dark", "light", or "custom").
    pub fn theme(&self) -> String {
        // Check if it matches a known preset
        let dark = raycore::ThemeConfig::dark();
        if self.theme_config.colors.background == dark.colors.background
            && self.theme_config.colors.bullish == dark.colors.bullish
        {
            "dark".to_string()
        } else {
            let light = raycore::ThemeConfig::light();
            if self.theme_config.colors.background == light.colors.background {
                "light".to_string()
            } else {
                "custom".to_string()
            }
        }
    }

    /// Get current CSS variables as a JS object.
    pub fn get_css_variables(&self) -> JsValue {
        let obj = js_sys::Object::new();
        for (key, value) in self.theme_config.to_css_variables() {
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str(&key),
                &JsValue::from_str(&value),
            );
        }
        obj.into()
    }

    // ── Data loading ─────────────────────────────────────────────────────────

    pub fn set_data_arrays(
        &mut self,
        open: &[f32],
        high: &[f32],
        low: &[f32],
        close: &[f32],
        volume: &[f32],
        timestamps: &[u64],
    ) -> Result<(), JsValue> {
        let count = open.len();
        ensure_equal_len("open", count, "high", high.len())?;
        ensure_equal_len("open", count, "low", low.len())?;
        ensure_equal_len("open", count, "close", close.len())?;
        ensure_equal_len("open", count, "volume", volume.len())?;
        ensure_equal_len("open", count, "timestamps", timestamps.len())?;
        ensure_finite_slice("open", open)?;
        ensure_finite_slice("high", high)?;
        ensure_finite_slice("low", low)?;
        ensure_finite_slice("close", close)?;
        ensure_finite_slice("volume", volume)?;

        let bars: Vec<Bar> = (0..count)
            .map(|i| Bar {
                timestamp: timestamps[i],
                open: open[i],
                high: high[i],
                low: low[i],
                close: close[i],
                volume: volume[i],
                _pad: 0.0,
            })
            .collect();
        self.inner
            .borrow_mut()
            .engine
            .set_data(bars)
            .map_err(js_err)?;
        self.dirty.set(true);
        log::info!("set_data_arrays: {} bars", count);
        Ok(())
    }

    // ── Demo mode ────────────────────────────────────────────────────────────

    pub fn demo_mode(&mut self) {
        let now_ms = js_sys::Date::now() as u64;
        let num_bars = 600;
        let interval_ms = 60_000;
        let start_ms = now_ms - (num_bars as u64) * interval_ms;
        let bars = generate_sample_data(num_bars, start_ms, interval_ms);
        match self.inner.borrow_mut().engine.set_data(bars) {
            Ok(()) => log::info!("demo_mode: {} bars loaded", num_bars),
            Err(e) => log::error!("demo_mode failed: {}", e),
        }
    }

    // ── Viewport control ─────────────────────────────────────────────────────

    pub fn zoom_to_range(&mut self, start: u64, end: u64) {
        self.inner.borrow_mut().engine.zoom_to_range(start, end);
    }

    /// Set visible bar range using fractional bar indices.
    pub fn set_visible_range(&mut self, start: f64, end: f64) {
        let mut s = self.inner.borrow_mut();
        let should_fit = !s.engine.viewport.price_locked;
        s.engine.viewport.set_range(start, end);
        if should_fit {
            let raycore::ChartEngine { viewport, bars, .. } = &mut s.engine;
            viewport.auto_fit_price(bars);
        }
        let sb = s.engine.viewport.start_bar;
        let eb = s.engine.viewport.end_bar;
        s.engine.event_bus.emit(raycore::ChartEvent::VisibleRangeChange {
            start_bar: sb,
            end_bar: eb,
        });
    }

    pub fn visible_range(&self) -> Vec<f64> {
        let s = self.inner.borrow();
        vec![s.engine.viewport.start_bar, s.engine.viewport.end_bar]
    }

    /// Set crosshair mode: "normal" or "magnet_ohlc".
    ///
    /// Legacy alias:
    /// - "magnet" is accepted and treated as "magnet_ohlc".
    pub fn set_crosshair_mode(&mut self, mode: &str) {
        let mut s = self.inner.borrow_mut();
        s.engine.crosshair.mode = match mode {
            "magnet" => raycore::CrosshairMode::MagnetOHLC,
            _ => parse_crosshair_mode(mode),
        };
    }

    pub fn crosshair_mode(&self) -> String {
        let s = self.inner.borrow();
        crosshair_mode_key(s.engine.crosshair.mode).to_string()
    }

    /// Returns `[active, x, y, bar_index, price]`.
    pub fn crosshair_state(&self) -> Vec<f64> {
        let s = self.inner.borrow();
        let bar_index = s
            .engine
            .crosshair
            .bar_index
            .map(|idx| idx as f64)
            .unwrap_or(-1.0);
        vec![
            if s.engine.crosshair.active { 1.0 } else { 0.0 },
            s.engine.crosshair.x,
            s.engine.crosshair.y,
            bar_index,
            s.engine.crosshair.price,
        ]
    }

    /// Set crosshair state for synchronized groups.
    pub fn set_crosshair_state(
        &mut self,
        active: bool,
        x: f64,
        y: f64,
        bar_index: f64,
        price: f64,
        mode: &str,
    ) {
        let mut s = self.inner.borrow_mut();
        let (pw, ph) = s.layout.pane_css_size();

        s.engine.crosshair.active = active;
        s.engine.crosshair.mode = parse_crosshair_mode(mode);
        s.engine.crosshair.x = x;
        s.engine.crosshair.y = y;
        s.engine.crosshair.bar_index = if bar_index.is_finite() && bar_index >= 0.0 {
            Some(bar_index as usize)
        } else if pw > 0.0 {
            s.engine.viewport.bar_index_for_crosshair(x, pw)
        } else {
            None
        };
        if price.is_finite() {
            s.engine.crosshair.price = price;
        } else if ph > 0.0 {
            let candle_h = ph * s.engine.viewport.candle_height_frac();
            s.engine.crosshair.price = s.engine.viewport.pixel_to_price(y, candle_h);
        }
    }

    pub fn set_symbol(&mut self, symbol: &str) {
        self.symbol = symbol.to_string();
        self.inner.borrow_mut().engine.event_bus.emit(
            raycore::ChartEvent::SymbolChange { symbol: symbol.to_string() },
        );
        self.dirty.set(true);
    }

    pub fn symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn set_interval(&mut self, interval: &str) {
        self.interval = interval.to_string();
        self.inner.borrow_mut().engine.event_bus.emit(
            raycore::ChartEvent::IntervalChange { interval: interval.to_string() },
        );
        self.dirty.set(true);
    }

    pub fn interval(&self) -> String {
        self.interval.clone()
    }

    /// Data timestamp range as `[from_ts, to_ts]`, or empty if no bars.
    pub fn data_range(&self) -> Vec<f64> {
        let s = self.inner.borrow();
        let len = s.engine.bars.len();
        if len == 0 {
            return Vec::new();
        }
        let first = s.engine.bars.timestamp(0) as f64;
        let last = s.engine.bars.timestamp(len - 1) as f64;
        vec![first, last]
    }

    // ── Drawing tools ─────────────────────────────────────────────────────────

    /// Set active drawing tool: "none", "trend_line", "rectangle", "fibonacci",
    /// "scale", "brush", "horizontal_line", "vertical_line", "ray".
    pub fn set_drawing_tool(&mut self, tool: &str) {
        let mut s = self.inner.borrow_mut();
        s.engine.drawings.active_tool = match tool {
            "trend_line" => raycore::DrawingTool::TrendLine,
            "rectangle" => raycore::DrawingTool::Rectangle,
            "fibonacci" => raycore::DrawingTool::Fibonacci,
            "scale" => raycore::DrawingTool::Scale,
            "brush" => raycore::DrawingTool::Brush,
            "horizontal_line" => raycore::DrawingTool::HorizontalLine,
            "vertical_line" => raycore::DrawingTool::VerticalLine,
            "ray" => raycore::DrawingTool::Ray,
            _ => raycore::DrawingTool::None,
        };
    }

    /// Remove the currently selected drawing (e.g. on Delete key).
    pub fn remove_selected_drawing(&mut self) {
        self.inner.borrow_mut().engine.drawings.remove_selected();
    }

    /// Cancel the drawing currently being created (e.g. on Escape key).
    pub fn cancel_drawing(&mut self) {
        self.inner.borrow_mut().engine.drawings.cancel_creation();
    }

    // ── Keyboard Events ────────────────────────────────────────────────────────

    /// Handle keyboard events. Returns true if the key was handled.
    ///
    /// Supported shortcuts:
    /// - Delete / Backspace: Remove selected drawing
    /// - Escape: Cancel drawing creation, deselect all
    /// - Arrow Left/Right: Scroll chart by one bar
    /// - Arrow Up/Down: Zoom price axis in/out
    /// - Home: Scroll to first bar
    /// - End: Scroll to last bar
    /// - +/=: Zoom in (time axis)
    /// - -: Zoom out (time axis)
    /// - 0: Reset zoom to fit all data
    pub fn on_key_down(&mut self, key: &str, ctrl: bool, shift: bool, _alt: bool) -> bool {
        let mut s = self.inner.borrow_mut();
        let bar_count = s.engine.bars.len() as f64;

        match key {
            // Delete selected drawing
            "Delete" | "Backspace" => {
                s.engine.drawings.remove_selected();
                true
            }

            // Cancel/deselect
            "Escape" => {
                s.engine.drawings.cancel_creation();
                s.engine.drawings.deselect_all();
                true
            }

            // Scroll by bars
            "ArrowLeft" => {
                let amount = if ctrl {
                    10.0
                } else if shift {
                    5.0
                } else {
                    1.0
                };
                s.engine.viewport.start_bar -= amount;
                s.engine.viewport.end_bar -= amount;
                // Clamp to valid range
                if s.engine.viewport.start_bar < 0.0 {
                    let offset = -s.engine.viewport.start_bar;
                    s.engine.viewport.start_bar = 0.0;
                    s.engine.viewport.end_bar += offset;
                }
                true
            }
            "ArrowRight" => {
                let amount = if ctrl {
                    10.0
                } else if shift {
                    5.0
                } else {
                    1.0
                };
                s.engine.viewport.start_bar += amount;
                s.engine.viewport.end_bar += amount;
                // Allow scrolling past end for right margin
                let max_start = bar_count + 50.0; // Some margin
                if s.engine.viewport.start_bar > max_start {
                    let offset = s.engine.viewport.start_bar - max_start;
                    s.engine.viewport.start_bar = max_start;
                    s.engine.viewport.end_bar -= offset;
                }
                true
            }

            // Zoom price axis
            "ArrowUp" => {
                let factor = if ctrl { 1.2 } else { 1.05 };
                let mid = (s.engine.viewport.price_max + s.engine.viewport.price_min) / 2.0;
                let half_range =
                    (s.engine.viewport.price_max - s.engine.viewport.price_min) / 2.0 / factor;
                s.engine.viewport.price_min = mid - half_range;
                s.engine.viewport.price_max = mid + half_range;
                true
            }
            "ArrowDown" => {
                let factor = if ctrl { 1.2 } else { 1.05 };
                let mid = (s.engine.viewport.price_max + s.engine.viewport.price_min) / 2.0;
                let half_range =
                    (s.engine.viewport.price_max - s.engine.viewport.price_min) / 2.0 * factor;
                s.engine.viewport.price_min = mid - half_range;
                s.engine.viewport.price_max = mid + half_range;
                true
            }

            // Jump to start/end
            "Home" => {
                let visible_bars = s.engine.viewport.end_bar - s.engine.viewport.start_bar;
                s.engine.viewport.start_bar = 0.0;
                s.engine.viewport.end_bar = visible_bars;
                true
            }
            "End" => {
                let visible_bars = s.engine.viewport.end_bar - s.engine.viewport.start_bar;
                s.engine.viewport.end_bar = bar_count - 1.0 + visible_bars * 0.1; // Small right margin
                s.engine.viewport.start_bar = s.engine.viewport.end_bar - visible_bars;
                true
            }

            // Zoom time axis (only without Ctrl — let browser handle Ctrl+/- for page zoom)
            "+" | "=" if !ctrl => {
                // Zoom in: reduce visible range
                let mid = (s.engine.viewport.start_bar + s.engine.viewport.end_bar) / 2.0;
                let half_range =
                    (s.engine.viewport.end_bar - s.engine.viewport.start_bar) / 2.0 / 1.2;
                s.engine.viewport.start_bar = mid - half_range;
                s.engine.viewport.end_bar = mid + half_range;
                true
            }
            "-" | "_" if !ctrl => {
                // Zoom out: increase visible range
                let mid = (s.engine.viewport.start_bar + s.engine.viewport.end_bar) / 2.0;
                let half_range =
                    (s.engine.viewport.end_bar - s.engine.viewport.start_bar) / 2.0 * 1.2;
                s.engine.viewport.start_bar = (mid - half_range).max(0.0);
                s.engine.viewport.end_bar = mid + half_range;
                true
            }

            // Reset zoom to fit all data
            "0" => {
                s.engine.viewport.start_bar = 0.0;
                s.engine.viewport.end_bar = bar_count - 1.0 + bar_count * 0.05; // 5% right margin
                s.engine.viewport.price_min = f64::MAX;
                s.engine.viewport.price_max = f64::MIN;
                // Let auto-fit recalculate price range on next render
                true
            }

            _ => false, // Key not handled
        }
    }

    /// Remove all drawings.
    pub fn clear_drawings(&mut self) {
        let mut s = self.inner.borrow_mut();
        while s.engine.drawings.len() > 0 {
            let id = s.engine.drawings.all()[0].id();
            s.engine.drawings.remove(id);
        }
    }

    /// Remove all scale (measurement) drawings.
    pub fn remove_all_scale_drawings(&mut self) {
        self.inner.borrow_mut().engine.drawings.remove_all_scale();
    }

    // ── Runtime Style Configuration API ────────────────────────────────────────

    /// Set crosshair line color.
    /// `target`: "vert", "horz", or "both".
    pub fn set_crosshair_line_color(&mut self, target: &str, r: f32, g: f32, b: f32, a: f32) {
        let mut s = self.inner.borrow_mut();
        with_crosshair_lines_mut(&mut s.engine.style, target, |line| {
            line.color = [r, g, b, a];
        });
    }

    /// Set crosshair line style.
    /// `target`: "vert", "horz", or "both".
    /// `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
    pub fn set_crosshair_line_style(&mut self, target: &str, line_style: &str) {
        let style = LineStyle::from_str(line_style);
        let mut s = self.inner.borrow_mut();
        with_crosshair_lines_mut(&mut s.engine.style, target, |line| {
            line.style = style;
        });
    }

    /// Set crosshair line width in CSS pixels.
    /// `target`: "vert", "horz", or "both".
    pub fn set_crosshair_line_width(&mut self, target: &str, width: f32) {
        let width = width.max(1.0) as f64;
        let mut s = self.inner.borrow_mut();
        with_crosshair_lines_mut(&mut s.engine.style, target, |line| {
            line.width = width;
        });
    }

    /// Set crosshair line visibility.
    /// `target`: "vert", "horz", or "both".
    pub fn set_crosshair_line_visible(&mut self, target: &str, visible: bool) {
        let mut s = self.inner.borrow_mut();
        with_crosshair_lines_mut(&mut s.engine.style, target, |line| {
            line.visible = visible;
        });
    }

    /// Set crosshair axis-label visibility.
    /// `target`: "vert", "horz", or "both".
    pub fn set_crosshair_label_visible(&mut self, target: &str, visible: bool) {
        let mut s = self.inner.borrow_mut();
        with_crosshair_lines_mut(&mut s.engine.style, target, |line| {
            line.label_visible = visible;
        });
    }

    /// Set crosshair label background color.
    /// `target`: "vert", "horz", or "both".
    pub fn set_crosshair_line_label_bg_color(
        &mut self,
        target: &str,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    ) {
        let mut s = self.inner.borrow_mut();
        with_crosshair_lines_mut(&mut s.engine.style, target, |line| {
            line.label_bg_color = [r, g, b, a];
        });
    }

    /// Set the shared crosshair label text color (applies to both axes).
    pub fn set_crosshair_label_text_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.crosshair_label_text = [r, g, b, a];
    }

    /// Set live last-price line style.
    /// `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
    pub fn set_last_price_line_style(&mut self, line_style: &str) {
        self.inner.borrow_mut().engine.style.last_price_line.style =
            LineStyle::from_str(line_style);
    }

    /// Set live last-price line width in CSS pixels.
    pub fn set_last_price_line_width(&mut self, width: f32) {
        self.inner.borrow_mut().engine.style.last_price_line.width = width.max(1.0) as f64;
    }

    /// Set live last-price line visibility.
    pub fn set_last_price_line_visible(&mut self, visible: bool) {
        self.inner.borrow_mut().engine.style.last_price_line.visible = visible;
    }

    /// Set live last-price label visibility on the Y axis.
    pub fn set_last_price_label_visible(&mut self, visible: bool) {
        self.inner
            .borrow_mut()
            .engine
            .style
            .last_price_line
            .label_visible = visible;
    }

    /// Set bullish (up) candle colors: body fill and wick/border.
    pub fn set_bullish_color(
        &mut self,
        fill_r: f32,
        fill_g: f32,
        fill_b: f32,
        fill_a: f32,
        wick_r: f32,
        wick_g: f32,
        wick_b: f32,
        wick_a: f32,
    ) {
        let mut s = self.inner.borrow_mut();
        s.engine.style.bullish_color = [fill_r, fill_g, fill_b, fill_a];
        s.engine.style.wick_bullish_color = [wick_r, wick_g, wick_b, wick_a];
    }

    /// Set bearish (down) candle colors: body fill and wick/border.
    pub fn set_bearish_color(
        &mut self,
        fill_r: f32,
        fill_g: f32,
        fill_b: f32,
        fill_a: f32,
        wick_r: f32,
        wick_g: f32,
        wick_b: f32,
        wick_a: f32,
    ) {
        let mut s = self.inner.borrow_mut();
        s.engine.style.bearish_color = [fill_r, fill_g, fill_b, fill_a];
        s.engine.style.wick_bearish_color = [wick_r, wick_g, wick_b, wick_a];
    }

    /// Set volume bar colors: bullish and bearish.
    pub fn set_volume_colors(
        &mut self,
        up_r: f32,
        up_g: f32,
        up_b: f32,
        up_a: f32,
        down_r: f32,
        down_g: f32,
        down_b: f32,
        down_a: f32,
    ) {
        let mut s = self.inner.borrow_mut();
        s.engine.style.bullish_volume_color = [up_r, up_g, up_b, up_a];
        s.engine.style.bearish_volume_color = [down_r, down_g, down_b, down_a];
    }

    /// Set the font size for axis labels (in CSS pixels).
    pub fn set_font_size(&mut self, size: f32) {
        self.inner.borrow_mut().engine.style.font_size = size;
    }

    /// Set the font family for axis labels.
    pub fn set_font_family(&mut self, family: &str) {
        self.inner.borrow_mut().engine.style.font_family = family.to_string();
    }

    // ── Axis / Grid appearance ──────────────────────────────────────────

    /// Set the axis border (separator line) color (RGBA 0-1).
    pub fn set_axis_border_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.axis_border_color = [r, g, b, a];
    }

    /// Show or hide the axis border line. Layout is unaffected.
    pub fn set_axis_border_visible(&mut self, visible: bool) {
        self.inner.borrow_mut().engine.style.axis_border_visible = visible;
    }

    /// Show or hide axis tick marks. Layout is unaffected.
    pub fn set_axis_ticks_visible(&mut self, visible: bool) {
        self.inner.borrow_mut().engine.style.axis_ticks_visible = visible;
    }

    /// Set the grid line color (RGBA 0-1).
    pub fn set_grid_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.grid_color = [r, g, b, a];
    }

    /// Set the axis label text color (RGBA 0-1).
    pub fn set_axis_text_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.axis_text_color = [r, g, b, a];
    }

    /// Set the price scale tick mark density multiplier.
    pub fn set_price_scale_tick_density(&mut self, density: f32) {
        self.inner.borrow_mut().engine.style.price_scale_tick_mark_density = density;
    }

    /// Set indicator sub-pane separator visible line thickness (CSS px).
    pub fn set_subpane_separator_thickness(&mut self, thickness_css: f64) {
        let mut s = self.inner.borrow_mut();
        s.subpane_separator_style.line_thickness_css = thickness_css;
        s.subpane_separator_style.normalize();
        let sep_style = s.subpane_separator_style.clone();
        for sp in &s.subpanes {
            sp.apply_separator_style(&sep_style);
        }
    }

    /// Set indicator sub-pane separator drag hit-area thickness (CSS px).
    pub fn set_subpane_separator_hit_area(&mut self, hit_area_css: f64) {
        let mut s = self.inner.borrow_mut();
        s.subpane_separator_style.hit_area_css = hit_area_css;
        s.subpane_separator_style.normalize();
        let sep_style = s.subpane_separator_style.clone();
        for sp in &s.subpanes {
            sp.apply_separator_style(&sep_style);
        }
    }

    /// Set indicator sub-pane separator line color (RGBA, 0.0-1.0).
    pub fn set_subpane_separator_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        let mut s = self.inner.borrow_mut();
        s.subpane_separator_style.color = [r, g, b, a];
        let sep_style = s.subpane_separator_style.clone();
        for sp in &s.subpanes {
            sp.apply_separator_style(&sep_style);
        }
    }

    /// Set indicator sub-pane separator hover/active color (RGBA, 0.0-1.0).
    pub fn set_subpane_separator_hover_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        let mut s = self.inner.borrow_mut();
        s.subpane_separator_style.hover_color = [r, g, b, a];
        let sep_style = s.subpane_separator_style.clone();
        for sp in &s.subpanes {
            sp.apply_separator_style(&sep_style);
        }
    }

    /// Set the bar width ratio (0.0-1.0, default 0.8).
    pub fn set_bar_width_ratio(&mut self, ratio: f32) {
        self.inner.borrow_mut().engine.style.bar_width_ratio = ratio.clamp(0.1, 1.0);
    }

    /// Set the price scale margins (top and bottom as fractions 0.0-1.0).
    /// Default is 0.2 top, 0.1 bottom.
    pub fn set_price_scale_margins(&mut self, top: f64, bottom: f64) {
        let mut s = self.inner.borrow_mut();
        s.engine.viewport.scale_margin_top = top.clamp(0.0, 0.5);
        s.engine.viewport.scale_margin_bottom = bottom.clamp(0.0, 0.5);
        s.engine.viewport.price_invalidated = true;
    }

    /// Enable or disable auto-scroll on new bars.
    ///
    /// When `true` (default) the viewport advances by 1 bar each time a new bar
    /// is appended and the chart is already showing the latest data — identical
    /// to LWC's `shiftVisibleRangeOnNewBar` behaviour.
    ///
    /// When `false` the viewport never moves during live streaming regardless of
    /// the current scroll position, giving the user a fully static view even
    /// while data is updating in real time.
    pub fn set_auto_scroll(&mut self, enabled: bool) {
        self.inner.borrow_mut().engine.viewport.auto_scroll = enabled;
    }

    /// Return whether auto-scroll is currently enabled.
    pub fn get_auto_scroll(&self) -> bool {
        self.inner.borrow().engine.viewport.auto_scroll
    }

    /// Set the price scale mode.
    ///
    /// Accepted values: "normal", "logarithmic" (or "log"), "percentage" (or "percent"),
    /// "indexed_to_100" (or "indexedTo100", "indexed").
    pub fn set_price_scale_mode(&mut self, mode: &str) {
        use raycore::PriceScaleMode;
        let parsed = PriceScaleMode::from_str(mode);
        let mut s = self.inner.borrow_mut();
        s.engine.viewport.set_price_scale_mode(parsed);
        s.engine.event_bus.emit(raycore::ChartEvent::PriceScaleChange {
            mode: mode.to_string(),
        });
    }

    // ── Main Chart Type API ────────────────────────────────────────────────────

    /// Set the main chart type.
    ///
    /// Accepted values: "candlestick", "candles", "ohlc", "bars", "line", "area",
    /// "heikin_ashi", "ha", "baseline".
    pub fn set_chart_type(&mut self, chart_type: &str) {
        use raycore::MainChartType;
        let ct = MainChartType::from_str(chart_type);
        let mut s = self.inner.borrow_mut();
        s.engine.set_main_chart_type(ct);
        s.engine.event_bus.emit(raycore::ChartEvent::ChartTypeChange {
            chart_type: ct.as_str().to_string(),
        });
        log::info!("set_chart_type: {}", ct.as_str());
    }

    /// Get the current chart type as a string.
    pub fn get_chart_type(&self) -> String {
        self.inner
            .borrow()
            .engine
            .main_chart_type()
            .as_str()
            .to_string()
    }

    /// Get all available chart types as a comma-separated string.
    pub fn get_available_chart_types() -> String {
        use raycore::MainChartType;
        MainChartType::all()
            .iter()
            .map(|ct| ct.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Get the number of drawings.
    pub fn drawing_count(&self) -> usize {
        self.inner.borrow().engine.drawings.len()
    }

    // ── Price Lines API ────────────────────────────────────────────────────────

    /// Create a new price line at the specified price level. Returns the price line ID.
    ///
    /// `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
    pub fn create_price_line(
        &mut self,
        price: f64,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
        line_width: f32,
        line_style: &str,
        draggable: bool,
    ) -> u32 {
        let mut opts = PriceLineOptions::default();
        opts.price = price;
        opts.color = [color_r, color_g, color_b, color_a];
        opts.line_width = line_width as f64;
        opts.line_style = LineStyle::from_str(line_style);
        opts.draggable = draggable;
        let id = self.inner.borrow_mut().engine.price_lines.create(opts);
        log::info!("create_price_line: id={}, price={}", id.0, price);
        id.0
    }

    /// Update the price of an existing price line.
    pub fn set_price_line_price(&mut self, id: u32, price: f64) {
        use raycore::PriceLineId;
        if let Some(line) = self
            .inner
            .borrow_mut()
            .engine
            .price_lines
            .get_mut(PriceLineId(id))
        {
            line.set_price(price);
        }
    }

    /// Set whether a price line is visible.
    pub fn set_price_line_visible(&mut self, id: u32, visible: bool) {
        use raycore::PriceLineId;
        if let Some(line) = self
            .inner
            .borrow_mut()
            .engine
            .price_lines
            .get_mut(PriceLineId(id))
        {
            line.options.visible = visible;
        }
    }

    /// Set the label text of a price line. Empty string uses formatted price.
    pub fn set_price_line_label(&mut self, id: u32, label: &str) {
        use raycore::PriceLineId;
        if let Some(line) = self
            .inner
            .borrow_mut()
            .engine
            .price_lines
            .get_mut(PriceLineId(id))
        {
            line.options.label_text = label.to_string();
        }
    }

    /// Remove a price line by ID.
    pub fn remove_price_line(&mut self, id: u32) -> bool {
        use raycore::PriceLineId;
        self.inner
            .borrow_mut()
            .engine
            .price_lines
            .remove(PriceLineId(id))
    }

    /// Get the number of price lines.
    pub fn price_line_count(&self) -> usize {
        self.inner.borrow().engine.price_lines.len()
    }

    // ── Series Markers API ─────────────────────────────────────────────────────

    /// Add a marker to a series at the specified bar index.
    ///
    /// `shape`: "arrow_up", "arrow_down", "circle", "square"
    /// `position`: "above_bar", "below_bar", "at_price"
    /// `price`: Used only when position is "at_price"
    ///
    /// Returns the marker ID.
    pub fn add_marker(
        &mut self,
        series_id: u32,
        bar_index: u32,
        shape: &str,
        position: &str,
        price: f64,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
        size: f32,
        text: &str,
    ) -> u32 {
        let marker = SeriesMarker {
            bar_index: bar_index as usize,
            shape: MarkerShape::from_str(shape),
            position: MarkerPosition::from_str(position),
            price,
            color: [color_r, color_g, color_b, color_a],
            size: size as f64,
            text: text.to_string(),
            text_color: raycore::ThemeConfig::default().series_defaults.marker_text_color,
            id: 0, // will be assigned
        };
        let id = self
            .inner
            .borrow_mut()
            .engine
            .markers
            .for_series(series_id)
            .add(marker);
        log::info!(
            "add_marker: series={}, bar={}, shape={}, id={}",
            series_id,
            bar_index,
            shape,
            id
        );
        id
    }

    /// Remove a specific marker from a series.
    pub fn remove_marker(&mut self, series_id: u32, marker_id: u32) -> bool {
        self.inner
            .borrow_mut()
            .engine
            .markers
            .for_series(series_id)
            .remove(marker_id)
    }

    /// Clear all markers for a series.
    pub fn clear_markers(&mut self, series_id: u32) {
        self.inner
            .borrow_mut()
            .engine
            .markers
            .clear_series(series_id);
    }

    /// Clear all markers for all series.
    pub fn clear_all_markers(&mut self) {
        self.inner.borrow_mut().engine.markers.clear_all();
    }

    /// Set multiple markers for a series at once (replaces existing).
    /// `marker_data` is a flat array: [bar_index, shape_idx, position_idx, price, r, g, b, a, size, ...]
    /// where shape_idx: 0=arrowUp, 1=arrowDown, 2=circle, 3=square
    /// and position_idx: 0=aboveBar, 1=belowBar, 2=atPrice
    pub fn set_markers(&mut self, series_id: u32, marker_data: &[f64]) {
        const STRIDE: usize = 9; // bar_index, shape, position, price, r, g, b, a, size
        let mut markers = Vec::new();

        for chunk in marker_data.chunks_exact(STRIDE) {
            let bar_index = chunk[0] as usize;
            let shape = match chunk[1] as u32 {
                0 => MarkerShape::ArrowUp,
                1 => MarkerShape::ArrowDown,
                2 => MarkerShape::Circle,
                _ => MarkerShape::Square,
            };
            let position = match chunk[2] as u32 {
                0 => MarkerPosition::AboveBar,
                1 => MarkerPosition::BelowBar,
                _ => MarkerPosition::AtPrice,
            };
            let price = chunk[3];
            let color = [
                chunk[4] as f32,
                chunk[5] as f32,
                chunk[6] as f32,
                chunk[7] as f32,
            ];
            let size = chunk[8];

            markers.push(SeriesMarker {
                bar_index,
                shape,
                position,
                price,
                color,
                size,
                text: String::new(),
                text_color: raycore::ThemeConfig::default().series_defaults.marker_text_color,
                id: 0,
            });
        }

        self.inner
            .borrow_mut()
            .engine
            .markers
            .for_series(series_id)
            .set(markers);
        log::info!(
            "set_markers: series={}, count={}",
            series_id,
            marker_data.len() / STRIDE
        );
    }

    // ── Series overlay API ────────────────────────────────────────────────────

    /// Add a new line series overlay. Returns the series ID.
    ///
    /// Default color is TradingView blue (#2962FF). Use RGBA [0.0–1.0].
    /// `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
    pub fn add_line_series(
        &mut self,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
        line_width: f32,
        line_style: &str,
    ) -> u32 {
        let mut opts = LineSeriesOptions::default();
        opts.color = [color_r, color_g, color_b, color_a];
        opts.line_width = line_width as f64;
        opts.line_style = LineStyle::from_str(line_style);
        let id = self.inner.borrow_mut().engine.add_line_series(opts);
        log::info!("add_line_series: id={}, style={}", id.0, line_style);
        id.0
    }

    /// Add a new area series overlay. Returns the series ID.
    ///
    /// `line_color_*`: RGBA for the line stroke.
    /// `top_color_*`: RGBA for the fill at the line (top of gradient).
    /// `bottom_color_*`: RGBA for the fill at the base (bottom of gradient).
    pub fn add_area_series(
        &mut self,
        line_color_r: f32,
        line_color_g: f32,
        line_color_b: f32,
        line_color_a: f32,
        top_color_r: f32,
        top_color_g: f32,
        top_color_b: f32,
        top_color_a: f32,
        bottom_color_r: f32,
        bottom_color_g: f32,
        bottom_color_b: f32,
        bottom_color_a: f32,
        line_width: f32,
    ) -> u32 {
        let mut opts = AreaSeriesOptions::default();
        opts.line_color = [line_color_r, line_color_g, line_color_b, line_color_a];
        opts.top_color = [top_color_r, top_color_g, top_color_b, top_color_a];
        opts.bottom_color = [
            bottom_color_r,
            bottom_color_g,
            bottom_color_b,
            bottom_color_a,
        ];
        opts.line_width = line_width as f64;
        let id = self.inner.borrow_mut().engine.add_area_series(opts);
        log::info!("add_area_series: id={}", id.0);
        id.0
    }

    /// Add a new histogram series overlay. Returns the series ID.
    ///
    /// `color_*`: RGBA for the default bar color.
    /// `base`: the base value (bars extend from base to data value).
    pub fn add_histogram_series(
        &mut self,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
        base: f64,
    ) -> u32 {
        let mut opts = HistogramSeriesOptions::default();
        opts.color = [color_r, color_g, color_b, color_a];
        opts.base = base;
        let id = self.inner.borrow_mut().engine.add_histogram_series(opts);
        log::info!("add_histogram_series: id={}", id.0);
        id.0
    }

    /// Set data for a histogram series. `values` and `timestamps` must be same length.
    /// Per-bar colors are optional — pass empty arrays to use the series default color.
    pub fn set_histogram_data(
        &mut self,
        id: u32,
        values: &[f32],
        timestamps: &[u64],
        colors_r: &[f32],
        colors_g: &[f32],
        colors_b: &[f32],
        colors_a: &[f32],
    ) -> Result<(), JsValue> {
        ensure_equal_len("values", values.len(), "timestamps", timestamps.len())?;
        ensure_finite_slice("values", values)?;
        let count = values.len();

        let has_any_color = !colors_r.is_empty()
            || !colors_g.is_empty()
            || !colors_b.is_empty()
            || !colors_a.is_empty();
        let has_colors = if has_any_color {
            ensure_equal_len("colors_r", colors_r.len(), "values", count)?;
            ensure_equal_len("colors_g", colors_g.len(), "values", count)?;
            ensure_equal_len("colors_b", colors_b.len(), "values", count)?;
            ensure_equal_len("colors_a", colors_a.len(), "values", count)?;
            ensure_finite_slice("colors_r", colors_r)?;
            ensure_finite_slice("colors_g", colors_g)?;
            ensure_finite_slice("colors_b", colors_b)?;
            ensure_finite_slice("colors_a", colors_a)?;
            true
        } else {
            false
        };

        let mut s = self.inner.borrow_mut();
        if has_colors {
            let data: Vec<HistogramPoint> = (0..count)
                .map(|i| HistogramPoint {
                    timestamp: timestamps[i],
                    value: values[i],
                    color: [colors_r[i], colors_g[i], colors_b[i], colors_a[i]],
                })
                .collect();
            s.engine
                .set_histogram_data(SeriesId(id), data)
                .map_err(js_err)?;
        } else {
            s.engine
                .set_histogram_data_arrays(SeriesId(id), timestamps, values)
                .map_err(js_err)?;
        }
        log::info!(
            "set_histogram_data: id={}, {} points, colors={}",
            id,
            count,
            has_colors
        );
        Ok(())
    }

    /// Add a new bar (OHLC) series overlay. Returns the series ID.
    ///
    /// `up_color_*`: RGBA for bullish bars (close >= open).
    /// `down_color_*`: RGBA for bearish bars (close < open).
    /// `open_visible`: whether to show the open tick.
    /// `thin_bars`: use 1px stems (like LWC thinBars option).
    pub fn add_bar_series(
        &mut self,
        up_color_r: f32,
        up_color_g: f32,
        up_color_b: f32,
        up_color_a: f32,
        down_color_r: f32,
        down_color_g: f32,
        down_color_b: f32,
        down_color_a: f32,
        open_visible: bool,
        thin_bars: bool,
    ) -> u32 {
        let mut opts = BarSeriesOptions::default();
        opts.up_color = [up_color_r, up_color_g, up_color_b, up_color_a];
        opts.down_color = [down_color_r, down_color_g, down_color_b, down_color_a];
        opts.open_visible = open_visible;
        opts.thin_bars = thin_bars;
        let id = self.inner.borrow_mut().engine.add_bar_series(opts);
        log::info!("add_bar_series: id={}", id.0);
        id.0
    }

    /// Set data for a bar (OHLC) series.
    /// All arrays must be the same length.
    pub fn set_bar_series_data(
        &mut self,
        id: u32,
        timestamps: &[u64],
        open: &[f32],
        high: &[f32],
        low: &[f32],
        close: &[f32],
    ) -> Result<(), JsValue> {
        ensure_equal_len("timestamps", timestamps.len(), "open", open.len())?;
        ensure_equal_len("timestamps", timestamps.len(), "high", high.len())?;
        ensure_equal_len("timestamps", timestamps.len(), "low", low.len())?;
        ensure_equal_len("timestamps", timestamps.len(), "close", close.len())?;
        ensure_finite_slice("open", open)?;
        ensure_finite_slice("high", high)?;
        ensure_finite_slice("low", low)?;
        ensure_finite_slice("close", close)?;

        let mut s = self.inner.borrow_mut();
        s.engine
            .set_bar_data_arrays(SeriesId(id), timestamps, open, high, low, close)
            .map_err(js_err)?;
        let count = timestamps.len();
        log::info!("set_bar_series_data: id={}, {} bars", id, count);
        Ok(())
    }

    /// Add a new baseline series overlay. Returns the series ID.
    ///
    /// A baseline series renders a line with two-tone fill above/below a base value.
    /// Above the base: `top_line_color` line + `top_fill_color1`→`top_fill_color2` gradient.
    /// Below the base: `bottom_line_color` line + `bottom_fill_color1`→`bottom_fill_color2` gradient.
    pub fn add_baseline_series(
        &mut self,
        base_value: f64,
        top_line_r: f32,
        top_line_g: f32,
        top_line_b: f32,
        top_line_a: f32,
        bottom_line_r: f32,
        bottom_line_g: f32,
        bottom_line_b: f32,
        bottom_line_a: f32,
        top_fill1_r: f32,
        top_fill1_g: f32,
        top_fill1_b: f32,
        top_fill1_a: f32,
        top_fill2_r: f32,
        top_fill2_g: f32,
        top_fill2_b: f32,
        top_fill2_a: f32,
        bottom_fill1_r: f32,
        bottom_fill1_g: f32,
        bottom_fill1_b: f32,
        bottom_fill1_a: f32,
        bottom_fill2_r: f32,
        bottom_fill2_g: f32,
        bottom_fill2_b: f32,
        bottom_fill2_a: f32,
        line_width: f32,
    ) -> u32 {
        let mut opts = BaselineSeriesOptions::default();
        opts.base_value = base_value;
        opts.top_line_color = [top_line_r, top_line_g, top_line_b, top_line_a];
        opts.bottom_line_color = [bottom_line_r, bottom_line_g, bottom_line_b, bottom_line_a];
        opts.top_fill_color1 = [top_fill1_r, top_fill1_g, top_fill1_b, top_fill1_a];
        opts.top_fill_color2 = [top_fill2_r, top_fill2_g, top_fill2_b, top_fill2_a];
        opts.bottom_fill_color1 = [
            bottom_fill1_r,
            bottom_fill1_g,
            bottom_fill1_b,
            bottom_fill1_a,
        ];
        opts.bottom_fill_color2 = [
            bottom_fill2_r,
            bottom_fill2_g,
            bottom_fill2_b,
            bottom_fill2_a,
        ];
        opts.line_width = line_width as f64;
        let id = self.inner.borrow_mut().engine.add_baseline_series(opts);
        log::info!(
            "add_baseline_series: id={}, base_value={}",
            id.0,
            base_value
        );
        id.0
    }

    /// Set data for a line series. `values` and `timestamps` must be same length.
    pub fn set_series_data(
        &mut self,
        id: u32,
        values: &[f32],
        timestamps: &[u64],
    ) -> Result<(), JsValue> {
        ensure_equal_len("values", values.len(), "timestamps", timestamps.len())?;
        ensure_finite_slice("values", values)?;
        let count = values.len();
        let data: Vec<LinePoint> = (0..count)
            .map(|i| LinePoint {
                timestamp: timestamps[i],
                value: values[i],
            })
            .collect();
        self.inner
            .borrow_mut()
            .engine
            .set_series_data(SeriesId(id), data)
            .map_err(js_err)?;
        log::info!("set_series_data: id={}, {} points", id, count);
        Ok(())
    }

    /// Remove a series by ID.
    pub fn remove_series(&mut self, id: u32) -> bool {
        let removed = self.inner.borrow_mut().engine.remove_series(SeriesId(id));
        log::info!("remove_series: id={}, removed={}", id, removed);
        removed
    }

    /// Show or hide a series.
    pub fn set_series_visible(&mut self, id: u32, visible: bool) {
        self.inner
            .borrow_mut()
            .engine
            .set_series_visible(SeriesId(id), visible);
    }

    /// Get the number of overlay series.
    pub fn series_count(&self) -> usize {
        self.inner.borrow().engine.series.len()
    }

    // ── Study API ─────────────────────────────────────────────────────────

    /// Create a new study instance. Returns the study ID, or 0 if the type is unknown.
    ///
    /// Supported types: "sma", "ema", "rsi", "macd".
    pub fn create_study(&mut self, study_type: &str) -> u32 {
        let mut s = self.inner.borrow_mut();
        match s.engine.create_study(study_type) {
            Some(id) => {
                // If we have bar data, run initial calculation
                if s.engine.bars.len() > 0 {
                    s.engine.recalculate_studies();
                }
                log::info!("create_study: type='{}', id={}", study_type, id.0);
                id.0
            }
            None => {
                log::warn!("create_study: unknown type '{}'", study_type);
                0
            }
        }
    }

    /// Remove a study by ID.
    pub fn remove_study(&mut self, id: u32) -> bool {
        let removed = self
            .inner
            .borrow_mut()
            .engine
            .remove_study(raycore::StudyId(id));
        log::info!("remove_study: id={}, removed={}", id, removed);
        removed
    }

    /// Set a study parameter (e.g., "period" for SMA/EMA, "fast_period" for MACD).
    /// The study will be recalculated on the next render.
    pub fn set_study_parameter(&mut self, id: u32, key: &str, value: f64) {
        let mut s = self.inner.borrow_mut();
        s.engine
            .set_study_parameter(raycore::StudyId(id), key, value);
        // Recalculate immediately if we have data
        if s.engine.bars.len() > 0 {
            s.engine.recalculate_studies();
        }
        log::info!("set_study_parameter: id={}, {}={}", id, key, value);
    }

    /// Get study output data as a JS object { timestamps: BigUint64Array, values: Float32Array }.
    /// Returns null if the study or output index doesn't exist.
    pub fn get_study_output(&self, id: u32, output_index: u32) -> JsValue {
        let s = self.inner.borrow();
        if let Some(study) = s.engine.studies.get_study(raycore::StudyId(id)) {
            if let Some(output) = study.get_output(output_index as usize) {
                let obj = js_sys::Object::new();
                let ts_arr =
                    js_sys::BigUint64Array::new_with_length(output.data.timestamps.len() as u32);
                let val_arr =
                    js_sys::Float32Array::new_with_length(output.data.values.len() as u32);
                // Copy data
                for i in 0..output.data.timestamps.len() {
                    ts_arr.set_index(i as u32, output.data.timestamps[i]);
                }
                for i in 0..output.data.values.len() {
                    val_arr.set_index(i as u32, output.data.values[i]);
                }
                let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("timestamps"), &ts_arr);
                let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("values"), &val_arr);
                let _ = js_sys::Reflect::set(
                    &obj,
                    &JsValue::from_str("name"),
                    &JsValue::from_str(&output.name),
                );
                return obj.into();
            }
        }
        JsValue::NULL
    }

    /// Get the number of studies.
    pub fn study_count(&self) -> usize {
        self.inner.borrow().engine.study_count()
    }

    // ── Indicator Sub-Panes ──────────────────────────────────────────────────

    /// Create a new indicator sub-pane below the main chart.
    /// Returns the pane ID. The indicator type should be one of: "rsi", "stochastic", "atr".
    /// The study must already be created with `create_study()`.
    pub fn add_indicator_pane(
        &mut self,
        study_id: u32,
        indicator_type: &str,
        height_css: f64,
    ) -> u32 {
        let inner_for_events = Rc::clone(&self.inner);

        // ── Phase 1: Create sub-pane, extract DOM refs + shared state ──
        let creation_result: Option<(
            u32,
            web_sys::Element,
            web_sys::Element,
            Rc<Cell<f64>>,
            Rc<Cell<bool>>,
        )> = {
            let mut s = self.inner.borrow_mut();

            let window = match web_sys::window() {
                Some(w) => w,
                None => return 0,
            };
            let doc = match window.document() {
                Some(d) => d,
                None => return 0,
            };

            let id = s.next_subpane_id;
            s.next_subpane_id += 1;
            let dpr = s.engine.dpr;

            let grid_row = 2 + (s.subpanes.len() as u32 * 2);

            // Use IndicatorConfig for colors
            let config = IndicatorConfig::for_type(indicator_type);

            // Register with coordinator to get coordinated height
            s.pane_coordinator.register_subpane(id);

            // Get total height and update coordinator
            let (_, total_h) = s.layout.pane_css_size();
            let subpane_count = s.subpanes.len();
            // Reserve space: main pane + subpane heights + time axis
            let time_axis_h = s.engine.style.time_axis_height();
            let available_h =
                total_h + (subpane_count as f64 * height_css) + height_css + time_axis_h;
            s.pane_coordinator.set_total_height(available_h);

            // Use coordinator's computed height, or fall back to requested height
            let coordinated_height = s.pane_coordinator.get_height(id);
            let initial_height = if coordinated_height > 0.0 {
                coordinated_height
            } else {
                height_css
            };

            let mut subpane = match SubPane::new(
                &doc,
                &s.layout.grid_wrapper,
                id,
                study_id,
                indicator_type,
                grid_row,
                initial_height,
                dpr,
                &s.engine.style,
                &s.subpane_separator_style,
            ) {
                Ok(sp) => sp,
                Err(e) => {
                    log::error!("Failed to create sub-pane: {:?}", e);
                    s.pane_coordinator.unregister_subpane(id);
                    return 0;
                }
            };

            // Populate with current study data using config colors
            if let Some(study) = s.engine.studies.get_study(raycore::StudyId(study_id)) {
                let mut data = Vec::new();
                let mut colors = Vec::new();
                for i in 0..study.outputs.len() {
                    if let Some(output) = study.get_output(i) {
                        data.push(output.data.clone());
                        colors.push(
                            config
                                .colors
                                .get(i)
                                .copied()
                                .unwrap_or(raycore::ThemeConfig::default().indicator_palette.fallback),
                        );
                    }
                }
                subpane.set_data(data, colors);
            }

            subpane.resize(dpr);

            // Extract refs before pushing into vec
            let chart_el: web_sys::Element = subpane.chart_container.clone().unchecked_into();
            let axis_el: web_sys::Element = subpane.axis_container.clone().unchecked_into();
            let crosshair_y_rc = subpane.crosshair_y.clone();
            let crosshair_active_rc = subpane.crosshair_active.clone();

            s.subpanes.push(subpane);

            log::info!(
                "Created indicator sub-pane: id={}, type={}, height={:.1}",
                id,
                indicator_type,
                initial_height
            );
            Some((id, chart_el, axis_el, crosshair_y_rc, crosshair_active_rc))
        }; // borrow dropped

        let (id, chart_el, axis_el, crosshair_y, crosshair_active) = match creation_result {
            Some(r) => r,
            None => return 0,
        };

        // ── Phase 2: Wire interaction events ──
        let mut interaction_closures: Vec<Closure<dyn FnMut(web_sys::Event)>> = Vec::new();
        let mut wheel_closures: Vec<Closure<dyn FnMut(web_sys::WheelEvent)>> = Vec::new();
        let mut touch_closures: Vec<Closure<dyn FnMut(web_sys::TouchEvent)>> = Vec::new();

        // Shared drag state for chart scroll (X and Y axis)
        let sp_drag_active = Rc::new(Cell::new(false));
        let sp_drag_start_x: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_drag_start_y: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_drag_start_start_bar: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_drag_start_end_bar: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_drag_start_price_min: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_drag_start_price_max: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));

        // Shared drag state for price axis drag-to-scale
        let axis_drag_active = Rc::new(Cell::new(false));
        let axis_drag_start_y: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let axis_drag_start_price_min: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let axis_drag_start_price_max: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));

        // Pinch zoom state
        let sp_pinch_active = Rc::new(Cell::new(false));
        let sp_pinch_start_dist: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_pinch_start_start_bar: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_pinch_start_end_bar: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_pinch_center_x: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));

        // Touch detection for kinetic scrolling (only enable on touch devices, like main chart)
        let sp_is_touch = Rc::new(Cell::new(false));

        let pane_id = id;

        // ── chart: pointerenter ──
        {
            let inner = Rc::clone(&inner_for_events);
            let ca = crosshair_active.clone();
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_: web_sys::Event| {
                    log::info!("SubPane {} pointerenter", pid);
                    ca.set(true);
                    let mut s = inner.borrow_mut();
                    s.engine.crosshair.active = true;
                    s.active_subpane_id = Some(pid);
                }));
            let _ = chart_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointerleave ──
        {
            let inner = Rc::clone(&inner_for_events);
            let ca = crosshair_active.clone();
            let drag = sp_drag_active.clone();
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_: web_sys::Event| {
                    log::info!("SubPane {} pointerleave", pid);
                    ca.set(false);
                    if !drag.get() {
                        let mut s = inner.borrow_mut();
                        s.engine.crosshair.active = false;
                        s.active_subpane_id = None;
                    }
                }));
            let _ = chart_el
                .add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointermove ──
        {
            let inner = Rc::clone(&inner_for_events);
            let cy = crosshair_y.clone();
            let ca = crosshair_active.clone();
            let chart_c = chart_el.clone();
            let drag = sp_drag_active.clone();
            let drag_sx = sp_drag_start_x.clone();
            let drag_sy = sp_drag_start_y.clone();
            let drag_sb = sp_drag_start_start_bar.clone();
            let drag_eb = sp_drag_start_end_bar.clone();
            let drag_pmin = sp_drag_start_price_min.clone();
            let drag_pmax = sp_drag_start_price_max.clone();
            let is_touch = sp_is_touch.clone();
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let rect = chart_c.get_bounding_client_rect();
                    let x = pe.client_x() as f64 - rect.left();
                    let y = pe.client_y() as f64 - rect.top();
                    let pw = rect.width();
                    let ph = rect.height();
                    let now_ms = js_sys::Date::now();

                    cy.set(y);
                    ca.set(true);

                    let mut s = inner.borrow_mut();
                    s.engine.crosshair.active = true;
                    s.active_subpane_id = Some(pid);

                    // Get grid-snapped index (can be beyond data_len in empty space)
                    let grid_idx = s.engine.viewport.bar_index_for_crosshair(x, pw);

                    // bar_index is only set if we have actual data at this position
                    s.engine.crosshair.bar_index =
                        grid_idx.filter(|&idx| idx < s.engine.bars.len());

                    // X snaps to bar grid (like LWC) - even in empty space
                    if let Some(idx) = grid_idx {
                        s.engine.crosshair.x = s.engine.viewport.bar_center_css(idx, pw);
                    } else {
                        s.engine.crosshair.x = x;
                    }

                    // Get bar coordinate from main viewport (shared time axis)
                    let bar = s.engine.viewport.pixel_to_bar(x, pw);

                    // Update drawing preview or drag if active
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        let price = sp.viewport.pixel_to_price(y, ph);

                        // Update drawing creation preview
                        if sp.drawings.is_creating() {
                            sp.drawings.update_creation_preview(bar, price);
                        }

                        // Update drawing drag
                        if let Some(id) = sp.drawings.selected_id {
                            if matches!(
                                sp.drawings.get(id).map(|d| d.state()),
                                Some(raycore::core::drawings::types::DrawingState::Dragging { .. })
                            ) {
                                sp.drawings.update_drag(id, bar, price);
                            }
                        }
                    }

                    // Handle drag scroll (only if not in drawing mode)
                    if drag.get() {
                        // Update scroll tracking for kinetic animation (touch only)
                        if is_touch.get() {
                            if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                                sp.scroll_state.borrow_mut().update_drag(x, now_ms);
                            }
                        }

                        // Horizontal drag - scroll time axis (shared with main chart)
                        let delta_x = x - drag_sx.get();
                        let bar_range = drag_eb.get() - drag_sb.get();
                        if pw > 0.0 {
                            let bars_per_px = bar_range / pw;
                            let delta_bars = -delta_x * bars_per_px;
                            s.engine.viewport.start_bar = drag_sb.get() + delta_bars;
                            s.engine.viewport.end_bar = drag_eb.get() + delta_bars;
                            let bar_len = s.engine.bars.len();
                            s.engine.viewport.clamp_to_data(bar_len);
                            // Main chart auto-fits if not locked
                            s.engine.auto_fit_price_if_unlocked();
                        }

                        // Vertical drag - ONLY if subpane price is locked (same as main chart)
                        if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                            if sp.viewport.price_locked {
                                let delta_y = y - drag_sy.get();
                                let price_range = drag_pmax.get() - drag_pmin.get();
                                if ph > 1.0 && price_range > 0.0 {
                                    let price_per_px = price_range / (ph - 1.0);
                                    let price_delta = delta_y * price_per_px;
                                    sp.viewport.price_min = drag_pmin.get() + price_delta;
                                    sp.viewport.price_max = drag_pmax.get() + price_delta;
                                }
                            }
                            // If not locked, auto-scale would happen in render (for indicators with data)
                        }

                        let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                        let _ = html_el.style().set_property("cursor", "grabbing");
                    }
                }));
            let _ = chart_el
                .add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointerdown ──
        {
            let inner = Rc::clone(&inner_for_events);
            let drag = sp_drag_active.clone();
            let drag_sx = sp_drag_start_x.clone();
            let drag_sy = sp_drag_start_y.clone();
            let drag_sb = sp_drag_start_start_bar.clone();
            let drag_eb = sp_drag_start_end_bar.clone();
            let drag_pmin = sp_drag_start_price_min.clone();
            let drag_pmax = sp_drag_start_price_max.clone();
            let chart_c = chart_el.clone();
            let is_touch = sp_is_touch.clone();
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    if pe.button() == 2 {
                        return;
                    }
                    let rect = chart_c.get_bounding_client_rect();
                    let x = pe.client_x() as f64 - rect.left();
                    let y = pe.client_y() as f64 - rect.top();
                    let pw = rect.width();
                    let ph = rect.height();
                    let now_ms = js_sys::Date::now();

                    // Detect touch input (same as main chart)
                    is_touch.set(pe.pointer_type() == "touch");

                    let mut s = inner.borrow_mut();

                    // Get bar coordinate from main viewport (shared time axis)
                    let bar = s.engine.viewport.pixel_to_bar(x, pw);

                    // Check if drawing tool is active (shared from main chart)
                    let active_tool = s.engine.drawings.active_tool;
                    if active_tool != raycore::DrawingTool::None {
                        if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                            let price = sp.viewport.pixel_to_price(y, ph);

                            // Set the same tool on this subpane's DrawingManager
                            sp.drawings.active_tool = active_tool;

                            if !sp.drawings.is_creating() {
                                sp.drawings.start_creating(bar, price);
                            } else {
                                // Multi-step tools: place next anchor on click
                                sp.drawings.finalize_creation_step(bar, price);
                            }
                        }
                        drop(s);
                        return; // Don't pan while creating drawing
                    }

                    // Pre-read main viewport values for hybrid viewport
                    let main_start_bar = s.engine.viewport.start_bar;
                    let main_end_bar = s.engine.viewport.end_bar;

                    // Check for existing drawing hit-test in this subpane
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        let price = sp.viewport.pixel_to_price(y, ph);

                        // Create hybrid viewport for hit-test (main time + subpane price)
                        let mut hybrid_vp = Viewport::new(pw as u32, ph as u32);
                        hybrid_vp.start_bar = main_start_bar;
                        hybrid_vp.end_bar = main_end_bar;
                        hybrid_vp.price_min = sp.viewport.price_min;
                        hybrid_vp.price_max = sp.viewport.price_max;
                        hybrid_vp.volume_height_ratio = 0.0;

                        let hit = sp.drawings.hit_test(x, y, &hybrid_vp, pw, ph);
                        if let Some((id, result)) = hit {
                            use raycore::core::drawings::types::HitPart;
                            let tool = sp
                                .drawings
                                .get(id)
                                .map(|d| d.tool())
                                .unwrap_or(raycore::DrawingTool::None);
                            let anchor_idx = match result.part {
                                HitPart::Anchor(i) => Some(i),
                                _ => None,
                            };

                            // Rectangle: body clicks select only (fall through to pan)
                            // Edge/anchor clicks start drag
                            if tool == raycore::DrawingTool::Rectangle
                                && result.part == HitPart::Body
                            {
                                sp.drawings.select(id);
                            } else {
                                sp.drawings.select(id);
                                sp.drawings.start_drag(id, anchor_idx, bar, price);
                                drop(s);
                                return; // Don't pan while dragging drawing
                            }
                        } else {
                            // Click on empty space: deselect
                            sp.drawings.deselect_all();
                        }
                    }

                    drag.set(true);
                    drag_sx.set(x);
                    drag_sy.set(y);
                    drag_sb.set(s.engine.viewport.start_bar);
                    drag_eb.set(s.engine.viewport.end_bar);

                    // Capture subpane's price range for vertical dragging
                    if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                        drag_pmin.set(sp.viewport.price_min);
                        drag_pmax.set(sp.viewport.price_max);
                    }

                    // Start scroll tracking for kinetic animation (touch only)
                    if is_touch.get() {
                        if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                            sp.scroll_state.borrow_mut().start_drag(
                                x,
                                s.engine.viewport.start_bar,
                                now_ms,
                            );
                        }
                    }
                    drop(s);

                    // Capture pointer for reliable drag across boundaries
                    let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                    let _ = html_el.set_pointer_capture(pe.pointer_id());
                }));
            let _ = chart_el
                .add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointerup ──
        {
            let inner = Rc::clone(&inner_for_events);
            let drag = sp_drag_active.clone();
            let chart_c = chart_el.clone();
            let is_touch = sp_is_touch.clone();
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let rect = chart_c.get_bounding_client_rect();
                    let x = pe.client_x() as f64 - rect.left();
                    let y = pe.client_y() as f64 - rect.top();
                    let pw = rect.width();
                    let ph = rect.height();
                    let now_ms = js_sys::Date::now();
                    drag.set(false);

                    // Finalize drawing creation or drag
                    {
                        let mut s = inner.borrow_mut();
                        let bar = s.engine.viewport.pixel_to_bar(x, pw);
                        if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                            let price = sp.viewport.pixel_to_price(y, ph);

                            // Finalize drawing creation (drag-to-create style)
                            if sp.drawings.is_creating() {
                                sp.drawings.finalize_creation_step(bar, price);
                            }

                            // End any drawing drag
                            if let Some(id) = sp.drawings.selected_id {
                                sp.drawings.end_drag(id);
                            }
                        }
                    }

                    // End scroll tracking and potentially start kinetic animation (TOUCH ONLY)
                    {
                        let s = inner.borrow();
                        if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                            if is_touch.get() {
                                // Touch: end drag and start kinetic animation
                                sp.scroll_state.borrow_mut().end_drag(now_ms);
                            } else {
                                // Mouse: just stop any animation, no kinetic scrolling
                                sp.scroll_state.borrow_mut().animation.stop();
                            }
                        }
                    }

                    let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let _ = html_el.style().set_property("cursor", "crosshair");
                }));
            let _ =
                chart_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointercancel ──
        {
            let inner = Rc::clone(&inner_for_events);
            let drag = sp_drag_active.clone();
            let chart_c = chart_el.clone();
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    drag.set(false);

                    // Cancel scroll tracking (no kinetic animation)
                    {
                        let s = inner.borrow();
                        if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                            sp.scroll_state.borrow_mut().animation.stop();
                        }
                    }

                    let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let _ = html_el.style().set_property("cursor", "crosshair");
                }));
            let _ = chart_el
                .add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: wheel (forward to main chart zoom) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let chart_c = chart_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let rect = chart_c.get_bounding_client_rect();
                    let x = e.client_x() as f64 - rect.left();
                    let mut s = inner.borrow_mut();
                    s.on_pane_wheel(x, e.delta_x(), e.delta_y(), e.delta_mode());
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = chart_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel",
                cb.as_ref().unchecked_ref(),
                &opts,
            );
            wheel_closures.push(cb);
        }

        // ── chart: contextmenu ──
        {
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    e.prevent_default();
                }));
            let _ = chart_el
                .add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: touchstart (pinch zoom detection) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let chart_c = chart_el.clone();
            let pinch = sp_pinch_active.clone();
            let pinch_dist = sp_pinch_start_dist.clone();
            let pinch_sb = sp_pinch_start_start_bar.clone();
            let pinch_eb = sp_pinch_start_end_bar.clone();
            let pinch_cx = sp_pinch_center_x.clone();
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(
                move |e: web_sys::TouchEvent| {
                    e.prevent_default();
                    let touches = e.touches();
                    if touches.length() >= 2 {
                        let t0 = touches.get(0).unwrap();
                        let t1 = touches.get(1).unwrap();
                        let rect = chart_c.get_bounding_client_rect();
                        let x0 = t0.client_x() as f64 - rect.left();
                        let x1 = t1.client_x() as f64 - rect.left();
                        let y0 = t0.client_y() as f64 - rect.top();
                        let y1 = t1.client_y() as f64 - rect.top();
                        let dx = x1 - x0;
                        let dy = y1 - y0;
                        let distance = (dx * dx + dy * dy).sqrt();

                        pinch.set(true);
                        pinch_dist.set(distance);
                        pinch_cx.set((x0 + x1) / 2.0);

                        let s = inner.borrow();
                        pinch_sb.set(s.engine.viewport.start_bar);
                        pinch_eb.set(s.engine.viewport.end_bar);
                    }
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = chart_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchstart",
                cb.as_ref().unchecked_ref(),
                &opts,
            );
            touch_closures.push(cb);
        }

        // ── chart: touchmove (pinch zoom update) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pinch = sp_pinch_active.clone();
            let pinch_dist = sp_pinch_start_dist.clone();
            let pinch_sb = sp_pinch_start_start_bar.clone();
            let pinch_eb = sp_pinch_start_end_bar.clone();
            let pinch_cx = sp_pinch_center_x.clone();
            let chart_c = chart_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(
                move |e: web_sys::TouchEvent| {
                    e.prevent_default();
                    let touches = e.touches();
                    if touches.length() >= 2 && pinch.get() {
                        let t0 = touches.get(0).unwrap();
                        let t1 = touches.get(1).unwrap();
                        let dx = (t1.client_x() - t0.client_x()) as f64;
                        let dy = (t1.client_y() - t0.client_y()) as f64;
                        let distance = (dx * dx + dy * dy).sqrt();
                        let scale = distance / pinch_dist.get().max(1.0);

                        // Apply zoom: scale around the pinch center
                        let rect = chart_c.get_bounding_client_rect();
                        let css_w = rect.width();
                        let start = pinch_sb.get();
                        let end = pinch_eb.get();
                        let visible = end - start;
                        let new_visible = visible / scale;
                        let cx_frac = pinch_cx.get() / css_w;
                        let new_start = start + (visible - new_visible) * cx_frac;
                        let new_end = new_start + new_visible;

                        let mut s = inner.borrow_mut();
                        s.engine.viewport.start_bar = new_start;
                        s.engine.viewport.end_bar = new_end;
                        let bar_len = s.engine.bars.len();
                        s.engine.viewport.clamp_to_data(bar_len);
                    }
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = chart_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchmove",
                cb.as_ref().unchecked_ref(),
                &opts,
            );
            touch_closures.push(cb);
        }

        // ── chart: touchend (end pinch) ──
        {
            let pinch = sp_pinch_active.clone();
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(
                move |e: web_sys::TouchEvent| {
                    let touches = e.touches();
                    if touches.length() < 2 {
                        pinch.set(false);
                    }
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = chart_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchend",
                cb.as_ref().unchecked_ref(),
                &opts,
            );
            touch_closures.push(cb);
        }

        // ── chart: dblclick (reset viewport to default) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let mut s = inner.borrow_mut();
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        sp.reset_price_viewport();
                        log::info!("SubPane {} viewport reset via double-click", pid);
                    }
                }));
            let _ =
                chart_el.add_event_listener_with_callback("dblclick", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: dblclick (toggle auto-scale) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let mut s = inner.borrow_mut();
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        sp.toggle_auto_scale();
                        log::info!("SubPane {} auto-scale toggled: {}", pid, sp.auto_scale);
                    }
                }));
            let _ =
                axis_el.add_event_listener_with_callback("dblclick", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: wheel (zoom sub-pane price range) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let dy = e.delta_y();
                    let factor = if dy > 0.0 { 1.1 } else { 0.9 };
                    let mut s = inner.borrow_mut();
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        let center = (sp.viewport.price_min + sp.viewport.price_max) / 2.0;
                        let half = (sp.viewport.price_max - sp.viewport.price_min) / 2.0 * factor;
                        sp.viewport.price_min = center - half;
                        sp.viewport.price_max = center + half;
                        // Lock price axis after manual zoom (same as main chart)
                        sp.viewport.price_locked = true;
                    }
                },
            ));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = axis_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel",
                cb.as_ref().unchecked_ref(),
                &opts,
            );
            wheel_closures.push(cb);
        }

        // ── axis: pointerdown (start price axis drag) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pid = pane_id;
            let adrag = axis_drag_active.clone();
            let ady = axis_drag_start_y.clone();
            let apmin = axis_drag_start_price_min.clone();
            let apmax = axis_drag_start_price_max.clone();
            let axis_c = axis_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let rect = axis_c.get_bounding_client_rect();
                    let y = pe.client_y() as f64 - rect.top();
                    adrag.set(true);
                    ady.set(y);
                    let s = inner.borrow();
                    if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                        apmin.set(sp.viewport.price_min);
                        apmax.set(sp.viewport.price_max);
                    }
                    drop(s);
                    let html_el: &web_sys::HtmlElement = axis_c.unchecked_ref();
                    let _ = html_el.set_pointer_capture(pe.pointer_id());
                }));
            let _ = axis_el
                .add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: pointermove (price axis drag-to-scale) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pid = pane_id;
            let adrag = axis_drag_active.clone();
            let ady = axis_drag_start_y.clone();
            let apmin = axis_drag_start_price_min.clone();
            let apmax = axis_drag_start_price_max.clone();
            let axis_c = axis_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    if !adrag.get() {
                        return;
                    }
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let rect = axis_c.get_bounding_client_rect();
                    let y = pe.client_y() as f64 - rect.top();
                    let css_h = rect.height();
                    if css_h <= 1.0 {
                        return;
                    }

                    let delta_y = y - ady.get();
                    let factor = (1.0 + delta_y / css_h).max(0.1);

                    let center = (apmin.get() + apmax.get()) / 2.0;
                    let half = (apmax.get() - apmin.get()) / 2.0 * factor;

                    let mut s = inner.borrow_mut();
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        sp.viewport.price_min = center - half;
                        sp.viewport.price_max = center + half;
                        // Lock price axis after manual scaling (same as main chart)
                        sp.viewport.price_locked = true;
                    }
                }));
            let _ = axis_el
                .add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: pointerup ──
        {
            let adrag = axis_drag_active.clone();
            let axis_c = axis_el.clone();
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    adrag.set(false);
                    let html_el: &web_sys::HtmlElement = axis_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                }));
            let _ =
                axis_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: contextmenu ──
        {
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    e.prevent_default();
                }));
            let _ = axis_el
                .add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── Phase 3: Store closures in sub-pane (prevents GC) ──
        {
            let mut s = self.inner.borrow_mut();
            if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == id) {
                sp._interaction_closures = interaction_closures;
                sp._wheel_closures = wheel_closures;
                sp._touch_closures = touch_closures;
            }
        }

        id
    }

    /// Remove an indicator sub-pane by ID.
    pub fn remove_indicator_pane(&mut self, pane_id: u32) -> bool {
        let mut s = self.inner.borrow_mut();
        if let Some(pos) = s.subpanes.iter().position(|sp| sp.id == pane_id) {
            let sp = s.subpanes.remove(pos);
            sp.remove();

            // Unregister from coordinator
            s.pane_coordinator.unregister_subpane(pane_id);

            // Reassign grid rows for remaining subpanes
            for (i, subpane) in s.subpanes.iter_mut().enumerate() {
                let sep_row = 2 + (i as u32 * 2);
                let pane_row = sep_row + 1;
                subpane.grid_row = sep_row;
                let _ = subpane
                    .separator
                    .style()
                    .set_property("grid-row", &sep_row.to_string());
                let _ = subpane
                    .chart_container
                    .style()
                    .set_property("grid-row", &pane_row.to_string());
                let _ = subpane
                    .axis_container
                    .style()
                    .set_property("grid-row", &pane_row.to_string());
            }
            // Grid rows will be corrected on next render frame

            log::info!("Removed indicator sub-pane: id={}", pane_id);
            true
        } else {
            false
        }
    }

    /// Update indicator sub-pane data from a study.
    pub fn update_indicator_pane(&mut self, pane_id: u32, study_id: u32) {
        let mut s = self.inner.borrow_mut();

        let study_data: Option<(Vec<raycore::core::series::LineDataArray>, String)> = {
            if let Some(study) = s.engine.studies.get_study(raycore::StudyId(study_id)) {
                let mut data = Vec::new();
                for i in 0..study.outputs.len() {
                    if let Some(output) = study.get_output(i) {
                        data.push(output.data.clone());
                    }
                }
                Some((data, study.study_type.clone()))
            } else {
                None
            }
        };

        if let Some((data, indicator_type)) = study_data {
            if let Some(subpane) = s.subpanes.iter_mut().find(|sp| sp.id == pane_id) {
                // Use IndicatorConfig for colors instead of hardcoded values
                let config = IndicatorConfig::for_type(&indicator_type);
                let colors: Vec<[f32; 4]> = data
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        config
                            .colors
                            .get(i)
                            .copied()
                            .unwrap_or(raycore::ThemeConfig::default().indicator_palette.fallback)
                    })
                    .collect();

                subpane.set_data(data, colors);
            }
        }
    }

    /// Drag a separator to resize adjacent panes.
    /// `separator_idx` is 0 for separator between main and first subpane.
    /// `delta_y` is positive for moving down, negative for up.
    /// This uses the PaneManager's coordinated height algorithm.
    pub fn drag_pane_separator(&mut self, separator_idx: u32, delta_y: f64) {
        let mut s = self.inner.borrow_mut();

        // Drag using coordinator (PaneManager)
        s.pane_coordinator
            .drag_separator(separator_idx as usize, delta_y);

        // Sync computed heights back to subpanes
        let heights = s.pane_coordinator.all_heights();
        for (subpane_id, height) in heights {
            if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == subpane_id) {
                sp.set_height(height);
            }
        }
    }

    /// Get the number of indicator sub-panes.
    pub fn indicator_pane_count(&self) -> usize {
        self.inner.borrow().subpanes.len()
    }

    // ── Real-time data updates ────────────────────────────────────────────

    /// Append a single bar to the data array. Used for real-time streaming.
    pub fn append_bar(
        &mut self,
        timestamp: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        volume: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "append_bar",
            &[
                ("open", open),
                ("high", high),
                ("low", low),
                ("close", close),
                ("volume", volume),
            ],
        )?;
        let bar = Bar {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            _pad: 0.0,
        };
        self.inner
            .borrow_mut()
            .engine
            .append_bar(bar)
            .map_err(js_err)
    }

    /// Update the last bar in the data array. Used for real-time tick updates.
    pub fn update_last_bar(
        &mut self,
        timestamp: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        volume: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "update_last_bar",
            &[
                ("open", open),
                ("high", high),
                ("low", low),
                ("close", close),
                ("volume", volume),
            ],
        )?;
        let bar = Bar {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            _pad: 0.0,
        };
        self.inner
            .borrow_mut()
            .engine
            .update_bar(bar)
            .map_err(js_err)
    }

    /// LWC-style main series update semantics:
    /// update last bar if timestamp matches, append if timestamp is newer.
    pub fn upsert_bar(
        &mut self,
        timestamp: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        volume: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "upsert_bar",
            &[
                ("open", open),
                ("high", high),
                ("low", low),
                ("close", close),
                ("volume", volume),
            ],
        )?;
        self.inner
            .borrow_mut()
            .engine
            .upsert_bar(Bar {
                timestamp,
                open,
                high,
                low,
                close,
                volume,
                _pad: 0.0,
            })
            .map_err(js_err)
    }

    /// Append a single point to a line/area/baseline overlay series.
    pub fn append_series_point(
        &mut self,
        id: u32,
        timestamp: u64,
        value: f32,
    ) -> Result<(), JsValue> {
        if !value.is_finite() {
            return Err(js_err("append_series_point: value must be finite"));
        }
        self.inner
            .borrow_mut()
            .engine
            .append_series_point(SeriesId(id), LinePoint { timestamp, value })
            .map_err(js_err)
    }

    /// Update the last point in a line/area/baseline overlay series.
    pub fn update_last_series_point(
        &mut self,
        id: u32,
        timestamp: u64,
        value: f32,
    ) -> Result<(), JsValue> {
        if !value.is_finite() {
            return Err(js_err("update_last_series_point: value must be finite"));
        }
        self.inner
            .borrow_mut()
            .engine
            .update_last_series_point(SeriesId(id), LinePoint { timestamp, value })
            .map_err(js_err)
    }

    /// LWC-style update semantics for line/area/baseline overlays:
    /// update last point if timestamp matches, append if timestamp is newer.
    pub fn upsert_series_point(
        &mut self,
        id: u32,
        timestamp: u64,
        value: f32,
    ) -> Result<(), JsValue> {
        if !value.is_finite() {
            return Err(js_err("upsert_series_point: value must be finite"));
        }
        self.inner
            .borrow_mut()
            .engine
            .upsert_series_point(SeriesId(id), LinePoint { timestamp, value })
            .map_err(js_err)
    }

    /// Append a single point to a histogram overlay series.
    pub fn append_histogram_point(
        &mut self,
        id: u32,
        timestamp: u64,
        value: f32,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "append_histogram_point",
            &[
                ("value", value),
                ("color_r", color_r),
                ("color_g", color_g),
                ("color_b", color_b),
                ("color_a", color_a),
            ],
        )?;
        self.inner
            .borrow_mut()
            .engine
            .append_histogram_point(
                SeriesId(id),
                HistogramPoint {
                    timestamp,
                    value,
                    color: [color_r, color_g, color_b, color_a],
                },
            )
            .map_err(js_err)
    }

    /// Update the last point in a histogram overlay series.
    pub fn update_last_histogram_point(
        &mut self,
        id: u32,
        timestamp: u64,
        value: f32,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "update_last_histogram_point",
            &[
                ("value", value),
                ("color_r", color_r),
                ("color_g", color_g),
                ("color_b", color_b),
                ("color_a", color_a),
            ],
        )?;
        self.inner
            .borrow_mut()
            .engine
            .update_last_histogram_point(
                SeriesId(id),
                HistogramPoint {
                    timestamp,
                    value,
                    color: [color_r, color_g, color_b, color_a],
                },
            )
            .map_err(js_err)
    }

    /// LWC-style update semantics for histogram overlays:
    /// update last point if timestamp matches, append if timestamp is newer.
    pub fn upsert_histogram_point(
        &mut self,
        id: u32,
        timestamp: u64,
        value: f32,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "upsert_histogram_point",
            &[
                ("value", value),
                ("color_r", color_r),
                ("color_g", color_g),
                ("color_b", color_b),
                ("color_a", color_a),
            ],
        )?;
        self.inner
            .borrow_mut()
            .engine
            .upsert_histogram_point(
                SeriesId(id),
                HistogramPoint {
                    timestamp,
                    value,
                    color: [color_r, color_g, color_b, color_a],
                },
            )
            .map_err(js_err)
    }

    /// Append a single point to a bar (OHLC) overlay series.
    pub fn append_bar_series_point(
        &mut self,
        id: u32,
        timestamp: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "append_bar_series_point",
            &[
                ("open", open),
                ("high", high),
                ("low", low),
                ("close", close),
            ],
        )?;
        self.inner
            .borrow_mut()
            .engine
            .append_bar_series_point(
                SeriesId(id),
                OhlcPoint {
                    timestamp,
                    open,
                    high,
                    low,
                    close,
                },
            )
            .map_err(js_err)
    }

    /// Update the last point in a bar (OHLC) overlay series.
    pub fn update_last_bar_series_point(
        &mut self,
        id: u32,
        timestamp: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "update_last_bar_series_point",
            &[
                ("open", open),
                ("high", high),
                ("low", low),
                ("close", close),
            ],
        )?;
        self.inner
            .borrow_mut()
            .engine
            .update_last_bar_series_point(
                SeriesId(id),
                OhlcPoint {
                    timestamp,
                    open,
                    high,
                    low,
                    close,
                },
            )
            .map_err(js_err)
    }

    /// LWC-style update semantics for OHLC bar overlays:
    /// update last point if timestamp matches, append if timestamp is newer.
    pub fn upsert_bar_series_point(
        &mut self,
        id: u32,
        timestamp: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "upsert_bar_series_point",
            &[
                ("open", open),
                ("high", high),
                ("low", low),
                ("close", close),
            ],
        )?;
        self.inner
            .borrow_mut()
            .engine
            .upsert_bar_series_point(
                SeriesId(id),
                OhlcPoint {
                    timestamp,
                    open,
                    high,
                    low,
                    close,
                },
            )
            .map_err(js_err)
    }

    // ── Render ───────────────────────────────────────────────────────────────

    /// Render one frame. Call from requestAnimationFrame.
    pub fn render(&mut self) {
        render_frame::do_render_frame(&self.inner, &self.dirty, &self.event_emitter);
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    /// Mark the chart as needing a re-render (for auto-render mode).
    fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    /// Apply CSS custom properties from the current theme to the container element.
    fn apply_css_variables(&self) {
        let vars = self.theme_config.to_css_variables();
        let s = self.inner.borrow();
        let container = s.layout.container();
        let style = container.style();
        for (key, value) in &vars {
            let _ = style.set_property(key, value);
        }
    }

    /// Start the internal requestAnimationFrame loop.
    ///
    /// Uses a self-scheduling RAF callback that checks the dirty flag
    /// and calls the render pipeline when needed. The closure captures
     /// `SharedInner` directly to avoid borrow-checker issues with `&mut self`.
     fn start_auto_render_internal(&mut self) {
        // Already running?
        if self._raf_closure.is_some() {
            return;
        }

        let inner = Rc::clone(&self.inner);
        let dirty = Rc::clone(&self.dirty);
        let raf_id = Rc::clone(&self._raf_id);
        let event_emitter = Rc::clone(&self.event_emitter);

        // Self-referencing closure pattern for RAF:
        //
        //  1. Allocate a shared slot: Rc<RefCell<Option<Closure>>>
        //  2. The closure captures a clone of that Rc
        //  3. Store the closure INTO the slot (not take it out!) so the clone
        //     the closure holds always has Some(closure) for rescheduling
        //  4. Store the Rc itself on self to keep the closure alive
        //
        // If we called `.take()` on the slot (old bug) the clone inside the
        // closure would see None and the loop would fire exactly once.
        let closure_slot: Rc<RefCell<Option<Closure<dyn FnMut()>>>> =
            Rc::new(RefCell::new(None));

        // Clone captured by the closure for self-rescheduling
        let slot_for_reschedule = Rc::clone(&closure_slot);

        let tick_closure = Closure::wrap(Box::new(move || {
            // Always render every frame in auto-render mode.
            // The dirty flag is NOT used as a render gate here because interaction
            // closures (pointer move, wheel, drag) don't hold a reference to `dirty`
            // and therefore can never mark it — any dirty-guard would prevent
            // interactions from ever producing a new frame.
            render_frame::do_render_frame(&inner, &dirty, &event_emitter);

            // Reschedule: read the closure from the shared slot and pass it to RAF.
            if let Some(window) = web_sys::window() {
                if let Some(c) = slot_for_reschedule.borrow().as_ref() {
                    if let Ok(id) = window.request_animation_frame(c.as_ref().unchecked_ref()) {
                        raf_id.set(id);
                    }
                }
            }
        }) as Box<dyn FnMut()>);

        // Kick off the first frame BEFORE storing the closure so we have a
        // valid JS function reference to pass to request_animation_frame.
        if let Some(window) = web_sys::window() {
            if let Ok(id) = window.request_animation_frame(tick_closure.as_ref().unchecked_ref()) {
                self._raf_id.set(id);
            }
        }

        // Keep the closure INSIDE the slot so slot_for_reschedule (captured
        // by the closure) can find it on every tick.
        *closure_slot.borrow_mut() = Some(tick_closure);

        // Store the Rc on self — this keeps the closure alive for the lifetime
        // of RayCore and allows stop_auto_render_internal to drop it.
        self._raf_closure = Some(closure_slot);
        self.dirty.set(true);
    }

    /// Stop the internal requestAnimationFrame loop.
    fn stop_auto_render_internal(&mut self) {
        // Cancel the pending RAF callback (prevents one extra frame after stop)
        let raf_id = self._raf_id.get();
        if raf_id != 0 {
            if let Some(window) = web_sys::window() {
                let _ = window.cancel_animation_frame(raf_id);
            }
            self._raf_id.set(0);
        }

        // Drop the closure by clearing the slot, then the Rc.
        // The closure captures slot_for_reschedule which holds the same Rc,
        // so clearing the slot breaks the reference cycle.
        if let Some(slot) = &self._raf_closure {
            slot.borrow_mut().take(); // drop the Closure, breaking the cycle
        }
        self._raf_closure = None;
    }

    /// Dispose: remove all event listeners, disconnect resize observer, and clean up resources.
    ///
    /// IMPORTANT: Call this when destroying the chart to prevent memory leaks.
    /// Event listeners attached to DOM elements will keep the closures alive
    /// even after RayCore is dropped, unless explicitly removed.
    pub fn dispose(&mut self) {
        // 1. Disconnect resize observer
        if let Some(obs) = self._resize_observer.take() {
            obs.disconnect();
        }

        // 2. Cancel any pending long-press timer
        if let Some(tid) = self._long_press_timer.borrow_mut().take() {
            if let Some(window) = web_sys::window() {
                let _ = window.clear_timeout_with_handle(tid);
            }
        }

        // 3. Remove all tracked event listeners
        self._event_registry.remove_all();

        // 3b. Stop auto-render and clean up event emitter
        self.stop_auto_render_internal();
        self.event_emitter.borrow_mut().remove_all_listeners();

        // 4. Clear closure vectors (closures will be dropped, but DOM refs are now removed)
        self._closures.clear();
        self._wheel_closures.clear();
        self._touch_closures.clear();
        self._resize_closure = None;

        // 5. Clean up subpane event listeners
        {
            let mut inner = self.inner.borrow_mut();
            for subpane in inner.subpanes.iter_mut() {
                subpane.dispose();
            }
            inner.subpanes.clear();
        }

        log::info!("RayCore disposed: all event listeners removed");
    }
}

#[wasm_bindgen]
pub struct ChartGroup {
    inner: NativeChartGroup,
}

#[wasm_bindgen]
impl ChartGroup {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: NativeChartGroup::new(),
        }
    }

    pub fn add_pane(&mut self, symbol: &str, interval: &str) -> u32 {
        self.inner.add_pane(symbol, interval).0 as u32
    }

    pub fn remove_pane(&mut self, pane_id: u32) -> bool {
        self.inner.remove_pane(ChartPaneId(pane_id as u64))
    }

    pub fn pane_count(&self) -> usize {
        self.inner.pane_count()
    }

    pub fn set_auto_link(&mut self, enabled: bool) {
        self.inner.set_auto_link(enabled);
    }

    pub fn link_panes(&mut self, a: u32, b: u32) -> bool {
        self.inner
            .link_panes(&ChartPaneId(a as u64), &ChartPaneId(b as u64))
    }

    pub fn unlink_panes(&mut self, a: u32, b: u32) -> bool {
        self.inner
            .unlink_panes(&ChartPaneId(a as u64), &ChartPaneId(b as u64))
    }

    pub fn set_sync(&mut self, feature: &str, enabled: bool) -> Result<(), JsValue> {
        self.inner.try_set_sync(feature, enabled).map_err(js_err)
    }

    pub fn set_sync_for_pane(
        &mut self,
        pane_id: u32,
        feature: &str,
        enabled: bool,
    ) -> Result<(), JsValue> {
        self.inner
            .set_sync_for_pane(ChartPaneId(pane_id as u64), feature, enabled)
            .map_err(js_err)
    }

    pub fn set_sync_for_link(
        &mut self,
        pane_a: u32,
        pane_b: u32,
        feature: &str,
        enabled: bool,
    ) -> Result<(), JsValue> {
        self.inner
            .set_sync_for_link(
                ChartPaneId(pane_a as u64),
                ChartPaneId(pane_b as u64),
                feature,
                enabled,
            )
            .map_err(js_err)
    }

    pub fn update_symbol(&mut self, source: u32, symbol: &str) -> js_sys::Array {
        ids_to_js_array(
            self.inner
                .update_symbol(ChartPaneId(source as u64), symbol.to_string()),
        )
    }

    pub fn update_interval(&mut self, source: u32, interval: &str) -> js_sys::Array {
        ids_to_js_array(
            self.inner
                .update_interval(ChartPaneId(source as u64), interval.to_string()),
        )
    }

    /// `crosshair` format: `[active, x, y, bar_index, price, magnet]`.
    /// `magnet`: 0 = normal, 1 = OHLC magnet.
    pub fn update_crosshair(&mut self, source: u32, crosshair: &[f64]) -> js_sys::Array {
        if crosshair.len() < 6 {
            return js_sys::Array::new();
        }
        let snapshot = CrosshairSnapshot {
            active: crosshair[0] > 0.5,
            x: crosshair[1],
            y: crosshair[2],
            bar_index: if crosshair[3].is_finite() && crosshair[3] >= 0.0 {
                Some(crosshair[3])
            } else {
                None
            },
            price: if crosshair[4].is_finite() {
                Some(crosshair[4])
            } else {
                None
            },
            magnet: if crosshair[5] > 0.5 {
                CrosshairMagnetMode::Ohlc
            } else {
                CrosshairMagnetMode::Normal
            },
        };
        ids_to_js_array(
            self.inner
                .update_crosshair(ChartPaneId(source as u64), snapshot),
        )
    }

    pub fn update_time_range(
        &mut self,
        source: u32,
        start_bar: f64,
        end_bar: f64,
    ) -> js_sys::Array {
        ids_to_js_array(
            self.inner
                .update_time_range(ChartPaneId(source as u64), TimeRange { start_bar, end_bar }),
        )
    }

    pub fn update_data_range(
        &mut self,
        source: u32,
        from_timestamp: f64,
        to_timestamp: f64,
    ) -> js_sys::Array {
        let from = if from_timestamp.is_finite() && from_timestamp >= 0.0 {
            Some(from_timestamp as u64)
        } else {
            None
        };
        let to = if to_timestamp.is_finite() && to_timestamp >= 0.0 {
            Some(to_timestamp as u64)
        } else {
            None
        };
        ids_to_js_array(self.inner.update_data_range(
            ChartPaneId(source as u64),
            DataRange {
                from_timestamp: from,
                to_timestamp: to,
            },
        ))
    }

    pub fn pane_symbol(&self, pane_id: u32) -> String {
        self.inner
            .pane(ChartPaneId(pane_id as u64))
            .map(|p| p.symbol.clone())
            .unwrap_or_default()
    }

    pub fn pane_interval(&self, pane_id: u32) -> String {
        self.inner
            .pane(ChartPaneId(pane_id as u64))
            .map(|p| p.interval.clone())
            .unwrap_or_default()
    }

    /// Returns `[start_bar, end_bar]`, or empty if pane is missing.
    pub fn pane_time_range(&self, pane_id: u32) -> Vec<f64> {
        if let Some(p) = self.inner.pane(ChartPaneId(pane_id as u64)) {
            vec![p.time_range.start_bar, p.time_range.end_bar]
        } else {
            Vec::new()
        }
    }

    /// Returns `[from_timestamp, to_timestamp]`, or empty if unavailable.
    pub fn pane_data_range(&self, pane_id: u32) -> Vec<f64> {
        if let Some(p) = self.inner.pane(ChartPaneId(pane_id as u64)) {
            match (p.data_range.from_timestamp, p.data_range.to_timestamp) {
                (Some(from), Some(to)) => vec![from as f64, to as f64],
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }
}

fn ids_to_js_array(ids: Vec<ChartPaneId>) -> js_sys::Array {
    let out = js_sys::Array::new_with_length(ids.len() as u32);
    for (i, id) in ids.into_iter().enumerate() {
        out.set(i as u32, JsValue::from_f64(id.0 as f64));
    }
    out
}
