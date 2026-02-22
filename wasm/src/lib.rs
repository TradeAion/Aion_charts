//! RayCore WASM bindings — LWC-style widget-based chart library.
//!
//! Architecture (matches LWC):
//!   - WidgetLayout creates CSS-grid DOM: [pane|price_axis] / [time_axis]
//!   - Each widget has its own DOM container + canvases + event handlers
//!   - InteractionHandler processes per-widget events (zone from DOM, not pixel math)
//!   - ChartEngine renders the pane only; axis renderers are separate
//!
//! Public WASM API:
//!   RayCore.create("container-id")      → sets up everything, attaches all events
//!   core.demo_mode()                    → load sample data
//!   core.set_data(...)                  → load bar data
//!   core.render()                       → draw one frame (call from RAF)
//!   core.dispose()                      → detach events, cleanup

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::cell::RefCell;
use std::rc::Rc;
use raycore::{
    Bar, ChartEngine, ChartStyle,
    GpuContext, WgpuRenderer, Canvas2DRenderer,
    RendererBackend, OverlayRenderer,
    PriceAxisRenderer, TimeAxisRenderer,
    InteractionHandler, HitZone,
    generate_sample_data, tick_marks,
    LinePoint, LineSeriesOptions, SeriesId,
};

mod canvas_manager;
use canvas_manager::WidgetLayout;

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

/// Exact device-pixel sizes for each widget container, reported by
/// `ResizeObserver` with `device-pixel-content-box`. When available these
/// replace the lossy `round(css * dpr)` fallback and eliminate ±1px blur.
#[derive(Debug, Clone, Copy, Default)]
struct ExactPixelSizes {
    /// Set to true once the observer has fired at least once.
    available: bool,
    pane_pw: u32,
    pane_ph: u32,
    price_axis_pw: u32,
    price_axis_ph: u32,
    time_axis_pw: u32,
    time_axis_ph: u32,
    corner_stub_pw: u32,
    corner_stub_ph: u32,
}

/// Internal chart state shared between event closures and the public API.
struct ChartInner {
    engine: ChartEngine,
    overlay: OverlayRenderer,
    price_axis_renderer: PriceAxisRenderer,
    time_axis_renderer: TimeAxisRenderer,
    layout: WidgetLayout,
    interaction: InteractionHandler,
    /// Exact pixel sizes from device-pixel-content-box ResizeObserver.
    exact_sizes: ExactPixelSizes,
}

/// Helper methods that destructure `self` to satisfy the borrow checker.
/// Each method borrows `interaction` and `engine` fields separately.
impl ChartInner {
    fn on_pointer_enter(&mut self, zone: HitZone) {
        let Self { interaction, engine, .. } = self;
        interaction.pointer_enter(zone, &mut engine.crosshair);
    }

    fn on_pointer_leave(&mut self, zone: HitZone) {
        let Self { interaction, engine, .. } = self;
        interaction.pointer_leave(zone, &mut engine.crosshair);
    }

    fn on_pane_pointer_move(&mut self, x: f64, y: f64) {
        let (pw, ph) = self.layout.pane_css_size();
        let dpr = self.engine.dpr;

        // Pre-compute logical coords from viewport (before any mutable drawing borrow)
        let bar = self.engine.viewport.pixel_to_bar(x, pw);
        // Use candle area height for drawing coordinates — matches price_to_css_y().
        // Candles occupy the top (1 - volume_ratio) of the pane; volume is below.
        let candle_css_h = ph * self.engine.viewport.candle_height_frac();
        let price = self.engine.viewport.pixel_to_price(y, candle_css_h);

        // Drawing tool: update preview or drag
        {
            let drawings = &mut self.engine.drawings;
            if drawings.is_creating() {
                drawings.update_creation_preview(bar, price);
                // Still fall through so crosshair updates
            } else if let Some(id) = drawings.selected_id {
                if matches!(drawings.get(id).map(|d| d.state()),
                    Some(raycore::core::drawings::types::DrawingState::Dragging { .. })) {
                    drawings.update_drag(id, bar, price);
                    return; // don't move chart while dragging drawing
                }
            }
        }

        let Self { interaction, engine, .. } = self;
        interaction.pane_pointer_move(
            x, y, pw, ph,
            &mut engine.viewport, &mut engine.crosshair,
            &engine.bars, dpr,
        );
    }

