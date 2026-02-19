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
    RendererBackend, OverlayRenderer, GridRenderer,
    PriceAxisRenderer, TimeAxisRenderer,
    InteractionHandler, HitZone,
    generate_sample_data, tick_marks,
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

/// Internal chart state shared between event closures and the public API.
struct ChartInner {
    engine: ChartEngine,
    grid: GridRenderer,
    overlay: OverlayRenderer,
    price_axis_renderer: PriceAxisRenderer,
    time_axis_renderer: TimeAxisRenderer,
    layout: WidgetLayout,
    interaction: InteractionHandler,
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
        let Self { interaction, engine, .. } = self;
        interaction.pane_pointer_move(
            x, y, pw, ph,
            &mut engine.viewport, &mut engine.crosshair,
            engine.bars.as_slice(), dpr,
        );
    }

    fn on_pointer_down(&mut self, x: f64, y: f64, zone: HitZone) {
        let (_, pane_css_h) = self.layout.pane_css_size();
        let Self { interaction, engine, .. } = self;
        interaction.pointer_down(x, y, zone, &engine.viewport, pane_css_h);
    }

    fn on_pointer_up(&mut self) {
        let now_ms = js_sys::Date::now();
        let Self { interaction, engine, .. } = self;
        interaction.pointer_up(&mut engine.viewport, engine.bars.as_slice(), now_ms);
    }

    fn on_pane_wheel(&mut self, x: f64, dx: f64, dy: f64, dm: u32) {
        let (pw, _) = self.layout.pane_css_size();
        let Self { interaction, engine, .. } = self;
        interaction.pane_wheel(x, dx, dy, dm, pw, &mut engine.viewport, engine.bars.as_slice());
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
        interaction.time_axis_pointer_move(x, pw, &mut engine.viewport, engine.bars.as_slice());
    }

    fn on_time_axis_wheel(&mut self, x: f64, dy: f64, dm: u32) {
        let (pw, _) = self.layout.pane_css_size();
        let Self { interaction, engine, .. } = self;
        interaction.time_axis_wheel(x, dy, dm, pw, &mut engine.viewport, engine.bars.as_slice());
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
    _resize_closure: Option<Closure<dyn FnMut(js_sys::Array)>>,
    _resize_observer: Option<web_sys::ResizeObserver>,
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

        // Pane renderers
        let grid = GridRenderer::new(layout.pane.grid.clone(), dpr)
            .map_err(|e| JsValue::from_str(&e))?;
        let overlay = OverlayRenderer::new(layout.pane.top.clone(), dpr)
            .map_err(|e| JsValue::from_str(&e))?;

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
            grid,
            overlay,
            price_axis_renderer,
            time_axis_renderer,
            layout,
            interaction,
        }));

        let mut closures: Vec<Closure<dyn FnMut(web_sys::Event)>> = Vec::new();
        let mut wheel_closures: Vec<Closure<dyn FnMut(web_sys::WheelEvent)>> = Vec::new();

        // ── PANE events (on pane's top canvas — topmost z-index in pane) ──
        let pane_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.pane.top.clone().unchecked_into()
        };
        let pane_container_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.pane_container.clone().unchecked_into()
        };

        // pane: pointerenter
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                inner.borrow_mut().on_pointer_enter(HitZone::Chart);
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let (x, y) = event_css_pos(&pe, &pane_c);
                let mut s = inner.borrow_mut();
                s.on_pane_pointer_move(x, y);
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
            }));
            pane_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointerdown
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let pane_top = {
                let borrow = inner.borrow();
                borrow.layout.pane.top.clone()
            };
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let (x, y) = event_css_pos(&pe, &pane_c);
                let mut s = inner.borrow_mut();
                s.on_pointer_down(x, y, HitZone::Chart);
                let _ = pane_top.set_pointer_capture(pe.pointer_id());
            }));
            pane_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointerup
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let pane_top = {
                let borrow = inner.borrow();
                borrow.layout.pane.top.clone()
            };
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let mut s = inner.borrow_mut();
                s.on_pointer_up();
                let _ = pane_top.release_pointer_capture(pe.pointer_id());
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
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
                inner.borrow_mut().on_pane_wheel(x, e.delta_x(), e.delta_y(), e.delta_mode());
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

        // ── PRICE AXIS events ──
        let price_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.price_axis.top.clone().unchecked_into()
        };
        let price_container_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.price_axis_container.clone().unchecked_into()
        };

        // price axis: pointerenter
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                inner.borrow_mut().on_pointer_enter(HitZone::PriceAxis);
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let (_x, y) = event_css_pos(&pe, &price_c);
                let mut s = inner.borrow_mut();
                s.on_price_axis_move(y);
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
            }));
            price_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerdown
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let price_top = {
                let borrow = inner.borrow();
                borrow.layout.price_axis.top.clone()
            };
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let (_x, y) = event_css_pos(&pe, &price_c);
                let mut s = inner.borrow_mut();
                s.on_pointer_down(0.0, y, HitZone::PriceAxis);
                let _ = price_top.set_pointer_capture(pe.pointer_id());
            }));
            price_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerup
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let price_top = {
                let borrow = inner.borrow();
                borrow.layout.price_axis.top.clone()
            };
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let mut s = inner.borrow_mut();
                s.on_pointer_up();
                let _ = price_top.release_pointer_capture(pe.pointer_id());
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
            }));
            price_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: wheel
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(move |e: web_sys::WheelEvent| {
                e.prevent_default();
                inner.borrow_mut().on_price_axis_wheel(e.delta_y(), e.delta_mode());
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

        // ── TIME AXIS events ──
        let time_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.time_axis.top.clone().unchecked_into()
        };
        let time_container_el: web_sys::Element = {
            let borrow = inner.borrow();
            borrow.layout.time_axis_container.clone().unchecked_into()
        };

        // time axis: pointerenter
        {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                inner.borrow_mut().on_pointer_enter(HitZone::TimeAxis);
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
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let (x, _y) = event_css_pos(&pe, &time_c);
                let mut s = inner.borrow_mut();
                s.on_time_axis_move(x);
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
            }));
            time_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerdown
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let time_top = {
                let borrow = inner.borrow();
                borrow.layout.time_axis.top.clone()
            };
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let (x, _y) = event_css_pos(&pe, &time_c);
                let mut s = inner.borrow_mut();
                s.on_pointer_down(x, 0.0, HitZone::TimeAxis);
                let _ = time_top.set_pointer_capture(pe.pointer_id());
            }));
            time_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerup
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let time_top = {
                let borrow = inner.borrow();
                borrow.layout.time_axis.top.clone()
            };
            let cb = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                let pe: web_sys::PointerEvent = e.unchecked_into();
                let mut s = inner.borrow_mut();
                s.on_pointer_up();
                let _ = time_top.release_pointer_capture(pe.pointer_id());
                let cursor = s.cursor_css();
                let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                let _ = html_el.style().set_property("cursor", cursor);
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
                inner.borrow_mut().on_time_axis_wheel(x, e.delta_y(), e.delta_mode());
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

        // ── ResizeObserver on the outer container ──
        let container_el: web_sys::HtmlElement = {
            let borrow = inner.borrow();
            borrow.layout.container().clone()
        };
        let (resize_closure, resize_observer) = {
            let inner = Rc::clone(&inner);
            let cb = Closure::<dyn FnMut(js_sys::Array)>::wrap(Box::new(move |_entries: js_sys::Array| {
                let mut s = inner.borrow_mut();
                let dpr = get_dpr();
                s.engine.dpr = dpr;

                // Resize all widget canvases
                s.layout.resize_all_canvases(dpr);

                // Resize pane engine
                let (pw, ph) = s.layout.pane_css_size();
                let ppw = (pw * dpr).round() as u32;
                let pph = (ph * dpr).round() as u32;
                s.engine.resize(ppw.max(1), pph.max(1), dpr);
                s.grid.resize(ppw.max(1), pph.max(1), dpr);
                s.overlay.resize(ppw.max(1), pph.max(1), dpr);

                // Resize axis renderers
                let (aw, ah) = s.layout.price_axis_css_size();
                let apw = (aw * dpr).round() as u32;
                let aph = (ah * dpr).round() as u32;
                s.price_axis_renderer.resize(apw.max(1), aph.max(1), dpr);

                let (tw, th) = s.layout.time_axis_css_size();
                let tpw = (tw * dpr).round() as u32;
                let tph = (th * dpr).round() as u32;
                s.time_axis_renderer.resize(tpw.max(1), tph.max(1), dpr);
            }));
            let observer = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref())?;
            observer.observe(&container_el.clone().unchecked_into());
            (cb, observer)
        };

        Ok(RayCore {
            inner,
            _closures: closures,
            _wheel_closures: wheel_closures,
            _resize_closure: Some(resize_closure),
            _resize_observer: Some(resize_observer),
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

    // ── Render ───────────────────────────────────────────────────────────────

    /// Render one frame. Call from requestAnimationFrame.
    pub fn render(&mut self) {
        let mut s = self.inner.borrow_mut();

        let dpr = s.engine.dpr;
        let style = s.engine.style.clone();

        // Get pane dimensions (chart area only)
        let (pane_css_w, pane_css_h) = s.layout.pane_css_size();
        let pane_pw = (pane_css_w * dpr).round();
        let pane_ph = (pane_css_h * dpr).round();

        if pane_pw <= 0.0 || pane_ph <= 0.0 { return; }

        // 1. Compute tick marks (single source of truth)
        let y_ticks = tick_marks::compute_y_ticks(&s.engine.viewport, pane_ph, dpr);
        let x_ticks = tick_marks::compute_x_ticks(
            &s.engine.viewport, s.engine.bars.as_slice(), pane_pw, dpr,
        );

        // 2. Measure price axis width from tick labels
        let max_text_w_phys = s.price_axis_renderer.measure_max_tick_width(&style, &y_ticks);
        let max_text_w_css = max_text_w_phys / dpr;
        let price_axis_css_w = style.price_axis_width(max_text_w_css);
        let time_axis_css_h = style.time_axis_height();

        // 3. Update CSS grid layout (this may cause pane to resize)
        s.layout.update_axis_sizes(price_axis_css_w, time_axis_css_h);

        // 4. Grid (pane base canvas) — background + grid lines
        s.grid.render(&style, &y_ticks, &x_ticks);

        // 5. Engine render — candles + volume on pane chart canvas
        if let Err(e) = s.engine.render(&y_ticks, &x_ticks) {
            log::warn!("render error: {}", e);
        }

        // 6. Overlay — crosshair lines + watermark on pane top canvas
        s.overlay.render(&s.engine.crosshair, &style);

        // 7. Price axis — base (ticks + labels) + top (crosshair label)
        s.price_axis_renderer.render_base(&style, &y_ticks, pane_ph);
        s.price_axis_renderer.render_top(
            &s.engine.crosshair, &s.engine.viewport, &style, pane_css_h,
        );

        // 8. Time axis — base (ticks + labels) + top (crosshair label)
        s.time_axis_renderer.render_base(&style, &x_ticks, pane_pw);
        s.time_axis_renderer.render_top(
            &s.engine.crosshair, s.engine.bars.as_slice(),
            &s.engine.viewport, &style, pane_css_w,
        );

        // 9. Corner stub — background + borders (LWC: PriceAxisStub)
        Self::render_corner_stub(&s.layout, &style, dpr);
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
