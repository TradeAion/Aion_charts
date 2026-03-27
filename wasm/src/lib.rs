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
    generate_footprint_sample_data, generate_sample_data, AreaSeriesOptions, Bar, BarSeriesOptions,
    BaselineSeriesOptions, Canvas2DRenderer, ChartEngine, ChartGroup as NativeChartGroup,
    ChartPaneId, ChartStyle, CrosshairMagnetMode, CrosshairSnapshot, DataRange, GpuContext,
    HistogramPoint, HistogramSeriesOptions, HitZone, InteractionHandler, LinePoint,
    LineSeriesOptions, LineStyle, MainChartType, MainViewportPreset, MarkerPosition, MarkerShape,
    MtfMode, MtfRequest, MtfResolvedSample, OhlcPoint, OverlayRenderer, PriceAxisRenderer,
    PriceLineOptions, RendererBackend, ResourceLimits, RuntimeEvent, SeriesId, SeriesMarker,
    SnapshotMtfResolver, TimeAxisRenderer, TimeRange, Viewport, WgpuRenderer,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
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
    event_css_pos, wheel_css_pos, ChartInner, EventListenerRegistry, ExactPixelSizes,
    ReplayEdgeBehavior, SharedInner,
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
}

fn parse_main_viewport_preset(mode: Option<&str>) -> MainViewportPreset {
    match mode.unwrap_or("default") {
        "fit_all" | "fit-all" | "fitAll" => MainViewportPreset::FitAll,
        _ => MainViewportPreset::DefaultRecent,
    }
}

fn emit_visible_range_change(engine: &mut ChartEngine) {
    let start_bar = engine.viewport.start_bar;
    let end_bar = engine.viewport.end_bar;
    engine
        .event_bus
        .emit(raycore::ChartEvent::VisibleRangeChange { start_bar, end_bar });
}

fn reset_main_viewport_and_emit(engine: &mut ChartEngine, mode: Option<&str>) {
    let preset = parse_main_viewport_preset(mode);
    engine.reset_main_viewport(preset);
    emit_visible_range_change(engine);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResizeSignature {
    container_w: u32,
    container_h: u32,
    pane_pw: u32,
    pane_ph: u32,
    price_axis_pw: u32,
    price_axis_ph: u32,
    time_axis_pw: u32,
    time_axis_ph: u32,
    exact_available: bool,
    dpr_milli: u32,
}

fn read_resize_signature(s: &ChartInner, dpr: f64) -> ResizeSignature {
    let (cw, ch) = s.layout.container_css_size();
    let exact_available = has_exact_widget_sizes(s);
    let es = s.exact_sizes;

    let ((pane_pw, pane_ph), (price_axis_pw, price_axis_ph), (time_axis_pw, time_axis_ph)) =
        if exact_available {
            (
                (es.pane_pw, es.pane_ph),
                (es.price_axis_pw, es.price_axis_ph),
                (es.time_axis_pw, es.time_axis_ph),
            )
        } else {
            let (pcw, pch) = s.layout.pane_css_size();
            let (acw, ach) = s.layout.price_axis_css_size();
            let (tcw, tch) = s.layout.time_axis_css_size();
            (
                (
                    ((pcw * dpr).round() as u32).max(1),
                    ((pch * dpr).round() as u32).max(1),
                ),
                (
                    ((acw * dpr).round() as u32).max(1),
                    ((ach * dpr).round() as u32).max(1),
                ),
                (
                    ((tcw * dpr).round() as u32).max(1),
                    ((tch * dpr).round() as u32).max(1),
                ),
            )
        };

    ResizeSignature {
        container_w: cw.max(0.0).round() as u32,
        container_h: ch.max(0.0).round() as u32,
        pane_pw,
        pane_ph,
        price_axis_pw,
        price_axis_ph,
        time_axis_pw,
        time_axis_ph,
        exact_available,
        dpr_milli: (dpr * 1000.0).round().max(0.0) as u32,
    }
}

fn apply_pending_exact_sizes(s: &mut ChartInner, pending: ExactPixelSizes) {
    if !pending.available {
        return;
    }

    if pending.pane_pw > 0 && pending.pane_ph > 0 {
        s.exact_sizes.pane_pw = pending.pane_pw;
        s.exact_sizes.pane_ph = pending.pane_ph;
    }
    if pending.price_axis_pw > 0 && pending.price_axis_ph > 0 {
        s.exact_sizes.price_axis_pw = pending.price_axis_pw;
        s.exact_sizes.price_axis_ph = pending.price_axis_ph;
    }
    if pending.time_axis_pw > 0 && pending.time_axis_ph > 0 {
        s.exact_sizes.time_axis_pw = pending.time_axis_pw;
        s.exact_sizes.time_axis_ph = pending.time_axis_ph;
    }
    s.exact_sizes.available = true;
}

fn extract_device_pixel_content_box_size(
    entry: &web_sys::ResizeObserverEntry,
) -> Option<(u32, u32)> {
    let raw = js_sys::Reflect::get(entry, &JsValue::from_str("devicePixelContentBoxSize")).ok()?;
    if raw.is_undefined() || raw.is_null() {
        return None;
    }
    let arr: &js_sys::Array = raw.unchecked_ref();
    if arr.length() == 0 {
        return None;
    }
    let item = arr.get(0);
    let inline_size = js_sys::Reflect::get(&item, &JsValue::from_str("inlineSize"))
        .ok()
        .and_then(|v| v.as_f64())?;
    let block_size = js_sys::Reflect::get(&item, &JsValue::from_str("blockSize"))
        .ok()
        .and_then(|v| v.as_f64())?;
    if inline_size <= 0.0 || block_size <= 0.0 {
        return None;
    }
    Some((inline_size as u32, block_size as u32))
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

        s.layout.resize_pane_exact(es.pane_pw, es.pane_ph, pcw, pch);
        s.layout
            .resize_price_axis_exact(es.price_axis_pw, es.price_axis_ph, acw, ach);
        s.layout
            .resize_time_axis_exact(es.time_axis_pw, es.time_axis_ph, tcw, tch);

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

pub(crate) struct RenderInvalidation {
    dirty: Cell<bool>,
    inner: SharedInner,
    event_emitter: Rc<RefCell<EventEmitter>>,
    auto_render: Rc<Cell<bool>>,
    raf_closure: Rc<RefCell<Option<Closure<dyn FnMut()>>>>,
    raf_id: Rc<Cell<i32>>,
    replay_forced_auto_render: Rc<Cell<bool>>,
    self_ref: RefCell<Weak<RenderInvalidation>>,
}

impl RenderInvalidation {
    fn new(
        inner: SharedInner,
        event_emitter: Rc<RefCell<EventEmitter>>,
        auto_render: Rc<Cell<bool>>,
        raf_closure: Rc<RefCell<Option<Closure<dyn FnMut()>>>>,
        raf_id: Rc<Cell<i32>>,
        replay_forced_auto_render: Rc<Cell<bool>>,
    ) -> Rc<Self> {
        let this = Rc::new(Self {
            dirty: Cell::new(false),
            inner,
            event_emitter,
            auto_render,
            raf_closure,
            raf_id,
            replay_forced_auto_render,
            self_ref: RefCell::new(Weak::new()),
        });
        *this.self_ref.borrow_mut() = Rc::downgrade(&this);
        this
    }

    pub(crate) fn get(&self) -> bool {
        self.dirty.get()
    }

    pub(crate) fn set(&self, value: bool) {
        self.dirty.set(value);
        if value {
            if let Some(this) = self.self_ref.borrow().upgrade() {
                request_auto_render_frame_if_needed(&this);
            }
        }
    }

    pub(crate) fn event_emitter(&self) -> &Rc<RefCell<EventEmitter>> {
        &self.event_emitter
    }
}

fn ensure_auto_render_closure(dirty: &Rc<RenderInvalidation>) {
    if dirty.raf_closure.borrow().is_some() {
        return;
    }

    let inner = Rc::clone(&dirty.inner);
    let dirty_for_tick = Rc::clone(dirty);

    let tick_closure = Closure::wrap(Box::new(move || {
        dirty_for_tick.raf_id.set(0);

        let keep_animating = render_frame::do_render_frame(&inner, &dirty_for_tick);
        if dirty_for_tick.replay_forced_auto_render.get() {
            let should_restore = inner
                .try_borrow()
                .map(|s| !s.replay_active || !s.replay_playing)
                .unwrap_or(false);
            if should_restore {
                dirty_for_tick.replay_forced_auto_render.set(false);
                if !dirty_for_tick.auto_render.get() {
                    return;
                }
            }
        }

        if keep_animating {
            request_auto_render_frame_if_needed(&dirty_for_tick);
        }
    }) as Box<dyn FnMut()>);

    *dirty.raf_closure.borrow_mut() = Some(tick_closure);
}

fn request_auto_render_frame_if_needed(dirty: &Rc<RenderInvalidation>) {
    if (!dirty.auto_render.get() && !dirty.replay_forced_auto_render.get())
        || dirty.raf_id.get() != 0
    {
        return;
    }

    ensure_auto_render_closure(dirty);

    let Some(window) = web_sys::window() else {
        return;
    };
    let slot = dirty.raf_closure.borrow();
    let Some(callback) = slot.as_ref() else {
        return;
    };
    if let Ok(id) = window.request_animation_frame(callback.as_ref().unchecked_ref()) {
        dirty.raf_id.set(id);
    }
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

fn replay_edge_behavior_key(behavior: ReplayEdgeBehavior) -> &'static str {
    behavior.as_key()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RendererModeRequest {
    Auto,
    WebGpu,
    Canvas2D,
}

impl RendererModeRequest {
    fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "webgpu" => Self::WebGpu,
            "canvas2d" => Self::Canvas2D,
            _ => Self::Auto,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::WebGpu => "webgpu",
            Self::Canvas2D => "canvas2d",
        }
    }
}

struct RendererResolution {
    backend: RendererBackend,
    active: &'static str,
    fallback_reason: Option<String>,
}

async fn resolve_renderer_backend(
    requested: RendererModeRequest,
    canvas: web_sys::HtmlCanvasElement,
    pane_pw: u32,
    pane_ph: u32,
    dpr: f64,
) -> Result<RendererResolution, JsValue> {
    let try_webgpu = matches!(
        requested,
        RendererModeRequest::Auto | RendererModeRequest::WebGpu
    );
    let mut fallback_reason = None;

    if try_webgpu {
        if webgpu_available() {
            match GpuContext::new(
                wgpu::SurfaceTarget::Canvas(canvas.clone()),
                pane_pw.max(1),
                pane_ph.max(1),
            )
            .await
            {
                Ok(gpu) => {
                    return Ok(RendererResolution {
                        backend: RendererBackend::WebGPU(WgpuRenderer::new(gpu)),
                        active: "webgpu",
                        fallback_reason: None,
                    });
                }
                Err(err) => {
                    fallback_reason = Some(format!("webgpu initialization failed: {err}"));
                }
            }
        } else {
            fallback_reason = Some("webgpu is not available in this browser".to_string());
        }
    }

    let canvas_renderer = Canvas2DRenderer::new(canvas, dpr).map_err(|e| JsValue::from_str(&e))?;
    Ok(RendererResolution {
        backend: RendererBackend::Canvas2D(canvas_renderer),
        active: "canvas2d",
        fallback_reason,
    })
}

fn js_err(message: impl Into<String>) -> JsValue {
    JsValue::from_str(&message.into())
}

fn json_value_to_js(value: &JsonValue) -> JsValue {
    match value {
        JsonValue::Null => JsValue::NULL,
        JsonValue::Bool(v) => JsValue::from_bool(*v),
        JsonValue::Number(v) => v.as_f64().map(JsValue::from_f64).unwrap_or(JsValue::NULL),
        JsonValue::String(v) => JsValue::from_str(v),
        JsonValue::Array(items) => {
            let arr = js_sys::Array::new();
            for item in items {
                arr.push(&json_value_to_js(item));
            }
            arr.into()
        }
        JsonValue::Object(map) => {
            let obj = js_sys::Object::new();
            for (k, v) in map {
                let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(k), &json_value_to_js(v));
            }
            obj.into()
        }
    }
}

fn diagnostics_to_js(diagnostics: &[raycore::CompileDiagnostic]) -> JsValue {
    let out = js_sys::Array::new();
    for d in diagnostics {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("code"),
            &JsValue::from_str(&d.code),
        );
        let severity = match d.severity {
            raycore::DiagnosticSeverity::Error => "error",
            raycore::DiagnosticSeverity::Warning => "warning",
            raycore::DiagnosticSeverity::Info => "info",
        };
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("severity"),
            &JsValue::from_str(severity),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("message"),
            &JsValue::from_str(&d.message),
        );
        if let Some(hint) = &d.hint {
            let _ =
                js_sys::Reflect::set(&obj, &JsValue::from_str("hint"), &JsValue::from_str(hint));
        }
        if let Some(span) = &d.span {
            let span_obj = js_sys::Object::new();
            let _ = js_sys::Reflect::set(
                &span_obj,
                &JsValue::from_str("line"),
                &JsValue::from_f64(span.line as f64),
            );
            let _ = js_sys::Reflect::set(
                &span_obj,
                &JsValue::from_str("column"),
                &JsValue::from_f64(span.column as f64),
            );
            let _ = js_sys::Reflect::set(
                &span_obj,
                &JsValue::from_str("len"),
                &JsValue::from_f64(span.len as f64),
            );
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("span"), &span_obj);
        }
        out.push(&obj);
    }
    out.into()
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

fn resolve_synced_crosshair_state(
    viewport: &Viewport,
    time_scale: &raycore::core::renderer::value_projection::TimeScaleIndex,
    pane_width: f64,
    pane_height: f64,
    fallback_x: f64,
    fallback_y: f64,
    bar_index: Option<usize>,
    price: Option<f64>,
) -> (f64, f64, Option<usize>, f64) {
    let resolved_logical_slot = bar_index
        .and_then(|idx| time_scale.logical_index_for_main_bar(idx))
        .map(|slot| slot as usize)
        .or_else(|| {
            if pane_width > 0.0 {
                viewport.bar_index_for_crosshair(fallback_x, pane_width)
            } else {
                None
            }
        });
    let resolved_bar_index = resolved_logical_slot
        .and_then(|slot| time_scale.main_bar_index_at_slot(slot))
        .or(bar_index);

    let x = if let Some(slot) = resolved_logical_slot {
        if pane_width > 0.0 {
            viewport.bar_center_css(slot, pane_width)
        } else {
            fallback_x
        }
    } else {
        fallback_x
    };

    let resolved_price = if let Some(p) = price {
        p
    } else if pane_height > 0.0 {
        let candle_h = pane_height * viewport.candle_height_frac();
        viewport.pixel_to_price(fallback_y, candle_h)
    } else {
        0.0
    };

    let y = if pane_height > 0.0 {
        if price.is_some() {
            viewport.price_to_css_y(resolved_price, pane_height)
        } else {
            fallback_y
        }
    } else {
        fallback_y
    };

    (x, y, resolved_bar_index, resolved_price)
}

/// Walk a nested object path, returning `None` on the first missing segment.
fn js_get_path(obj: &JsValue, path: &[&str]) -> Option<JsValue> {
    let mut cur = obj.clone();
    for key in path {
        cur = js_get(&cur, key)?;
    }
    Some(cur)
}

/// Accept plain objects or JSON strings for options payloads.
fn normalize_options(options: JsValue) -> JsValue {
    if let Some(raw) = options.as_string() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return JsValue::UNDEFINED;
        }
        return js_sys::JSON::parse(trimmed).unwrap_or(JsValue::UNDEFINED);
    }
    options
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn parse_rgb_component(token: &str) -> Option<f32> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(stripped) = t.strip_suffix('%') {
        let v = stripped.trim().parse::<f32>().ok()? / 100.0;
        return Some(clamp01(v));
    }
    let mut v = t.parse::<f32>().ok()?;
    if v > 1.0 {
        v /= 255.0;
    }
    Some(clamp01(v))
}

fn parse_alpha_component(token: &str) -> Option<f32> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(stripped) = t.strip_suffix('%') {
        let v = stripped.trim().parse::<f32>().ok()? / 100.0;
        return Some(clamp01(v));
    }
    let mut v = t.parse::<f32>().ok()?;
    if v > 1.0 {
        v /= 255.0;
    }
    Some(clamp01(v))
}

fn parse_hex_byte(token: &str) -> Option<u8> {
    u8::from_str_radix(token, 16).ok()
}

fn parse_hex_color(token: &str) -> Option<[f32; 4]> {
    let hex = token.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = parse_hex_byte(&hex[0..1].repeat(2))? as f32 / 255.0;
            let g = parse_hex_byte(&hex[1..2].repeat(2))? as f32 / 255.0;
            let b = parse_hex_byte(&hex[2..3].repeat(2))? as f32 / 255.0;
            Some([r, g, b, 1.0])
        }
        4 => {
            let r = parse_hex_byte(&hex[0..1].repeat(2))? as f32 / 255.0;
            let g = parse_hex_byte(&hex[1..2].repeat(2))? as f32 / 255.0;
            let b = parse_hex_byte(&hex[2..3].repeat(2))? as f32 / 255.0;
            let a = parse_hex_byte(&hex[3..4].repeat(2))? as f32 / 255.0;
            Some([r, g, b, a])
        }
        6 => {
            let r = parse_hex_byte(&hex[0..2])? as f32 / 255.0;
            let g = parse_hex_byte(&hex[2..4])? as f32 / 255.0;
            let b = parse_hex_byte(&hex[4..6])? as f32 / 255.0;
            Some([r, g, b, 1.0])
        }
        8 => {
            let r = parse_hex_byte(&hex[0..2])? as f32 / 255.0;
            let g = parse_hex_byte(&hex[2..4])? as f32 / 255.0;
            let b = parse_hex_byte(&hex[4..6])? as f32 / 255.0;
            let a = parse_hex_byte(&hex[6..8])? as f32 / 255.0;
            Some([r, g, b, a])
        }
        _ => None,
    }
}

fn parse_rgb_function(token: &str) -> Option<[f32; 4]> {
    let lowered = token.trim().to_ascii_lowercase();
    if !lowered.starts_with("rgb(") && !lowered.starts_with("rgba(") {
        return None;
    }
    let open = token.find('(')?;
    let close = token.rfind(')')?;
    if close <= open {
        return None;
    }
    let inner = token[open + 1..close].replace('/', ",");
    let parts: Vec<&str> = inner
        .split(',')
        .flat_map(|chunk| chunk.split_whitespace())
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let r = parse_rgb_component(parts[0])?;
    let g = parse_rgb_component(parts[1])?;
    let b = parse_rgb_component(parts[2])?;
    let a = if parts.len() >= 4 {
        parse_alpha_component(parts[3])?
    } else {
        1.0
    };
    Some([r, g, b, a])
}

fn normalize_css_color(token: &str) -> Option<String> {
    let window = web_sys::window()?;
    let document = window.document()?;
    let el = document
        .create_element("span")
        .ok()?
        .dyn_into::<web_sys::HtmlElement>()
        .ok()?;
    let style = el.style();
    let _ = style.set_property("color", "");
    if style.set_property("color", token).is_err() {
        return None;
    }
    let normalized = style.get_property_value("color").ok()?;
    let normalized = normalized.trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn parse_css_color(token: &str) -> Option<[f32; 4]> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("transparent") {
        return Some([0.0, 0.0, 0.0, 0.0]);
    }
    if let Some(color) = parse_hex_color(trimmed) {
        return Some(color);
    }
    if let Some(color) = parse_rgb_function(trimmed) {
        return Some(color);
    }
    if let Some(normalized) = normalize_css_color(trimmed) {
        if let Some(color) = parse_rgb_function(&normalized) {
            return Some(color);
        }
        if let Some(color) = parse_hex_color(&normalized) {
            return Some(color);
        }
    }
    None
}

fn parse_color_js(value: &JsValue) -> Option<[f32; 4]> {
    if value.is_undefined() || value.is_null() {
        return None;
    }

    if js_sys::Array::is_array(value) {
        let arr = js_sys::Array::from(value);
        if arr.length() < 3 {
            return None;
        }
        let mut r = arr.get(0).as_f64()? as f32;
        let mut g = arr.get(1).as_f64()? as f32;
        let mut b = arr.get(2).as_f64()? as f32;
        let mut a = arr.get(3).as_f64().unwrap_or(1.0) as f32;
        if r > 1.0 || g > 1.0 || b > 1.0 {
            r /= 255.0;
            g /= 255.0;
            b /= 255.0;
        }
        if a > 1.0 {
            a /= 255.0;
        }
        return Some([clamp01(r), clamp01(g), clamp01(b), clamp01(a)]);
    }

    if let (Some(r), Some(g), Some(b)) = (
        js_get_f64(value, "r").or_else(|| js_get_f64(value, "red")),
        js_get_f64(value, "g").or_else(|| js_get_f64(value, "green")),
        js_get_f64(value, "b").or_else(|| js_get_f64(value, "blue")),
    ) {
        let mut rr = r as f32;
        let mut gg = g as f32;
        let mut bb = b as f32;
        let mut aa = js_get_f64(value, "a")
            .or_else(|| js_get_f64(value, "alpha"))
            .unwrap_or(1.0) as f32;
        if rr > 1.0 || gg > 1.0 || bb > 1.0 {
            rr /= 255.0;
            gg /= 255.0;
            bb /= 255.0;
        }
        if aa > 1.0 {
            aa /= 255.0;
        }
        return Some([clamp01(rr), clamp01(gg), clamp01(bb), clamp01(aa)]);
    }

    value.as_string().and_then(|s| parse_css_color(&s))
}

fn parse_line_style_js(value: &JsValue) -> Option<LineStyle> {
    if let Some(style) = value.as_string() {
        return Some(LineStyle::from_str(style.trim()));
    }
    if let Some(raw) = value.as_f64() {
        let key = raw.round() as i32;
        return Some(match key {
            1 => LineStyle::Dotted,
            2 => LineStyle::Dashed,
            3 => LineStyle::LargeDashed,
            4 => LineStyle::SparseDotted,
            _ => LineStyle::Solid,
        });
    }
    None
}

fn line_style_key(style: LineStyle) -> &'static str {
    match style {
        LineStyle::Solid => "solid",
        LineStyle::Dotted => "dotted",
        LineStyle::Dashed => "dashed",
        LineStyle::LargeDashed => "large_dashed",
        LineStyle::SparseDotted => "sparse_dotted",
    }
}

fn parse_crosshair_mode_js(value: &JsValue) -> Option<raycore::CrosshairMode> {
    if let Some(mode) = value.as_string() {
        return Some(parse_crosshair_mode(mode.trim()));
    }
    if let Some(raw) = value.as_f64() {
        let key = raw.round() as i32;
        return Some(match key {
            1 => raycore::CrosshairMode::Magnet,
            2 => raycore::CrosshairMode::MagnetOHLC,
            _ => raycore::CrosshairMode::Normal,
        });
    }
    None
}

fn parse_price_scale_mode_js(value: &JsValue) -> Option<raycore::PriceScaleMode> {
    if let Some(mode) = value.as_string() {
        return Some(raycore::PriceScaleMode::from_str(mode.trim()));
    }
    if let Some(raw) = value.as_f64() {
        let key = raw.round() as i32;
        return Some(match key {
            1 => raycore::PriceScaleMode::Logarithmic,
            2 => raycore::PriceScaleMode::Percentage,
            3 => raycore::PriceScaleMode::IndexedTo100,
            _ => raycore::PriceScaleMode::Normal,
        });
    }
    None
}