    fn on_pointer_down(&mut self, x: f64, y: f64, zone: HitZone) {
        let (pw, ph) = self.layout.pane_css_size();

        if zone == HitZone::Chart {
            let bar = self.engine.viewport.pixel_to_bar(x, pw);
            // Use candle area height — consistent with point_to_css / price_to_css_y.
            let candle_css_h = ph * self.engine.viewport.candle_height_frac();
            let price = self.engine.viewport.pixel_to_price(y, candle_css_h);
            let drawings = &mut self.engine.drawings;

            if drawings.is_tool_active() {
                // Start creating a new drawing
                if !drawings.is_creating() {
                    drawings.start_creating(bar, price);
                } else {
                    // Multi-step tools: place next anchor on click
                    drawings.finalize_creation_step(bar, price);
                }
                return; // don't pan/zoom while drawing
            } else {
                // No tool active: check if user clicked on an existing drawing
                let hit = drawings.hit_test(x, y, &self.engine.viewport, pw, ph);
                if let Some((id, result)) = hit {
                    use raycore::core::drawings::types::HitPart;
                    let anchor_idx = match result.part {
                        HitPart::Anchor(i) => Some(i),
                        _ => None,
                    };
                    drawings.select(id);
                    drawings.start_drag(id, anchor_idx, bar, price);
                    return; // don't pan while starting a drawing drag
                } else {
                    // Click on empty space: deselect
                    drawings.deselect_all();
                }
            }
        }

        let Self { interaction, engine, .. } = self;
        interaction.pointer_down(x, y, zone, &engine.viewport, ph);
    }

    fn on_pointer_up(&mut self) {
        let now_ms = js_sys::Date::now();
        let (pw, ph) = self.layout.pane_css_size();

        // If a drawing was being created (drag-to-create: release = place second anchor)
        {
            let drawings = &mut self.engine.drawings;
            if drawings.is_creating() {
                // Read the preview anchor position first (immutable borrow scope)
                let anchor_pos: Option<(f64, f64)> = {
                    drawings.all().iter()
                        .find(|d| matches!(d.state(), raycore::core::drawings::types::DrawingState::Creating { .. }))
                        .and_then(|d| {
                            let anchors = d.anchors();
                            if anchors.len() >= 2 {
                                Some((anchors[1].point.bar_index, anchors[1].point.price))
                            } else if !anchors.is_empty() {
                                Some((anchors[0].point.bar_index, anchors[0].point.price))
                            } else {
                                None
                            }
                        })
                };
                // Now finalize with the stored position (mutable borrow)
                if let Some((bar, price)) = anchor_pos {
                    drawings.finalize_creation_step(bar, price);
                }
                return;
            }
        }

        // End any drawing drag
        {
            let drawings = &mut self.engine.drawings;
            if let Some(id) = drawings.selected_id {
                if matches!(drawings.get(id).map(|d| d.state()),
                    Some(raycore::core::drawings::types::DrawingState::Dragging { .. })) {
                    drawings.end_drag(id);
                    return;
                }
            }
        }

        let _ = (pw, ph); // suppress unused warning
        let Self { interaction, engine, .. } = self;
        interaction.pointer_up(&mut engine.viewport, &engine.bars, now_ms);
    }

    fn on_pane_wheel(&mut self, x: f64, dx: f64, dy: f64, dm: u32) {
        let (pw, _) = self.layout.pane_css_size();
        let Self { interaction, engine, .. } = self;
        interaction.pane_wheel(x, dx, dy, dm, pw, &mut engine.viewport, &engine.bars);
    }

    fn on_price_axis_move(&mut self, y: f64) {
        let (_, ph) = self.layout.pane_css_size();
        let Self { interaction, engine, .. } = self;
        interaction.price_axis_pointer_move(y, ph, &mut engine.viewport);
    }

    fn on_price_axis_wheel(&mut self, dy: f64, dm: u32) {
        let Self { interaction, engine, .. } = self;
        interaction.price_axis_wheel(dy, dm, &mut engine.viewport);
    }

    fn on_time_axis_move(&mut self, x: f64) {
        let (pw, _) = self.layout.pane_css_size();
        let Self { interaction, engine, .. } = self;
        interaction.time_axis_pointer_move(x, pw, &mut engine.viewport, &engine.bars);
    }

    fn on_time_axis_wheel(&mut self, x: f64, dy: f64, dm: u32) {
        let (pw, _) = self.layout.pane_css_size();
        let Self { interaction, engine, .. } = self;
        interaction.time_axis_wheel(x, dy, dm, pw, &mut engine.viewport, &engine.bars);
    }

    fn on_pinch_start(&mut self, cx: f64, _cy: f64, distance: f64) {
        let (pw, _) = self.layout.pane_css_size();
        let Self { interaction, engine, .. } = self;
        interaction.pinch_start(cx, _cy, distance, pw, &engine.viewport);
    }

