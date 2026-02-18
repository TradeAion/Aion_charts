//! RayCore WASM bindings — JS interop layer.
//!
//! 3-canvas architecture (like LWC):
//!   - grid canvas    (z-index:0) — grid lines behind candles (Canvas2D always)
//!   - main canvas    (z-index:1) — candles + volume (WebGPU or Canvas2D)
//!   - overlay canvas (z-index:2) — axes, crosshair, watermark (Canvas2D always)

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use raycore::{
    Bar, ChartEngine, ChartLayout,
    GpuContext, WgpuRenderer, Canvas2DRenderer,
    RendererBackend, OverlayRenderer, GridRenderer,
    geometry_generator,
};

fn init_logging() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
}

fn get_canvas(canvas_id: &str) -> Result<HtmlCanvasElement, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;
    document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("canvas '{}' not found", canvas_id)))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("element is not a canvas"))
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

#[wasm_bindgen]
pub struct RayCore {
    engine: ChartEngine,
    grid: GridRenderer,
    overlay: OverlayRenderer,
    canvas_id: String,
    #[allow(dead_code)]
    grid_id: String,
    #[allow(dead_code)]
    overlay_id: String,
}

#[wasm_bindgen]
impl RayCore {
    pub async fn create(grid_id: &str, canvas_id: &str, overlay_id: &str) -> Result<RayCore, JsValue> {
        let preferred = if webgpu_available() { "webgpu" } else { "canvas2d" };
        Self::create_with(grid_id, canvas_id, overlay_id, preferred).await
    }

    pub async fn create_with(grid_id: &str, canvas_id: &str, overlay_id: &str, renderer: &str) -> Result<RayCore, JsValue> {
        init_logging();

        let grid_canvas = get_canvas(grid_id)?;
        let canvas = get_canvas(canvas_id)?;
        let overlay_canvas = get_canvas(overlay_id)?;
        let dpr = get_dpr();

        let css_w = canvas.client_width() as f64;
        let css_h = canvas.client_height() as f64;
        let phys_w = (css_w * dpr).round() as u32;
        let phys_h = (css_h * dpr).round() as u32;

        for c in [&grid_canvas, &canvas, &overlay_canvas] {
            c.set_width(phys_w.max(1));
            c.set_height(phys_h.max(1));
        }

        log::info!(
            "RayCore: creating '{}' renderer — CSS {}x{}, physical {}x{}, dpr={}",
            renderer, css_w, css_h, phys_w, phys_h, dpr
        );

        let backend = match renderer {
            "webgpu" => {
                match GpuContext::new(
                    wgpu::SurfaceTarget::Canvas(canvas.clone()),
                    phys_w.max(1),
                    phys_h.max(1),
                )
                .await
                {
                    Ok(gpu) => {
                        log::info!("WebGPU adapter: {:?}", gpu.format);
                        RendererBackend::Wgpu(WgpuRenderer::new(gpu))
                    }
                    Err(e) => {
                        log::warn!("WebGPU unavailable: {}. Falling back to Canvas2D.", e);
                        let r = Canvas2DRenderer::new(canvas, dpr)
                            .map_err(|e| JsValue::from_str(&e))?;
                        RendererBackend::Canvas2D(r)
                    }
                }
            }
            _ => {
                let r = Canvas2DRenderer::new(canvas, dpr)
                    .map_err(|e| JsValue::from_str(&e))?;
                RendererBackend::Canvas2D(r)
            }
        };

        let grid = GridRenderer::new(grid_canvas, dpr)
            .map_err(|e| JsValue::from_str(&e))?;
        let overlay = OverlayRenderer::new(overlay_canvas, dpr)
            .map_err(|e| JsValue::from_str(&e))?;

        let engine = ChartEngine::new(backend, phys_w.max(1), phys_h.max(1), dpr);

        log::info!("RayCore initialized with renderer: {}", engine.renderer_name());

        Ok(RayCore {
            engine,
            grid,
            overlay,
            canvas_id: canvas_id.to_string(),
            grid_id: grid_id.to_string(),
            overlay_id: overlay_id.to_string(),
        })
    }