fn price_scale_mode_key(mode: raycore::PriceScaleMode) -> &'static str {
    match mode {
        raycore::PriceScaleMode::Normal => "normal",
        raycore::PriceScaleMode::Logarithmic => "logarithmic",
        raycore::PriceScaleMode::Percentage => "percentage",
        raycore::PriceScaleMode::IndexedTo100 => "indexed_to_100",
    }
}

#[derive(Clone, Copy, Default)]
struct CrosshairLinePatch {
    color: Option<[f32; 4]>,
    width: Option<f64>,
    style: Option<LineStyle>,
    visible: Option<bool>,
    label_visible: Option<bool>,
    label_bg_color: Option<[f32; 4]>,
}

fn parse_crosshair_line_patch(obj: Option<JsValue>) -> CrosshairLinePatch {
    let Some(line_obj) = obj else {
        return CrosshairLinePatch::default();
    };

    let color = js_get(&line_obj, "color").and_then(|v| parse_color_js(&v));
    let width = js_get_f64(&line_obj, "width")
        .filter(|v| v.is_finite())
        .map(|v| v.max(1.0));
    let style = js_get(&line_obj, "style").and_then(|v| parse_line_style_js(&v));
    let visible = js_get_bool(&line_obj, "visible");
    let label_visible = js_get_bool(&line_obj, "labelVisible");
    let label_bg_color = js_get(&line_obj, "labelBackgroundColor").and_then(|v| parse_color_js(&v));

    CrosshairLinePatch {
        color,
        width,
        style,
        visible,
        label_visible,
        label_bg_color,
    }
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

fn ensure_strictly_increasing_timestamps(ctx: &str, timestamps: &[u64]) -> Result<(), JsValue> {
    for i in 1..timestamps.len() {
        if timestamps[i] <= timestamps[i - 1] {
            return Err(js_err(format!(
                "{}: timestamps must be strictly increasing (index {}: {} <= {})",
                ctx,
                i,
                timestamps[i],
                timestamps[i - 1]
            )));
        }
    }
    Ok(())
}

fn build_main_bars_from_arrays(
    ctx: &str,
    open: &[f32],
    high: &[f32],
    low: &[f32],
    close: &[f32],
    volume: &[f32],
    timestamps: &[u64],
) -> Result<Vec<Bar>, JsValue> {
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
    ensure_strictly_increasing_timestamps(ctx, timestamps)?;

    Ok((0..count)
        .map(|i| Bar {
            timestamp: timestamps[i],
            open: open[i],
            high: high[i],
            low: low[i],
            close: close[i],
            volume: volume[i],
            _pad: 0.0,
        })
        .collect())
}

fn ensure_ohlcv_sanity_for_footprint(ctx: &str, bars: &[Bar]) -> Result<(), JsValue> {
    for (idx, bar) in bars.iter().enumerate() {
        if bar.high < bar.low {
            return Err(js_err(format!(
                "{}: bar {} timestamp {} has high < low ({} < {})",
                ctx, idx, bar.timestamp, bar.high, bar.low
            )));
        }
        if bar.open < bar.low || bar.open > bar.high {
            return Err(js_err(format!(
                "{}: bar {} timestamp {} has open outside [low, high]",
                ctx, idx, bar.timestamp
            )));
        }
        if bar.close < bar.low || bar.close > bar.high {
            return Err(js_err(format!(
                "{}: bar {} timestamp {} has close outside [low, high]",
                ctx, idx, bar.timestamp
            )));
        }
    }
    Ok(())
}

fn build_footprint_levels(
    ctx: &str,
    prices: &[f32],
    bid_volumes: &[f32],
    ask_volumes: &[f32],
) -> Result<Vec<raycore::FootprintLevel>, JsValue> {
    let len = prices.len();
    ensure_equal_len("prices", len, "bid_volumes", bid_volumes.len())?;
    ensure_equal_len("prices", len, "ask_volumes", ask_volumes.len())?;
    ensure_finite_slice("prices", prices)?;
    ensure_finite_slice("bid_volumes", bid_volumes)?;
    ensure_finite_slice("ask_volumes", ask_volumes)?;

    for i in 1..len {
        if prices[i] < prices[i - 1] {
            return Err(js_err(format!(
                "{}: prices must be sorted ascending (index {}: {} < {})",
                ctx,
                i,
                prices[i],
                prices[i - 1]
            )));
        }
    }
    if let Some((idx, v)) = bid_volumes.iter().enumerate().find(|(_, v)| **v < 0.0) {
        return Err(js_err(format!(
            "{}: bid_volumes must be >= 0 (index {}: {})",
            ctx, idx, v
        )));
    }
    if let Some((idx, v)) = ask_volumes.iter().enumerate().find(|(_, v)| **v < 0.0) {
        return Err(js_err(format!(
            "{}: ask_volumes must be >= 0 (index {}: {})",
            ctx, idx, v
        )));
    }

    Ok((0..len)
        .map(|i| raycore::FootprintLevel {
            price: prices[i],
            bid_volume: bid_volumes[i],
            ask_volume: ask_volumes[i],
        })
        .collect())
}

fn validate_footprint_bar_alignment(
    ctx: &str,
    bar_index: usize,
    bar: &Bar,
    levels: &[raycore::FootprintLevel],
) -> Result<(), JsValue> {
    if levels.is_empty() {
        return Ok(());
    }
    let tick = if levels.len() > 1 {
        let bar = raycore::FootprintBar {
            levels: levels.to_vec(),
        };
        bar.inferred_tick_size()
    } else {
        (bar.high - bar.low).abs().max(0.0001)
    };
    let lowest = levels.first().map(|l| l.price).unwrap_or(bar.low);
    let highest = levels.last().map(|l| l.price + tick).unwrap_or(bar.high);
    let slack = tick.max((bar.high - bar.low).abs() * 0.05).max(0.0001);

    if lowest > bar.low + slack || highest < bar.high - slack {
        return Err(js_err(format!(
            "{}: bar {} timestamp {} footprint range [{:.6}, {:.6}] does not cover OHLC range [{:.6}, {:.6}]",
            ctx,
            bar_index,
            bar.timestamp,
            lowest,
            highest,
            bar.low,
            bar.high
        )));
    }
    Ok(())
}

fn build_footprint_data_from_aligned_arrays(
    ctx: &str,
    bars: &[Bar],
    level_offsets: &[u32],
    prices: &[f32],
    bid_volumes: &[f32],
    ask_volumes: &[f32],
) -> Result<raycore::FootprintData, JsValue> {
    ensure_equal_len("prices", prices.len(), "bid_volumes", bid_volumes.len())?;
    ensure_equal_len("prices", prices.len(), "ask_volumes", ask_volumes.len())?;
    ensure_finite_slice("prices", prices)?;
    ensure_finite_slice("bid_volumes", bid_volumes)?;
    ensure_finite_slice("ask_volumes", ask_volumes)?;

    if level_offsets.len() != bars.len() + 1 {
        return Err(js_err(format!(
            "{}: level_offsets length must be bars.len()+1 ({} != {}+1)",
            ctx,
            level_offsets.len(),
            bars.len()
        )));
    }
    if level_offsets.first().copied().unwrap_or(0) != 0 {
        return Err(js_err(format!("{}: level_offsets[0] must be 0", ctx)));
    }
    if let Some((i, _)) = level_offsets
        .windows(2)
        .enumerate()
        .find(|(_, w)| w[1] < w[0])
    {
        return Err(js_err(format!(
            "{}: level_offsets must be non-decreasing (index {})",
            ctx,
            i + 1
        )));
    }
    let total_levels = prices.len();
    let last_offset = level_offsets.last().copied().unwrap_or(0) as usize;
    if last_offset != total_levels {
        return Err(js_err(format!(
            "{}: last level offset {} must equal level array length {}",
            ctx, last_offset, total_levels
        )));
    }

    let mut footprint = raycore::FootprintData::new();
    for i in 0..bars.len() {
        let start = level_offsets[i] as usize;
        let end = level_offsets[i + 1] as usize;
        if start == end {
            continue;
        }
        let bar_ctx = format!("{}: bar {}", ctx, i);
        let levels = build_footprint_levels(
            &bar_ctx,
            &prices[start..end],
            &bid_volumes[start..end],
            &ask_volumes[start..end],
        )?;
        validate_footprint_bar_alignment(ctx, i, &bars[i], &levels)?;
        footprint.set_bar(i, raycore::FootprintBar { levels });
    }
    Ok(footprint)
}

fn build_historical_footprint_dataset_from_arrays(
    ctx: &str,
    open: &[f32],
    high: &[f32],
    low: &[f32],
    close: &[f32],
    volume: &[f32],
    timestamps: &[u64],
    level_offsets: &[u32],
    prices: &[f32],
    bid_volumes: &[f32],
    ask_volumes: &[f32],
) -> Result<(Vec<Bar>, raycore::FootprintData), JsValue> {
    let bars = build_main_bars_from_arrays(ctx, open, high, low, close, volume, timestamps)?;
    ensure_ohlcv_sanity_for_footprint(ctx, &bars)?;
    let footprint = build_footprint_data_from_aligned_arrays(
        ctx,
        &bars,
        level_offsets,
        prices,
        bid_volumes,
        ask_volumes,
    )?;
    Ok((bars, footprint))
}

fn parse_color_json_value(value: &serde_json::Value) -> Option<[f32; 4]> {
    match value {
        serde_json::Value::String(s) => parse_css_color(s),
        serde_json::Value::Array(items) if items.len() == 4 => {
            let mut out = [0.0; 4];
            for (idx, item) in items.iter().enumerate() {
                let mut v = item.as_f64()? as f32;
                if idx < 3 && v > 1.0 {
                    v /= 255.0;
                } else if idx == 3 && v > 1.0 {
                    v /= 255.0;
                }
                out[idx] = clamp01(v);
            }
            Some(out)
        }
        _ => None,
    }
}

fn parse_historical_footprint_json_dataset(
    json: &str,
) -> Result<(Vec<Bar>, raycore::FootprintData), JsValue> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|e| js_err(format!("JSON parse error: {}", e)))?;

    let items = match parsed {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(map) => map
            .get("bars")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| {
                js_err("set_data_with_footprint_json: expected array or object with bars array")
            })?,
        _ => {
            return Err(js_err(
                "set_data_with_footprint_json: expected array or object with bars array",
            ))
        }
    };

    let mut bars = Vec::with_capacity(items.len());
    let mut footprint = raycore::FootprintData::new();

    for (bar_index, item) in items.iter().enumerate() {
        let timestamp = item
            .get("timestamp")
            .or_else(|| item.get("time"))
            .or_else(|| item.get("ts"))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                js_err(format!(
                    "set_data_with_footprint_json: bar {} missing timestamp",
                    bar_index
                ))
            })?;
        let open = item.get("open").and_then(|v| v.as_f64()).ok_or_else(|| {
            js_err(format!(
                "set_data_with_footprint_json: bar {} missing open",
                bar_index
            ))
        })? as f32;
        let high = item.get("high").and_then(|v| v.as_f64()).ok_or_else(|| {
            js_err(format!(
                "set_data_with_footprint_json: bar {} missing high",
                bar_index
            ))
        })? as f32;
        let low = item.get("low").and_then(|v| v.as_f64()).ok_or_else(|| {
            js_err(format!(
                "set_data_with_footprint_json: bar {} missing low",
                bar_index
            ))
        })? as f32;
        let close = item.get("close").and_then(|v| v.as_f64()).ok_or_else(|| {
            js_err(format!(
                "set_data_with_footprint_json: bar {} missing close",
                bar_index
            ))
        })? as f32;
        let volume = item.get("volume").and_then(|v| v.as_f64()).ok_or_else(|| {
            js_err(format!(
                "set_data_with_footprint_json: bar {} missing volume",
                bar_index
            ))
        })? as f32;

        ensure_finite_fields(
            "set_data_with_footprint_json",
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
        bars.push(bar);

        let levels_arr = item
            .get("levels")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if levels_arr.is_empty() {
            continue;
        }

        let mut prices = Vec::with_capacity(levels_arr.len());
        let mut bids = Vec::with_capacity(levels_arr.len());
        let mut asks = Vec::with_capacity(levels_arr.len());
        for (level_index, level) in levels_arr.iter().enumerate() {
            let price = level.get("price").and_then(|v| v.as_f64()).ok_or_else(|| {
                js_err(format!(
                    "set_data_with_footprint_json: bar {} level {} missing price",
                    bar_index, level_index
                ))
            })? as f32;
            let bid = level
                .get("bid")
                .or_else(|| level.get("bid_volume"))
                .or_else(|| level.get("bidVolume"))
                .and_then(|v| v.as_f64())
                .ok_or_else(|| {
                    js_err(format!(
                        "set_data_with_footprint_json: bar {} level {} missing bid volume",
                        bar_index, level_index
                    ))
                })? as f32;
            let ask = level
                .get("ask")
                .or_else(|| level.get("ask_volume"))
                .or_else(|| level.get("askVolume"))
                .and_then(|v| v.as_f64())
                .ok_or_else(|| {
                    js_err(format!(
                        "set_data_with_footprint_json: bar {} level {} missing ask volume",
                        bar_index, level_index
                    ))
                })? as f32;
            prices.push(price);
            bids.push(bid);
            asks.push(ask);
        }

        let level_ctx = format!("set_data_with_footprint_json: bar {}", bar_index);
        let levels = build_footprint_levels(&level_ctx, &prices, &bids, &asks)?;
        validate_footprint_bar_alignment(
            "set_data_with_footprint_json",
            bar_index,
            bars.last().expect("just pushed bar"),
            &levels,
        )?;
        footprint.set_bar(bar_index, raycore::FootprintBar { levels });
    }

    let timestamps: Vec<u64> = bars.iter().map(|b| b.timestamp).collect();
    ensure_strictly_increasing_timestamps("set_data_with_footprint_json", &timestamps)?;
    ensure_ohlcv_sanity_for_footprint("set_data_with_footprint_json", &bars)?;
    Ok((bars, footprint))
}