    fn on_pinch_update(&mut self, scale: f64) {
        let Self { interaction, engine, .. } = self;
        interaction.pinch_update(scale, &mut engine.viewport, &engine.bars);
    }

    fn on_pinch_end(&mut self) {
        self.interaction.pinch_end();
    }

    fn on_long_press(&mut self, x: f64, y: f64) {
        let Self { interaction, engine, .. } = self;
        interaction.long_press(x, y, &mut engine.crosshair);
    }

    fn on_touch_double_tap(&mut self) {
        let Self { interaction, engine, .. } = self;
        interaction.touch_double_tap(
            &mut engine.crosshair,
            &mut engine.viewport,
            &engine.bars,
        );
    }

    fn cursor_css(&self) -> &'static str {
        self.interaction.cursor_hint()
    }
}

type SharedInner = Rc<RefCell<ChartInner>>;

/// Helper: get CSS coords from PointerEvent relative to an element.
fn event_css_pos(e: &web_sys::PointerEvent, el: &web_sys::Element) -> (f64, f64) {
    let rect = el.get_bounding_client_rect();
    (e.client_x() as f64 - rect.left(), e.client_y() as f64 - rect.top())
}

fn wheel_css_pos(e: &web_sys::WheelEvent, el: &web_sys::Element) -> (f64, f64) {
    let rect = el.get_bounding_client_rect();
    (e.client_x() as f64 - rect.left(), e.client_y() as f64 - rect.top())
}

#[wasm_bindgen]
pub struct RayCore {
    inner: SharedInner,
    _closures: Vec<Closure<dyn FnMut(web_sys::Event)>>,
    _wheel_closures: Vec<Closure<dyn FnMut(web_sys::WheelEvent)>>,
    _touch_closures: Vec<Closure<dyn FnMut(web_sys::TouchEvent)>>,
    _resize_closure: Option<Closure<dyn FnMut(js_sys::Array)>>,
    _resize_observer: Option<web_sys::ResizeObserver>,
    /// Long-press timer ID (from setTimeout), shared with closures.
    _long_press_timer: Rc<RefCell<Option<i32>>>,
    /// Last touch-tap time for double-tap detection.
    _last_tap_time: Rc<RefCell<f64>>,
}

