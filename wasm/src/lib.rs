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
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use raycore::{
    Bar, ChartEngine, ChartStyle,
    GpuContext, WgpuRenderer, Canvas2DRenderer,
    RendererBackend, OverlayRenderer,
    PriceAxisRenderer, TimeAxisRenderer,
    InteractionHandler, HitZone,
    generate_sample_data, tick_marks,
    LinePoint, LineSeriesOptions, LineStyle,
    AreaSeriesOptions, HistogramSeriesOptions, BarSeriesOptions, BaselineSeriesOptions, SeriesId,
    PriceLineOptions,
    SeriesMarker, MarkerShape, MarkerPosition,
};

mod canvas_manager;
mod subpane;
use canvas_manager::WidgetLayout;
use subpane::{SubPane, IndicatorConfig, PaneHeightCoordinator};

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
    /// Sub-panes for indicators (RSI, ATR, etc.)
    subpanes: Vec<SubPane>,
    /// Next sub-pane ID
    next_subpane_id: u32,
    /// Which sub-pane the cursor is currently over (None = main pane or outside).
    /// Used for proper crosshair coordination instead of y=-1000 hack.
    active_subpane_id: Option<u32>,
    /// Coordinates pane heights using stretch factors (PaneManager bridge).
    pane_coordinator: PaneHeightCoordinator,
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

        let mut is_drawing_drag = false;
        let mut hover_cursor: Option<&'static str> = None;

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
                    is_drawing_drag = true;
                }
            }

            // Hover hit-test for cursor feedback (not during drag/creation, no tool active)
            if !is_drawing_drag && !drawings.is_creating()
                && drawings.active_tool == raycore::DrawingTool::None
            {
                if let Some((hit_id, result)) = drawings.hit_test(x, y, &self.engine.viewport, pw, ph) {
                    use raycore::core::drawings::types::cursor_for_drawing_hit;
                    let tool = drawings.get(hit_id)
                        .map(|d| d.tool())
                        .unwrap_or(raycore::DrawingTool::None);
                    hover_cursor = Some(cursor_for_drawing_hit(tool, result.part, None));
                }
            }
        }

        // Update hover cursor only when not in a drawing drag
        if !self.interaction.drawing_drag_active {
            self.interaction.set_drawing_cursor(hover_cursor);
        }

        if is_drawing_drag {
            // Suppress crosshair while dragging a drawing
            self.engine.crosshair.active = false;
            return; // don't move chart while dragging drawing
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

            let mut should_return = false;
            let mut drag_cursor: Option<&'static str> = None;

            {
                let drawings = &mut self.engine.drawings;

                if drawings.is_tool_active() {
                    // Start creating a new drawing
                    if !drawings.is_creating() {
                        drawings.start_creating(bar, price);
                    } else {
                        // Multi-step tools: place next anchor on click
                        drawings.finalize_creation_step(bar, price);
                    }
                    should_return = true;
                } else {
                    // No tool active: check if user clicked on an existing drawing
                    let hit = drawings.hit_test(x, y, &self.engine.viewport, pw, ph);
                    if let Some((id, result)) = hit {
                        use raycore::core::drawings::types::{HitPart, cursor_for_drawing_hit};
                        let tool = drawings.get(id)
                            .map(|d| d.tool())
                            .unwrap_or(raycore::DrawingTool::None);
                        let anchor_idx = match result.part {
                            HitPart::Anchor(i) => Some(i),
                            _ => None,
                        };

                        // Rectangle: body clicks select only and fall through to
                        // chart pan. Edge clicks move the whole rectangle.
                        // Anchor clicks resize (move single anchor).
                        if tool == raycore::DrawingTool::Rectangle && result.part == HitPart::Body {
                            drawings.select(id);
                            // Don't start drag — fall through to chart pan
                        } else {
                            drawings.select(id);
                            drawings.start_drag(id, anchor_idx, bar, price);
                            drag_cursor = Some(cursor_for_drawing_hit(tool, result.part, anchor_idx));
                            should_return = true;
                        }
                    } else {
                        // Click on empty space: deselect
                        drawings.deselect_all();
                    }
                }
            }

            if should_return {
                if let Some(cursor) = drag_cursor {
                    self.interaction.drawing_drag_active = true;
                    self.interaction.set_drawing_cursor(Some(cursor));
                }
                return; // don't pan while drawing tool / drawing drag
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
        let mut ended_drag = false;
        {
            let drawings = &mut self.engine.drawings;
            if let Some(id) = drawings.selected_id {
                if matches!(drawings.get(id).map(|d| d.state()),
                    Some(raycore::core::drawings::types::DrawingState::Dragging { .. })) {
                    drawings.end_drag(id);
                    ended_drag = true;
                }
            }
        }
        if ended_drag {
            self.interaction.drawing_drag_active = false;
            self.interaction.set_drawing_cursor(None);
            // Restore crosshair for mouse (touch handled by tracking mode)
            if !self.interaction.is_touch {
                self.engine.crosshair.active = true;
            }
            return;
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
                // Clear subpane focus — cursor is now in the main pane
                s.active_subpane_id = None;
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
                let is_dragging = s.interaction.is_dragging() || s.interaction.drawing_drag_active;
                
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

                // Ignore right-click (button 2) — handled by contextmenu
                if pe.button() == 2 { return; }

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
        // pane: contextmenu — remove all scale drawings and exit scale mode on right-click
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
                let amount = if ctrl { 10.0 } else if shift { 5.0 } else { 1.0 };
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
                let amount = if ctrl { 10.0 } else if shift { 5.0 } else { 1.0 };
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
                let half_range = (s.engine.viewport.price_max - s.engine.viewport.price_min) / 2.0 / factor;
                s.engine.viewport.price_min = mid - half_range;
                s.engine.viewport.price_max = mid + half_range;
                true
            }
            "ArrowDown" => {
                let factor = if ctrl { 1.2 } else { 1.05 };
                let mid = (s.engine.viewport.price_max + s.engine.viewport.price_min) / 2.0;
                let half_range = (s.engine.viewport.price_max - s.engine.viewport.price_min) / 2.0 * factor;
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
            
            // Zoom time axis
            "+" | "=" => {
                // Zoom in: reduce visible range
                let mid = (s.engine.viewport.start_bar + s.engine.viewport.end_bar) / 2.0;
                let half_range = (s.engine.viewport.end_bar - s.engine.viewport.start_bar) / 2.0 / 1.2;
                s.engine.viewport.start_bar = mid - half_range;
                s.engine.viewport.end_bar = mid + half_range;
                true
            }
            "-" | "_" => {
                // Zoom out: increase visible range
                let mid = (s.engine.viewport.start_bar + s.engine.viewport.end_bar) / 2.0;
                let half_range = (s.engine.viewport.end_bar - s.engine.viewport.start_bar) / 2.0 * 1.2;
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

    /// Set watermark text displayed centered on the chart pane.
    pub fn set_watermark(&mut self, text: &str) {
        self.inner.borrow_mut().engine.style.watermark_text = text.to_string();
    }

    // ── Runtime Style Configuration API ────────────────────────────────────────

    /// Set the chart background color (RGBA, 0.0-1.0).
    pub fn set_background_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        let mut s = self.inner.borrow_mut();
        s.engine.style.bg_color = [r, g, b, a];
        s.engine.style.axis_bg_color = [r, g, b, a]; // sync axis bg
    }

    /// Set the grid line color (RGBA, 0.0-1.0).
    pub fn set_grid_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.grid_color = [r, g, b, a];
    }

    /// Set the axis border color (RGBA, 0.0-1.0).
    pub fn set_axis_border_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.axis_border_color = [r, g, b, a];
    }

    /// Set the axis text color (RGBA, 0.0-1.0).
    pub fn set_axis_text_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.axis_text_color = [r, g, b, a];
    }

    /// Set the crosshair line color (RGBA, 0.0-1.0).
    pub fn set_crosshair_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.crosshair_color = [r, g, b, a];
    }

    /// Set the crosshair label background color (RGBA, 0.0-1.0).
    pub fn set_crosshair_label_bg_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.crosshair_label_bg = [r, g, b, a];
    }

    /// Set the crosshair label text color (RGBA, 0.0-1.0).
    pub fn set_crosshair_label_text_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.crosshair_label_text = [r, g, b, a];
    }

    /// Set bullish (up) candle colors: body fill and wick/border.
    pub fn set_bullish_color(&mut self, fill_r: f32, fill_g: f32, fill_b: f32, fill_a: f32,
                              wick_r: f32, wick_g: f32, wick_b: f32, wick_a: f32) {
        let mut s = self.inner.borrow_mut();
        s.engine.style.bullish_color = [fill_r, fill_g, fill_b, fill_a];
        s.engine.style.wick_bullish_color = [wick_r, wick_g, wick_b, wick_a];
    }

    /// Set bearish (down) candle colors: body fill and wick/border.
    pub fn set_bearish_color(&mut self, fill_r: f32, fill_g: f32, fill_b: f32, fill_a: f32,
                              wick_r: f32, wick_g: f32, wick_b: f32, wick_a: f32) {
        let mut s = self.inner.borrow_mut();
        s.engine.style.bearish_color = [fill_r, fill_g, fill_b, fill_a];
        s.engine.style.wick_bearish_color = [wick_r, wick_g, wick_b, wick_a];
    }

    /// Set volume bar colors: bullish and bearish.
    pub fn set_volume_colors(&mut self, up_r: f32, up_g: f32, up_b: f32, up_a: f32,
                              down_r: f32, down_g: f32, down_b: f32, down_a: f32) {
        let mut s = self.inner.borrow_mut();
        s.engine.style.bullish_volume_color = [up_r, up_g, up_b, up_a];
        s.engine.style.bearish_volume_color = [down_r, down_g, down_b, down_a];
    }

    /// Set the watermark text color (RGBA, 0.0-1.0).
    pub fn set_watermark_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner.borrow_mut().engine.style.watermark_color = [r, g, b, a];
    }

    /// Set the font size for axis labels (in CSS pixels).
    pub fn set_font_size(&mut self, size: f32) {
        self.inner.borrow_mut().engine.style.font_size = size;
    }

    /// Set the font family for axis labels.
    pub fn set_font_family(&mut self, family: &str) {
        self.inner.borrow_mut().engine.style.font_family = family.to_string();
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

    /// Set the price scale mode.
    ///
    /// Accepted values: "normal", "logarithmic" (or "log"), "percentage" (or "percent"),
    /// "indexed_to_100" (or "indexedTo100", "indexed").
    pub fn set_price_scale_mode(&mut self, mode: &str) {
        use raycore::PriceScaleMode;
        let mode = PriceScaleMode::from_str(mode);
        self.inner.borrow_mut().engine.viewport.set_price_scale_mode(mode);
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
        if let Some(line) = self.inner.borrow_mut().engine.price_lines.get_mut(PriceLineId(id)) {
            line.set_price(price);
        }
    }

    /// Set whether a price line is visible.
    pub fn set_price_line_visible(&mut self, id: u32, visible: bool) {
        use raycore::PriceLineId;
        if let Some(line) = self.inner.borrow_mut().engine.price_lines.get_mut(PriceLineId(id)) {
            line.options.visible = visible;
        }
    }

    /// Set the label text of a price line. Empty string uses formatted price.
    pub fn set_price_line_label(&mut self, id: u32, label: &str) {
        use raycore::PriceLineId;
        if let Some(line) = self.inner.borrow_mut().engine.price_lines.get_mut(PriceLineId(id)) {
            line.options.label_text = label.to_string();
        }
    }

    /// Remove a price line by ID.
    pub fn remove_price_line(&mut self, id: u32) -> bool {
        use raycore::PriceLineId;
        self.inner.borrow_mut().engine.price_lines.remove(PriceLineId(id))
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
            text_color: [1.0, 1.0, 1.0, 0.9],
            id: 0, // will be assigned
        };
        let id = self.inner.borrow_mut().engine.markers.for_series(series_id).add(marker);
        log::info!("add_marker: series={}, bar={}, shape={}, id={}", series_id, bar_index, shape, id);
        id
    }

    /// Remove a specific marker from a series.
    pub fn remove_marker(&mut self, series_id: u32, marker_id: u32) -> bool {
        self.inner.borrow_mut().engine.markers.for_series(series_id).remove(marker_id)
    }

    /// Clear all markers for a series.
    pub fn clear_markers(&mut self, series_id: u32) {
        self.inner.borrow_mut().engine.markers.clear_series(series_id);
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
            let color = [chunk[4] as f32, chunk[5] as f32, chunk[6] as f32, chunk[7] as f32];
            let size = chunk[8];

            markers.push(SeriesMarker {
                bar_index,
                shape,
                position,
                price,
                color,
                size,
                text: String::new(),
                text_color: [1.0, 1.0, 1.0, 0.9],
                id: 0,
            });
        }

        self.inner.borrow_mut().engine.markers.for_series(series_id).set(markers);
        log::info!("set_markers: series={}, count={}", series_id, marker_data.len() / STRIDE);
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
        opts.bottom_color = [bottom_color_r, bottom_color_g, bottom_color_b, bottom_color_a];
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
    ) {
        let count = values.len().min(timestamps.len());
        let has_colors = colors_r.len() >= count
            && colors_g.len() >= count
            && colors_b.len() >= count
            && colors_a.len() >= count;

        let mut s = self.inner.borrow_mut();
        if has_colors {
            // Build per-bar color array
            let mut colors = Vec::with_capacity(count);
            for i in 0..count {
                colors.push([colors_r[i], colors_g[i], colors_b[i], colors_a[i]]);
            }
            if let Some(series) = s.engine.series.get_mut(SeriesId(id)) {
                series.histogram_data.set_from_arrays_with_colors(timestamps, values, &colors);
            }
        } else {
            s.engine.set_histogram_data_arrays(SeriesId(id), timestamps, values);
        }
        log::info!("set_histogram_data: id={}, {} points, colors={}", id, count, has_colors);
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
    ) {
        let mut s = self.inner.borrow_mut();
        s.engine.set_bar_data_arrays(SeriesId(id), timestamps, open, high, low, close);
        let count = timestamps.len().min(open.len()).min(high.len()).min(low.len()).min(close.len());
        log::info!("set_bar_series_data: id={}, {} bars", id, count);
    }

    /// Add a new baseline series overlay. Returns the series ID.
    ///
    /// A baseline series renders a line with two-tone fill above/below a base value.
    /// Above the base: `top_line_color` line + `top_fill_color1`→`top_fill_color2` gradient.
    /// Below the base: `bottom_line_color` line + `bottom_fill_color1`→`bottom_fill_color2` gradient.
    pub fn add_baseline_series(
        &mut self,
        base_value: f64,
        top_line_r: f32, top_line_g: f32, top_line_b: f32, top_line_a: f32,
        bottom_line_r: f32, bottom_line_g: f32, bottom_line_b: f32, bottom_line_a: f32,
        top_fill1_r: f32, top_fill1_g: f32, top_fill1_b: f32, top_fill1_a: f32,
        top_fill2_r: f32, top_fill2_g: f32, top_fill2_b: f32, top_fill2_a: f32,
        bottom_fill1_r: f32, bottom_fill1_g: f32, bottom_fill1_b: f32, bottom_fill1_a: f32,
        bottom_fill2_r: f32, bottom_fill2_g: f32, bottom_fill2_b: f32, bottom_fill2_a: f32,
        line_width: f32,
    ) -> u32 {
        let mut opts = BaselineSeriesOptions::default();
        opts.base_value = base_value;
        opts.top_line_color = [top_line_r, top_line_g, top_line_b, top_line_a];
        opts.bottom_line_color = [bottom_line_r, bottom_line_g, bottom_line_b, bottom_line_a];
        opts.top_fill_color1 = [top_fill1_r, top_fill1_g, top_fill1_b, top_fill1_a];
        opts.top_fill_color2 = [top_fill2_r, top_fill2_g, top_fill2_b, top_fill2_a];
        opts.bottom_fill_color1 = [bottom_fill1_r, bottom_fill1_g, bottom_fill1_b, bottom_fill1_a];
        opts.bottom_fill_color2 = [bottom_fill2_r, bottom_fill2_g, bottom_fill2_b, bottom_fill2_a];
        opts.line_width = line_width as f64;
        let id = self.inner.borrow_mut().engine.add_baseline_series(opts);
        log::info!("add_baseline_series: id={}, base_value={}", id.0, base_value);
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

    // ── Study API ─────────────────────────────────────────────────────────

    /// Create a new study instance. Returns the study ID, or 0 if the type is unknown.
    ///
    /// Supported types: "sma", "ema", "rsi", "macd".
    pub fn create_study(&mut self, study_type: &str) -> u32 {
        let mut s = self.inner.borrow_mut();
        match s.engine.create_study(study_type) {
            Some(id) => {
                // If we have bar data, run initial calculation
                // We need to split the borrow: collect bar data pointer via raw parts
                // to avoid simultaneous mutable+immutable borrow on engine.
                let bar_len = s.engine.bars.len();
                if bar_len > 0 {
                    // Safe: update_studies only reads bars, doesn't modify engine.bars
                    let bars_ptr = &s.engine.bars as *const raycore::BarArray;
                    unsafe { s.engine.studies.update_studies(&*bars_ptr); }
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
        let removed = self.inner.borrow_mut().engine.remove_study(raycore::StudyId(id));
        log::info!("remove_study: id={}, removed={}", id, removed);
        removed
    }

    /// Set a study parameter (e.g., "period" for SMA/EMA, "fast_period" for MACD).
    /// The study will be recalculated on the next render.
    pub fn set_study_parameter(&mut self, id: u32, key: &str, value: f64) {
        let mut s = self.inner.borrow_mut();
        s.engine.set_study_parameter(raycore::StudyId(id), key, value);
        // Recalculate immediately if we have data
        let bar_len = s.engine.bars.len();
        if bar_len > 0 {
            let bars_ptr = &s.engine.bars as *const raycore::BarArray;
            unsafe { s.engine.studies.update_studies(&*bars_ptr); }
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
                let ts_arr = js_sys::BigUint64Array::new_with_length(output.data.timestamps.len() as u32);
                let val_arr = js_sys::Float32Array::new_with_length(output.data.values.len() as u32);
                // Copy data
                for i in 0..output.data.timestamps.len() {
                    ts_arr.set_index(i as u32, output.data.timestamps[i]);
                }
                for i in 0..output.data.values.len() {
                    val_arr.set_index(i as u32, output.data.values[i]);
                }
                let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("timestamps"), &ts_arr);
                let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("values"), &val_arr);
                let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("name"), &JsValue::from_str(&output.name));
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
    pub fn add_indicator_pane(&mut self, study_id: u32, indicator_type: &str, height_css: f64) -> u32 {
        let inner_for_events = Rc::clone(&self.inner);

        // ── Phase 1: Create sub-pane, extract DOM refs + shared state ──
        let creation_result: Option<(u32, web_sys::Element, web_sys::Element, Rc<Cell<f64>>, Rc<Cell<bool>>)> = {
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
            let available_h = total_h + (subpane_count as f64 * height_css) + height_css + time_axis_h;
            s.pane_coordinator.set_total_height(available_h);
            
            // Use coordinator's computed height, or fall back to requested height
            let coordinated_height = s.pane_coordinator.get_height(id);
            let initial_height = if coordinated_height > 0.0 { coordinated_height } else { height_css };

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
                        colors.push(config.colors.get(i).copied().unwrap_or([0.5, 0.5, 0.5, 1.0]));
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

            log::info!("Created indicator sub-pane: id={}, type={}, height={:.1}", id, indicator_type, initial_height);
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

        // Shared drag state for chart scroll
        let sp_drag_active = Rc::new(Cell::new(false));
        let sp_drag_start_x: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_drag_start_start_bar: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let sp_drag_start_end_bar: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));

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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_: web_sys::Event| {
                log::info!("SubPane {} pointerenter", pid);
                ca.set(true);
                let mut s = inner.borrow_mut();
                s.engine.crosshair.active = true;
                s.active_subpane_id = Some(pid);
            }));
            let _ = chart_el.add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointerleave ──
        {
            let inner = Rc::clone(&inner_for_events);
            let ca = crosshair_active.clone();
            let drag = sp_drag_active.clone();
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_: web_sys::Event| {
                log::info!("SubPane {} pointerleave", pid);
                ca.set(false);
                if !drag.get() {
                    let mut s = inner.borrow_mut();
                    s.engine.crosshair.active = false;
                    s.active_subpane_id = None;
                }
            }));
            let _ = chart_el.add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref());
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
            let drag_sb = sp_drag_start_start_bar.clone();
            let drag_eb = sp_drag_start_end_bar.clone();
            let is_touch = sp_is_touch.clone();
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let rect = chart_c.get_bounding_client_rect();
                let x = pe.client_x() as f64 - rect.left();
                let y = pe.client_y() as f64 - rect.top();
                let now_ms = js_sys::Date::now();

                cy.set(y);
                ca.set(true);

                let mut s = inner.borrow_mut();
                s.engine.crosshair.active = true;
                s.active_subpane_id = Some(pid);
                s.engine.crosshair.x = x;
                // Don't set crosshair.y here — main pane reads active_subpane_id
                // to decide whether to draw its own horizontal crosshair

                // Update bar_index for time axis label
                let (pw, _) = s.layout.pane_css_size();
                s.engine.crosshair.bar_index =
                    s.engine.viewport.bar_index_at_pixel(x, pw, s.engine.bars.len());

                // Handle drag scroll
                if drag.get() {
                    // Update scroll tracking for kinetic animation (touch only)
                    if is_touch.get() {
                        if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                            sp.scroll_state.borrow_mut().update_drag(x, now_ms);
                        }
                    }
                    
                    let delta_x = x - drag_sx.get();
                    let bar_range = drag_eb.get() - drag_sb.get();
                    if pw > 0.0 {
                        let bars_per_px = bar_range / pw;
                        let delta_bars = -delta_x * bars_per_px;
                        s.engine.viewport.start_bar = drag_sb.get() + delta_bars;
                        s.engine.viewport.end_bar = drag_eb.get() + delta_bars;
                        let bar_len = s.engine.bars.len();
                        s.engine.viewport.clamp_to_data(bar_len);
                        if !s.engine.viewport.price_locked {
                            let bars_ptr = &s.engine.bars as *const raycore::BarArray;
                            unsafe { s.engine.viewport.auto_fit_price(&*bars_ptr); }
                        }
                    }
                    let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", "grabbing");
                }
            }));
            let _ = chart_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointerdown ──
        {
            let inner = Rc::clone(&inner_for_events);
            let drag = sp_drag_active.clone();
            let drag_sx = sp_drag_start_x.clone();
            let drag_sb = sp_drag_start_start_bar.clone();
            let drag_eb = sp_drag_start_end_bar.clone();
            let chart_c = chart_el.clone();
            let is_touch = sp_is_touch.clone();
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                if pe.button() == 2 { return; }
                let rect = chart_c.get_bounding_client_rect();
                let x = pe.client_x() as f64 - rect.left();
                let now_ms = js_sys::Date::now();

                // Detect touch input (same as main chart)
                is_touch.set(pe.pointer_type() == "touch");

                drag.set(true);
                drag_sx.set(x);

                let mut s = inner.borrow_mut();
                drag_sb.set(s.engine.viewport.start_bar);
                drag_eb.set(s.engine.viewport.end_bar);
                
                // Start scroll tracking for kinetic animation (touch only)
                if is_touch.get() {
                    if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                        sp.scroll_state.borrow_mut().start_drag(x, s.engine.viewport.start_bar, now_ms);
                    }
                }
                drop(s);

                // Capture pointer for reliable drag across boundaries
                let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                let _ = html_el.set_pointer_capture(pe.pointer_id());
            }));
            let _ = chart_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointerup ──
        {
            let inner = Rc::clone(&inner_for_events);
            let drag = sp_drag_active.clone();
            let chart_c = chart_el.clone();
            let is_touch = sp_is_touch.clone();
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let now_ms = js_sys::Date::now();
                drag.set(false);
                
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
            let _ = chart_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: pointercancel ──
        {
            let inner = Rc::clone(&inner_for_events);
            let drag = sp_drag_active.clone();
            let chart_c = chart_el.clone();
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            let _ = chart_el.add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: wheel (forward to main chart zoom) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let chart_c = chart_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(move |e: web_sys::WheelEvent| {
                e.prevent_default();
                let rect = chart_c.get_bounding_client_rect();
                let x = e.client_x() as f64 - rect.left();
                let mut s = inner.borrow_mut();
                s.on_pane_wheel(x, e.delta_x(), e.delta_y(), e.delta_mode());
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = chart_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel", cb.as_ref().unchecked_ref(), &opts,
            );
            wheel_closures.push(cb);
        }

        // ── chart: contextmenu ──
        {
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                e.prevent_default();
            }));
            let _ = chart_el.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref());
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
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(move |e: web_sys::TouchEvent| {
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
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = chart_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchstart", cb.as_ref().unchecked_ref(), &opts,
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
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(move |e: web_sys::TouchEvent| {
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
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = chart_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchmove", cb.as_ref().unchecked_ref(), &opts,
            );
            touch_closures.push(cb);
        }

        // ── chart: touchend (end pinch) ──
        {
            let pinch = sp_pinch_active.clone();
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(move |e: web_sys::TouchEvent| {
                let touches = e.touches();
                if touches.length() < 2 {
                    pinch.set(false);
                }
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = chart_el.add_event_listener_with_callback_and_add_event_listener_options(
                "touchend", cb.as_ref().unchecked_ref(), &opts,
            );
            touch_closures.push(cb);
        }

        // ── chart: dblclick (reset viewport to default) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                let mut s = inner.borrow_mut();
                if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                    sp.reset_price_viewport();
                    log::info!("SubPane {} viewport reset via double-click", pid);
                }
            }));
            let _ = chart_el.add_event_listener_with_callback("dblclick", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: dblclick (toggle auto-scale) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                let mut s = inner.borrow_mut();
                if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                    sp.toggle_auto_scale();
                    log::info!("SubPane {} auto-scale toggled: {}", pid, sp.auto_scale);
                }
            }));
            let _ = axis_el.add_event_listener_with_callback("dblclick", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: wheel (zoom sub-pane price range) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(move |e: web_sys::WheelEvent| {
                e.prevent_default();
                let dy = e.delta_y();
                let factor = if dy > 0.0 { 1.1 } else { 0.9 };
                let mut s = inner.borrow_mut();
                if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                    let center = (sp.viewport.price_min + sp.viewport.price_max) / 2.0;
                    let half = (sp.viewport.price_max - sp.viewport.price_min) / 2.0 * factor;
                    sp.viewport.price_min = center - half;
                    sp.viewport.price_max = center + half;
                }
            }));
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = axis_el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel", cb.as_ref().unchecked_ref(), &opts,
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
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
            let _ = axis_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref());
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                if !adrag.get() { return; }
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let rect = axis_c.get_bounding_client_rect();
                let y = pe.client_y() as f64 - rect.top();
                let css_h = rect.height();
                if css_h <= 1.0 { return; }

                let delta_y = y - ady.get();
                let factor = (1.0 + delta_y / css_h).max(0.1);

                let center = (apmin.get() + apmax.get()) / 2.0;
                let half = (apmax.get() - apmin.get()) / 2.0 * factor;

                let mut s = inner.borrow_mut();
                if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                    sp.viewport.price_min = center - half;
                    sp.viewport.price_max = center + half;
                }
            }));
            let _ = axis_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: pointerup ──
        {
            let adrag = axis_drag_active.clone();
            let axis_c = axis_el.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                adrag.set(false);
                let html_el: &web_sys::HtmlElement = axis_c.unchecked_ref();
                let _ = html_el.release_pointer_capture(pe.pointer_id());
            }));
            let _ = axis_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: contextmenu ──
        {
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                e.prevent_default();
            }));
            let _ = axis_el.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref());
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
                let _ = subpane.separator.style().set_property("grid-row", &sep_row.to_string());
                let _ = subpane.chart_container.style().set_property("grid-row", &pane_row.to_string());
                let _ = subpane.axis_container.style().set_property("grid-row", &pane_row.to_string());
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
                let colors: Vec<[f32; 4]> = data.iter().enumerate()
                    .map(|(i, _)| config.colors.get(i).copied().unwrap_or([0.5, 0.5, 0.5, 1.0]))
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
        s.pane_coordinator.drag_separator(separator_idx as usize, delta_y);
        
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
    ) {
        let bar = Bar {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            _pad: 0.0,
        };
        self.inner.borrow_mut().engine.append_bar(bar);
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
    ) {
        let bar = Bar {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            _pad: 0.0,
        };
        self.inner.borrow_mut().engine.update_bar(bar);
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
                // Resize sub-panes
                for subpane in s.subpanes.iter_mut() {
                    subpane.resize(current_dpr);
                }
            }
        }

        let dpr = s.engine.dpr;
        let anim_time = js_sys::Date::now(); // For pulsing animations

        let (pane_css_w, pane_css_h) = s.layout.pane_css_size();

        // Update main chart kinetic scrolling
        {
            let ChartInner { ref mut interaction, ref mut engine, .. } = *s;
            interaction.update_gliding(
                pane_css_w,
                pane_css_h,
                &mut engine.viewport,
                &engine.bars,
            );
        }

        // Update subpane kinetic scrolling (shares time axis with main chart)
        // Collect all kinetic deltas first, then apply to viewport
        {
            let bar_len = s.engine.bars.len();
            let pane_css_w_for_kinetic = pane_css_w;
            
            // Collect kinetic deltas from all subpanes
            let mut total_kinetic_delta_px = 0.0;
            for subpane in s.subpanes.iter() {
                if let Some(delta_px) = subpane.update_kinetic(anim_time) {
                    total_kinetic_delta_px += delta_px;
                }
            }
            
            // Apply accumulated kinetic delta to shared viewport
            if total_kinetic_delta_px.abs() > 0.001 && pane_css_w_for_kinetic > 0.0 {
                let bar_range = s.engine.viewport.end_bar - s.engine.viewport.start_bar;
                let delta_bars = -total_kinetic_delta_px * bar_range / pane_css_w_for_kinetic;
                s.engine.viewport.pan_clamped(delta_bars, bar_len);
                if !s.engine.viewport.price_locked {
                    let bars_ptr = &s.engine.bars as *const raycore::BarArray;
                    unsafe { s.engine.viewport.auto_fit_price(&*bars_ptr); }
                }
            }
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
        // Destructure to borrow price_axis_renderer mutably while engine is borrowed immutably
        {
            let ChartInner { ref mut price_axis_renderer, ref engine, ref mut layout, ref subpanes, .. } = *s;
            let max_text_w_phys = price_axis_renderer.measure_max_tick_width(&engine.style, &y_ticks);
            let max_text_w_css = max_text_w_phys / dpr;
            let price_axis_css_w = engine.style.price_axis_width(max_text_w_css);
            let time_axis_css_h = engine.style.time_axis_height();

            // 3. Update CSS grid layout (subpane-aware — keeps time axis visible)
            if subpanes.is_empty() {
                layout.update_axis_sizes(price_axis_css_w, time_axis_css_h);
            } else {
                let heights: Vec<f64> = subpanes.iter()
                    .map(|sp| sp.get_height())
                    .collect();
                layout.update_axis_sizes_with_subpanes(
                    price_axis_css_w, time_axis_css_h, &heights,
                );
            }
        }

        // 4. Engine render — candles + volume on pane chart canvas
        if let Err(e) = s.engine.render(&y_ticks, &x_ticks) {
            log::warn!("render error: {}", e);
        }

        // 4b. Dashed line series — rendered via Canvas2D strokePath (not rects).
        // Build bar timestamps for line_generator point lookup.
        let bar_ts: Vec<u64> = (0..s.engine.bars.len())
            .map(|i| s.engine.bars.timestamps.value(i))
            .collect();

        // 5. Generate drawing geometry (base = Idle/Selected, top = Creating/Dragging)
        let (base_drawings, top_drawings) = s.engine.drawings.generate_all_geometry(
            &s.engine.viewport, pane_css_w, pane_css_h, dpr,
            s.engine.h_pixel_ratio, s.engine.v_pixel_ratio,
        );

        let is_webgpu = s.engine.renderer_name() == "webgpu";

        // 6. Render overlay, dashed series, price lines, last price lines, drawings, crosshair, markers
        // When cursor is in a subpane, suppress horizontal crosshair in main pane
        // by creating a modified crosshair state (proper replacement for the y=-1000 hack)
        {
            let ChartInner { ref mut overlay, ref engine, ref active_subpane_id, .. } = *s;
            let main_crosshair = if active_subpane_id.is_some() && engine.crosshair.active {
                // Cursor is in a subpane: keep vertical line, suppress horizontal
                let mut ch = engine.crosshair;
                ch.y = -1.0; // Outside valid range so horizontal line won't draw
                ch
            } else {
                engine.crosshair
            };
            if is_webgpu {
                let mut all_drawings = base_drawings;
                all_drawings.extend(top_drawings);
                // Clear and render base layer first (watermark, legend, drawings, crosshair)
                overlay.render_with_drawings(&main_crosshair, &engine.style, &all_drawings, Some(&engine.bars));
                // Then render on top of the cleared canvas:
                overlay.render_dashed_series(
                    &engine.series, &engine.viewport, &bar_ts,
                    pane_pw, pane_ph, engine.v_pixel_ratio, true,
                );
                // Custom price lines
                overlay.render_price_lines(
                    &engine.price_lines, &engine.viewport,
                    &engine.style, pane_css_w, pane_css_h,
                );
                // Last price lines (below crosshair markers, above dashed series)
                overlay.render_last_price_lines(
                    &engine.series, &engine.bars, &engine.viewport,
                    &engine.style, pane_css_w, pane_css_h, anim_time,
                );
                // Series markers (arrows, circles, squares at bar indices)
                overlay.render_markers(
                    &engine.markers, &engine.bars, &engine.viewport,
                    &engine.style, pane_css_w, pane_css_h,
                );
                // Crosshair marker circles on series (above crosshair lines)
                overlay.render_crosshair_markers(
                    &main_crosshair, &engine.series, &engine.bars, &bar_ts,
                    &engine.viewport, &engine.style, pane_css_w, pane_css_h,
                );
            } else {
                // Clear and render base layer first
                overlay.render_with_drawings(&main_crosshair, &engine.style, &top_drawings, Some(&engine.bars));
                // Then render additional elements on top:
                overlay.render_dashed_series(
                    &engine.series, &engine.viewport, &bar_ts,
                    pane_pw, pane_ph, engine.v_pixel_ratio, false,
                );
                // Custom price lines
                overlay.render_price_lines(
                    &engine.price_lines, &engine.viewport,
                    &engine.style, pane_css_w, pane_css_h,
                );
                // Last price lines on overlay canvas
                overlay.render_last_price_lines(
                    &engine.series, &engine.bars, &engine.viewport,
                    &engine.style, pane_css_w, pane_css_h, anim_time,
                );
                // Series markers (arrows, circles, squares at bar indices)
                overlay.render_markers(
                    &engine.markers, &engine.bars, &engine.viewport,
                    &engine.style, pane_css_w, pane_css_h,
                );
                overlay.render_base_drawings(&base_drawings);
                // Crosshair marker circles on series (above crosshair lines)
                overlay.render_crosshair_markers(
                    &main_crosshair, &engine.series, &engine.bars, &bar_ts,
                    &engine.viewport, &engine.style, pane_css_w, pane_css_h,
                );
            }
        }

        // 7. Price axis — base (ticks + labels) + last price labels + price line labels + top (crosshair label)
        {
            let ChartInner { ref mut price_axis_renderer, ref engine, ref active_subpane_id, .. } = *s;
            price_axis_renderer.render_base(&engine.style, &y_ticks, pane_ph);
            price_axis_renderer.render_last_price_labels(
                &engine.series, &engine.bars, &engine.viewport, &engine.style, pane_css_h,
            );
            price_axis_renderer.render_price_line_labels(
                &engine.price_lines, &engine.viewport, &engine.style, pane_css_h,
            );
            // Suppress price axis crosshair label when cursor is in a subpane
            let main_ch = if active_subpane_id.is_some() && engine.crosshair.active {
                let mut ch = engine.crosshair;
                ch.y = -1.0;
                ch
            } else {
                engine.crosshair
            };
            price_axis_renderer.render_top(
                &main_ch, &engine.viewport, &engine.style, pane_css_h,
            );
        }

        // 8. Time axis — base (ticks + labels) + scrollbar + top (crosshair label)
        {
            let ChartInner { ref mut time_axis_renderer, ref engine, .. } = *s;
            time_axis_renderer.render_base(&engine.style, &x_ticks, pane_pw);
            // Scrollbar indicator at top of time axis
            time_axis_renderer.render_scrollbar(
                &engine.style,
                &engine.viewport,
                engine.bars.len(),
                pane_pw,
            );
            time_axis_renderer.render_top(
                &engine.crosshair, &engine.bars,
                &engine.viewport, &engine.style, pane_css_w,
            );
        }

        // 9. Indicator sub-panes — update data from studies and render
        {
            // First, collect updated study data for each subpane
            let study_updates: Vec<(u32, Vec<raycore::core::series::LineDataArray>, String)> = {
                let ChartInner { ref subpanes, ref engine, .. } = *s;
                subpanes.iter().filter_map(|subpane| {
                    if let Some(study) = engine.studies.get_study(raycore::StudyId(subpane.study_id)) {
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
                }).collect()
            };
            
            // Update subpane data using IndicatorConfig for colors
            for (pane_id, data, indicator_type) in study_updates {
                if let Some(subpane) = s.subpanes.iter_mut().find(|sp| sp.id == pane_id) {
                    if !data.is_empty() {
                        let config = IndicatorConfig::for_type(&indicator_type);
                        let colors: Vec<[f32; 4]> = data.iter().enumerate()
                            .map(|(i, _)| config.colors.get(i).copied().unwrap_or([0.5, 0.5, 0.5, 1.0]))
                            .collect();
                        subpane.set_data(data, colors);
                    }
                }
            }
            
            // Resize, render, and draw crosshair
            let ChartInner { ref mut subpanes, ref engine, active_subpane_id: _, .. } = *s;
            let crosshair_x = if engine.crosshair.active {
                Some(engine.crosshair.x)
            } else {
                None
            };
            for subpane in subpanes.iter_mut() {
                subpane.resize(dpr);
                subpane.render(&engine.viewport, &engine.style);
                
                // Clear crosshair overlay first
                subpane.clear_crosshair_overlay();
                
                // Vertical crosshair line in every subpane (like LWC)
                if let Some(x) = crosshair_x {
                    subpane.render_crosshair_vert(x, &engine.style);
                }
                // Horizontal crosshair line + price label (if mouse is in this sub-pane)
                subpane.render_crosshair_horiz(&engine.style);
            }
        }

        // 10. Corner stub — background + borders (LWC: PriceAxisStub)
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