#[inline]
fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[inline]
fn finite_or_f32(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn normalize_price_range(min: f64, max: f64, fallback_min: f64, fallback_max: f64) -> (f64, f64) {
    let mut lo = finite_or(min, fallback_min);
    let mut hi = finite_or(max, fallback_max);
    if hi < lo {
        std::mem::swap(&mut lo, &mut hi);
    }
    (lo, hi)
}

fn validate_drawing_snapshot(snapshot: &raycore::DrawingSnapshot) -> Result<(), String> {
    let mut manager = raycore::DrawingManager::new();
    manager.replace_from_snapshot(snapshot.clone())
}

const DRAWING_STORE_VERSION: u32 = 1;
const CHART_PERSISTENCE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaneDrawingStore {
    #[serde(rename = "paneId")]
    pane_id: u32,
    drawings: raycore::DrawingSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DrawingStore {
    version: u32,
    main: raycore::DrawingSnapshot,
    #[serde(default)]
    subpanes: Vec<PaneDrawingStore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSubPane {
    pub id: u32,
    #[serde(rename = "studyId")]
    pub study_id: u32,
    #[serde(rename = "indicatorType")]
    pub indicator_type: String,
    #[serde(rename = "heightCss")]
    pub height_css: f64,
    #[serde(rename = "autoScale")]
    pub auto_scale: bool,
    #[serde(rename = "priceMin")]
    pub price_min: f64,
    #[serde(rename = "priceMax")]
    pub price_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedViewport {
    #[serde(rename = "startBar")]
    pub start_bar: f64,
    #[serde(rename = "endBar")]
    pub end_bar: f64,
    #[serde(rename = "priceMin")]
    pub price_min: f64,
    #[serde(rename = "priceMax")]
    pub price_max: f64,
    #[serde(rename = "priceLocked")]
    pub price_locked: bool,
    #[serde(rename = "priceScaleMode")]
    pub price_scale_mode: String,
    #[serde(rename = "scaleMarginTop")]
    pub scale_margin_top: f64,
    #[serde(rename = "scaleMarginBottom")]
    pub scale_margin_bottom: f64,
    #[serde(rename = "autoScroll")]
    pub auto_scroll: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChartPersistenceState {
    pub version: u32,
    #[serde(rename = "layoutId", default)]
    pub layout_id: String,
    pub options: JsonValue,
    pub viewport: PersistedViewport,
    #[serde(default)]
    pub panes: Vec<PersistedSubPane>,
    pub drawings: DrawingStore,
}

#[wasm_bindgen]
pub struct RayCore {
    inner: SharedInner,
    mtf_resolver: Arc<SnapshotMtfResolver>,
    active_renderer_name: String,
    symbol: String,
    interval: String,
    _closures: Vec<Closure<dyn FnMut(web_sys::Event)>>,
    _wheel_closures: Vec<Closure<dyn FnMut(web_sys::WheelEvent)>>,
    _touch_closures: Vec<Closure<dyn FnMut(web_sys::TouchEvent)>>,
    _resize_closure: Option<Closure<dyn Fn(js_sys::Array)>>,
    _resize_observer: Option<web_sys::ResizeObserver>,
    /// One-shot RAF callback used to coalesce ResizeObserver bursts.
    _resize_raf_closure: Option<Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>>,
    /// Pending RAF id for resize coalescing.
    _resize_raf_id: Rc<Cell<i32>>,
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
    auto_render: Rc<Cell<bool>>,
    /// RAF closure slot for auto-render mode. One frame is queued at a time.
    _raf_closure: Rc<RefCell<Option<Closure<dyn FnMut()>>>>,
    /// Current RAF ID for cancellation.
    _raf_id: Rc<Cell<i32>>,
    /// True when replay playback temporarily forced auto-render in manual mode.
    replay_forced_auto_render: Rc<Cell<bool>>,
    /// Dirty flag — set on any mutation, cleared after render.
    dirty: Rc<RenderInvalidation>,
}

#[wasm_bindgen]
impl RayCore {
    /// Create with a specific renderer backend (`auto`, `webgpu`, `canvas2d`).
    pub async fn create_with(container_id: &str, renderer: &str) -> Result<RayCore, JsValue> {
        init_logging();

        let layout = WidgetLayout::new(container_id)?;
        let dpr = get_dpr();
        let requested_renderer = RendererModeRequest::from_str(renderer);
        let requested_renderer_str = requested_renderer.as_str().to_string();

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
            requested_renderer.as_str(),
            pane_css_w,
            pane_css_h,
            pane_pw,
            pane_ph,
            dpr
        );

        // Create pane renderer backend (pane/chart canvas only).
        let resolution = resolve_renderer_backend(
            requested_renderer,
            layout.pane.chart.clone(),
            pane_pw.max(1),
            pane_ph.max(1),
            dpr,
        )
        .await?;
        let active_renderer_name = resolution.active.to_string();
        if let Some(reason) = &resolution.fallback_reason {
            log::warn!(
                "RayCore: renderer '{}' fell back to '{}': {}",
                requested_renderer.as_str(),
                active_renderer_name,
                reason
            );
        }
        let backend = resolution.backend;

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
        let mut engine = ChartEngine::new(backend, pane_pw.max(1), pane_ph.max(1), dpr);
        if let Some(reason) = resolution.fallback_reason.clone() {
            engine
                .event_bus
                .emit(raycore::ChartEvent::RendererFallback {
                    requested: requested_renderer.as_str().to_string(),
                    active: active_renderer_name.clone(),
                    reason,
                });
        }
        let mtf_resolver = Arc::new(SnapshotMtfResolver::default());
        engine.indicators.set_mtf_resolver(mtf_resolver.clone());
        let interaction = InteractionHandler::new();

        log::info!("RayCore initialized: {}", engine.renderer_name());

        // Initialize pane height coordinator with main pane height
        let pane_coordinator = PaneHeightCoordinator::new(pane_css_h);

        let inner = Rc::new(RefCell::new(ChartInner {
            requested_renderer_mode: requested_renderer_str.clone(),
            active_renderer_name: active_renderer_name.clone(),
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
            replay_active: false,
            replay_trim_edit_mode: false,
            replay_playing: false,
            replay_cutoff_index: None,
            replay_archive: Vec::new(),
            replay_speed_bps: 1.0,
            replay_edge_behavior: ReplayEdgeBehavior::AutoPause,
            replay_last_tick_ms: 0.0,
            replay_tick_accum_bars: 0.0,
            symbol: "DEMO".to_string(),
            execution_mark_hit_areas: Vec::new(),
            hovered_execution_mark_id: None,
            selected_execution_mark_id: None,
            price_line_drag_id: None,
        }));

        let event_emitter = Rc::new(RefCell::new(EventEmitter::new()));
        let auto_render = Rc::new(Cell::new(false));
        let raf_closure = Rc::new(RefCell::new(None));
        let raf_id = Rc::new(Cell::new(0));
        let replay_forced_auto_render = Rc::new(Cell::new(false));
        let dirty = RenderInvalidation::new(
            Rc::clone(&inner),
            Rc::clone(&event_emitter),
            Rc::clone(&auto_render),
            Rc::clone(&raf_closure),
            Rc::clone(&raf_id),
            Rc::clone(&replay_forced_auto_render),
        );

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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, y) = event_css_pos(&pe, &pane_c);
                    let shift_pressed = pe.shift_key();
                    let ctrl_pressed = pe.ctrl_key() || pe.meta_key(); // meta for Mac Cmd
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.interaction.set_touch(pe.pointer_type() == "touch");
                    s.on_pointer_enter(HitZone::Chart);
                    // Initialize crosshair position on first enter so it doesn't
                    // flash at default (0,0) before the first pointermove.
                    s.on_pane_pointer_move(x, y, shift_pressed, ctrl_pressed);
                    // Clear subpane focus — cursor is now in the main pane
                    s.active_subpane_id = None;
                    let cursor = s.cursor_css();
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);
                    dirty.set(true);
                }));
            pane_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointerleave
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_leave(HitZone::Chart);
                    // Clear the override to let CSS default take over (crosshair)
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", "");
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, y) = event_css_pos(&pe, &pane_c);
                    let shift_pressed = pe.shift_key();
                    let ctrl_pressed = pe.ctrl_key() || pe.meta_key(); // meta for Mac Cmd
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };

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
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();

                    // Ignore right-click (button 2) — handled by contextmenu
                    if pe.button() == 2 {
                        return;
                    }

                    let (x, y) = event_css_pos(&pe, &pane_c);
                    let shift_pressed = pe.shift_key();
                    let ctrl_pressed = pe.ctrl_key() || pe.meta_key(); // meta for Mac Cmd
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };

                    // Detect touch input
                    let is_touch = pe.pointer_type() == "touch";
                    s.interaction.is_touch = is_touch;

                    // Cancel any existing long-press timer from a previous gesture.
                    if let Some(tid) = lp_timer.borrow_mut().take() {
                        let _ = web_sys::window().unwrap().clear_timeout_with_handle(tid);
                    }
                    *lp_cb.borrow_mut() = None;

                    s.on_pointer_enter(HitZone::Chart);

                    // Replay trim-edit mode: chart-pane click sets replay cutoff only.
                    if s.replay_active && s.replay_trim_edit_mode {
                        if let Some(cutoff_idx) = s.replay_cutoff_from_pane_x(x) {
                            if let Err(err) = s.replay_set_cutoff_bar(cutoff_idx) {
                                log::warn!("replay trim click failed: {}", err);
                            } else {
                                // Trim action exits replay edit mode and pauses playback.
                                if s.replay_playing {
                                    s.replay_set_playing(false);
                                }
                                s.interaction.pressed = false;
                                s.interaction.drag_active = false;
                                s.replay_set_trim_edit_mode(false);
                            }
                        }
                        let cursor = s.cursor_css();
                        let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                        let _ = html_el.style().set_property("cursor", cursor);
                        dirty.set(true);
                        return;
                    }

                    s.on_pointer_down(x, y, HitZone::Chart, shift_pressed, ctrl_pressed);
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.set_pointer_capture(pe.pointer_id());
                    dirty.set(true);

                    if is_touch {
                        // Double-tap detection (500ms)
                        let now = js_sys::Date::now();
                        let last = *last_tap.borrow();
                        if now - last < 500.0 {
                            if s.interaction
                                .is_double_click_candidate(HitZone::Chart, now, 30.0)
                            {
                                *last_tap.borrow_mut() = 0.0;
                                s.on_touch_double_tap();
                                dirty.set(true);
                                return;
                            }
                        }
                        *last_tap.borrow_mut() = now;

                        // Start long-press timer (240ms like LWC)
                        let inner_lp = Rc::clone(&inner);
                        let lp_timer_inner = Rc::clone(&lp_timer);
                        let dirty_lp = Rc::clone(&dirty);
                        let lp_x = x;
                        let lp_y = y;

                        drop(s); // release borrow before setTimeout callback

                        let timeout_cb = Closure::<dyn FnMut()>::wrap(Box::new(move || {
                            if let Ok(mut s) = inner_lp.try_borrow_mut() {
                                if s.interaction.pressed
                                    && !s.interaction.drag_active
                                    && !s.interaction.pinch_active
                                {
                                    s.on_long_press(lp_x, lp_y);
                                    dirty_lp.set(true);
                                }
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, y) = event_css_pos(&pe, &pane_c);
                    let shift_pressed = pe.shift_key();
                    let ctrl_pressed = pe.ctrl_key() || pe.meta_key();

                    // Cancel long-press timer
                    if let Some(tid) = lp_timer.borrow_mut().take() {
                        let _ = web_sys::window().unwrap().clear_timeout_with_handle(tid);
                    }
                    *lp_cb.borrow_mut() = None;

                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_up();
                    s.on_pane_pointer_move(x, y, shift_pressed, ctrl_pressed);
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);

                    // Clear grid wrapper override
                    let grid_el: &web_sys::HtmlElement = grid_up.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                    dirty.set(true);
                }));
            pane_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: wheel
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let dirty = Rc::clone(&dirty);
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let (x, y) = wheel_css_pos(&e, &pane_c);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_enter(HitZone::Chart);
                    s.on_pane_wheel(x, y, e.delta_x(), e.delta_y(), e.delta_mode());
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    e.prevent_default();
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.engine.drawings.remove_all_scale();
                    // Also exit scale drawing mode if active
                    if s.engine.drawings.active_tool == raycore::DrawingTool::Scale {
                        s.engine.drawings.cancel_creation();
                        s.engine.drawings.active_tool = raycore::DrawingTool::None;
                    }
                    dirty.set(true);
                }));
            pane_el.add_event_listener_with_callback("contextmenu", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // pane: pointercancel
        {
            let inner = Rc::clone(&inner);
            let pane_c = pane_container_el.clone();
            let grid_can = grid_c.clone();
            let lp_timer = Rc::clone(&long_press_timer);
            let lp_cb = Rc::clone(&long_press_cb_handle);
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, y) = event_css_pos(&pe, &pane_c);
                    let shift_pressed = pe.shift_key();
                    let ctrl_pressed = pe.ctrl_key() || pe.meta_key();

                    if let Some(tid) = lp_timer.borrow_mut().take() {
                        let _ = web_sys::window().unwrap().clear_timeout_with_handle(tid);
                    }
                    *lp_cb.borrow_mut() = None;

                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_cancel();
                    s.on_pane_pointer_move(x, y, shift_pressed, ctrl_pressed);
                    let html_el: &web_sys::HtmlElement = pane_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);
                    let grid_el: &web_sys::HtmlElement = grid_can.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
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
                        let Ok(mut s) = inner.try_borrow_mut() else {
                            return;
                        };
                        s.on_pinch_start(cx, cy, distance);
                        dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
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
                        let Ok(mut s) = inner.try_borrow_mut() else {
                            return;
                        };
                        s.on_pinch_update(scale);
                        dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(
                move |e: web_sys::TouchEvent| {
                    let touches = e.touches();
                    if touches.length() < 2 {
                        let Ok(mut s) = inner.try_borrow_mut() else {
                            return;
                        };
                        if s.interaction.pinch_active {
                            s.on_pinch_end();
                            dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_enter(HitZone::PriceAxis);
                    let cursor = s.cursor_css();
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);
                    dirty.set(true);
                }));
            price_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerleave
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_leave(HitZone::PriceAxis);
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", "");
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (_x, y) = event_css_pos(&pe, &price_c);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
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
                    dirty.set(true);
                }));
            price_el
                .add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: pointerdown
        {
            let inner = Rc::clone(&inner);
            let price_c = price_container_el.clone();
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (_x, y) = event_css_pos(&pe, &price_c);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.interaction.is_touch = pe.pointer_type() == "touch";
                    s.on_pointer_enter(HitZone::PriceAxis);
                    s.on_pointer_down(0.0, y, HitZone::PriceAxis, false, false);
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.set_pointer_capture(pe.pointer_id());
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);

                    // Clear grid wrapper override
                    let grid_el: &web_sys::HtmlElement = grid_up_p.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                    dirty.set(true);
                }));
            price_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // price axis: wheel
        {
            let inner = Rc::clone(&inner);
            let dirty = Rc::clone(&dirty);
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_enter(HitZone::PriceAxis);
                    s.on_price_axis_wheel(e.delta_y(), e.delta_mode());
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = price_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);
                    let grid_el: &web_sys::HtmlElement = grid_can_p.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_enter(HitZone::TimeAxis);
                    let cursor = s.cursor_css();
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", cursor);
                    dirty.set(true);
                }));
            time_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerleave
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_leave(HitZone::TimeAxis);
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.style().set_property("cursor", "");
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, _y) = event_css_pos(&pe, &time_c);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
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
                    dirty.set(true);
                }));
            time_el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerdown
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let (x, _y) = event_css_pos(&pe, &time_c);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.interaction.is_touch = pe.pointer_type() == "touch";
                    s.on_pointer_enter(HitZone::TimeAxis);
                    s.on_pointer_down(x, 0.0, HitZone::TimeAxis, false, false);
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.set_pointer_capture(pe.pointer_id());
                    dirty.set(true);
                }));
            time_el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: pointerup
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let grid_up_t = grid_c.clone();
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);

                    // Clear grid wrapper override
                    let grid_el: &web_sys::HtmlElement = grid_up_t.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                    dirty.set(true);
                }));
            time_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }
        // time axis: wheel
        {
            let inner = Rc::clone(&inner);
            let time_c = time_container_el.clone();
            let dirty = Rc::clone(&dirty);
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let (x, _y) = wheel_css_pos(&e, &time_c);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_enter(HitZone::TimeAxis);
                    s.on_time_axis_wheel(x, e.delta_y(), e.delta_mode());
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pointer_up();
                    let html_el: &web_sys::HtmlElement = time_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = s.cursor_css();
                    let _ = html_el.style().set_property("cursor", cursor);
                    let grid_el: &web_sys::HtmlElement = grid_can_t.unchecked_ref();
                    let _ = grid_el.style().set_property("cursor", "");
                    dirty.set(true);
                }));
            time_el
                .add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref())?;
            closures.push(cb);
        }

        // ── ResizeObserver on widget containers (device-pixel-content-box) ──
        //
        // LWC (fancy-canvas) uses ResizeObserver with `device-pixel-content-box`
        // to get the exact integer device-pixel size of each canvas element.
        // This eliminates the ±1px rounding error from `round(css * dpr)`
        // that causes blur at non-integer zoom levels.
        //
        // We observe pane + price-axis + time-axis containers and store exact sizes.
        // On each callback we also resize canvases and renderers.

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

        let resize_pending_exact = Rc::new(RefCell::new(ExactPixelSizes::default()));
        let resize_last_applied = Rc::new(RefCell::new(None::<ResizeSignature>));
        let resize_raf_scheduled = Rc::new(Cell::new(false));
        let resize_raf_id = Rc::new(Cell::new(0));
        let resize_raf_closure_slot: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> =
            Rc::new(RefCell::new(None));

        let (resize_closure, resize_observer) = {
            let inner = Rc::clone(&inner);
            let pane_ref = pane_container_for_ro.clone();
            let price_ref = price_container_for_ro.clone();
            let time_ref = time_container_for_ro.clone();
            let pending_exact = Rc::clone(&resize_pending_exact);
            let last_applied = Rc::clone(&resize_last_applied);
            let raf_scheduled = Rc::clone(&resize_raf_scheduled);
            let raf_id = Rc::clone(&resize_raf_id);
            let raf_slot = Rc::clone(&resize_raf_closure_slot);
            let dirty = Rc::clone(&dirty);

            let cb =
                Closure::<dyn Fn(js_sys::Array)>::wrap(Box::new(move |entries: js_sys::Array| {
                    // Collect exact pixel sizes from all observed widget containers.
                    // Processing is deferred to a RAF callback so bursts are coalesced.
                    let mut pending = pending_exact.borrow_mut();
                    for i in 0..entries.length() {
                        let entry: web_sys::ResizeObserverEntry = entries.get(i).unchecked_into();
                        let target = entry.target();
                        if let Some((exact_w, exact_h)) =
                            extract_device_pixel_content_box_size(&entry)
                        {
                            if target == pane_ref {
                                pending.pane_pw = exact_w;
                                pending.pane_ph = exact_h;
                                pending.available = true;
                            } else if target == price_ref {
                                pending.price_axis_pw = exact_w;
                                pending.price_axis_ph = exact_h;
                                pending.available = true;
                            } else if target == time_ref {
                                pending.time_axis_pw = exact_w;
                                pending.time_axis_ph = exact_h;
                                pending.available = true;
                            }
                        }
                    }
                    drop(pending);

                    if raf_scheduled.get() {
                        return;
                    }
                    raf_scheduled.set(true);

                    let inner_for_raf = Rc::clone(&inner);
                    let pending_for_raf = Rc::clone(&pending_exact);
                    let last_applied_for_raf = Rc::clone(&last_applied);
                    let raf_scheduled_for_raf = Rc::clone(&raf_scheduled);
                    let raf_id_for_raf = Rc::clone(&raf_id);
                    let raf_slot_for_raf = Rc::clone(&raf_slot);
                    let dirty_for_raf = Rc::clone(&dirty);

                    let raf_cb = Closure::<dyn FnMut(f64)>::wrap(Box::new(move |_ts: f64| {
                        raf_id_for_raf.set(0);
                        raf_scheduled_for_raf.set(false);

                        let pending_exact = {
                            let mut pending = pending_for_raf.borrow_mut();
                            let next = *pending;
                            *pending = ExactPixelSizes::default();
                            next
                        };

                        let Ok(mut s) = inner_for_raf.try_borrow_mut() else {
                            raf_slot_for_raf.borrow_mut().take();
                            return;
                        };

                        apply_pending_exact_sizes(&mut *s, pending_exact);

                        let dpr = get_dpr();
                        s.engine.dpr = dpr;
                        let next_signature = read_resize_signature(&*s, dpr);
                        let should_apply = {
                            let mut last = last_applied_for_raf.borrow_mut();
                            if last.as_ref() == Some(&next_signature) {
                                false
                            } else {
                                *last = Some(next_signature);
                                true
                            }
                        };
                        if should_apply {
                            sync_widget_sizes(&mut *s, dpr, true);

                            let (cw, ch) = s.layout.container_css_size();
                            s.engine.event_bus.emit(raycore::ChartEvent::Resize {
                                width: cw,
                                height: ch,
                            });
                            dirty_for_raf.set(true);
                        }

                        raf_slot_for_raf.borrow_mut().take();
                    }));
                    *raf_slot.borrow_mut() = Some(raf_cb);

                    let raf_request = if let Some(window) = web_sys::window() {
                        let borrowed = raf_slot.borrow();
                        borrowed.as_ref().and_then(|cb| {
                            window
                                .request_animation_frame(cb.as_ref().unchecked_ref())
                                .ok()
                        })
                    } else {
                        None
                    };

                    if let Some(id) = raf_request {
                        raf_id.set(id);
                        return;
                    }

                    raf_scheduled.set(false);
                    raf_id.set(0);
                    raf_slot.borrow_mut().take();
                }));
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

            (cb, observer)
        };

        Ok(RayCore {
            inner,
            mtf_resolver,
            active_renderer_name,
            symbol: "DEMO".to_string(),
            interval: "1m".to_string(),
            _closures: closures,
            _wheel_closures: wheel_closures,
            _touch_closures: touch_closures,
            _resize_closure: Some(resize_closure),
            _resize_observer: Some(resize_observer),
            _resize_raf_closure: Some(resize_raf_closure_slot),
            _resize_raf_id: resize_raf_id,
            _long_press_timer: long_press_timer,
            _last_tap_time: last_tap_time,
            _event_registry: EventListenerRegistry::new(),
            event_emitter,
            theme_config: raycore::ThemeConfig::default(),
            auto_render,
            _raf_closure: raf_closure,
            _raf_id: raf_id,
            replay_forced_auto_render,
            dirty,
        })
    }

    // ── Public API ───────────────────────────────────────────────────────────

    pub fn renderer_name(&self) -> String {
        self.active_renderer_name.clone()
    }

    pub fn get_supported_renderers() -> js_sys::Array {
        let arr = js_sys::Array::new();
        if webgpu_available() {
            arr.push(&JsValue::from_str("webgpu"));
        }
        arr.push(&JsValue::from_str("canvas2d"));
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
    ///   renderer: "webgpu" | "canvas2d" | "auto",
    ///   autoRender: true,
    ///   symbol: "BTCUSD",
    ///   interval: "1D",
    ///   crosshair: { mode: "normal" | "magnet_ohlc" },
    ///   priceScale: { mode: "normal", margins: { top: 0.1, bottom: 0.1 } },
    /// }
    /// ```
    pub async fn create_chart(container: JsValue, options: JsValue) -> Result<RayCore, JsValue> {
        let options = normalize_options(options);

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

        // Default to WebGPU unless a renderer option is explicitly provided.
        let requested_renderer = js_get_str(&options, "renderer")
            .map(|v| RendererModeRequest::from_str(&v))
            .unwrap_or(RendererModeRequest::WebGpu);
        let mut chart = Self::create_with(&container_id, requested_renderer.as_str()).await?;

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
        chart.auto_render.set(auto_render);
        if auto_render {
            chart.start_auto_render_internal();
        }

        // Apply symbol
        if let Some(symbol) = js_get_str(&options, "symbol") {
            chart.symbol = symbol.clone();
            chart.inner.borrow_mut().symbol = symbol;
        }

        // Apply interval
        if let Some(interval) = js_get_str(&options, "interval") {
            chart.interval = interval;
        }

        chart.apply_lwc_compat_options(&options);

        // Apply CSS variables from theme
        chart.apply_css_variables();

        Ok(chart)
    }

    /// Apply partial options update at runtime.
    ///
    /// Accepts the same options shape as `create_chart()`. Only provided
    /// fields are updated; omitted fields keep their current values.
    pub fn apply_options(&mut self, options: JsValue) {
        let options = normalize_options(options);
        if options.is_undefined() || options.is_null() {
            return;
        }
        let mut css_changed = false;

        if js_get(&options, "renderer").is_some() {
            self.inner
                .borrow_mut()
                .engine
                .event_bus
                .emit(raycore::ChartEvent::Error {
                    message: "renderer is create-time only; apply_options({ renderer }) is ignored"
                        .to_string(),
                });
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
                css_changed = true;
            }
        }

        // Symbol
        if let Some(symbol) = js_get_str(&options, "symbol") {
            self.symbol = symbol.clone();
            {
                let mut s = self.inner.borrow_mut();
                s.symbol = symbol.clone();
                s.engine
                    .event_bus
                    .emit(raycore::ChartEvent::SymbolChange { symbol });
            }
        }

        // Interval
        if let Some(interval) = js_get_str(&options, "interval") {
            self.interval = interval.clone();
            self.inner
                .borrow_mut()
                .engine
                .event_bus
                .emit(raycore::ChartEvent::IntervalChange { interval });
        }

        css_changed = self.apply_lwc_compat_options(&options) || css_changed;

        // Auto render
        if let Some(auto) = js_get_bool(&options, "autoRender") {
            if auto && !self.auto_render.get() {
                self.auto_render.set(true);
                self.replay_forced_auto_render.set(false);
                self.start_auto_render_internal();
            } else if !auto && self.auto_render.get() {
                self.auto_render.set(false);
                self.stop_auto_render_internal();
                self.ensure_forced_auto_render_for_replay();
            }
        }

        if css_changed {
            self.apply_css_variables();
        }
        self.mark_dirty();
    }

    /// Apply Lightweight Charts style-compatible nested options directly to
    /// RayCore style/runtime state. Returns true when theme CSS variables
    /// should be refreshed.
    fn apply_lwc_compat_options(&mut self, options: &JsValue) -> bool {
        if options.is_undefined() || options.is_null() {
            return false;
        }

        let layout_bg = js_get_path(options, &["layout", "background", "color"])
            .or_else(|| js_get_path(options, &["layout", "background"]))
            .and_then(|v| parse_color_js(&v));
        let layout_text =
            js_get_path(options, &["layout", "textColor"]).and_then(|v| parse_color_js(&v));
        let layout_font_family = js_get_path(options, &["layout", "fontFamily"])
            .and_then(|v| v.as_string())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let layout_font_size = js_get_path(options, &["layout", "fontSize"])
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v as f32);

        let mut grid_color = js_get_path(options, &["grid", "vertLines", "color"])
            .or_else(|| js_get_path(options, &["grid", "horzLines", "color"]))
            .or_else(|| js_get_path(options, &["grid", "color"]))
            .and_then(|v| parse_color_js(&v));
        let grid_vert_visible =
            js_get_path(options, &["grid", "vertLines", "visible"]).and_then(|v| v.as_bool());
        let grid_horz_visible =
            js_get_path(options, &["grid", "horzLines", "visible"]).and_then(|v| v.as_bool());
        let grid_visible = if grid_vert_visible.is_some() || grid_horz_visible.is_some() {
            Some(grid_vert_visible.unwrap_or(true) || grid_horz_visible.unwrap_or(true))
        } else {
            None
        };

        let axis_border_color = js_get_path(options, &["rightPriceScale", "borderColor"])
            .or_else(|| js_get_path(options, &["timeScale", "borderColor"]))
            .or_else(|| js_get_path(options, &["priceScale", "borderColor"]))
            .and_then(|v| parse_color_js(&v));
        let right_border_visible = js_get_path(options, &["rightPriceScale", "borderVisible"])
            .or_else(|| js_get_path(options, &["priceScale", "borderVisible"]))
            .and_then(|v| v.as_bool());
        let time_border_visible =
            js_get_path(options, &["timeScale", "borderVisible"]).and_then(|v| v.as_bool());
        let axis_border_visible = if right_border_visible.is_some() || time_border_visible.is_some()
        {
            Some(right_border_visible.unwrap_or(true) || time_border_visible.unwrap_or(true))
        } else {
            None
        };
        let axis_ticks_visible = js_get_path(options, &["rightPriceScale", "ticksVisible"])
            .or_else(|| js_get_path(options, &["priceScale", "ticksVisible"]))
            .or_else(|| js_get_path(options, &["priceScale", "ticks_visible"]))
            .and_then(|v| v.as_bool());
        let price_scale_tick_density = js_get_path(options, &["rightPriceScale", "tickDensity"])
            .or_else(|| js_get_path(options, &["priceScale", "tickDensity"]))
            .or_else(|| js_get_path(options, &["rightPriceScale", "tickMarkDensity"]))
            .or_else(|| js_get_path(options, &["priceScale", "tickMarkDensity"]))
            .or_else(|| js_get(options, "priceScaleTickDensity"))
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v as f32);

        let crosshair_mode = js_get_path(options, &["crosshair", "mode"])
            .or_else(|| js_get(options, "crosshairMode"))
            .and_then(|v| parse_crosshair_mode_js(&v));
        let vert_patch =
            parse_crosshair_line_patch(js_get_path(options, &["crosshair", "vertLine"]));
        let horz_patch =
            parse_crosshair_line_patch(js_get_path(options, &["crosshair", "horzLine"]));
        let crosshair_label_text = js_get_path(options, &["crosshair", "labelTextColor"])
            .or_else(|| js_get_path(options, &["crosshair", "label_text_color"]))
            .and_then(|v| parse_color_js(&v));

        let price_scale_mode = js_get_path(options, &["priceScale", "mode"])
            .or_else(|| js_get_path(options, &["rightPriceScale", "mode"]))
            .and_then(|v| parse_price_scale_mode_js(&v));
        let margins_obj = js_get_path(options, &["priceScale", "margins"])
            .or_else(|| js_get_path(options, &["priceScale", "scaleMargins"]))
            .or_else(|| js_get_path(options, &["rightPriceScale", "margins"]))
            .or_else(|| js_get_path(options, &["rightPriceScale", "scaleMargins"]));
        let margin_top = margins_obj
            .as_ref()
            .and_then(|m| js_get_f64(m, "top"))
            .filter(|v| v.is_finite());
        let margin_bottom = margins_obj
            .as_ref()
            .and_then(|m| js_get_f64(m, "bottom"))
            .filter(|v| v.is_finite());

        let candles_obj = js_get(options, "candles").or_else(|| js_get(options, "candlestick"));
        let bullish_color = candles_obj
            .as_ref()
            .and_then(|o| js_get(o, "upColor"))
            .and_then(|v| parse_color_js(&v));
        let bearish_color = candles_obj
            .as_ref()
            .and_then(|o| js_get(o, "downColor"))
            .and_then(|v| parse_color_js(&v));
        let wick_bullish_color = candles_obj
            .as_ref()
            .and_then(|o| js_get(o, "wickUpColor").or_else(|| js_get(o, "borderUpColor")))
            .and_then(|v| parse_color_js(&v))
            .or(bullish_color);
        let wick_bearish_color = candles_obj
            .as_ref()
            .and_then(|o| js_get(o, "wickDownColor").or_else(|| js_get(o, "borderDownColor")))
            .and_then(|v| parse_color_js(&v))
            .or(bearish_color);
        let bar_width_ratio = candles_obj
            .as_ref()
            .and_then(|o| js_get_f64(o, "barWidth").or_else(|| js_get_f64(o, "bar_width")))
            .filter(|v| v.is_finite())
            .map(|v| v.clamp(0.1, 1.0) as f32);

        let line_obj = js_get(options, "lineSeries").or_else(|| js_get(options, "line"));
        let line_color = line_obj
            .as_ref()
            .and_then(|o| js_get(o, "color").or_else(|| js_get(o, "lineColor")))
            .and_then(|v| parse_color_js(&v));
        let line_width = line_obj
            .as_ref()
            .and_then(|o| js_get_f64(o, "lineWidth").or_else(|| js_get_f64(o, "width")))
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v as f32);

        let area_obj = js_get(options, "areaSeries").or_else(|| js_get(options, "area"));
        let area_line_color = area_obj
            .as_ref()
            .and_then(|o| js_get(o, "lineColor").or_else(|| js_get(o, "color")))
            .and_then(|v| parse_color_js(&v));
        let area_top_color = area_obj
            .as_ref()
            .and_then(|o| js_get(o, "topColor"))
            .and_then(|v| parse_color_js(&v));
        let area_bottom_color = area_obj
            .as_ref()
            .and_then(|o| js_get(o, "bottomColor"))
            .and_then(|v| parse_color_js(&v));
        let area_line_width = area_obj
            .as_ref()
            .and_then(|o| js_get_f64(o, "lineWidth").or_else(|| js_get_f64(o, "width")))
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v as f32);

        let volume_obj = js_get(options, "volume");
        let volume_color = volume_obj
            .as_ref()
            .and_then(|o| js_get(o, "color"))
            .and_then(|v| parse_color_js(&v));
        let bullish_volume_color = volume_obj
            .as_ref()
            .and_then(|o| js_get(o, "upColor"))
            .and_then(|v| parse_color_js(&v))
            .or(volume_color);
        let bearish_volume_color = volume_obj
            .as_ref()
            .and_then(|o| js_get(o, "downColor"))
            .and_then(|v| parse_color_js(&v))
            .or(volume_color);
        let volume_visible = volume_obj.as_ref().and_then(|o| js_get_bool(o, "visible"));

        let last_price_obj =
            js_get(options, "lastPriceLine").or_else(|| js_get(options, "last_price_line"));
        let last_price_visible = last_price_obj
            .as_ref()
            .and_then(|o| js_get_bool(o, "visible"));
        let last_price_label_visible = last_price_obj.as_ref().and_then(|o| {
            js_get_bool(o, "labelVisible").or_else(|| js_get_bool(o, "label_visible"))
        });
        let last_price_width = last_price_obj
            .as_ref()
            .and_then(|o| js_get_f64(o, "width"))
            .filter(|v| v.is_finite() && *v > 0.0);
        let last_price_style = last_price_obj
            .as_ref()
            .and_then(|o| js_get(o, "style"))
            .and_then(|v| parse_line_style_js(&v));

        let separator_obj = js_get(options, "separator");
        let separator_color = separator_obj
            .as_ref()
            .and_then(|o| js_get(o, "color"))
            .and_then(|v| parse_color_js(&v));
        let separator_hover_color = separator_obj
            .as_ref()
            .and_then(|o| js_get(o, "hoverColor").or_else(|| js_get(o, "hover_color")))
            .and_then(|v| parse_color_js(&v));
        let separator_thickness = separator_obj
            .as_ref()
            .and_then(|o| js_get_f64(o, "thickness"))
            .filter(|v| v.is_finite() && *v > 0.0);
        let separator_hit_area = separator_obj
            .as_ref()
            .and_then(|o| js_get_f64(o, "hitArea").or_else(|| js_get_f64(o, "hit_area")))
            .filter(|v| v.is_finite() && *v > 0.0);
        let auto_scroll =
            js_get_bool(options, "autoScroll").or_else(|| js_get_bool(options, "auto_scroll"));
        let chart_type = js_get(options, "chartType")
            .or_else(|| js_get(options, "chart_type"))
            .and_then(|v| v.as_string());

        {
            let mut s = self.inner.borrow_mut();
            if let Some(mode) = crosshair_mode {
                s.engine.crosshair.mode = mode;
            }
            if let Some(mode) = price_scale_mode {
                s.engine.viewport.set_price_scale_mode(mode);
            }
            if let Some(auto) = auto_scroll {
                s.engine.viewport.auto_scroll = auto;
            }
            if margin_top.is_some() || margin_bottom.is_some() {
                let top = margin_top.unwrap_or(s.engine.viewport.scale_margin_top);
                let bottom = margin_bottom.unwrap_or(s.engine.viewport.scale_margin_bottom);
                s.engine.set_price_scale_margins(top, bottom);
            }
            if let Some(visible) = volume_visible {
                s.engine.set_user_volume_visible(visible);
            }
            if let Some(chart_type_key) = chart_type.as_deref() {
                let next_chart_type = MainChartType::from_str(chart_type_key);
                if s.engine.main_chart_options.chart_type != next_chart_type {
                    s.engine.set_main_chart_type(next_chart_type);
                    s.engine
                        .event_bus
                        .emit(raycore::ChartEvent::ChartTypeChange {
                            chart_type: next_chart_type.as_str().to_string(),
                        });
                }
            }

            {
                let style = &mut s.engine.style;

                if let Some(color) = layout_bg {
                    style.bg_color = color;
                    style.axis_bg_color = color;
                }
                if let Some(color) = layout_text {
                    style.axis_text_color = color;
                }
                if let Some(family) = layout_font_family.as_ref() {
                    style.font_family = family.clone();
                }
                if let Some(size) = layout_font_size {
                    style.font_size = size;
                }

                if let Some(color) = grid_color {
                    style.grid_color = color;
                }
                if let Some(visible) = grid_visible {
                    if visible {
                        if style.grid_color[3] <= 0.0 {
                            style.grid_color[3] = 0.5;
                        }
                    } else {
                        style.grid_color[3] = 0.0;
                    }
                    grid_color = Some(style.grid_color);
                }

                if let Some(color) = axis_border_color {
                    style.axis_border_color = color;
                }
                if let Some(visible) = axis_border_visible {
                    style.axis_border_visible = visible;
                }
                if let Some(visible) = axis_ticks_visible {
                    style.axis_ticks_visible = visible;
                }
                if let Some(density) = price_scale_tick_density {
                    style.price_scale_tick_mark_density = density;
                }

                if let Some(color) = vert_patch.color {
                    style.crosshair_vert_line.color = color;
                }
                if let Some(width) = vert_patch.width {
                    style.crosshair_vert_line.width = width;
                }
                if let Some(line_style) = vert_patch.style {
                    style.crosshair_vert_line.style = line_style;
                }
                if let Some(visible) = vert_patch.visible {
                    style.crosshair_vert_line.visible = visible;
                }
                if let Some(visible) = vert_patch.label_visible {
                    style.crosshair_vert_line.label_visible = visible;
                }
                if let Some(color) = vert_patch.label_bg_color {
                    style.crosshair_vert_line.label_bg_color = color;
                }

                if let Some(color) = horz_patch.color {
                    style.crosshair_horz_line.color = color;
                }
                if let Some(width) = horz_patch.width {
                    style.crosshair_horz_line.width = width;
                }
                if let Some(line_style) = horz_patch.style {
                    style.crosshair_horz_line.style = line_style;
                }
                if let Some(visible) = horz_patch.visible {
                    style.crosshair_horz_line.visible = visible;
                }
                if let Some(visible) = horz_patch.label_visible {
                    style.crosshair_horz_line.label_visible = visible;
                }
                if let Some(color) = horz_patch.label_bg_color {
                    style.crosshair_horz_line.label_bg_color = color;
                }
                if let Some(color) = crosshair_label_text {
                    style.crosshair_label_text = color;
                }

                if let Some(color) = bullish_color {
                    style.bullish_color = color;
                }
                if let Some(color) = bearish_color {
                    style.bearish_color = color;
                }
                if let Some(color) = wick_bullish_color {
                    style.wick_bullish_color = color;
                }
                if let Some(color) = wick_bearish_color {
                    style.wick_bearish_color = color;
                }
                if let Some(ratio) = bar_width_ratio {
                    style.bar_width_ratio = ratio;
                }
                if let Some(color) = bullish_volume_color {
                    style.bullish_volume_color = color;
                }
                if let Some(color) = bearish_volume_color {
                    style.bearish_volume_color = color;
                }
                if let Some(visible) = last_price_visible {
                    style.last_price_line.visible = visible;
                }
                if let Some(visible) = last_price_label_visible {
                    style.last_price_line.label_visible = visible;
                }
                if let Some(width) = last_price_width {
                    style.last_price_line.width = width;
                }
                if let Some(line_style) = last_price_style {
                    style.last_price_line.style = line_style;
                }
            }

            if let Some(color) = line_color {
                s.engine.main_chart_options.line_color = color;
            }
            if let Some(width) = line_width {
                s.engine.main_chart_options.line_width = width;
            }
            if let Some(color) = area_line_color {
                s.engine.main_chart_options.line_color = color;
            }
            if let Some(color) = area_top_color {
                s.engine.main_chart_options.area_top_color = color;
            }
            if let Some(color) = area_bottom_color {
                s.engine.main_chart_options.area_bottom_color = color;
            }
            if let Some(width) = area_line_width {
                s.engine.main_chart_options.line_width = width;
            }

            let mut separator_style_changed = false;
            if let Some(color) = separator_color {
                s.subpane_separator_style.color = color;
                separator_style_changed = true;
            }
            if let Some(color) = separator_hover_color {
                s.subpane_separator_style.hover_color = color;
                separator_style_changed = true;
            }
            if let Some(thickness) = separator_thickness {
                s.subpane_separator_style.line_thickness_css = thickness;
                separator_style_changed = true;
            }
            if let Some(hit_area) = separator_hit_area {
                s.subpane_separator_style.hit_area_css = hit_area;
                separator_style_changed = true;
            }
            if separator_style_changed {
                s.subpane_separator_style.normalize();
                let sep_style = s.subpane_separator_style.clone();
                for sp in &s.subpanes {
                    sp.apply_separator_style(&sep_style);
                }
            }
        }

        let mut css_changed = false;

        if let Some(color) = layout_bg {
            self.theme_config.colors.background = color;
            css_changed = true;
        }
        if let Some(color) = layout_text {
            self.theme_config.colors.axis_text = color;
            css_changed = true;
        }
        if let Some(family) = layout_font_family {
            self.theme_config.typography.font_family = family;
            css_changed = true;
        }
        if let Some(size) = layout_font_size {
            self.theme_config.typography.font_size = size;
            css_changed = true;
        }
        if let Some(color) = grid_color {
            self.theme_config.colors.grid = color;
            css_changed = true;
        }
        if let Some(color) = axis_border_color {
            self.theme_config.colors.axis_border = color;
            css_changed = true;
        }
        if let Some(density) = price_scale_tick_density {
            self.theme_config.layout.price_scale_tick_mark_density = density;
            css_changed = true;
        }
        if let Some(color) = bullish_color {
            self.theme_config.colors.bullish = color;
            css_changed = true;
        }
        if let Some(color) = bearish_color {
            self.theme_config.colors.bearish = color;
            css_changed = true;
        }
        if let Some(color) = wick_bullish_color {
            self.theme_config.colors.wick_bullish = color;
            css_changed = true;
        }
        if let Some(color) = wick_bearish_color {
            self.theme_config.colors.wick_bearish = color;
            css_changed = true;
        }
        if let Some(ratio) = bar_width_ratio {
            self.theme_config.layout.bar_width_ratio = ratio;
            css_changed = true;
        }
        if let Some(color) = bullish_volume_color {
            self.theme_config.colors.bullish_volume = color;
            css_changed = true;
        }
        if let Some(color) = bearish_volume_color {
            self.theme_config.colors.bearish_volume = color;
            css_changed = true;
        }
        if let Some(color) = line_color {
            self.theme_config.series_defaults.line_color = color;
            self.theme_config.series_defaults.area_line_color = color;
            css_changed = true;
        }
        if let Some(color) = area_line_color {
            self.theme_config.series_defaults.line_color = color;
            self.theme_config.series_defaults.area_line_color = color;
            css_changed = true;
        }
        if let Some(color) = area_top_color {
            self.theme_config.series_defaults.area_top_fill = color;
            css_changed = true;
        }
        if let Some(color) = area_bottom_color {
            self.theme_config.series_defaults.area_bottom_fill = color;
            css_changed = true;
        }

        if let Some(color) = vert_patch.color.or(horz_patch.color) {
            self.theme_config.crosshair.line_color = color;
            css_changed = true;
        }
        if let Some(width) = vert_patch.width.or(horz_patch.width) {
            self.theme_config.crosshair.line_width = width;
            css_changed = true;
        }
        if let Some(line_style) = vert_patch.style.or(horz_patch.style) {
            self.theme_config.crosshair.line_style = line_style;
            css_changed = true;
        }
        if let Some(visible) = vert_patch.visible {
            self.theme_config.crosshair.vert_visible = visible;
            css_changed = true;
        }
        if let Some(visible) = horz_patch.visible {
            self.theme_config.crosshair.horz_visible = visible;
            css_changed = true;
        }
        if let Some(visible) = vert_patch.label_visible {
            self.theme_config.crosshair.vert_label_visible = visible;
            css_changed = true;
        }
        if let Some(visible) = horz_patch.label_visible {
            self.theme_config.crosshair.horz_label_visible = visible;
            css_changed = true;
        }
        if let Some(color) = vert_patch.label_bg_color.or(horz_patch.label_bg_color) {
            self.theme_config.crosshair.label_bg = color;
            css_changed = true;
        }
        if let Some(color) = crosshair_label_text {
            self.theme_config.crosshair.label_text = color;
            css_changed = true;
        }
        if let Some(visible) = last_price_visible {
            self.theme_config.last_price_line.visible = visible;
            css_changed = true;
        }
        if let Some(visible) = last_price_label_visible {
            self.theme_config.last_price_line.label_visible = visible;
            css_changed = true;
        }
        if let Some(width) = last_price_width {
            self.theme_config.last_price_line.width = width;
            css_changed = true;
        }
        if let Some(line_style) = last_price_style {
            self.theme_config.last_price_line.style = line_style;
            css_changed = true;
        }
        if let Some(color) = separator_color {
            self.theme_config.subpane_separator.color = color;
            css_changed = true;
        }
        if let Some(color) = separator_hover_color {
            self.theme_config.subpane_separator.hover_color = color;
            css_changed = true;
        }

        css_changed
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
        if !self.auto_render.get() {
            self.auto_render.set(true);
            self.replay_forced_auto_render.set(false);
            self.start_auto_render_internal();
        }
    }

    /// Stop the auto-render RAF loop. Caller must manually call render().
    pub fn stop_auto_render(&mut self) {
        if self.auto_render.get() {
            self.auto_render.set(false);
            self.stop_auto_render_internal();
        }
    }

    /// Returns whether auto-render is currently active.
    pub fn is_auto_render(&self) -> bool {
        self.auto_render.get()
    }

    // ── Replay ───────────────────────────────────────────────────────────────

    /// Enter/exit market replay mode.
    pub fn set_replay_mode(&mut self, enabled: bool) -> Result<(), JsValue> {
        {
            let mut s = self.inner.borrow_mut();
            if enabled {
                s.replay_enter().map_err(js_err)?;
            } else {
                s.replay_exit().map_err(js_err)?;
            }
        }
        self.restore_manual_render_if_forced_replay_stopped();
        self.refresh_pane_cursor_hint();
        self.mark_dirty();
        Ok(())
    }

    /// Whether replay mode is currently active.
    pub fn replay_mode(&self) -> bool {
        self.inner.borrow().replay_active
    }

    /// Start/pause replay playback.
    pub fn set_replay_playing(&mut self, playing: bool) {
        let now_playing = {
            let mut s = self.inner.borrow_mut();
            s.replay_set_playing(playing);
            s.replay_playing
        };

        if now_playing {
            self.ensure_forced_auto_render_for_replay();
        } else {
            self.restore_manual_render_if_forced_replay_stopped();
        }
        self.mark_dirty();
    }

    /// Whether replay playback is currently running.
    pub fn replay_playing(&self) -> bool {
        self.inner.borrow().replay_playing
    }

    /// Step replay backward by 1 bar.
    pub fn replay_step_back(&mut self) -> Result<(), JsValue> {
        self.inner.borrow_mut().replay_step_back().map_err(js_err)?;
        self.restore_manual_render_if_forced_replay_stopped();
        self.mark_dirty();
        Ok(())
    }

    /// Step replay forward by 1 bar.
    pub fn replay_step_forward(&mut self) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .replay_step_forward()
            .map_err(js_err)?;
        self.restore_manual_render_if_forced_replay_stopped();
        self.refresh_pane_cursor_hint();
        self.mark_dirty();
        Ok(())
    }

    /// Set replay cutoff bar (inclusive right-edge trim).
    pub fn set_replay_cutoff_bar(&mut self, index: usize) -> Result<(), JsValue> {
        let mut s = self.inner.borrow_mut();
        if s.replay_active && s.replay_playing {
            s.replay_set_playing(false);
        }
        s.replay_set_cutoff_bar(index).map_err(js_err)?;
        drop(s);
        self.restore_manual_render_if_forced_replay_stopped();
        self.mark_dirty();
        Ok(())
    }

    /// Get replay cutoff bar index, or -1 when unavailable.
    pub fn replay_cutoff_bar(&self) -> i64 {
        self.inner
            .borrow()
            .replay_cutoff_index
            .map(|idx| idx as i64)
            .unwrap_or(-1)
    }

    /// Update replay runtime options.
    pub fn set_replay_options(&mut self, options: JsValue) -> Result<(), JsValue> {
        let options = normalize_options(options);
        if options.is_undefined() || options.is_null() {
            return Ok(());
        }

        let mut s = self.inner.borrow_mut();
        if let Some(speed) = js_get_f64(&options, "speedBarsPerSecond") {
            if !speed.is_finite() || speed <= 0.0 {
                return Err(js_err(
                    "set_replay_options: speedBarsPerSecond must be a finite number > 0",
                ));
            }
            s.replay_speed_bps = speed;
        }
        if let Some(edge) = js_get_str(&options, "edgeBehavior") {
            let behavior = ReplayEdgeBehavior::from_key(edge.as_str()).ok_or_else(|| {
                js_err(
                    "set_replay_options: edgeBehavior must be one of auto_pause, live_continue, auto_exit",
                )
            })?;
            s.replay_edge_behavior = behavior;
        }
        drop(s);
        self.mark_dirty();
        Ok(())
    }

    /// Get current replay runtime options.
    pub fn replay_options(&self) -> JsValue {
        let s = self.inner.borrow();
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("speedBarsPerSecond"),
            &JsValue::from_f64(s.replay_speed_bps),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("edgeBehavior"),
            &JsValue::from_str(replay_edge_behavior_key(s.replay_edge_behavior)),
        );
        obj.into()
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
            let _ =
                js_sys::Reflect::set(&obj, &JsValue::from_str(&key), &JsValue::from_str(&value));
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
        let bars = build_main_bars_from_arrays(
            "set_data_arrays",
            open,
            high,
            low,
            close,
            volume,
            timestamps,
        )?;
        let count = bars.len();
        {
            let mut inner = self.inner.borrow_mut();
            if inner.replay_active {
                inner
                    .replay_replace_archive_from_data(bars)
                    .map_err(js_err)?;
            } else {
                inner.engine.set_data(bars).map_err(js_err)?;
            }
        }
        self.dirty.set(true);
        log::info!("set_data_arrays: {} bars", count);
        Ok(())
    }

    /// Atomically load OHLCV bars plus aligned footprint data from typed arrays.
    ///
    /// This is the canonical historical footprint initialization path for
    /// production integrations. `level_offsets` is bar-aligned and must have
    /// length `bars.len() + 1`; sparse bars use empty ranges.
    pub fn set_data_with_footprint_arrays(
        &mut self,
        open: &[f32],
        high: &[f32],
        low: &[f32],
        close: &[f32],
        volume: &[f32],
        timestamps: &[u64],
        level_offsets: &[u32],
        prices: &[f32],
        bid_volumes: &[f32],
        ask_volumes: &[f32],
    ) -> Result<(), JsValue> {
        let (bars, footprint) = build_historical_footprint_dataset_from_arrays(
            "set_data_with_footprint_arrays",
            open,
            high,
            low,
            close,
            volume,
            timestamps,
            level_offsets,
            prices,
            bid_volumes,
            ask_volumes,
        )?;

        let mut inner = self.inner.borrow_mut();
        if inner.replay_active {
            return Err(js_err(
                "set_data_with_footprint_arrays is not supported while replay is active",
            ));
        }
        let count = bars.len();
        inner
            .engine
            .set_data_with_footprint(bars, footprint)
            .map_err(js_err)?;
        drop(inner);
        self.dirty.set(true);
        log::info!("set_data_with_footprint_arrays: {} bars", count);
        Ok(())
    }

    /// Atomically load OHLCV bars plus footprint levels from JSON.
    ///
    /// Expected canonical format:
    /// `[{"timestamp": 1710000000000, "open": 100.0, "high": 101.0, "low": 99.5, "close": 100.5, "volume": 2500.0, "levels": [{"price": 99.5, "bid": 120.0, "ask": 80.0}]}]`
    ///
    /// Also accepts `{ "bars": [...] }` as the top-level wrapper and the
    /// existing `bid_volume` / `bidVolume` / `ask_volume` / `askVolume` level aliases.
    pub fn set_data_with_footprint_json(&mut self, json: &str) -> Result<(), JsValue> {
        let (bars, footprint) = parse_historical_footprint_json_dataset(json)?;

        let mut inner = self.inner.borrow_mut();
        if inner.replay_active {
            return Err(js_err(
                "set_data_with_footprint_json is not supported while replay is active",
            ));
        }
        let count = bars.len();
        inner
            .engine
            .set_data_with_footprint(bars, footprint)
            .map_err(js_err)?;
        drop(inner);
        self.dirty.set(true);
        log::info!("set_data_with_footprint_json: {} bars", count);
        Ok(())
    }

    // ── Footprint data ───────────────────────────────────────────────────────

    /// Set footprint (order-flow) data for a specific bar.
    ///
    /// `bar_index`: the bar index in the main data array.
    /// `prices`: price levels (ascending order).
    /// `bid_volumes`: bid volume at each price level.
    /// `ask_volumes`: ask volume at each price level.
    ///
    /// All three arrays must be the same length.
    pub fn set_footprint_bar(
        &mut self,
        bar_index: usize,
        prices: &[f32],
        bid_volumes: &[f32],
        ask_volumes: &[f32],
    ) -> Result<(), JsValue> {
        let levels = build_footprint_levels("set_footprint_bar", prices, bid_volumes, ask_volumes)?;

        let mut inner = self.inner.borrow_mut();
        let bar_count = inner.engine.bars.len();
        if bar_index >= bar_count {
            return Err(js_err(format!(
                "set_footprint_bar: bar_index {} out of range (bars={})",
                bar_index, bar_count
            )));
        }
        inner
            .engine
            .set_footprint_bar(bar_index, raycore::FootprintBar { levels });
        drop(inner);
        self.dirty.set(true);
        Ok(())
    }

    /// Bulk set footprint data with typed arrays (fast path for external feeds).
    ///
    /// Layout:
    /// - `bar_indices`: one entry per footprint bar.
    /// - `level_offsets`: length must be `bar_indices.len() + 1`.
    ///   Each bar `i` uses level range `[level_offsets[i], level_offsets[i + 1])`.
    /// - `prices`, `bid_volumes`, `ask_volumes`: flattened level arrays.
    ///
    /// Example:
    /// - bar_indices = [10, 11]
    /// - level_offsets = [0, 3, 5]
    /// - levels for bar 10 = [0..3), bar 11 = [3..5)
    pub fn set_footprint_data_arrays(
        &mut self,
        bar_indices: &[u32],
        level_offsets: &[u32],
        prices: &[f32],
        bid_volumes: &[f32],
        ask_volumes: &[f32],
    ) -> Result<(), JsValue> {
        ensure_equal_len("prices", prices.len(), "bid_volumes", bid_volumes.len())?;
        ensure_equal_len("prices", prices.len(), "ask_volumes", ask_volumes.len())?;
        ensure_finite_slice("prices", prices)?;
        ensure_finite_slice("bid_volumes", bid_volumes)?;
        ensure_finite_slice("ask_volumes", ask_volumes)?;

        if level_offsets.len() != bar_indices.len() + 1 {
            return Err(js_err(format!(
                "set_footprint_data_arrays: level_offsets length must be bar_indices.len()+1 ({} != {}+1)",
                level_offsets.len(),
                bar_indices.len()
            )));
        }
        if level_offsets.first().copied().unwrap_or(0) != 0 {
            return Err(js_err(
                "set_footprint_data_arrays: level_offsets[0] must be 0",
            ));
        }
        if let Some((i, _)) = level_offsets
            .windows(2)
            .enumerate()
            .find(|(_, w)| w[1] < w[0])
        {
            return Err(js_err(format!(
                "set_footprint_data_arrays: level_offsets must be non-decreasing (index {})",
                i + 1
            )));
        }
        let total_levels = prices.len();
        let last_offset = level_offsets.last().copied().unwrap_or(0) as usize;
        if last_offset != total_levels {
            return Err(js_err(format!(
                "set_footprint_data_arrays: last level offset {} must equal level array length {}",
                last_offset, total_levels
            )));
        }

        let mut inner = self.inner.borrow_mut();
        let bar_count = inner.engine.bars.len();
        let mut bars = Vec::with_capacity(bar_indices.len());
        for i in 0..bar_indices.len() {
            let bar_index = bar_indices[i] as usize;
            if bar_index >= bar_count {
                return Err(js_err(format!(
                    "set_footprint_data_arrays: bar_index {} out of range (bars={})",
                    bar_index, bar_count
                )));
            }
            let start = level_offsets[i] as usize;
            let end = level_offsets[i + 1] as usize;
            let levels = build_footprint_levels(
                "set_footprint_data_arrays",
                &prices[start..end],
                &bid_volumes[start..end],
                &ask_volumes[start..end],
            )?;
            bars.push((bar_index, raycore::FootprintBar { levels }));
        }

        inner.engine.set_footprint_bars(bars);
        drop(inner);
        self.dirty.set(true);
        Ok(())
    }

    /// Set footprint data from a JSON string for bulk loading.
    ///
    /// Expected format:
    /// `[{"bar_index": 0, "levels": [{"price": 100.0, "bid": 150, "ask": 200}, ...]}]`
    ///
    /// Also accepts aliases:
    /// - `barIndex` / `index` for `bar_index`
    /// - `bid_volume` / `bidVolume` for `bid`
    /// - `ask_volume` / `askVolume` for `ask`
    pub fn set_footprint_data_json(&mut self, json: &str) -> Result<(), JsValue> {
        let parsed: Vec<serde_json::Value> = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("JSON parse error: {}", e)))?;

        let mut inner = self.inner.borrow_mut();
        let bar_count = inner.engine.bars.len();
        let mut bars = Vec::with_capacity(parsed.len());

        for item in &parsed {
            let bar_index = item
                .get("bar_index")
                .or_else(|| item.get("barIndex"))
                .or_else(|| item.get("index"))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| js_err("set_footprint_data_json: missing bar_index"))?
                as usize;
            if bar_index >= bar_count {
                return Err(js_err(format!(
                    "set_footprint_data_json: bar_index {} out of range (bars={})",
                    bar_index, bar_count
                )));
            }

            let levels_arr = item
                .get("levels")
                .and_then(|v| v.as_array())
                .ok_or_else(|| js_err("set_footprint_data_json: missing levels array"))?;

            let mut prices = Vec::with_capacity(levels_arr.len());
            let mut bids = Vec::with_capacity(levels_arr.len());
            let mut asks = Vec::with_capacity(levels_arr.len());
            for level in levels_arr {
                let price = level
                    .get("price")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| js_err("set_footprint_data_json: level missing price"))?
                    as f32;
                let bid = level
                    .get("bid")
                    .or_else(|| level.get("bid_volume"))
                    .or_else(|| level.get("bidVolume"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| js_err("set_footprint_data_json: level missing bid volume"))?
                    as f32;
                let ask = level
                    .get("ask")
                    .or_else(|| level.get("ask_volume"))
                    .or_else(|| level.get("askVolume"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| js_err("set_footprint_data_json: level missing ask volume"))?
                    as f32;
                prices.push(price);
                bids.push(bid);
                asks.push(ask);
            }

            let levels = build_footprint_levels("set_footprint_data_json", &prices, &bids, &asks)?;
            bars.push((bar_index, raycore::FootprintBar { levels }));
        }
        inner.engine.set_footprint_bars(bars);
        drop(inner);
        self.dirty.set(true);
        Ok(())
    }

    /// Clear all footprint data.
    pub fn clear_footprint_data(&mut self) {
        self.inner.borrow_mut().engine.clear_footprint_data();
        self.dirty.set(true);
    }

    /// Set footprint display mode.
    /// Accepted values: "bid_ask", "delta", "volume", "delta_profile", "volume_profile".
    pub fn set_footprint_display_mode(&mut self, mode: &str) {
        let m = raycore::FootprintDisplayMode::from_str(mode);
        self.inner.borrow_mut().engine.set_footprint_display_mode(m);
        self.dirty.set(true);
        log::info!("set_footprint_display_mode: {}", m.as_str());
    }

    /// Set footprint tick size (price granularity). Pass 0.0 for auto-detection.
    pub fn set_footprint_tick_size(&mut self, tick_size: f32) {
        self.inner
            .borrow_mut()
            .engine
            .set_footprint_tick_size(tick_size);
        self.dirty.set(true);
    }

    /// Enable/disable footprint pane two-axis zoom (X+Y) for wheel and pinch.
    pub fn set_footprint_xy_zoom_enabled(&mut self, enabled: bool) {
        self.inner
            .borrow_mut()
            .engine
            .set_footprint_zoom_price_with_time(enabled);
        self.dirty.set(true);
    }

    /// Return whether footprint pane two-axis zoom (X+Y) is enabled.
    pub fn get_footprint_xy_zoom_enabled(&self) -> bool {
        self.inner.borrow().engine.footprint_zoom_price_with_time()
    }

    /// Configure footprint options from a JSON object.
    ///
    /// Supported keys:
    /// - `display_mode`: string ("bid_ask", "delta", "volume", etc.)
    /// - `tick_size`: number
    /// - `palette`: string (`"blue_red"` default, `"green_red"`)
    /// - `gradient_style`: string (`"no_glow"` default, `"soft_glow"`, `"strong_glow"`)
    /// - `poc_color`: CSS color string or `[r, g, b, a]`
    /// - `imbalance_ratio`: number (default 3.0)
    /// - `show_imbalances`: boolean
    /// - `show_poc`: boolean
    /// - `show_value_area`: boolean
    /// - `value_area_pct`: number (0.0-1.0, default 0.70)
    /// - `show_delta_bar`: boolean
    /// - `show_volume_text`: boolean
    /// - `show_unfinished_auction`: boolean
    /// - `zoom_price_with_time`: boolean (footprint wheel/pinch X+Y zoom)
    pub fn set_footprint_options(&mut self, json: &str) -> Result<(), JsValue> {
        let v: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("JSON parse error: {}", e)))?;

        let mut inner = self.inner.borrow_mut();
        let opts = &mut inner.engine.main_chart_options.footprint;
        let mut refresh_semantic_theme = false;

        if let Some(s) = v["display_mode"].as_str() {
            opts.display_mode = raycore::FootprintDisplayMode::from_str(s);
        }
        if let Some(n) = v["tick_size"].as_f64() {
            opts.tick_size = n as f32;
        }
        if let Some(s) = v["palette"].as_str() {
            opts.palette = raycore::FootprintPalette::from_str(s);
            refresh_semantic_theme = true;
        }
        if let Some(s) = v["gradient_style"]
            .as_str()
            .or_else(|| v["gradientStyle"].as_str())
        {
            opts.gradient_style = raycore::FootprintGradientStyle::from_str(s);
            refresh_semantic_theme = true;
        }
        if let Some(value) = v.get("poc_color").or_else(|| v.get("pocColor")) {
            let color = parse_color_json_value(value)
                .ok_or_else(|| js_err("set_footprint_options: invalid poc_color"))?;
            opts.poc_color = color;
            refresh_semantic_theme = true;
        }
        if let Some(n) = v["imbalance_ratio"].as_f64() {
            opts.imbalance_ratio = n as f32;
        }
        if let Some(b) = v["show_imbalances"].as_bool() {
            opts.show_imbalances = b;
        }
        if let Some(b) = v["show_stacked_imbalances"].as_bool() {
            opts.show_stacked_imbalances = b;
        }
        if let Some(b) = v["show_diagonal_imbalances"].as_bool() {
            opts.show_diagonal_imbalances = b;
        }
        if let Some(b) = v["show_poc"].as_bool() {
            opts.show_poc = b;
        }
        if let Some(b) = v["show_value_area"].as_bool() {
            opts.show_value_area = b;
        }
        if let Some(n) = v["value_area_pct"].as_f64() {
            opts.value_area_pct = n as f32;
        }
        if let Some(b) = v["show_delta_bar"].as_bool() {
            opts.show_delta_bar = b;
        }
        if let Some(b) = v["show_volume_text"].as_bool() {
            opts.show_volume_text = b;
        }
        if let Some(b) = v["show_unfinished_auction"].as_bool() {
            opts.show_unfinished_auction = b;
        }
        if let Some(b) = v["show_cumulative_delta"].as_bool() {
            opts.show_cumulative_delta = b;
        }
        if let Some(n) = v["font_size"].as_f64() {
            opts.font_size = n as f32;
        }
        if let Some(n) = v["min_cell_height"].as_f64() {
            opts.min_cell_height = n as f32;
        }
        if let Some(b) = v["zoom_price_with_time"].as_bool() {
            opts.zoom_price_with_time = b;
        }
        if refresh_semantic_theme {
            opts.apply_semantic_theme();
        }

        drop(inner);
        self.dirty.set(true);
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
        self.dirty.set(true);
    }

    /// Load synthetic demo data dedicated for footprint chart mode.
    ///
    /// This generates OHLCV bars plus aligned per-bar footprint levels and
    /// switches the chart type to `footprint`.
    pub fn demo_mode_footprint(&mut self) {
        let now_ms = js_sys::Date::now() as u64;
        let num_bars = 600;
        let interval_ms = 60_000;
        let start_ms = now_ms - (num_bars as u64) * interval_ms;
        let (bars, footprint) =
            generate_footprint_sample_data(num_bars, start_ms, interval_ms, 0.0);

        let mut inner = self.inner.borrow_mut();
        inner.engine.set_main_chart_type(MainChartType::Footprint);
        match inner.engine.set_data_with_footprint(bars, footprint) {
            Ok(()) => log::info!("demo_mode_footprint: {} bars loaded", num_bars),
            Err(e) => log::error!("demo_mode_footprint failed: {}", e),
        }
        drop(inner);
        self.dirty.set(true);
    }

    // ── Viewport control ─────────────────────────────────────────────────────

    pub fn zoom_to_range(&mut self, start: u64, end: u64) {
        let mut s = self.inner.borrow_mut();
        s.engine.zoom_to_range(start, end);
        emit_visible_range_change(&mut s.engine);
        drop(s);
        self.mark_dirty();
    }

    /// Set visible bar range using fractional bar indices.
    pub fn set_visible_range(&mut self, start: f64, end: f64) {
        let mut s = self.inner.borrow_mut();
        s.engine.viewport.set_range(start, end);
        s.engine.auto_fit_price_if_unlocked();
        emit_visible_range_change(&mut s.engine);
        drop(s);
        self.mark_dirty();
    }

    pub fn visible_range(&self) -> Vec<f64> {
        let s = self.inner.borrow();
        vec![s.engine.viewport.start_bar, s.engine.viewport.end_bar]
    }

    /// Reset the main chart viewport.
    ///
    /// Supported modes:
    /// - `"default"`: restore the recent-bars default view with a small right gap
    /// - `"fit_all"`: show the full dataset with a small right gap
    ///
    /// Unknown or omitted modes fall back to `"default"`.
    pub fn reset_viewport(&mut self, mode: Option<String>) {
        let mut s = self.inner.borrow_mut();
        reset_main_viewport_and_emit(&mut s.engine, mode.as_deref());
        drop(s);
        self.mark_dirty();
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
        let (resolved_x, resolved_y, resolved_bar_index, resolved_price) =
            resolve_synced_crosshair_state(
                &s.engine.viewport,
                &s.engine.time_scale,
                pw,
                ph,
                x,
                y,
                if bar_index.is_finite() && bar_index >= 0.0 {
                    Some(bar_index as usize)
                } else {
                    None
                },
                if price.is_finite() { Some(price) } else { None },
            );
        s.engine.crosshair.bar_index = resolved_bar_index;
        s.engine.crosshair.x = resolved_x;
        s.engine.crosshair.y = resolved_y;
        s.engine.crosshair.price = resolved_price;
        self.dirty.set(true);
    }

    /// Set crosshair state for synchronized panes by semantic values only.
    /// This keeps the target pane snapped to its own viewport/grid.
    pub fn set_crosshair_sync_state(
        &mut self,
        active: bool,
        bar_index: f64,
        price: f64,
        mode: &str,
    ) {
        let mut s = self.inner.borrow_mut();
        let (pw, ph) = s.layout.pane_css_size();

        s.engine.crosshair.active = active;
        s.engine.crosshair.mode = parse_crosshair_mode(mode);

        let (resolved_x, resolved_y, resolved_bar_index, resolved_price) =
            resolve_synced_crosshair_state(
                &s.engine.viewport,
                &s.engine.time_scale,
                pw,
                ph,
                0.0,
                0.0,
                if bar_index.is_finite() && bar_index >= 0.0 {
                    Some(bar_index as usize)
                } else {
                    None
                },
                if price.is_finite() { Some(price) } else { None },
            );

        s.engine.crosshair.bar_index = resolved_bar_index;
        s.engine.crosshair.x = resolved_x;
        s.engine.crosshair.y = resolved_y;
        s.engine.crosshair.price = resolved_price;
        self.dirty.set(true);
    }

    /// Hide crosshair immediately.
    pub fn clear_crosshair(&mut self) {
        let mut s = self.inner.borrow_mut();
        s.engine.crosshair.active = false;
        s.engine.crosshair.bar_index = None;
        self.dirty.set(true);
    }

    pub fn set_symbol(&mut self, symbol: &str) {
        self.symbol = symbol.to_string();
        {
            let mut s = self.inner.borrow_mut();
            s.symbol = symbol.to_string();
        }
        self.inner
            .borrow_mut()
            .engine
            .event_bus
            .emit(raycore::ChartEvent::SymbolChange {
                symbol: symbol.to_string(),
            });
        self.dirty.set(true);
    }

    pub fn symbol(&self) -> String {
        self.symbol.clone()
    }

    pub fn set_interval(&mut self, interval: &str) {
        self.interval = interval.to_string();
        self.inner
            .borrow_mut()
            .engine
            .event_bus
            .emit(raycore::ChartEvent::IntervalChange {
                interval: interval.to_string(),
            });
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
        s.engine.drawings.active_tool =
            raycore::DrawingTool::from_api_key(tool).unwrap_or(raycore::DrawingTool::None);
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
                reset_main_viewport_and_emit(&mut s.engine, Some("fit_all"));
                true
            }

            _ => false, // Key not handled
        }
    }

    /// Remove all drawings.
    pub fn clear_drawings(&mut self) {
        let mut s = self.inner.borrow_mut();
        let active_tool = s.engine.drawings.active_tool;
        s.engine.drawings.clear();
        s.engine.drawings.active_tool = active_tool;
    }

    /// Remove all scale (measurement) drawings.
    pub fn remove_all_scale_drawings(&mut self) {
        self.inner.borrow_mut().engine.drawings.remove_all_scale();
    }

    /// Export a full chart persistence snapshot (styles + viewport + pane layout + drawings).
    ///
    /// `layout_id` is an optional caller-defined identifier to help external storage routing.
    pub fn export_persistence_state(&self, layout_id: Option<String>) -> String {
        let s = self.inner.borrow();
        let style = &s.engine.style;
        let viewport = &s.engine.viewport;
        let layout_id = layout_id.unwrap_or_default();
        let start_bar = finite_or(viewport.start_bar, 0.0).max(0.0);
        let mut end_bar = finite_or(
            viewport.end_bar,
            start_bar + raycore::core::constants::DEFAULT_INITIAL_VISIBLE_BARS,
        );
        if end_bar < start_bar {
            end_bar = start_bar;
        }
        let (price_min, price_max) = normalize_price_range(
            viewport.price_min,
            viewport.price_max,
            0.0,
            raycore::core::constants::DEFAULT_PRICE_MAX,
        );
        let scale_margin_top = finite_or(
            viewport.scale_margin_top,
            raycore::core::constants::DEFAULT_SCALE_MARGIN_TOP,
        )
        .clamp(0.0, 0.5);
        let scale_margin_bottom = finite_or(
            viewport.scale_margin_bottom,
            raycore::core::constants::DEFAULT_SCALE_MARGIN_BOTTOM,
        )
        .clamp(0.0, 0.5);
        let price_scale_tick_density = finite_or_f32(
            style.price_scale_tick_mark_density,
            raycore::core::constants::DEFAULT_PRICE_SCALE_TICK_MARK_DENSITY as f32,
        )
        .max(0.1);
        let volume_visible = s.engine.user_volume_visible();

        let options = serde_json::json!({
            "symbol": self.symbol,
            "interval": self.interval,
            "autoScroll": viewport.auto_scroll,
            "chartType": s.engine.main_chart_options.chart_type.as_str(),
            "layout": {
                "background": { "color": style.bg_color },
                "textColor": style.axis_text_color,
                "fontFamily": style.font_family,
                "fontSize": style.font_size,
            },
            "grid": {
                "color": style.grid_color,
            },
            "rightPriceScale": {
                "borderColor": style.axis_border_color,
                "borderVisible": style.axis_border_visible,
                "ticksVisible": style.axis_ticks_visible,
                "tickDensity": price_scale_tick_density,
            },
            "timeScale": {
                "borderColor": style.axis_border_color,
                "borderVisible": style.axis_border_visible,
            },
            "crosshair": {
                "mode": crosshair_mode_key(s.engine.crosshair.mode),
                "vertLine": {
                    "color": style.crosshair_vert_line.color,
                    "width": style.crosshair_vert_line.width,
                    "style": line_style_key(style.crosshair_vert_line.style),
                    "visible": style.crosshair_vert_line.visible,
                    "labelVisible": style.crosshair_vert_line.label_visible,
                    "labelBackgroundColor": style.crosshair_vert_line.label_bg_color,
                },
                "horzLine": {
                    "color": style.crosshair_horz_line.color,
                    "width": style.crosshair_horz_line.width,
                    "style": line_style_key(style.crosshair_horz_line.style),
                    "visible": style.crosshair_horz_line.visible,
                    "labelVisible": style.crosshair_horz_line.label_visible,
                    "labelBackgroundColor": style.crosshair_horz_line.label_bg_color,
                },
                "labelTextColor": style.crosshair_label_text,
            },
            "priceScale": {
                "mode": price_scale_mode_key(viewport.price_scale_mode),
                "margins": {
                    "top": scale_margin_top,
                    "bottom": scale_margin_bottom,
                },
                "ticksVisible": style.axis_ticks_visible,
                "tickDensity": price_scale_tick_density,
            },
            "candles": {
                "upColor": style.bullish_color,
                "downColor": style.bearish_color,
                "wickUpColor": style.wick_bullish_color,
                "wickDownColor": style.wick_bearish_color,
                "barWidth": style.bar_width_ratio,
            },
            "lineSeries": {
                "color": s.engine.main_chart_options.line_color,
                "lineWidth": s.engine.main_chart_options.line_width,
            },
            "areaSeries": {
                "lineColor": s.engine.main_chart_options.line_color,
                "topColor": s.engine.main_chart_options.area_top_color,
                "bottomColor": s.engine.main_chart_options.area_bottom_color,
                "lineWidth": s.engine.main_chart_options.line_width,
            },
            "volume": {
                "upColor": style.bullish_volume_color,
                "downColor": style.bearish_volume_color,
                "visible": volume_visible,
            },
            "lastPriceLine": {
                "visible": style.last_price_line.visible,
                "labelVisible": style.last_price_line.label_visible,
                "width": style.last_price_line.width,
                "style": line_style_key(style.last_price_line.style),
            },
            "separator": {
                "color": s.subpane_separator_style.color,
                "hoverColor": s.subpane_separator_style.hover_color,
                "thickness": s.subpane_separator_style.line_thickness_css,
                "hitArea": s.subpane_separator_style.hit_area_css,
            },
        });

        let pane_entries: Vec<PersistedSubPane> = s
            .subpanes
            .iter()
            .map(|sp| {
                let (pane_price_min, pane_price_max) = normalize_price_range(
                    sp.viewport.price_min,
                    sp.viewport.price_max,
                    0.0,
                    raycore::core::constants::DEFAULT_PRICE_MAX,
                );
                PersistedSubPane {
                    id: sp.id,
                    study_id: sp.study_id,
                    indicator_type: sp.indicator_type.clone(),
                    height_css: finite_or(sp.get_height(), 160.0).max(0.0),
                    auto_scale: sp.auto_scale,
                    price_min: pane_price_min,
                    price_max: pane_price_max,
                }
            })
            .collect();

        let drawings = DrawingStore {
            version: DRAWING_STORE_VERSION,
            main: s.engine.drawings.snapshot(),
            subpanes: s
                .subpanes
                .iter()
                .map(|sp| PaneDrawingStore {
                    pane_id: sp.id,
                    drawings: sp.drawings.snapshot(),
                })
                .collect(),
        };

        let snapshot = ChartPersistenceState {
            version: CHART_PERSISTENCE_VERSION,
            layout_id: layout_id.clone(),
            options,
            viewport: PersistedViewport {
                start_bar,
                end_bar,
                price_min,
                price_max,
                price_locked: viewport.price_locked,
                price_scale_mode: price_scale_mode_key(viewport.price_scale_mode).to_string(),
                scale_margin_top,
                scale_margin_bottom,
                auto_scroll: viewport.auto_scroll,
            },
            panes: pane_entries,
            drawings,
        };

        serde_json::to_string(&snapshot).unwrap_or_else(|err| {
            log::error!("export_persistence_state: failed to serialize snapshot: {err}");
            let fallback = ChartPersistenceState {
                version: CHART_PERSISTENCE_VERSION,
                layout_id,
                options: serde_json::json!({}),
                viewport: PersistedViewport {
                    start_bar: 0.0,
                    end_bar: raycore::core::constants::DEFAULT_INITIAL_VISIBLE_BARS,
                    price_min: 0.0,
                    price_max: raycore::core::constants::DEFAULT_PRICE_MAX,
                    price_locked: false,
                    price_scale_mode: "normal".to_string(),
                    scale_margin_top: raycore::core::constants::DEFAULT_SCALE_MARGIN_TOP,
                    scale_margin_bottom: raycore::core::constants::DEFAULT_SCALE_MARGIN_BOTTOM,
                    auto_scroll: true,
                },
                panes: Vec::new(),
                drawings: DrawingStore {
                    version: DRAWING_STORE_VERSION,
                    main: raycore::DrawingSnapshot::default(),
                    subpanes: Vec::new(),
                },
            };
            serde_json::to_string(&fallback).unwrap_or_else(|_| "{}".to_string())
        })
    }

    /// Restore a full chart persistence snapshot (styles + viewport + pane layout + drawings).
    pub fn import_persistence_state(&mut self, json: &str) -> Result<(), JsValue> {
        let snapshot: ChartPersistenceState = serde_json::from_str(json)
            .map_err(|e| js_err(format!("Invalid persistence JSON: {e}")))?;

        if snapshot.version > CHART_PERSISTENCE_VERSION {
            return Err(js_err(format!(
                "Unsupported persistence version {} (max supported {})",
                snapshot.version, CHART_PERSISTENCE_VERSION
            )));
        }

        validate_drawing_snapshot(&snapshot.drawings.main).map_err(|e| {
            js_err(format!(
                "Invalid main-pane drawings in persistence snapshot: {e}"
            ))
        })?;
        for pane in &snapshot.drawings.subpanes {
            validate_drawing_snapshot(&pane.drawings).map_err(|e| {
                js_err(format!(
                    "Invalid drawings for pane {} in persistence snapshot: {e}",
                    pane.pane_id
                ))
            })?;
        }

        self.apply_options(json_value_to_js(&snapshot.options));

        {
            let mut s = self.inner.borrow_mut();
            let start_bar = finite_or(snapshot.viewport.start_bar, 0.0).max(0.0);
            let mut end_bar = finite_or(
                snapshot.viewport.end_bar,
                start_bar + raycore::core::constants::DEFAULT_INITIAL_VISIBLE_BARS,
            );
            if end_bar < start_bar {
                end_bar = start_bar;
            }
            let (price_min, price_max) = normalize_price_range(
                snapshot.viewport.price_min,
                snapshot.viewport.price_max,
                0.0,
                raycore::core::constants::DEFAULT_PRICE_MAX,
            );
            {
                let vp = &mut s.engine.viewport;
                vp.set_price_scale_mode(raycore::PriceScaleMode::from_str(
                    snapshot.viewport.price_scale_mode.as_str(),
                ));
                vp.set_range(start_bar, end_bar);
                vp.price_min = price_min;
                vp.price_max = price_max;
                vp.price_locked = snapshot.viewport.price_locked;
                vp.auto_scroll = snapshot.viewport.auto_scroll;
            }
            emit_visible_range_change(&mut s.engine);
            let margin_top = finite_or(
                snapshot.viewport.scale_margin_top,
                raycore::core::constants::DEFAULT_SCALE_MARGIN_TOP,
            )
            .clamp(0.0, 0.5);
            let margin_bottom = finite_or(
                snapshot.viewport.scale_margin_bottom,
                raycore::core::constants::DEFAULT_SCALE_MARGIN_BOTTOM,
            )
            .clamp(0.0, 0.5);
            s.engine.set_price_scale_margins(margin_top, margin_bottom);
        }

        let existing_descriptors: Vec<(u32, u32, String)> = {
            let s = self.inner.borrow();
            s.subpanes
                .iter()
                .map(|sp| (sp.id, sp.study_id, sp.indicator_type.clone()))
                .collect()
        };

        let mut pane_id_map: HashMap<u32, u32> = HashMap::new();
        let mut used_existing: HashSet<u32> = HashSet::new();

        for pane in &snapshot.panes {
            if existing_descriptors.iter().any(|(id, _, _)| *id == pane.id) {
                pane_id_map.insert(pane.id, pane.id);
                used_existing.insert(pane.id);
            }
        }

        for pane in &snapshot.panes {
            if pane_id_map.contains_key(&pane.id) {
                continue;
            }
            if let Some((matched_id, _, _)) =
                existing_descriptors.iter().find(|(id, study, kind)| {
                    !used_existing.contains(id)
                        && *study == pane.study_id
                        && kind.as_str() == pane.indicator_type.as_str()
                })
            {
                pane_id_map.insert(pane.id, *matched_id);
                used_existing.insert(*matched_id);
            }
        }

        for pane in &snapshot.panes {
            if pane_id_map.contains_key(&pane.id) {
                continue;
            }
            let study_exists = {
                let s = self.inner.borrow();
                s.engine
                    .studies
                    .get_study(raycore::StudyId(pane.study_id))
                    .is_some()
            };
            if !study_exists {
                continue;
            }
            let created_id =
                self.add_indicator_pane(pane.study_id, &pane.indicator_type, pane.height_css);
            if created_id > 0 {
                pane_id_map.insert(pane.id, created_id);
            }
        }

        let keep_ids: HashSet<u32> = pane_id_map.values().copied().collect();
        let current_ids: Vec<u32> = {
            let s = self.inner.borrow();
            s.subpanes.iter().map(|sp| sp.id).collect()
        };
        for pane_id in current_ids {
            if !keep_ids.contains(&pane_id) {
                let _ = self.remove_indicator_pane(pane_id);
            }
        }

        {
            let mut s = self.inner.borrow_mut();
            for pane in &snapshot.panes {
                let Some(actual_id) = pane_id_map.get(&pane.id).copied() else {
                    continue;
                };
                if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == actual_id) {
                    let height_css = finite_or(pane.height_css, 160.0).max(0.0);
                    let (pane_price_min, pane_price_max) = normalize_price_range(
                        pane.price_min,
                        pane.price_max,
                        0.0,
                        raycore::core::constants::DEFAULT_PRICE_MAX,
                    );
                    sp.set_height(height_css);
                    sp.auto_scale = pane.auto_scale;
                    sp.viewport.price_min = pane_price_min;
                    sp.viewport.price_max = pane_price_max;
                    sp.viewport.price_locked = !pane.auto_scale;
                }
            }
        }

        let mut remapped_drawings = snapshot.drawings.clone();
        remapped_drawings.subpanes = remapped_drawings
            .subpanes
            .into_iter()
            .filter_map(|mut pane| {
                let mapped = pane_id_map.get(&pane.pane_id).copied()?;
                pane.pane_id = mapped;
                Some(pane)
            })
            .collect();

        let drawings_json = serde_json::to_string(&remapped_drawings)
            .map_err(|e| js_err(format!("Failed to serialize remapped drawings: {e}")))?;
        self.import_drawings(&drawings_json)?;

        self.mark_dirty();
        Ok(())
    }

    /// Export all drawings (main pane + indicator subpanes) as JSON.
    ///
    /// The returned string is versioned and can be stored externally.
    pub fn export_drawings(&self) -> String {
        let s = self.inner.borrow();
        let payload = DrawingStore {
            version: DRAWING_STORE_VERSION,
            main: s.engine.drawings.snapshot(),
            subpanes: s
                .subpanes
                .iter()
                .map(|sp| PaneDrawingStore {
                    pane_id: sp.id,
                    drawings: sp.drawings.snapshot(),
                })
                .collect(),
        };

        serde_json::to_string(&payload).unwrap_or_else(|err| {
            log::error!("export_drawings: failed to serialize drawing snapshot: {err}");
            let fallback = DrawingStore {
                version: DRAWING_STORE_VERSION,
                main: raycore::DrawingSnapshot::default(),
                subpanes: Vec::new(),
            };
            serde_json::to_string(&fallback).unwrap_or_else(|_| "{}".to_string())
        })
    }

    /// Restore all drawings (main pane + indicator subpanes) from JSON.
    ///
    /// Existing drawings are replaced atomically. Unknown subpane IDs in the payload are ignored.
    pub fn import_drawings(&mut self, json: &str) -> Result<(), JsValue> {
        let payload: DrawingStore = serde_json::from_str(json)
            .map_err(|e| js_err(format!("Invalid drawing snapshot JSON: {e}")))?;

        if payload.version > DRAWING_STORE_VERSION {
            return Err(js_err(format!(
                "Unsupported drawing store version {} (max supported {})",
                payload.version, DRAWING_STORE_VERSION
            )));
        }

        validate_drawing_snapshot(&payload.main)
            .map_err(|e| js_err(format!("Invalid main-pane drawing snapshot: {e}")))?;
        let existing_pane_ids: HashSet<u32> = {
            let s = self.inner.borrow();
            s.subpanes.iter().map(|sp| sp.id).collect()
        };
        for pane_store in &payload.subpanes {
            if !existing_pane_ids.contains(&pane_store.pane_id) {
                continue;
            }
            validate_drawing_snapshot(&pane_store.drawings).map_err(|e| {
                js_err(format!(
                    "Invalid drawing snapshot for subpane {}: {e}",
                    pane_store.pane_id
                ))
            })?;
        }

        let mut s = self.inner.borrow_mut();
        s.engine
            .drawings
            .replace_from_snapshot(payload.main)
            .map_err(js_err)?;

        // Stamp timestamps on imported drawings from current bar data.
        s.engine.stamp_drawing_timestamps();

        // Clear all subpane drawings first so restore is deterministic.
        for sp in s.subpanes.iter_mut() {
            sp.drawings.clear();
        }

        for pane_store in payload.subpanes {
            if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pane_store.pane_id) {
                sp.drawings
                    .replace_from_snapshot(pane_store.drawings)
                    .map_err(|e| {
                        js_err(format!(
                            "Failed to restore drawings for subpane {}: {}",
                            pane_store.pane_id, e
                        ))
                    })?;
            }
        }

        self.dirty.set(true);
        Ok(())
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

    /// Show/hide volume bars in the main pane.
    pub fn set_volume_visible(&mut self, visible: bool) {
        self.inner
            .borrow_mut()
            .engine
            .set_user_volume_visible(visible);
    }

    /// Whether volume bars are currently visible in the main pane.
    pub fn get_volume_visible(&self) -> bool {
        self.inner.borrow().engine.user_volume_visible()
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

    /// Set chart and axis background color (RGBA 0-1).
    pub fn set_background_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        let color = [r, g, b, a];
        {
            let mut s = self.inner.borrow_mut();
            s.engine.style.bg_color = color;
            s.engine.style.axis_bg_color = color;
        }
        self.theme_config.colors.background = color;
        self.apply_css_variables();
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
        self.inner
            .borrow_mut()
            .engine
            .style
            .price_scale_tick_mark_density = density;
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
        self.inner
            .borrow_mut()
            .engine
            .set_price_scale_margins(top, bottom);
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
        s.engine
            .event_bus
            .emit(raycore::ChartEvent::PriceScaleChange {
                mode: mode.to_string(),
            });
    }

    // ── Main Chart Type API ────────────────────────────────────────────────────

    /// Set the main chart type.
    ///
    /// Accepted values: "candlestick", "candles", "ohlc", "bars", "line", "area",
    /// "heikin_ashi", "ha", "footprint", "fp", "order_flow".
    pub fn set_chart_type(&mut self, chart_type: &str) {
        use raycore::MainChartType;
        let ct = MainChartType::from_str(chart_type);
        let mut s = self.inner.borrow_mut();
        s.engine.set_main_chart_type(ct);
        s.engine
            .event_bus
            .emit(raycore::ChartEvent::ChartTypeChange {
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
            text_color: raycore::ThemeConfig::default()
                .series_defaults
                .marker_text_color,
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
                text_color: raycore::ThemeConfig::default()
                    .series_defaults
                    .marker_text_color,
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

    // ── Execution Marks API ────────────────────────────────────────────────────
    //
    // First-class execution mark support for trade visualization.
    // Unlike generic markers, execution marks are timestamp-based (not bar-index-based)
    // and designed specifically for trading workflows.

    /// Add a single execution mark to the chart.
    ///
    /// `side`: "buy" or "sell"
    /// `role`: "entry", "scale_in", "scale_out", or "exit"
    ///
    /// Returns the execution mark ID.
    pub fn add_execution_mark(
        &mut self,
        id: &str,
        timestamp_ms: u64,
        price: f64,
        quantity: f64,
        side: &str,
        role: &str,
    ) {
        use raycore::{ExecutionMark, ExecutionRole, ExecutionSide};

        let mark = ExecutionMark::new(
            id,
            timestamp_ms,
            price,
            quantity,
            ExecutionSide::from_str(side),
            ExecutionRole::from_str(role),
        );

        let mut s = self.inner.borrow_mut();
        let engine = &mut s.engine;
        engine.execution_marks.add(mark);
        engine.execution_marks.resolve_bar_indices(&engine.bars);

        log::info!(
            "add_execution_mark: id={}, ts={}, price={}, side={}, role={}",
            id,
            timestamp_ms,
            price,
            side,
            role
        );
    }

    /// Add an execution mark with all optional fields.
    ///
    /// `side`: "buy" or "sell"
    /// `role`: "entry", "scale_in", "scale_out", or "exit"
    /// `order_type`: e.g., "market", "limit", "stop" (empty string for none)
    /// `label`: custom label text (empty string for default)
    /// `group_id`: group ID for related fills (empty string for none)
    /// `color_*`: custom color override (pass all zeros to use default)
    /// `realized_pnl`: realized P&L (pass NaN for none)
    pub fn add_execution_mark_full(
        &mut self,
        id: &str,
        timestamp_ms: u64,
        price: f64,
        quantity: f64,
        side: &str,
        role: &str,
        order_type: &str,
        label: &str,
        group_id: &str,
        color_r: f32,
        color_g: f32,
        color_b: f32,
        color_a: f32,
        realized_pnl: f64,
    ) {
        use raycore::{ExecutionMark, ExecutionRole, ExecutionSide};

        let mut mark = ExecutionMark::new(
            id,
            timestamp_ms,
            price,
            quantity,
            ExecutionSide::from_str(side),
            ExecutionRole::from_str(role),
        );

        if !order_type.is_empty() {
            mark = mark.with_order_type(order_type);
        }
        if !label.is_empty() {
            mark = mark.with_label(label);
        }
        if !group_id.is_empty() {
            mark = mark.with_group_id(group_id);
        }
        if color_a > 0.0 {
            mark = mark.with_color([color_r, color_g, color_b, color_a]);
        }
        if realized_pnl.is_finite() {
            mark = mark.with_realized_pnl(realized_pnl);
        }

        let mut s = self.inner.borrow_mut();
        let engine = &mut s.engine;
        engine.execution_marks.add(mark);
        engine.execution_marks.resolve_bar_indices(&engine.bars);
    }

    /// Remove an execution mark by ID.
    pub fn remove_execution_mark(&mut self, id: &str) -> bool {
        self.inner.borrow_mut().engine.execution_marks.remove(id)
    }

    /// Clear all execution marks.
    pub fn clear_execution_marks(&mut self) {
        self.inner.borrow_mut().engine.execution_marks.clear();
    }

    /// Show/hide execution mark text labels.
    pub fn set_execution_mark_text_visible(&mut self, visible: bool) {
        self.inner
            .borrow_mut()
            .engine
            .set_execution_mark_text_visible(visible);
    }

    /// Whether execution mark text labels are currently rendered.
    pub fn get_execution_mark_text_visible(&self) -> bool {
        self.inner.borrow().engine.execution_mark_text_visible()
    }

    /// Serialize all execution marks to JSON.
    pub fn get_execution_marks_json(&self) -> String {
        let marks: Vec<_> = self
            .inner
            .borrow()
            .engine
            .execution_marks
            .iter()
            .map(|mark| {
                serde_json::json!({
                    "id": mark.id,
                    "timestamp_ms": mark.timestamp_ms,
                    "price": mark.price,
                    "quantity": mark.quantity,
                    "side": mark.side.as_str(),
                    "role": mark.role.as_str(),
                    "order_type": mark.order_type,
                    "realized_pnl": mark.realized_pnl,
                    "label": mark.label,
                    "color": mark.color,
                    "group_id": mark.group_id,
                })
            })
            .collect();

        serde_json::to_string(&marks).unwrap_or_else(|_| "[]".to_string())
    }

    /// Set multiple execution marks at once (replaces existing).
    ///
    /// `mark_data` is a flat array of execution mark data with stride 6:
    /// [timestamp_ms, price, quantity, side_idx, role_idx, ...]
    /// where side_idx: 0=buy, 1=sell
    /// and role_idx: 0=entry, 1=scale_in, 2=scale_out, 3=exit
    ///
    /// `ids` is an array of string IDs (must match mark_data length / 5).
    pub fn set_execution_marks(&mut self, ids: Vec<String>, mark_data: &[f64]) {
        use raycore::{ExecutionMark, ExecutionRole, ExecutionSide};

        const STRIDE: usize = 5; // timestamp_ms, price, quantity, side_idx, role_idx
        let expected_count = mark_data.len() / STRIDE;

        if ids.len() != expected_count {
            log::warn!(
                "set_execution_marks: ids.len()={} but expected {} from mark_data",
                ids.len(),
                expected_count
            );
            return;
        }

        let mut marks = Vec::with_capacity(expected_count);
        for (i, chunk) in mark_data.chunks_exact(STRIDE).enumerate() {
            let timestamp_ms = chunk[0] as u64;
            let price = chunk[1];
            let quantity = chunk[2];
            let side = match chunk[3] as u32 {
                0 => ExecutionSide::Buy,
                _ => ExecutionSide::Sell,
            };
            let role = match chunk[4] as u32 {
                0 => ExecutionRole::Entry,
                1 => ExecutionRole::ScaleIn,
                2 => ExecutionRole::ScaleOut,
                _ => ExecutionRole::Exit,
            };

            marks.push(ExecutionMark::new(
                ids[i].clone(),
                timestamp_ms,
                price,
                quantity,
                side,
                role,
            ));
        }

        let mut s = self.inner.borrow_mut();
        let engine = &mut s.engine;
        engine.execution_marks.set(marks);
        engine.execution_marks.resolve_bar_indices(&engine.bars);

        log::info!("set_execution_marks: count={}", expected_count);
    }

    /// Set execution marks from a JSON string.
    ///
    /// Expected format:
    /// ```json
    /// [
    ///   {
    ///     "id": "exec-1",
    ///     "timestamp_ms": 1234567890000,
    ///     "price": 100.5,
    ///     "quantity": 1.0,
    ///     "side": "buy",
    ///     "role": "entry",
    ///     "order_type": "market",
    ///     "label": "Entry Long",
    ///     "group_id": "trade-1",
    ///     "color": [0.2, 0.8, 0.4, 1.0],
    ///     "realized_pnl": 0.0
    ///   },
    ///   ...
    /// ]
    /// ```
    pub fn set_execution_marks_json(&mut self, json: &str) -> Result<(), JsValue> {
        use raycore::{ExecutionMark, ExecutionRole, ExecutionSide};

        let items: Vec<serde_json::Value> = serde_json::from_str(json)
            .map_err(|e| js_err(format!("Invalid execution marks JSON: {}", e)))?;

        let mut marks = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| js_err(format!("execution mark {} missing id", i)))?;
            let timestamp_ms = item
                .get("timestamp_ms")
                .or_else(|| item.get("timestampMs"))
                .or_else(|| item.get("timestamp"))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| js_err(format!("execution mark {} missing timestamp_ms", i)))?;
            let price = item
                .get("price")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| js_err(format!("execution mark {} missing price", i)))?;
            let quantity = item
                .get("quantity")
                .or_else(|| item.get("qty"))
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            let side = item
                .get("side")
                .and_then(|v| v.as_str())
                .ok_or_else(|| js_err(format!("execution mark {} missing side", i)))?;
            let role = item
                .get("role")
                .and_then(|v| v.as_str())
                .ok_or_else(|| js_err(format!("execution mark {} missing role", i)))?;

            let mut mark = ExecutionMark::new(
                id,
                timestamp_ms,
                price,
                quantity,
                ExecutionSide::from_str(side),
                ExecutionRole::from_str(role),
            );

            if let Some(order_type) = item
                .get("order_type")
                .or_else(|| item.get("orderType"))
                .and_then(|v| v.as_str())
            {
                mark = mark.with_order_type(order_type);
            }
            if let Some(label) = item.get("label").and_then(|v| v.as_str()) {
                mark = mark.with_label(label);
            }
            if let Some(group_id) = item
                .get("group_id")
                .or_else(|| item.get("groupId"))
                .and_then(|v| v.as_str())
            {
                mark = mark.with_group_id(group_id);
            }
            if let Some(color) =
                parse_color_json_value(item.get("color").unwrap_or(&serde_json::Value::Null))
            {
                mark = mark.with_color(color);
            }
            if let Some(pnl) = item
                .get("realized_pnl")
                .or_else(|| item.get("realizedPnl"))
                .and_then(|v| v.as_f64())
            {
                mark = mark.with_realized_pnl(pnl);
            }

            marks.push(mark);
        }

        let mut s = self.inner.borrow_mut();
        let engine = &mut s.engine;
        engine.execution_marks.set(marks);
        engine.execution_marks.resolve_bar_indices(&engine.bars);

        log::info!("set_execution_marks_json: count={}", items.len());
        Ok(())
    }

    /// Get the number of execution marks.
    pub fn execution_mark_count(&self) -> usize {
        self.inner.borrow().engine.execution_marks.len()
    }

    /// Set the selected execution mark ID (shows selected-trade execution locators).
    /// Pass empty string or null to deselect.
    pub fn set_selected_execution_mark(&mut self, mark_id: Option<String>) {
        let mut s = self.inner.borrow_mut();
        s.selected_execution_mark_id = mark_id.filter(|id| !id.is_empty());
    }

    /// Get the currently selected execution mark ID, or null if none.
    pub fn get_selected_execution_mark(&self) -> Option<String> {
        self.inner.borrow().selected_execution_mark_id.clone()
    }

    /// Clear the selected execution mark.
    pub fn clear_selected_execution_mark(&mut self) {
        self.inner.borrow_mut().selected_execution_mark_id = None;
    }

    /// Convert a timestamp (in milliseconds) to a bar index.
    /// Returns -1 if the timestamp is before all bars.
    pub fn timestamp_to_bar_index(&self, timestamp_ms: u64) -> i64 {
        let s = self.inner.borrow();
        raycore::timestamp_to_bar_index(timestamp_ms, &s.engine.bars)
            .map(|idx| idx as i64)
            .unwrap_or(-1)
    }

    /// Convert a bar index to a timestamp (in milliseconds).
    /// Returns 0 if the bar index is out of bounds.
    pub fn bar_index_to_timestamp(&self, bar_index: u32) -> u64 {
        let s = self.inner.borrow();
        raycore::bar_index_to_timestamp(bar_index as usize, &s.engine.bars).unwrap_or(0)
    }

    /// Project a timestamp/price coordinate into current pane CSS coordinates.
    pub fn project_point(&self, timestamp_ms: u64, price: f64) -> JsValue {
        let s = self.inner.borrow();
        let obj = js_sys::Object::new();
        let (pane_css_w, pane_css_h) = s.layout.pane_css_size();

        let Some(logical_index) = s
            .engine
            .time_scale
            .logical_index_for_timestamp(timestamp_ms)
        else {
            let _ =
                js_sys::Reflect::set(&obj, &JsValue::from_str("x"), &JsValue::from_f64(f64::NAN));
            let _ =
                js_sys::Reflect::set(&obj, &JsValue::from_str("y"), &JsValue::from_f64(f64::NAN));
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("visible"),
                &JsValue::from_bool(false),
            );
            return obj.into();
        };

        let x = raycore::core::renderer::transforms::bar_to_x(
            logical_index + 0.5,
            &s.engine.viewport,
            pane_css_w,
        );
        let y = s.engine.viewport.price_to_css_y(price, pane_css_h);
        let visible = x.is_finite()
            && y.is_finite()
            && x >= 0.0
            && x <= pane_css_w
            && y >= 0.0
            && y <= pane_css_h;

        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("x"), &JsValue::from_f64(x));
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("y"), &JsValue::from_f64(y));
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("visible"),
            &JsValue::from_bool(visible),
        );
        obj.into()
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

    // ── Indicator Runtime API (User DSL) ───────────────────────────────────

    fn with_indicator_input_defaults(&self, inputs: JsonValue) -> JsonValue {
        let mut obj = match inputs {
            JsonValue::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        if !obj.contains_key("symbol") {
            obj.insert("symbol".to_string(), JsonValue::String(self.symbol.clone()));
        }
        if !obj.contains_key("chartTimeframe") && !obj.contains_key("chart_timeframe") {
            obj.insert(
                "chartTimeframe".to_string(),
                JsonValue::String(self.interval.clone()),
            );
        }
        JsonValue::Object(obj)
    }

    /// Compile user indicator source into the internal IR program artifact.
    /// Returns: `{ indicatorId, diagnostics }`.
    pub fn indicator_compile(&self, source: &str, meta_json: &str) -> JsValue {
        let feature_flags = serde_json::from_str::<JsonValue>(meta_json)
            .ok()
            .and_then(|v| v.get("featureFlags").cloned())
            .and_then(|v| v.as_array().cloned())
            .map(|arr| {
                arr.into_iter()
                    .filter_map(|x| x.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let Ok(mut s) = self.inner.try_borrow_mut() else {
            let diagnostics = vec![raycore::CompileDiagnostic {
                code: "INDL-3004".to_string(),
                severity: raycore::DiagnosticSeverity::Error,
                message: "indicator runtime is busy; retry compile".to_string(),
                hint: Some("wait a moment and compile again".to_string()),
                span: None,
            }];
            let obj = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("indicatorId"), &JsValue::NULL);
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("diagnostics"),
                &diagnostics_to_js(&diagnostics),
            );
            return obj.into();
        };
        let result = s.engine.indicators.compile(source, &feature_flags);
        let obj = js_sys::Object::new();
        let indicator_id = result
            .indicator_id
            .map(|id| JsValue::from_f64(id as f64))
            .unwrap_or(JsValue::NULL);
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("indicatorId"), &indicator_id);
        if let Some(id) = result.indicator_id {
            if let Some(mode) = s.engine.indicators.get_program_compile_mode(id) {
                let _ = js_sys::Reflect::set(
                    &obj,
                    &JsValue::from_str("compileMode"),
                    &JsValue::from_str(&mode),
                );
            }
        }
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("diagnostics"),
            &diagnostics_to_js(&result.diagnostics),
        );
        obj.into()
    }

    /// Attach a compiled indicator program to the current chart.
    /// Returns a runtime instance ID, or 0 on failure.
    pub fn indicator_attach(&self, indicator_id: u32, opts_json: &str) -> u32 {
        let inputs = serde_json::from_str::<JsonValue>(opts_json)
            .ok()
            .and_then(|v| v.get("inputs").cloned())
            .unwrap_or(JsonValue::Null);
        let inputs = self.with_indicator_input_defaults(inputs);
        let instance_id = {
            let Ok(mut inner) = self.inner.try_borrow_mut() else {
                return 0;
            };
            let instance_id = inner
                .engine
                .indicators
                .attach(indicator_id, inputs)
                .unwrap_or(0);
            if instance_id > 0 {
                inner.engine.recompute_indicators();
            }
            instance_id
        };
        if instance_id > 0 {
            self.mark_dirty();
        }
        instance_id
    }

    /// Detach an indicator runtime instance.
    pub fn indicator_detach(&self, instance_id: u32) -> bool {
        let detached = self
            .inner
            .try_borrow_mut()
            .map(|mut inner| inner.engine.indicators.detach(instance_id))
            .unwrap_or(false);
        if detached {
            self.mark_dirty();
        }
        detached
    }

    /// Set runtime inputs for an attached indicator instance.
    pub fn indicator_set_inputs(&self, instance_id: u32, inputs_json: &str) -> bool {
        let inputs = serde_json::from_str::<JsonValue>(inputs_json).unwrap_or(JsonValue::Null);
        let inputs = self.with_indicator_input_defaults(inputs);
        let updated = {
            let Ok(mut inner) = self.inner.try_borrow_mut() else {
                return false;
            };
            let updated = inner.engine.indicators.set_inputs(instance_id, inputs);
            if updated {
                inner.engine.recompute_indicators();
            }
            updated
        };
        if updated {
            self.mark_dirty();
        }
        updated
    }

    /// Load backend-resolved MTF series snapshots into the runtime resolver cache.
    ///
    /// JSON payload:
    /// `{ clear?: bool, series: [{ symbol, chartTimeframe, requestId?, timeframe, field, mode?, points: [...] }] }`
    pub fn indicator_set_mtf_snapshot(&self, snapshot_json: &str) -> bool {
        fn json_u64(raw: Option<&JsonValue>) -> Option<u64> {
            match raw {
                Some(JsonValue::Number(v)) => v
                    .as_u64()
                    .or_else(|| v.as_i64().and_then(|n| (n >= 0).then_some(n as u64))),
                _ => None,
            }
        }

        let parsed = serde_json::from_str::<JsonValue>(snapshot_json).unwrap_or(JsonValue::Null);
        let Some(root) = parsed.as_object() else {
            return false;
        };

        let mut changed = false;
        if root.get("clear").and_then(|v| v.as_bool()).unwrap_or(false) {
            self.mtf_resolver.clear();
            changed = true;
        }

        if let Some(series_list) = root.get("series").and_then(|v| v.as_array()) {
            for (idx, series) in series_list.iter().enumerate() {
                let Some(item) = series.as_object() else {
                    continue;
                };
                let symbol = item
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .filter(|v| !v.is_empty())
                    .unwrap_or(self.symbol.as_str())
                    .to_string();
                let chart_timeframe = item
                    .get("chartTimeframe")
                    .or_else(|| item.get("chart_timeframe"))
                    .and_then(|v| v.as_str())
                    .filter(|v| !v.is_empty())
                    .unwrap_or(self.interval.as_str())
                    .to_string();
                let timeframe = item
                    .get("timeframe")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let field = item
                    .get("field")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                if timeframe.is_empty() || field.is_empty() {
                    continue;
                }
                let mode = MtfMode::parse(item.get("mode").and_then(|v| v.as_str()));
                let request_id = item
                    .get("requestId")
                    .or_else(|| item.get("request_id"))
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("mtf_{}_{}_{}", idx, timeframe, field));
                let request = MtfRequest {
                    request_id: request_id.clone(),
                    symbol,
                    chart_timeframe,
                    timeframe: timeframe.clone(),
                    field: field.clone(),
                    mode,
                    gaps: Default::default(),
                    lookahead: Default::default(),
                };

                let mut samples = Vec::<MtfResolvedSample>::new();
                if let Some(points) = item.get("points").and_then(|v| v.as_array()) {
                    for point in points {
                        let Some(p) = point.as_object() else {
                            continue;
                        };
                        let Some(ts) = json_u64(p.get("ts")) else {
                            continue;
                        };
                        let source_timeframe = p
                            .get("sourceTimeframe")
                            .or_else(|| p.get("source_timeframe"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(timeframe.as_str())
                            .to_string();
                        samples.push(MtfResolvedSample {
                            request_id: request_id.clone(),
                            timestamp: ts,
                            value: p.get("value").and_then(|v| v.as_f64()),
                            source_timeframe,
                            source_bar_open: json_u64(
                                p.get("sourceBarOpen").or_else(|| p.get("source_bar_open")),
                            ),
                            source_bar_close: json_u64(
                                p.get("sourceBarClose")
                                    .or_else(|| p.get("source_bar_close")),
                            ),
                            is_confirmed: p
                                .get("isConfirmed")
                                .or_else(|| p.get("is_confirmed"))
                                .and_then(|v| v.as_bool())
                                .unwrap_or(matches!(mode, MtfMode::Confirmed)),
                        });
                    }
                }

                if !samples.is_empty() {
                    self.mtf_resolver.set_series(&request, samples);
                    changed = true;
                }
            }
        }

        if changed {
            if let Ok(mut inner) = self.inner.try_borrow_mut() {
                inner.engine.recompute_indicators();
            }
            self.mark_dirty();
        }
        changed
    }

    /// Enable or disable a runtime indicator instance.
    pub fn indicator_set_enabled(&self, instance_id: u32, enabled: bool) -> bool {
        let changed = {
            let Ok(mut inner) = self.inner.try_borrow_mut() else {
                return false;
            };
            let changed = inner.engine.indicators.set_enabled(instance_id, enabled);
            if changed && enabled {
                inner.engine.recompute_indicators();
            }
            changed
        };
        if changed {
            self.mark_dirty();
        }
        changed
    }

    /// List attached indicator instances.
    pub fn indicator_list(&self) -> JsValue {
        let list = self.inner.borrow().engine.indicators.list_instances();
        let arr = js_sys::Array::new();
        for item in list {
            let obj = js_sys::Object::new();
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("instanceId"),
                &JsValue::from_f64(item.instance_id as f64),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("indicatorId"),
                &JsValue::from_f64(item.program_id as f64),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("enabled"),
                &JsValue::from_bool(item.enabled),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("updatesApplied"),
                &JsValue::from_f64(item.updates_applied as f64),
            );
            arr.push(&obj);
        }
        arr.into()
    }

    /// Get diagnostics for a compiled indicator.
    pub fn indicator_get_diagnostics(&self, indicator_id: u32) -> JsValue {
        let diags = self
            .inner
            .borrow()
            .engine
            .indicators
            .get_program_diagnostics(indicator_id);
        diagnostics_to_js(&diags)
    }

    /// Get compile-time-discovered MTF request templates from a compiled indicator.
    pub fn indicator_get_mtf_requests(&self, indicator_id: u32) -> JsValue {
        let requests = self
            .inner
            .borrow()
            .engine
            .indicators
            .get_program_mtf_requests(indicator_id);
        let arr = js_sys::Array::new();
        for req in requests {
            let obj = js_sys::Object::new();
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("symbol"),
                &JsValue::from_str(&req.symbol),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("timeframe"),
                &JsValue::from_str(&req.timeframe),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("field"),
                &JsValue::from_str(&req.field),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("mode"),
                &JsValue::from_str(&req.mode),
            );
            arr.push(&obj);
        }
        arr.into()
    }

    /// Get runtime stats for an indicator instance.
    pub fn indicator_get_stats(&self, instance_id: u32) -> JsValue {
        let stats = self
            .inner
            .borrow()
            .engine
            .indicators
            .get_instance_stats(instance_id);
        let Some(stats) = stats else {
            return JsValue::NULL;
        };
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("instanceId"),
            &JsValue::from_f64(stats.instance_id as f64),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("indicatorId"),
            &JsValue::from_f64(stats.program_id as f64),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("opsUsed"),
            &JsValue::from_f64(stats.ops_used as f64),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("lastElapsedMicros"),
            &JsValue::from_f64(stats.last_elapsed_micros as f64),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("peakObjects"),
            &JsValue::from_f64(stats.peak_objects as f64),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("peakVertices"),
            &JsValue::from_f64(stats.peak_vertices as f64),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("updatesApplied"),
            &JsValue::from_f64(stats.updates_applied as f64),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("recentEvents"),
            &json_value_to_js(
                &serde_json::to_value(&stats.recent_events).unwrap_or(JsonValue::Array(vec![])),
            ),
        );
        obj.into()
    }

    /// Drain and return pending runtime events from indicator instances.
    ///
    /// Returns an array of objects:
    /// `{ instanceId, indicatorId, type, code, message, barIndex }`
    pub fn indicator_drain_events(&self) -> JsValue {
        let events = self
            .inner
            .try_borrow_mut()
            .map(|mut inner| inner.engine.indicators.drain_runtime_events())
            .unwrap_or_default();
        let out = js_sys::Array::new();
        for item in events {
            let obj = js_sys::Object::new();
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("instanceId"),
                &JsValue::from_f64(item.instance_id as f64),
            );
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("indicatorId"),
                &JsValue::from_f64(item.program_id as f64),
            );
            match item.event {
                RuntimeEvent::RuntimeError {
                    code,
                    message,
                    bar_index,
                } => {
                    let _ = js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("type"),
                        &JsValue::from_str("runtimeError"),
                    );
                    let _ = js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("code"),
                        &JsValue::from_str(&code),
                    );
                    let _ = js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("message"),
                        &JsValue::from_str(&message),
                    );
                    let _ = js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("barIndex"),
                        &JsValue::from_f64(bar_index as f64),
                    );
                }
                RuntimeEvent::LimitsExceeded {
                    code,
                    message,
                    bar_index,
                } => {
                    let _ = js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("type"),
                        &JsValue::from_str("limitsExceeded"),
                    );
                    let _ = js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("code"),
                        &JsValue::from_str(&code),
                    );
                    let _ = js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("message"),
                        &JsValue::from_str(&message),
                    );
                    let _ = js_sys::Reflect::set(
                        &obj,
                        &JsValue::from_str("barIndex"),
                        &JsValue::from_f64(bar_index as f64),
                    );
                }
            }
            out.push(&obj);
        }
        out.into()
    }

    /// Privileged runtime-only resource limit override for an indicator instance.
    pub fn indicator_set_resource_limits(&mut self, instance_id: u32, limits_json: &str) -> bool {
        let parsed = serde_json::from_str::<JsonValue>(limits_json).unwrap_or(JsonValue::Null);
        let limits = ResourceLimits {
            max_ops_per_bar: parsed
                .get("max_ops_per_bar")
                .and_then(|v| v.as_u64())
                .unwrap_or(ResourceLimits::default().max_ops_per_bar),
            max_wall_time_per_bar_ms: parsed
                .get("max_wall_time_per_bar_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(ResourceLimits::default().max_wall_time_per_bar_ms),
            max_memory_bytes_per_instance: parsed
                .get("max_memory_bytes_per_instance")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(ResourceLimits::default().max_memory_bytes_per_instance),
            max_objects_per_instance: parsed
                .get("max_objects_per_instance")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(ResourceLimits::default().max_objects_per_instance),
            max_vertices_per_frame: parsed
                .get("max_vertices_per_frame")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(ResourceLimits::default().max_vertices_per_frame),
        };
        self.inner
            .borrow_mut()
            .engine
            .indicators
            .set_resource_limits(instance_id, limits)
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
        let dirty_for_events = Rc::clone(&self.dirty);

        // ── Phase 1: Create sub-pane, extract DOM refs + shared state ──
        let creation_result: Option<(
            u32,
            web_sys::Element,
            web_sys::Element,
            Rc<Cell<f64>>,
            Rc<Cell<bool>>,
        )> =
            {
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
                let separator_drag_cb: Rc<dyn Fn(f64)> = {
                    let inner = Rc::clone(&inner_for_events);
                    let dirty = Rc::clone(&dirty_for_events);
                    Rc::new(move |delta_y: f64| {
                        let Ok(mut s) = inner.try_borrow_mut() else {
                            return;
                        };
                        let Some(separator_idx) = s.pane_coordinator.separator_index(id) else {
                            return;
                        };
                        s.pane_coordinator.drag_separator(separator_idx, delta_y);
                        let heights = s.pane_coordinator.all_heights();
                        for (subpane_id, height) in heights {
                            if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == subpane_id) {
                                sp.set_height(height);
                            }
                        }
                        dirty.set(true);
                    })
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
                    separator_drag_cb,
                    Rc::clone(&dirty_for_events),
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
                            colors.push(config.colors.get(i).copied().unwrap_or(
                                raycore::ThemeConfig::default().indicator_palette.fallback,
                            ));
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
            let dirty = Rc::clone(&dirty_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_: web_sys::Event| {
                    log::info!("SubPane {} pointerenter", pid);
                    ca.set(true);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.engine.crosshair.active = true;
                    s.active_subpane_id = Some(pid);
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_: web_sys::Event| {
                    log::info!("SubPane {} pointerleave", pid);
                    ca.set(false);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    let mut drawing_gesture_active = false;
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        drawing_gesture_active =
                            sp.drawings.is_creating()
                                || sp
                                    .drawings
                                    .selected_id
                                    .and_then(|id| sp.drawings.get(id).map(|d| d.state()))
                                    .is_some_and(|state| {
                                        matches!(
                                        state,
                                        raycore::core::drawings::types::DrawingState::Dragging {
                                            ..
                                        }
                                    )
                                    });
                        sp.drawings.clear_hovered();
                    }
                    if !drag.get() && !drawing_gesture_active {
                        s.engine.crosshair.active = false;
                        s.active_subpane_id = None;
                    }
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
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

                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.engine.crosshair.active = true;
                    s.active_subpane_id = Some(pid);

                    // Get grid-snapped index (can be beyond data_len in empty space)
                    let grid_idx = s.engine.viewport.bar_index_for_crosshair(x, pw);

                    // bar_index is only set if we have actual data at this position
                    s.engine.crosshair.bar_index =
                        grid_idx.and_then(|idx| s.engine.time_scale.main_bar_index_at_slot(idx));

                    // X snaps to bar grid (like LWC) - even in empty space
                    if let Some(idx) = grid_idx {
                        s.engine.crosshair.x = s.engine.viewport.bar_center_css(idx, pw);
                    } else {
                        s.engine.crosshair.x = x;
                    }

                    // Get bar coordinate from main viewport (shared time axis)
                    let bar = s.engine.viewport.pixel_to_bar(x, pw);
                    let main_start_bar = s.engine.viewport.start_bar;
                    let main_end_bar = s.engine.viewport.end_bar;

                    // Update drawing preview or drag if active
                    let mut drawing_cursor: Option<&'static str> = None;
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        let price = sp.viewport.pixel_to_price(y, ph);
                        let mut is_drawing_drag = false;

                        // Update drawing creation preview
                        if sp.drawings.is_creating() {
                            sp.drawings.update_creation_preview(bar, price);
                        }

                        // Update drawing drag
                        if let Some(id) = sp.drawings.selected_id {
                            let drag_state = sp.drawings.get(id).map(|d| (d.tool(), d.state()));
                            if let Some((
                                tool,
                                raycore::core::drawings::types::DrawingState::Dragging {
                                    anchor_index,
                                    ..
                                },
                            )) = drag_state
                            {
                                sp.drawings.update_drag(id, bar, price);
                                is_drawing_drag = true;
                                let hit_part = match anchor_index {
                                    Some(i) => raycore::core::drawings::types::HitPart::Anchor(i),
                                    None => raycore::core::drawings::types::HitPart::Body,
                                };
                                let drag_cursor =
                                    raycore::core::drawings::types::cursor_for_drawing_hit(
                                        tool,
                                        hit_part,
                                        anchor_index,
                                    );
                                drawing_cursor = Some(if drag_cursor == "move" {
                                    "grabbing"
                                } else {
                                    drag_cursor
                                });
                            }
                        }

                        if !is_drawing_drag
                            && !sp.drawings.is_creating()
                            && sp.drawings.active_tool == raycore::DrawingTool::None
                        {
                            // Build a hybrid viewport (shared time + subpane price) for hover hit-test.
                            let mut hybrid_vp = Viewport::new(pw as u32, ph as u32);
                            hybrid_vp.start_bar = main_start_bar;
                            hybrid_vp.end_bar = main_end_bar;
                            hybrid_vp.price_min = sp.viewport.price_min;
                            hybrid_vp.price_max = sp.viewport.price_max;
                            hybrid_vp.volume_height_ratio = 0.0;

                            let hovered_id =
                                sp.drawings
                                    .hit_test(x, y, &hybrid_vp, pw, ph)
                                    .map(|(id, hit)| {
                                        let anchor_idx = match hit.part {
                                            raycore::core::drawings::types::HitPart::Anchor(i) => {
                                                Some(i)
                                            }
                                            _ => None,
                                        };
                                        if let Some(tool) = sp.drawings.get(id).map(|d| d.tool()) {
                                            drawing_cursor = Some(
                                            raycore::core::drawings::types::cursor_for_drawing_hit(
                                                tool,
                                                hit.part,
                                                anchor_idx,
                                            ),
                                        );
                                        }
                                        id
                                    });
                            sp.drawings.set_hovered(hovered_id);
                        } else {
                            sp.drawings.clear_hovered();
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
                        let before_start = s.engine.viewport.start_bar;
                        let before_end = s.engine.viewport.end_bar;
                        let before_price_min = s.engine.viewport.price_min;
                        let before_price_max = s.engine.viewport.price_max;
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
                        if (before_start - s.engine.viewport.start_bar).abs() > 1e-9
                            || (before_end - s.engine.viewport.end_bar).abs() > 1e-9
                            || (before_price_min - s.engine.viewport.price_min).abs() > 1e-9
                            || (before_price_max - s.engine.viewport.price_max).abs() > 1e-9
                        {
                            emit_visible_range_change(&mut s.engine);
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
                    } else {
                        let cursor = drawing_cursor.unwrap_or("crosshair");
                        let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                        let _ = html_el.style().set_property("cursor", cursor);
                    }
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
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

                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };

                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        sp.drawings.clear_hovered();
                    }

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
                        let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                        let _ = html_el.set_pointer_capture(pe.pointer_id());
                        let _ = html_el.style().set_property("cursor", "crosshair");
                        dirty.set(true);
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
                            let anchor_idx = match result.part {
                                HitPart::Anchor(i) => Some(i),
                                _ => None,
                            };

                            // Match main-pane drawing behavior: body hits move the
                            // whole drawing, while rectangle edges/corners resize
                            // through their dedicated anchor hits.
                            sp.drawings.select(id);
                            sp.drawings.start_drag(id, anchor_idx, bar, price);
                            let cursor = sp
                                .drawings
                                .get(id)
                                .map(|d| {
                                    raycore::core::drawings::types::cursor_for_drawing_hit(
                                        d.tool(),
                                        result.part,
                                        anchor_idx,
                                    )
                                })
                                .unwrap_or("move");
                            drop(s);
                            let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                            let _ = html_el.set_pointer_capture(pe.pointer_id());
                            let _ = html_el.style().set_property(
                                "cursor",
                                if cursor == "move" { "grabbing" } else { cursor },
                            );
                            dirty.set(true);
                            return; // Don't pan while dragging drawing
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
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
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
                        let Ok(mut s) = inner.try_borrow_mut() else {
                            return;
                        };
                        let bar = s.engine.viewport.pixel_to_bar(x, pw);
                        let main_start_bar = s.engine.viewport.start_bar;
                        let main_end_bar = s.engine.viewport.end_bar;
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

                            let mut hybrid_vp = Viewport::new(pw as u32, ph as u32);
                            hybrid_vp.start_bar = main_start_bar;
                            hybrid_vp.end_bar = main_end_bar;
                            hybrid_vp.price_min = sp.viewport.price_min;
                            hybrid_vp.price_max = sp.viewport.price_max;
                            hybrid_vp.volume_height_ratio = 0.0;
                            let hovered = sp.drawings.hit_test(x, y, &hybrid_vp, pw, ph);
                            sp.drawings.set_hovered(hovered.map(|(id, _)| id));
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
                    let cursor = {
                        let Ok(s) = inner.try_borrow() else {
                            return;
                        };
                        let mut hover_cursor = "crosshair";
                        let main_start_bar = s.engine.viewport.start_bar;
                        let main_end_bar = s.engine.viewport.end_bar;
                        if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                            let mut hybrid_vp = Viewport::new(pw as u32, ph as u32);
                            hybrid_vp.start_bar = main_start_bar;
                            hybrid_vp.end_bar = main_end_bar;
                            hybrid_vp.price_min = sp.viewport.price_min;
                            hybrid_vp.price_max = sp.viewport.price_max;
                            hybrid_vp.volume_height_ratio = 0.0;
                            if let Some((id, hit)) = sp.drawings.hit_test(x, y, &hybrid_vp, pw, ph)
                            {
                                let anchor_idx = match hit.part {
                                    raycore::core::drawings::types::HitPart::Anchor(i) => Some(i),
                                    _ => None,
                                };
                                if let Some(tool) = sp.drawings.get(id).map(|d| d.tool()) {
                                    hover_cursor =
                                        raycore::core::drawings::types::cursor_for_drawing_hit(
                                            tool, hit.part, anchor_idx,
                                        );
                                }
                            }
                        }
                        hover_cursor
                    };
                    let _ = html_el.style().set_property("cursor", cursor);
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let rect = chart_c.get_bounding_client_rect();
                    let x = pe.client_x() as f64 - rect.left();
                    let y = pe.client_y() as f64 - rect.top();
                    let pw = rect.width();
                    let ph = rect.height();
                    drag.set(false);

                    // Mirror pointerup teardown so canceled drawing gestures do not remain latched.
                    {
                        let Ok(mut s) = inner.try_borrow_mut() else {
                            return;
                        };
                        let main_start_bar = s.engine.viewport.start_bar;
                        let main_end_bar = s.engine.viewport.end_bar;
                        if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                            if sp.drawings.is_creating() {
                                sp.drawings.cancel_creation();
                            }
                            if let Some(id) = sp.drawings.selected_id {
                                sp.drawings.end_drag(id);
                            }
                            let mut hybrid_vp = Viewport::new(pw as u32, ph as u32);
                            hybrid_vp.start_bar = main_start_bar;
                            hybrid_vp.end_bar = main_end_bar;
                            hybrid_vp.price_min = sp.viewport.price_min;
                            hybrid_vp.price_max = sp.viewport.price_max;
                            hybrid_vp.volume_height_ratio = 0.0;
                            let hovered = sp.drawings.hit_test(x, y, &hybrid_vp, pw, ph);
                            sp.drawings.set_hovered(hovered.map(|(id, _)| id));
                            sp.scroll_state.borrow_mut().animation.stop();
                        }
                    }

                    let html_el: &web_sys::HtmlElement = chart_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    let cursor = {
                        let Ok(s) = inner.try_borrow() else {
                            return;
                        };
                        let mut hover_cursor = "crosshair";
                        let main_start_bar = s.engine.viewport.start_bar;
                        let main_end_bar = s.engine.viewport.end_bar;
                        if let Some(sp) = s.subpanes.iter().find(|sp| sp.id == pid) {
                            let mut hybrid_vp = Viewport::new(pw as u32, ph as u32);
                            hybrid_vp.start_bar = main_start_bar;
                            hybrid_vp.end_bar = main_end_bar;
                            hybrid_vp.price_min = sp.viewport.price_min;
                            hybrid_vp.price_max = sp.viewport.price_max;
                            hybrid_vp.volume_height_ratio = 0.0;
                            if let Some((id, hit)) = sp.drawings.hit_test(x, y, &hybrid_vp, pw, ph)
                            {
                                let anchor_idx = match hit.part {
                                    raycore::core::drawings::types::HitPart::Anchor(i) => Some(i),
                                    _ => None,
                                };
                                if let Some(tool) = sp.drawings.get(id).map(|d| d.tool()) {
                                    hover_cursor =
                                        raycore::core::drawings::types::cursor_for_drawing_hit(
                                            tool, hit.part, anchor_idx,
                                        );
                                }
                            }
                        }
                        hover_cursor
                    };
                    let _ = html_el.style().set_property("cursor", cursor);
                    dirty.set(true);
                }));
            let _ = chart_el
                .add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── chart: wheel (forward to main chart zoom) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let chart_c = chart_el.clone();
            let dirty = Rc::clone(&dirty_for_events);
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let rect = chart_c.get_bounding_client_rect();
                    let x = e.client_x() as f64 - rect.left();
                    let y = e.client_y() as f64 - rect.top();
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.on_pane_wheel(x, y, e.delta_x(), e.delta_y(), e.delta_mode());
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
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
                        dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
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

                        let Ok(mut s) = inner.try_borrow_mut() else {
                            return;
                        };
                        s.engine.viewport.start_bar = new_start;
                        s.engine.viewport.end_bar = new_end;
                        let bar_len = s.engine.bars.len();
                        s.engine.viewport.clamp_to_data(bar_len);
                        dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
            let cb = Closure::<dyn FnMut(web_sys::TouchEvent)>::wrap(Box::new(
                move |e: web_sys::TouchEvent| {
                    let touches = e.touches();
                    if touches.length() < 2 {
                        pinch.set(false);
                        dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        sp.reset_price_viewport();
                        log::info!("SubPane {} viewport reset via double-click", pid);
                    }
                    dirty.set(true);
                }));
            let _ =
                chart_el.add_event_listener_with_callback("dblclick", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: pointerenter ──
        {
            let inner = Rc::clone(&inner_for_events);
            let ca = crosshair_active.clone();
            let dirty = Rc::clone(&dirty_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    ca.set(true);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    s.engine.crosshair.active = true;
                    s.active_subpane_id = Some(pid);
                    dirty.set(true);
                }));
            let _ = axis_el
                .add_event_listener_with_callback("pointerenter", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: pointerleave ──
        {
            let inner = Rc::clone(&inner_for_events);
            let ca = crosshair_active.clone();
            let drag = sp_drag_active.clone();
            let adrag = axis_drag_active.clone();
            let dirty = Rc::clone(&dirty_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    ca.set(false);
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    if !drag.get() && !adrag.get() && s.active_subpane_id == Some(pid) {
                        s.engine.crosshair.active = false;
                        s.active_subpane_id = None;
                    }
                    dirty.set(true);
                }));
            let _ = axis_el
                .add_event_listener_with_callback("pointerleave", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: dblclick (toggle auto-scale) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let dirty = Rc::clone(&dirty_for_events);
            let pid = pane_id;
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        sp.toggle_auto_scale();
                        log::info!("SubPane {} auto-scale toggled: {}", pid, sp.auto_scale);
                    }
                    dirty.set(true);
                }));
            let _ =
                axis_el.add_event_listener_with_callback("dblclick", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: wheel (zoom sub-pane price range) ──
        {
            let inner = Rc::clone(&inner_for_events);
            let dirty = Rc::clone(&dirty_for_events);
            let pid = pane_id;
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::wrap(Box::new(
                move |e: web_sys::WheelEvent| {
                    e.prevent_default();
                    let dy = e.delta_y();
                    let factor = if dy > 0.0 { 1.1 } else { 0.9 };
                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        let center = (sp.viewport.price_min + sp.viewport.price_max) / 2.0;
                        let half = (sp.viewport.price_max - sp.viewport.price_min) / 2.0 * factor;
                        sp.viewport.price_min = center - half;
                        sp.viewport.price_max = center + half;
                        // Lock price axis after manual zoom (same as main chart)
                        sp.viewport.price_locked = true;
                    }
                    dirty.set(true);
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
            let dirty = Rc::clone(&dirty_for_events);
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
                    dirty.set(true);
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
            let cy = crosshair_y.clone();
            let ca = crosshair_active.clone();
            let dirty = Rc::clone(&dirty_for_events);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    let rect = axis_c.get_bounding_client_rect();
                    let y = pe.client_y() as f64 - rect.top();
                    cy.set(y);
                    ca.set(true);
                    if !adrag.get() {
                        let Ok(mut s) = inner.try_borrow_mut() else {
                            return;
                        };
                        s.engine.crosshair.active = true;
                        s.active_subpane_id = Some(pid);
                        dirty.set(true);
                        return;
                    }
                    let css_h = rect.height();
                    if css_h <= 1.0 {
                        return;
                    }

                    let delta_y = y - ady.get();
                    let factor = (1.0 + delta_y / css_h).max(0.1);

                    let center = (apmin.get() + apmax.get()) / 2.0;
                    let half = (apmax.get() - apmin.get()) / 2.0 * factor;

                    let Ok(mut s) = inner.try_borrow_mut() else {
                        return;
                    };
                    if let Some(sp) = s.subpanes.iter_mut().find(|sp| sp.id == pid) {
                        sp.viewport.price_min = center - half;
                        sp.viewport.price_max = center + half;
                        // Lock price axis after manual scaling (same as main chart)
                        sp.viewport.price_locked = true;
                    }
                    dirty.set(true);
                }));
            let _ = axis_el
                .add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: pointerup ──
        {
            let adrag = axis_drag_active.clone();
            let axis_c = axis_el.clone();
            let dirty = Rc::clone(&dirty_for_events);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    adrag.set(false);
                    let html_el: &web_sys::HtmlElement = axis_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    dirty.set(true);
                }));
            let _ =
                axis_el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref());
            interaction_closures.push(cb);
        }

        // ── axis: pointercancel ──
        {
            let adrag = axis_drag_active.clone();
            let axis_c = axis_el.clone();
            let dirty = Rc::clone(&dirty_for_events);
            let cb =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let pe: web_sys::PointerEvent = e.unchecked_into();
                    adrag.set(false);
                    let html_el: &web_sys::HtmlElement = axis_c.unchecked_ref();
                    let _ = html_el.release_pointer_capture(pe.pointer_id());
                    dirty.set(true);
                }));
            let _ = axis_el
                .add_event_listener_with_callback("pointercancel", cb.as_ref().unchecked_ref());
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
                let colors: Vec<[f32; 4]> =
                    data.iter()
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
        drop(s);
        self.dirty.set(true);
    }

    /// Get the number of indicator sub-panes.
    pub fn indicator_pane_count(&self) -> usize {
        self.inner.borrow().subpanes.len()
    }

    // ── Real-time data updates ────────────────────────────────────────────

    /// Append a single bar to the data array. Used for real-time streaming.
    pub fn append_bar(
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("append_bar: runtime busy"))?;
        if inner.replay_active {
            inner.replay_buffer_append_bar(bar).map_err(js_err)
        } else {
            inner.engine.append_bar(bar).map_err(js_err)
        }
    }

    /// Update the last bar in the data array. Used for real-time tick updates.
    pub fn update_last_bar(
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("update_last_bar: runtime busy"))?;
        if inner.replay_active {
            inner.replay_buffer_update_last_bar(bar).map_err(js_err)
        } else {
            inner.engine.update_bar(bar).map_err(js_err)
        }
    }

    /// LWC-style main series update semantics:
    /// update last bar if timestamp matches, append if timestamp is newer.
    pub fn upsert_bar(
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("upsert_bar: runtime busy"))?;
        let bar = Bar {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            _pad: 0.0,
        };
        if inner.replay_active {
            inner.replay_buffer_upsert_bar(bar).map_err(js_err)
        } else {
            inner.engine.upsert_bar(bar).map_err(js_err)
        }
    }

    /// Upsert a main bar and atomically set its footprint levels.
    ///
    /// This is the preferred real-time API for external order-flow feeds:
    /// one call updates OHLCV + footprint for the same logical bar.
    pub fn upsert_bar_with_footprint(
        &self,
        timestamp: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        volume: f32,
        prices: &[f32],
        bid_volumes: &[f32],
        ask_volumes: &[f32],
    ) -> Result<(), JsValue> {
        ensure_finite_fields(
            "upsert_bar_with_footprint",
            &[
                ("open", open),
                ("high", high),
                ("low", low),
                ("close", close),
                ("volume", volume),
            ],
        )?;
        let levels = build_footprint_levels(
            "upsert_bar_with_footprint",
            prices,
            bid_volumes,
            ask_volumes,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("upsert_bar_with_footprint: runtime busy"))?;
        if inner.replay_active {
            return Err(js_err(
                "upsert_bar_with_footprint is not supported while replay is active",
            ));
        }

        inner.engine.upsert_bar(bar).map_err(js_err)?;
        let bar_index = inner.engine.bars.len().saturating_sub(1);
        inner
            .engine
            .set_footprint_bar(bar_index, raycore::FootprintBar { levels });
        Ok(())
    }

    /// Append a single point to a line/area/baseline overlay series.
    pub fn append_series_point(&self, id: u32, timestamp: u64, value: f32) -> Result<(), JsValue> {
        if !value.is_finite() {
            return Err(js_err("append_series_point: value must be finite"));
        }
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("append_series_point: runtime busy"))?;
        inner
            .engine
            .append_series_point(SeriesId(id), LinePoint { timestamp, value })
            .map_err(js_err)
    }

    /// Update the last point in a line/area/baseline overlay series.
    pub fn update_last_series_point(
        &self,
        id: u32,
        timestamp: u64,
        value: f32,
    ) -> Result<(), JsValue> {
        if !value.is_finite() {
            return Err(js_err("update_last_series_point: value must be finite"));
        }
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("update_last_series_point: runtime busy"))?;
        inner
            .engine
            .update_last_series_point(SeriesId(id), LinePoint { timestamp, value })
            .map_err(js_err)
    }

    /// LWC-style update semantics for line/area/baseline overlays:
    /// update last point if timestamp matches, append if timestamp is newer.
    pub fn upsert_series_point(&self, id: u32, timestamp: u64, value: f32) -> Result<(), JsValue> {
        if !value.is_finite() {
            return Err(js_err("upsert_series_point: value must be finite"));
        }
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("upsert_series_point: runtime busy"))?;
        inner
            .engine
            .upsert_series_point(SeriesId(id), LinePoint { timestamp, value })
            .map_err(js_err)
    }

    /// Append a single point to a histogram overlay series.
    pub fn append_histogram_point(
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("append_histogram_point: runtime busy"))?;
        inner
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
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("update_last_histogram_point: runtime busy"))?;
        inner
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
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("upsert_histogram_point: runtime busy"))?;
        inner
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
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("append_bar_series_point: runtime busy"))?;
        inner
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
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("update_last_bar_series_point: runtime busy"))?;
        inner
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
        &self,
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
        let mut inner = self
            .inner
            .try_borrow_mut()
            .map_err(|_| js_err("upsert_bar_series_point: runtime busy"))?;
        inner
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
        self.dirty.set(true);
        let _ = render_frame::do_render_frame(&self.inner, &self.dirty);
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    /// Mark the chart as needing a re-render (for auto-render mode).
    fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    fn refresh_pane_cursor_hint(&self) {
        let Ok(s) = self.inner.try_borrow() else {
            return;
        };
        let cursor = s.cursor_css();
        let pane: &web_sys::HtmlElement = s.layout.pane_container.unchecked_ref();
        let _ = pane.style().set_property("cursor", cursor);
    }

    fn ensure_forced_auto_render_for_replay(&mut self) {
        if self.auto_render.get() || self.replay_forced_auto_render.get() {
            return;
        }
        self.replay_forced_auto_render.set(true);
        self.start_auto_render_internal();
    }

    fn restore_manual_render_if_forced_replay_stopped(&mut self) {
        if !self.replay_forced_auto_render.get() {
            return;
        }
        let should_restore = {
            let s = self.inner.borrow();
            !s.replay_active || !s.replay_playing
        };
        if should_restore {
            self.replay_forced_auto_render.set(false);
            self.stop_auto_render_internal();
        }
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

    /// Queue an internal requestAnimationFrame when auto-render is active.
    fn start_auto_render_internal(&mut self) {
        self.dirty.set(true);
        request_auto_render_frame_if_needed(&self.dirty);
    }

    /// Stop the internal requestAnimationFrame loop.
    fn stop_auto_render_internal(&mut self) {
        self.replay_forced_auto_render.set(false);

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
        self._raf_closure.borrow_mut().take();
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
        let resize_raf_id = self._resize_raf_id.get();
        if resize_raf_id != 0 {
            if let Some(window) = web_sys::window() {
                let _ = window.cancel_animation_frame(resize_raf_id);
            }
            self._resize_raf_id.set(0);
        }
        if let Some(slot) = &self._resize_raf_closure {
            slot.borrow_mut().take();
        }
        self._resize_raf_closure = None;

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
        // Keep the ResizeObserver callback alive until final drop so any
        // already-queued observer notifications do not call into a dropped
        // wasm closure. Observer has already been disconnected above.

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

#[cfg(test)]
mod tests {
    use super::{
        build_historical_footprint_dataset_from_arrays, parse_historical_footprint_json_dataset,
        parse_main_viewport_preset, reset_main_viewport_and_emit, resolve_synced_crosshair_state,
    };
    use raycore::core::events::ChartEvent;
    use raycore::{Bar, ChartEngine, MainViewportPreset, RendererBackend, Viewport};

    fn sample_bars(count: usize, start_ts: u64) -> Vec<Bar> {
        (0..count)
            .map(|i| Bar {
                timestamp: start_ts + i as u64,
                open: 100.0 + i as f32 * 0.1,
                high: 101.0 + i as f32 * 0.1,
                low: 99.0 + i as f32 * 0.1,
                close: 100.5 + i as f32 * 0.1,
                volume: 1.0,
                _pad: 0.0,
            })
            .collect()
    }

    #[test]
    fn synced_crosshair_uses_bar_and_price_projection() {
        let mut viewport = Viewport::new(1000, 600);
        viewport.set_range(10.0, 110.0);
        viewport.price_min = 10.0;
        viewport.price_max = 110.0;
        let pane_w = 1000.0;
        let pane_h = 600.0;

        let (x, y, bar_index, price) = resolve_synced_crosshair_state(
            &viewport,
            pane_w,
            pane_h,
            0.0,
            0.0,
            Some(59),
            Some(80.0),
        );

        assert_eq!(bar_index, Some(59));
        assert!((price - 80.0).abs() < f64::EPSILON);
        assert!((x - viewport.bar_center_css(59, pane_w)).abs() < 0.001);
        assert!((y - viewport.price_to_css_y(80.0, pane_h)).abs() < 0.001);
    }

    #[test]
    fn synced_crosshair_falls_back_to_pixel_inference() {
        let mut viewport = Viewport::new(800, 400);
        viewport.set_range(0.0, 100.0);
        viewport.price_min = 0.0;
        viewport.price_max = 100.0;

        let fallback_x = 400.0;
        let fallback_y = 120.0;
        let (x, y, bar_index, price) = resolve_synced_crosshair_state(
            &viewport, 800.0, 400.0, fallback_x, fallback_y, None, None,
        );

        assert_eq!(x, fallback_x);
        assert_eq!(y, fallback_y);
        assert!(bar_index.is_some());
        assert!(price.is_finite());
    }

    #[test]
    fn parse_main_viewport_preset_defaults_to_default_recent() {
        assert_eq!(
            parse_main_viewport_preset(None),
            MainViewportPreset::DefaultRecent
        );
        assert_eq!(
            parse_main_viewport_preset(Some("default")),
            MainViewportPreset::DefaultRecent
        );
        assert_eq!(
            parse_main_viewport_preset(Some("fit_all")),
            MainViewportPreset::FitAll
        );
    }

    #[test]
    fn reset_main_viewport_and_emit_updates_range_and_emits_visible_range_change() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        engine.set_data(sample_bars(120, 1_000)).unwrap();
        engine.event_bus.clear();

        reset_main_viewport_and_emit(&mut engine, Some("default"));

        let events: Vec<_> = engine.event_bus.drain().collect();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChartEvent::VisibleRangeChange { start_bar, end_bar } => {
                assert!(*end_bar > engine.bars.len() as f64);
                assert!(*end_bar > *start_bar);
            }
            other => panic!("expected VisibleRangeChange, got {other:?}"),
        }
    }

    #[test]
    fn reset_main_viewport_and_emit_fit_all_matches_engine_fit_all_range() {
        let mut engine = ChartEngine::new(RendererBackend::Noop, 800, 400, 1.0);
        engine.set_data(sample_bars(120, 2_000)).unwrap();
        engine.event_bus.clear();

        reset_main_viewport_and_emit(&mut engine, Some("fit_all"));

        let expected_end = engine.bars.len() as f64 + (engine.bars.len() as f64 * 0.05).max(2.0);
        assert!((engine.viewport.start_bar - 0.0).abs() < 1e-9);
        assert!((engine.viewport.end_bar - expected_end).abs() < 1e-9);

        let events: Vec<_> = engine.event_bus.drain().collect();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChartEvent::VisibleRangeChange { start_bar, end_bar } => {
                assert!((*start_bar - 0.0).abs() < 1e-9);
                assert!((*end_bar - expected_end).abs() < 1e-9);
            }
            other => panic!("expected VisibleRangeChange, got {other:?}"),
        }
    }

    #[test]
    fn build_historical_footprint_dataset_from_arrays_round_trips() {
        let open = [100.0, 101.0];
        let high = [101.0, 102.0];
        let low = [99.0, 100.5];
        let close = [100.5, 101.5];
        let volume = [500.0, 700.0];
        let timestamps = [1_000_u64, 2_000_u64];
        let level_offsets = [0_u32, 2, 4];
        let prices = [99.0, 100.0, 100.5, 101.5];
        let bid = [120.0, 80.0, 90.0, 60.0];
        let ask = [110.0, 140.0, 100.0, 150.0];

        let (bars, footprint) = build_historical_footprint_dataset_from_arrays(
            "test",
            &open,
            &high,
            &low,
            &close,
            &volume,
            &timestamps,
            &level_offsets,
            &prices,
            &bid,
            &ask,
        )
        .unwrap();

        assert_eq!(bars.len(), 2);
        assert_eq!(footprint.len(), 2);
        let first = footprint.get_bar(0).unwrap();
        assert_eq!(first.levels.len(), 2);
        assert_eq!(first.levels[0].price, 99.0);
        assert_eq!(first.levels[1].ask_volume, 140.0);
    }

    #[test]
    fn build_historical_footprint_dataset_from_arrays_rejects_misaligned_bar_range() {
        let open = [100.0];
        let high = [101.0];
        let low = [99.0];
        let close = [100.5];
        let volume = [500.0];
        let timestamps = [1_000_u64];
        let level_offsets = [0_u32, 1];
        let prices = [105.0];
        let bid = [120.0];
        let ask = [110.0];

        let err = build_historical_footprint_dataset_from_arrays(
            "test",
            &open,
            &high,
            &low,
            &close,
            &volume,
            &timestamps,
            &level_offsets,
            &prices,
            &bid,
            &ask,
        )
        .unwrap_err();

        assert!(
            err.as_string()
                .unwrap_or_default()
                .contains("does not cover OHLC range"),
            "expected alignment error, got {:?}",
            err
        );
    }

    #[test]
    fn parse_historical_footprint_json_dataset_round_trips() {
        let json = r#"{
            "bars": [
                {
                    "timestamp": 1000,
                    "open": 100.0,
                    "high": 101.0,
                    "low": 99.0,
                    "close": 100.5,
                    "volume": 500.0,
                    "levels": [
                        {"price": 99.0, "bid": 120.0, "ask": 80.0},
                        {"price": 100.0, "bidVolume": 90.0, "askVolume": 140.0}
                    ]
                },
                {
                    "timestamp": 2000,
                    "open": 101.0,
                    "high": 102.0,
                    "low": 100.5,
                    "close": 101.5,
                    "volume": 700.0,
                    "levels": []
                }
            ]
        }"#;

        let (bars, footprint) = parse_historical_footprint_json_dataset(json).unwrap();

        assert_eq!(bars.len(), 2);
        assert_eq!(bars[1].timestamp, 2_000);
        assert_eq!(footprint.len(), 1);
        assert!(footprint.get_bar(0).is_some());
        assert!(footprint.get_bar(1).is_none());
    }
}