#[wasm_bindgen]
impl RayCore {
    /// Create a new RayCore instance inside a container div.
    pub async fn create(container_id: &str) -> Result<RayCore, JsValue> {
        let preferred = if webgpu_available() { "webgpu" } else { "canvas2d" };
        Self::create_with(container_id, preferred).await
    }

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
            renderer, pane_css_w, pane_css_h, pane_pw, pane_ph, dpr
        );

        // Create pane renderer backend (only for the pane/chart canvas)
        let backend = match renderer {
            "webgpu" => {
                match GpuContext::new(
                    wgpu::SurfaceTarget::Canvas(layout.pane.chart.clone()),
                    pane_pw.max(1),
                    pane_ph.max(1),
                ).await {
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
        ).map_err(|e| JsValue::from_str(&e))?;

        let time_axis_renderer = TimeAxisRenderer::new(
            layout.time_axis.base.clone(),
            layout.time_axis.top.clone(),
            dpr,
        ).map_err(|e| JsValue::from_str(&e))?;

        // Engine only manages the pane
        let engine = ChartEngine::new(backend, pane_pw.max(1), pane_ph.max(1), dpr);
        let interaction = InteractionHandler::new();

        log::info!("RayCore initialized: {}", engine.renderer_name());

        let inner = Rc::new(RefCell::new(ChartInner {
            engine,
            overlay,
            price_axis_renderer,
            time_axis_renderer,
            layout,
            interaction,
            exact_sizes: ExactPixelSizes::default(),
        }));

        let mut closures: Vec<Closure<dyn FnMut(web_sys::Event)>> = Vec::new();
        let mut wheel_closures: Vec<Closure<dyn FnMut(web_sys::WheelEvent)>> = Vec::new();
        let mut touch_closures: Vec<Closure<dyn FnMut(web_sys::TouchEvent)>> = Vec::new();
        let long_press_timer: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
        let last_tap_time: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.0));
        // Shared closure handle for long-press timeout callback
        let long_press_cb_handle: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));

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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let mut s = inner.borrow_mut();
                s.interaction.set_touch(pe.pointer_type() == "touch");
                s.on_pointer_enter(HitZone::Chart);
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
            }));
            pane_el.add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointerleave
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                let mut s = inner.borrow_mut();
                s.on_pointer_leave(HitZone::Chart);
                // Clear the override to let CSS default take over (crosshair)
                let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", "");
            }));
            pane_el.add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointermove
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let grid_move = grid_c.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let (x, y) = event_css_pos(&pe, &pane_c);
                let mut s = inner.borrow_mut();
                
                // Detect touch on every move (not just pointerdown)
                s.interaction.set_touch(pe.pointer_type() == "touch");
                
                // Ensure zone is set (fixes missing pointerenter on page load)
                s.on_pointer_enter(HitZone::Chart);
                s.on_pane_pointer_move(x, y);
                
                let cursor = s.cursor_css();
                let is_dragging = s.interaction.is_dragging();
                
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
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
                        if s.interaction.pressed && !s.interaction.drag_active && !s.interaction.pinch_active {
                            s.on_long_press(lp_x, lp_y);
                        }
                        *lp_timer_inner.borrow_mut() = None;
                    }));

                    let window = web_sys::window().unwrap();
                    let tid = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        timeout_cb.as_ref().unchecked_ref(),
                        240,
                    ).unwrap_or(0);
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(move |e: web_sys::WheelEvent| {
                e.prevent_default();
                let (x, _y) = wheel_css_pos(&e, &pane_c);
                let mut s = inner.borrow_mut();
                s.on_pointer_enter(HitZone::Chart);
                s.on_pane_wheel(x, e.delta_x(), e.delta_y(), e.delta_mode());
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            pane_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel", cb.as_ref().unchecked_ref(), &opts,
            )?;
            wheel_closures.push(cb);
        }
        // pane: contextmenu
        {
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                e.prevent_default();
            }));
            pane_el.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointercancel
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let grid_can = grid_c.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            pane_el.add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }

        // ── PANE TOUCH events (for pinch zoom — needs raw TouchEvent for multi-touch) ──
        // touchstart: detect 2-finger pinch start; cancel long-press if multi-touch
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let lp_timer = Rc::clone(&long_press_timer);
            let lp_cb = Rc::clone(&long_press_cb_handle);
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(move |e: web_sys::TouchEvent| {
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
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            pane_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchstart", cb.as_ref().unchecked_ref(), &opts,
            )?;
            touch_closures.push(cb);
        }
        // touchmove: update pinch scale
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(move |e: web_sys::TouchEvent| {
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
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            pane_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchmove", cb.as_ref().unchecked_ref(), &opts,
            )?;
            touch_closures.push(cb);
        }
        // touchend: end pinch when fingers lift
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(move |e: web_sys::TouchEvent| {
                let touches = e.touches();
                if touches.length() < 2 {
                    let mut s = inner.borrow_mut();
                    if s.interaction.pinch_active {
                        s.on_pinch_end();
                    }
                }
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            pane_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchend", cb.as_ref().unchecked_ref(), &opts,
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                let mut s = inner.borrow_mut();
                s.on_pointer_enter(HitZone::PriceAxis);
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
            }));
            price_el.add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerleave
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                let mut s = inner.borrow_mut();
                s.on_pointer_leave(HitZone::PriceAxis);
                let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", "");
            }));
            price_el.add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointermove
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let grid_move_p = grid_c.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            price_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerdown
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let (_x, y) = event_css_pos(&pe, &price_c);
                let mut s = inner.borrow_mut();
                s.interaction.is_touch = pe.pointer_type() == "touch";
                s.on_pointer_enter(HitZone::PriceAxis);
                s.on_pointer_down(0.0, y, HitZone::PriceAxis);
                let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                let _ = html_el.set_pointer_capture(pe.pointer_id());
            }));
            price_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerup
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let grid_up_p = grid_c.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(move |e: web_sys::WheelEvent| {
                e.prevent_default();
                let mut s = inner.borrow_mut();
                s.on_pointer_enter(HitZone::PriceAxis);
                s.on_price_axis_wheel(e.delta_y(), e.delta_mode());
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            price_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel", cb.as_ref().unchecked_ref(), &opts,
            )?;
            wheel_closures.push(cb);
        }
        // price axis: contextmenu
        {
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                e.prevent_default();
            }));
            price_el.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointercancel
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let grid_can_p = grid_c.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            price_el.add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref())?;
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                let mut s = inner.borrow_mut();
                s.on_pointer_enter(HitZone::TimeAxis);
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
            }));
            time_el.add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerleave
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                let mut s = inner.borrow_mut();
                s.on_pointer_leave(HitZone::TimeAxis);
                let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", "");
            }));
            time_el.add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointermove
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let grid_move_t = grid_c.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(move |e: web_sys::WheelEvent| {
                e.prevent_default();
                let (x, _y) = wheel_css_pos(&e, &time_c);
                let mut s = inner.borrow_mut();
                s.on_pointer_enter(HitZone::TimeAxis);
                s.on_time_axis_wheel(x, e.delta_y(), e.delta_mode());
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            time_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel", cb.as_ref().unchecked_ref(), &opts,
            )?;
            wheel_closures.push(cb);
        }
        // time axis: contextmenu
        {
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            time_el.add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref())?;
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                let mut s = inner.borrow_mut();
                s.on_pointer_enter(HitZone::None);
            }));
            corner_el.add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
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

            let cb = Closure::<dyn FnMut(js_sys::Array)>::wrap(Box::new(move |entries: js_sys::Array| {
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
                        &entry, &JsValue::from_str("devicePixelContentBoxSize")
                    ).ok();
                    let (exact_w, exact_h) = if let Some(ref dp) = dpsize {
                        if !dp.is_undefined() && !dp.is_null() {
                            let arr: &js_sys::Array = dp.unchecked_ref();
                            if arr.length() > 0 {
                                let item = arr.get(0);
                                let iw = js_sys::Reflect::get(&item, &JsValue::from_str("inlineSize"))
                                    .ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let ih = js_sys::Reflect::get(&item, &JsValue::from_str("blockSize"))
                                    .ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
                                got_exact = true;
                                (iw as u32, ih as u32)
                            } else { (0, 0) }
                        } else { (0, 0) }
                    } else { (0, 0) };

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

                if s.exact_sizes.available {
                    // Use exact pixel sizes for canvas bitmap dimensions
                    let es = s.exact_sizes;
                    s.layout.resize_pane_exact(es.pane_pw, es.pane_ph);
                    s.layout.resize_price_axis_exact(es.price_axis_pw, es.price_axis_ph);
                    s.layout.resize_time_axis_exact(es.time_axis_pw, es.time_axis_ph);
                    s.layout.resize_corner_stub_exact(es.corner_stub_pw, es.corner_stub_ph);

                    // Compute per-axis pixel ratios
                    let (pcw, pch) = s.layout.pane_css_size();
                    let h_ratio = if pcw > 0.0 { es.pane_pw as f64 / pcw } else { dpr };
                    let v_ratio = if pch > 0.0 { es.pane_ph as f64 / pch } else { dpr };
                    s.engine.h_pixel_ratio = h_ratio;
                    s.engine.v_pixel_ratio = v_ratio;

                    s.engine.resize(es.pane_pw.max(1), es.pane_ph.max(1), dpr);
                    s.overlay.resize(es.pane_pw.max(1), es.pane_ph.max(1), dpr);
                    s.price_axis_renderer.resize(es.price_axis_pw.max(1), es.price_axis_ph.max(1), dpr);
                    s.time_axis_renderer.resize(es.time_axis_pw.max(1), es.time_axis_ph.max(1), dpr);
                } else {
                    // Fallback: round(css * dpr)
                    s.layout.resize_all_canvases(dpr);
                    s.engine.h_pixel_ratio = dpr;
                    s.engine.v_pixel_ratio = dpr;

                    let (pw, ph) = s.layout.pane_css_size();
                    let ppw = (pw * dpr).round() as u32;
                    let pph = (ph * dpr).round() as u32;
                    s.engine.resize(ppw.max(1), pph.max(1), dpr);
                    s.overlay.resize(ppw.max(1), pph.max(1), dpr);

                    let (aw, ah) = s.layout.price_axis_css_size();
                    s.price_axis_renderer.resize(
                        (aw * dpr).round() as u32, (ah * dpr).round() as u32, dpr,
                    );
                    let (tw, th) = s.layout.time_axis_css_size();
                    s.time_axis_renderer.resize(
                        (tw * dpr).round() as u32, (th * dpr).round() as u32, dpr,
                    );
                }
            }));
            let observer = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref())?;

            // Try to observe with device-pixel-content-box; fall back to content-box
            let observe_with_dpcb = js_sys::Function::new_with_args(
                "observer,element",
                "try { observer.observe(element, { box: 'device-pixel-content-box' }); return true; } catch(e) { observer.observe(element); return false; }"
            );
            let _ = observe_with_dpcb.call2(&JsValue::NULL, observer.as_ref(), &pane_container_for_ro);
            let _ = observe_with_dpcb.call2(&JsValue::NULL, observer.as_ref(), &price_container_for_ro);
            let _ = observe_with_dpcb.call2(&JsValue::NULL, observer.as_ref(), &time_container_for_ro);
            let _ = observe_with_dpcb.call2(&JsValue::NULL, observer.as_ref(), &corner_container_for_ro);

            // Also observe the outer container for general layout changes
            observer.observe(&container_el.clone().unchecked_into());

            (cb, observer)
        };

        Ok(RayCore {
            inner,
            _closures: closures,
            _wheel_closures: wheel_closures,
            _touch_closures: touch_closures,
            _resize_closure: Some(resize_closure),
            _resize_observer: Some(resize_observer),
            _long_press_timer: long_press_timer,
            _last_tap_time: last_tap_time,
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

    // ── Data loading ─────────────────────────────────────────────────────────

    pub fn set_data_arrays(
        &mut self,
        open: &[f32],
        high: &[f32],
        low: &[f32],
        close: &[f32],
        volume: &[f32],
        timestamps: &[u64],
    ) {
        let count = open.len()
            .min(high.len())
            .min(low.len())
            .min(close.len())
            .min(volume.len());

        let bars: Vec<Bar> = (0..count)
            .map(|i| Bar {
                timestamp: if i < timestamps.len() { timestamps[i] } else { i as u64 },
                open: open[i],
                high: high[i],
                low: low[i],
                close: close[i],
                volume: volume[i],
                _pad: 0.0,
            })
            .collect();

        self.inner.borrow_mut().engine.set_data(bars);
        log::info!("set_data_arrays: {} bars", count);
    }

    pub fn set_data(&mut self, data: &[f32]) {
        const N: usize = 8;
        if data.len() % N != 0 {
            log::error!("set_data: array length must be multiple of 8, got {}", data.len());
            return;
        }
        let bars: Vec<Bar> = (0..data.len() / N)
            .map(|i| {
                let b = i * N;
                let ts_lo = data[b] as u32;
                let ts_hi = data[b + 1] as u32;
                Bar {
                    timestamp: ((ts_hi as u64) << 32) | ts_lo as u64,
                    open: data[b + 2],
                    high: data[b + 3],
                    low: data[b + 4],
                    close: data[b + 5],
                    volume: data[b + 6],
                    _pad: 0.0,
                }
            })
            .collect();
        let n = bars.len();
        self.inner.borrow_mut().engine.set_data(bars);
        log::info!("set_data: {} bars", n);
    }

    // ── Demo mode ────────────────────────────────────────────────────────────

    pub fn demo_mode(&mut self) {
        let now_ms = js_sys::Date::now() as u64;
        let num_bars = 600;
        let interval_ms = 60_000;
        let start_ms = now_ms - (num_bars as u64) * interval_ms;
        let bars = generate_sample_data(num_bars, start_ms, interval_ms);
        self.inner.borrow_mut().engine.set_data(bars);
        log::info!("demo_mode: {} bars loaded", num_bars);
    }

    // ── Viewport control ─────────────────────────────────────────────────────

    pub fn zoom_to_range(&mut self, start: u64, end: u64) {
        self.inner.borrow_mut().engine.zoom_to_range(start, end);
    }

    pub fn visible_range(&self) -> Vec<f64> {
        let s = self.inner.borrow();
        vec![s.engine.viewport.start_bar, s.engine.viewport.end_bar]
    }

    /// Set crosshair mode: "normal", "magnet", or "magnet_ohlc".
    pub fn set_crosshair_mode(&mut self, mode: &str) {
        let mut s = self.inner.borrow_mut();
        s.engine.crosshair.mode = match mode {
            "magnet" => raycore::CrosshairMode::Magnet,
            "magnet_ohlc" => raycore::CrosshairMode::MagnetOHLC,
            _ => raycore::CrosshairMode::Normal,
        };
    }

    // ── Drawing tools ─────────────────────────────────────────────────────────

    /// Set active drawing tool: "none", "trend_line", "rectangle", "fibonacci", "scale".
    pub fn set_drawing_tool(&mut self, tool: &str) {
        let mut s = self.inner.borrow_mut();
        s.engine.drawings.active_tool = match tool {
            "trend_line" => raycore::DrawingTool::TrendLine,
            "rectangle" => raycore::DrawingTool::Rectangle,
            "fibonacci" => raycore::DrawingTool::Fibonacci,
            "scale" => raycore::DrawingTool::Scale,
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

    /// Remove all drawings.
    pub fn clear_drawings(&mut self) {
        let mut s = self.inner.borrow_mut();
        while s.engine.drawings.len() > 0 {
            let id = s.engine.drawings.all()[0].id();
            s.engine.drawings.remove(id);
        }
    }

    /// Get the number of drawings.
    pub fn drawing_count(&self) -> usize {
        self.inner.borrow().engine.drawings.len()
    }

    // ── Series overlay API ────────────────────────────────────────────────────

    /// Add a new line series overlay. Returns the series ID.
    ///
    /// Default color is TradingView blue (#2962FF). Use RGBA [0.0–1.0].
    pub fn add_line_series(
        &mut self,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
        line_width: f32,
    ) -> u32 {
        let mut opts = LineSeriesOptions::default();
        opts.color = [color_r, color_g, color_b, color_a];
        opts.line_width = line_width as f64;
        let id = self.inner.borrow_mut().engine.add_line_series(opts);
        log::info!("add_line_series: id={}", id.0);
        id.0
    }

    /// Set data for a line series. `values` and `timestamps` must be same length.
    pub fn set_series_data(&mut self, id: u32, values: &[f32], timestamps: &[u64]) {
        let count = values.len().min(timestamps.len());
        let data: Vec<LinePoint> = (0..count)
            .map(|i| LinePoint {
                timestamp: timestamps[i],
                value: values[i],
            })
            .collect();
        self.inner.borrow_mut().engine.set_series_data(SeriesId(id), data);
        log::info!("set_series_data: id={}, {} points", id, count);
    }

    /// Remove a series by ID.
    pub fn remove_series(&mut self, id: u32) -> bool {
        let removed = self.inner.borrow_mut().engine.remove_series(SeriesId(id));
        log::info!("remove_series: id={}, removed={}", id, removed);
        removed
    }

    /// Show or hide a series.
    pub fn set_series_visible(&mut self, id: u32, visible: bool) {
        self.inner.borrow_mut().engine.set_series_visible(SeriesId(id), visible);
    }

    /// Get the number of overlay series.
    pub fn series_count(&self) -> usize {
        self.inner.borrow().engine.series.len()
    }

    // ── Render ───────────────────────────────────────────────────────────────

    /// Render one frame. Call from requestAnimationFrame.
    pub fn render(&mut self) {
        let mut s = self.inner.borrow_mut();

        // Detect DPR changes (browser zoom) that may not trigger ResizeObserver
        let current_dpr = get_dpr();
        if (current_dpr - s.engine.dpr).abs() > 0.001 {
            s.engine.dpr = current_dpr;

            if s.exact_sizes.available {
                // exact sizes will be refreshed by ResizeObserver callback;
                // but update pixel ratios with new DPR for this frame
                let (pcw, pch) = s.layout.pane_css_size();
                let es = s.exact_sizes;
                let h_ratio = if pcw > 0.0 { es.pane_pw as f64 / pcw } else { current_dpr };
                let v_ratio = if pch > 0.0 { es.pane_ph as f64 / pch } else { current_dpr };
                s.engine.h_pixel_ratio = h_ratio;
                s.engine.v_pixel_ratio = v_ratio;
            } else {
                // Fallback: round(css * dpr)
                s.engine.h_pixel_ratio = current_dpr;
                s.engine.v_pixel_ratio = current_dpr;
                s.layout.resize_all_canvases(current_dpr);
                let (pw, ph) = s.layout.pane_css_size();
                let ppw = (pw * current_dpr).round() as u32;
                let pph = (ph * current_dpr).round() as u32;
                s.engine.resize(ppw.max(1), pph.max(1), current_dpr);
                s.overlay.resize(ppw.max(1), pph.max(1), current_dpr);
                let (aw, ah) = s.layout.price_axis_css_size();
                s.price_axis_renderer.resize(
                    (aw * current_dpr).round() as u32,
                    (ah * current_dpr).round() as u32,
                    current_dpr,
                );
                let (tw, th) = s.layout.time_axis_css_size();
                s.time_axis_renderer.resize(
                    (tw * current_dpr).round() as u32,
                    (th * current_dpr).round() as u32,
                    current_dpr,
                );
            }
        }

        let dpr = s.engine.dpr;

        let (pane_css_w, pane_css_h) = s.layout.pane_css_size();

        {
            let ChartInner { ref mut interaction, ref mut engine, .. } = *s;
            interaction.update_gliding(
                pane_css_w,
                pane_css_h,
                &mut engine.viewport,
                &engine.bars,
            );
        }

        let pane_pw = (pane_css_w * dpr).round();
        let pane_ph = (pane_css_h * dpr).round();

        if pane_pw <= 0.0 || pane_ph <= 0.0 { return; }

        // 1. Compute tick marks (single source of truth)
        let y_ticks = tick_marks::compute_y_ticks(&s.engine.viewport, pane_ph, dpr);
        let x_ticks = tick_marks::compute_x_ticks(
            &s.engine.viewport, &s.engine.bars, pane_pw, dpr,
        );

        // 2. Measure price axis width from tick labels
        let max_text_w_phys = s.price_axis_renderer.measure_max_tick_width(&s.engine.style, &y_ticks);
        let max_text_w_css = max_text_w_phys / dpr;
        let price_axis_css_w = s.engine.style.price_axis_width(max_text_w_css);
        let time_axis_css_h = s.engine.style.time_axis_height();

        // 3. Update CSS grid layout (this may cause pane to resize)
        s.layout.update_axis_sizes(price_axis_css_w, time_axis_css_h);

        // 4. Engine render — candles + volume on pane chart canvas
        if let Err(e) = s.engine.render(&y_ticks, &x_ticks) {
            log::warn!("render error: {}", e);
        }

        // 5. Generate drawing geometry (base = Idle/Selected, top = Creating/Dragging)
        let (base_drawings, top_drawings) = s.engine.drawings.generate_all_geometry(
            &s.engine.viewport, pane_css_w, pane_css_h, dpr,
            s.engine.h_pixel_ratio, s.engine.v_pixel_ratio,
        );

        let is_webgpu = s.engine.renderer_name() == "webgpu";

        if is_webgpu {
            // WebGPU: the chart canvas is a GPU surface — can't draw 2D on it.
            // Render ALL drawings on the overlay canvas (like LWC which renders
            // all primitives on the overlay, not the series canvas).
            let mut all_drawings = base_drawings;
            all_drawings.extend(top_drawings);
            s.overlay.render_with_drawings(&s.engine.crosshair, &s.engine.style, &all_drawings);
        } else {
            // Canvas2D: base drawings on chart canvas (above candles, below crosshair),
            // top drawings + crosshair on the overlay canvas.
            s.overlay.render_base_drawings(&base_drawings);
            s.overlay.render_with_drawings(&s.engine.crosshair, &s.engine.style, &top_drawings);
        }

        // 7. Price axis — base (ticks + labels) + top (crosshair label)
        s.price_axis_renderer.render_base(&s.engine.style, &y_ticks, pane_ph);
        s.price_axis_renderer.render_top(
            &s.engine.crosshair, &s.engine.viewport, &s.engine.style, pane_css_h,
        );

        // 8. Time axis — base (ticks + labels) + top (crosshair label)
        s.time_axis_renderer.render_base(&s.engine.style, &x_ticks, pane_pw);
        s.time_axis_renderer.render_top(
            &s.engine.crosshair, &s.engine.bars,
            &s.engine.viewport, &s.engine.style, pane_css_w,
        );

        // 9. Corner stub — background + borders (LWC: PriceAxisStub)
        Self::render_corner_stub(&s.layout, &s.engine.style, dpr);
    }

    /// Render the corner stub (bottom-right intersection of time axis row + price axis column).
    /// LWC: PriceAxisStub draws bg + horizontal border at top + vertical border at left.
    fn render_corner_stub(layout: &WidgetLayout, style: &ChartStyle, dpr: f64) {
        let canvas = &layout.corner_stub;
        let w = canvas.width() as f64;
        let h = canvas.height() as f64;
        if w <= 0.0 || h <= 0.0 { return; }

        let ctx = match canvas
            .get_context("2d")
            .ok()
            .flatten()
        {
            Some(c) => match c.dyn_into::<web_sys::CanvasRenderingContext2d>() {
                Ok(ctx) => ctx,
                Err(_) => return,
            },
            None => return,
        };

        // Background (LWC: model.backgroundBottomColor())
        let bg = format!(
            "rgba({},{},{},{})",
            (style.bg_color[0] * 255.0) as u8,
            (style.bg_color[1] * 255.0) as u8,
            (style.bg_color[2] * 255.0) as u8,
            style.bg_color[3],
        );
        ctx.set_fill_style_str(&bg);
        ctx.fill_rect(0.0, 0.0, w, h);

        // Border color
        let border = format!(
            "rgba({},{},{},{})",
            (style.axis_border_color[0] * 255.0) as u8,
            (style.axis_border_color[1] * 255.0) as u8,
            (style.axis_border_color[2] * 255.0) as u8,
            style.axis_border_color[3],
        );
        ctx.set_fill_style_str(&border);

        let border_size = (style.axis_border_size as f64 * dpr).max(1.0).floor();

        // Horizontal border at top (continuation of time axis border)
        ctx.fill_rect(0.0, 0.0, w, border_size);

        // Vertical border at left (continuation of price axis border)
        ctx.fill_rect(0.0, 0.0, border_size, h);
    }

    /// Dispose: disconnect resize observer.
    pub fn dispose(&mut self) {
        if let Some(obs) = self._resize_observer.take() {
            obs.disconnect();
        }
    }
}