    pub fn renderer_name(&self) -> String {
        self.engine.renderer_name().to_string()
    }

    pub fn get_supported_renderers() -> js_sys::Array {
        let arr = js_sys::Array::new();
        arr.push(&JsValue::from_str("canvas2d"));
        if webgpu_available() {
            arr.push(&JsValue::from_str("webgpu"));
        }
        arr
    }

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

        self.engine.set_data(bars);
        log::info!("set_data_arrays: {} bars loaded", count);
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
        self.engine.set_data(bars);
        log::info!("set_data: {} bars loaded", n);
    }

    pub fn resize(&mut self, css_width: f64, css_height: f64) {
        let dpr = self.engine.dpr.max(1.0);
        let phys_w = (css_width * dpr).round() as u32;
        let phys_h = (css_height * dpr).round() as u32;

        if let Some(canvas) = self.canvas() {
            canvas.set_width(phys_w.max(1));
            canvas.set_height(phys_h.max(1));
        }

        self.grid.resize(phys_w.max(1), phys_h.max(1), dpr);
        self.overlay.resize(phys_w.max(1), phys_h.max(1), dpr);
        self.engine.resize(phys_w.max(1), phys_h.max(1), dpr);
    }

    pub fn zoom_to_range(&mut self, start: u64, end: u64) {
        self.engine.zoom_to_range(start, end);
    }

    pub fn pan(&mut self, delta_bars: f64) {
        let data_len = self.engine.bars.len();
        self.engine.viewport.pan_clamped(delta_bars, data_len);
    }

    pub fn zoom(&mut self, focal_bar: f64, factor: f64) {
        self.engine.viewport.zoom(focal_bar, factor);
        if !self.engine.viewport.price_locked {
            self.engine.viewport.auto_fit_price(self.engine.bars.as_slice());
        }
    }

    pub fn visible_range(&self) -> Vec<f64> {
        vec![self.engine.viewport.start_bar, self.engine.viewport.end_bar]
    }

    pub fn set_crosshair(&mut self, x: f64, y: f64, active: bool) {
        self.engine.set_crosshair(x, y, active);
    }

    pub fn crosshair_info(&self) -> Vec<f64> {
        let ch = &self.engine.crosshair;
        vec![
            ch.x,
            ch.y,
            ch.price,
            ch.bar_index.map(|i| i as f64).unwrap_or(-1.0),
            if ch.active { 1.0 } else { 0.0 },
        ]
    }

    /// Render one frame:
    /// 1. Main canvas — bg + grid lines + candles + volume (unified DrawList)
    /// 2. Overlay canvas — axes, crosshair, watermark
    ///
    /// The grid canvas is no longer drawn to — the main canvas DrawList includes
    /// bg fill and grid lines, so no transparency/compositing issues arise.
    pub fn render(&mut self) {
        if !self.engine.viewport.price_locked {
            self.engine.viewport.auto_fit_price(self.engine.bars.as_slice());
        }

        let layout = ChartLayout::from_physical(
            self.engine.viewport.width,
            self.engine.viewport.height,
            self.engine.dpr,
            &self.engine.style,
        );

        // Main canvas render (bg + grid + candles + volume via DrawList)
        if let Err(e) = self.engine.render() {
            log::warn!("render error: {}", e);
        }

        // Compute ticks for overlay axis labels — same math as geometry_generator
        let (_, y_ticks, x_ticks) = geometry_generator::generate(
            self.engine.bars.as_slice(),
            &self.engine.viewport,
            &self.engine.style,
            &layout,
        );

        self.overlay.render(
            self.engine.bars.as_slice(),
            &self.engine.viewport,
            &self.engine.style,
            &self.engine.crosshair,
            &layout,
            &y_ticks,
            &x_ticks,
        );
    }

    fn canvas(&self) -> Option<HtmlCanvasElement> {
        get_canvas(&self.canvas_id).ok()
    }
}
