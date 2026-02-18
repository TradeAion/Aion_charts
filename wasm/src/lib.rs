//! RayCore WASM bindings — clean public API.
//!
//! Architecture:
//!   - CanvasManager creates the 3-canvas stack (grid, chart, overlay)
//!   - InteractionHandler processes pointer/wheel events (pure state machine)
//!   - ChartEngine owns viewport, data, style, renders via Canvas2D or WebGPU
//!   - GridRenderer / OverlayRenderer draw axes, crosshair, watermark
//!
//! Public WASM API:
//!   RayCore.create("container-id")      → sets up everything
//!   core.pointer_move(x, y)             → crosshair + drag
//!   core.pointer_down(x, y)             → start drag
//!   core.pointer_up()                   → end drag
//!   core.pointer_leave()                → hide crosshair
//!   core.wheel(x, y, delta_y)           → zoom
//!   core.resize()                       → handle container resize
//!   core.demo_mode()                    → load sample data
//!   core.render()                       → draw one frame (call from RAF)

use wasm_bindgen::prelude::*;
use raycore::{
    Bar, ChartEngine, ChartLayout,
    GpuContext, WgpuRenderer, Canvas2DRenderer,
    RendererBackend, OverlayRenderer, GridRenderer,
    InteractionHandler, generate_sample_data,
    geometry_generator,
};

mod canvas_manager;
use canvas_manager::CanvasManager;

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

#[wasm_bindgen]
pub struct RayCore {
    engine: ChartEngine,
    grid: GridRenderer,
    overlay: OverlayRenderer,
    canvas_mgr: CanvasManager,
    interaction: InteractionHandler,
    /// Cached Y-axis width in CSS px (updated each frame).
    y_axis_css_w: f64,
}

#[wasm_bindgen]
impl RayCore {
    /// Create a new RayCore instance inside a container div.
    /// Internally creates the 3-canvas stack and initializes rendering.
    pub async fn create(container_id: &str) -> Result<RayCore, JsValue> {
        let preferred = if webgpu_available() { "webgpu" } else { "canvas2d" };
        Self::create_with(container_id, preferred).await
    }

    /// Create with a specific renderer backend ("webgpu" or "canvas2d").
    pub async fn create_with(container_id: &str, renderer: &str) -> Result<RayCore, JsValue> {
        init_logging();

        let canvas_mgr = CanvasManager::new(container_id)?;
        let dpr = get_dpr();

        let (css_w, css_h) = canvas_mgr.css_size();
        let phys_w = (css_w * dpr).round() as u32;
        let phys_h = (css_h * dpr).round() as u32;
        canvas_mgr.set_physical_size(phys_w, phys_h);

        log::info!(
            "RayCore: creating '{}' — CSS {}x{}, physical {}x{}, dpr={}",
            renderer, css_w, css_h, phys_w, phys_h, dpr
        );

        let backend = match renderer {
            "webgpu" => {
                match GpuContext::new(
                    wgpu::SurfaceTarget::Canvas(canvas_mgr.chart_canvas.clone()),
                    phys_w.max(1),
                    phys_h.max(1),
                ).await {
                    Ok(gpu) => {
                        log::info!("WebGPU adapter: {:?}", gpu.format);
                        RendererBackend::Wgpu(WgpuRenderer::new(gpu))
                    }
                    Err(e) => {
                        log::warn!("WebGPU unavailable: {}. Falling back to Canvas2D.", e);
                        let r = Canvas2DRenderer::new(canvas_mgr.chart_canvas.clone(), dpr)
                            .map_err(|e| JsValue::from_str(&e))?;
                        RendererBackend::Canvas2D(r)
                    }
                }
            }
            _ => {
                let r = Canvas2DRenderer::new(canvas_mgr.chart_canvas.clone(), dpr)
                    .map_err(|e| JsValue::from_str(&e))?;
                RendererBackend::Canvas2D(r)
            }
        };

        let grid = GridRenderer::new(canvas_mgr.grid_canvas.clone(), dpr)
            .map_err(|e| JsValue::from_str(&e))?;
        let overlay = OverlayRenderer::new(canvas_mgr.overlay_canvas.clone(), dpr)
            .map_err(|e| JsValue::from_str(&e))?;

        let engine = ChartEngine::new(backend, phys_w.max(1), phys_h.max(1), dpr);

        let mut interaction = InteractionHandler::new();
        interaction.set_container_size(css_w, css_h);

        log::info!("RayCore initialized: {}", engine.renderer_name());

        Ok(RayCore {
            engine,
            grid,
            overlay,
            canvas_mgr,
            interaction,
            y_axis_css_w: 0.0,
        })
    }

    /// Get the active renderer name ("webgpu" or "canvas2d").
    pub fn renderer_name(&self) -> String {
        self.engine.renderer_name().to_string()
    }

    /// Get supported renderers on this platform.
    pub fn get_supported_renderers() -> js_sys::Array {
        let arr = js_sys::Array::new();
        arr.push(&JsValue::from_str("canvas2d"));
        if webgpu_available() {
            arr.push(&JsValue::from_str("webgpu"));
        }
        arr
    }

    // ── Data loading ─────────────────────────────────────────────────────────

    /// Load bar data from separate typed arrays.
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
        log::info!("set_data_arrays: {} bars", count);
    }

    /// Load bar data from packed f32 array (8 floats per bar).
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
        log::info!("set_data: {} bars", n);
    }

    // ── Demo mode ────────────────────────────────────────────────────────────

    /// Load built-in sample data for demo/testing.
    /// Generates 600 bars of realistic candlestick data.
    pub fn demo_mode(&mut self) {
        let now_ms = js_sys::Date::now() as u64;
        let num_bars = 600;
        let interval_ms = 60_000; // 1-minute bars
        let start_ms = now_ms - (num_bars as u64) * interval_ms;

        let bars = generate_sample_data(num_bars, start_ms, interval_ms);
        self.engine.set_data(bars);
        log::info!("demo_mode: {} bars loaded", num_bars);
    }

    // ── Interaction (pointer/wheel forwarded from JS) ────────────────────────

    /// Pointer move — CSS px relative to container.
    pub fn pointer_move(&mut self, x: f64, y: f64) {
        let y_axis_w = if self.y_axis_css_w > 0.0 { self.y_axis_css_w } else { 34.0 };
        self.interaction.pointer_move(
            x, y,
            &mut self.engine.viewport,
            &mut self.engine.crosshair,
            self.engine.bars.as_slice(),
            &self.engine.style,
            self.engine.dpr,
            y_axis_w,
        );
    }

    /// Pointer down — CSS px relative to container.
    pub fn pointer_down(&mut self, x: f64, _y: f64) {
        self.interaction.pointer_down(x, _y);
    }

    /// Pointer up.
    pub fn pointer_up(&mut self) {
        self.interaction.pointer_up();
    }

    /// Pointer leave — hide crosshair.
    pub fn pointer_leave(&mut self) {
        self.interaction.pointer_leave(&mut self.engine.crosshair);
    }

    /// Wheel event — zoom with focal point.
    pub fn wheel(&mut self, x: f64, y: f64, delta_y: f64) {
        let y_axis_w = if self.y_axis_css_w > 0.0 { self.y_axis_css_w } else { 34.0 };
        self.interaction.wheel(
            x, y, delta_y,
            &mut self.engine.viewport,
            self.engine.bars.as_slice(),
            &self.engine.style,
            self.engine.dpr,
            y_axis_w,
        );
    }

    // ── Layout ───────────────────────────────────────────────────────────────

    /// Handle container resize. Call from JS ResizeObserver.
    pub fn resize(&mut self) {
        let (css_w, css_h) = self.canvas_mgr.css_size();
        let dpr = get_dpr();
        let phys_w = (css_w * dpr).round() as u32;
        let phys_h = (css_h * dpr).round() as u32;

        self.canvas_mgr.set_physical_size(phys_w, phys_h);
        self.grid.resize(phys_w.max(1), phys_h.max(1), dpr);
        self.overlay.resize(phys_w.max(1), phys_h.max(1), dpr);
        self.engine.resize(phys_w.max(1), phys_h.max(1), dpr);
        self.interaction.set_container_size(css_w, css_h);
    }

    /// Get the current Y-axis width in CSS pixels.
    pub fn y_axis_width(&self) -> f64 {
        self.y_axis_css_w
    }

    /// Get the current X-axis height in CSS pixels.
    pub fn x_axis_height(&self) -> f64 {
        self.engine.style.time_axis_height()
    }

    // ── Viewport control ─────────────────────────────────────────────────────

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

    /// Is the user currently dragging?
    pub fn is_dragging(&self) -> bool {
        self.interaction.is_dragging()
    }

    // ── Render ───────────────────────────────────────────────────────────────

    /// Render one frame. Call from requestAnimationFrame.
    ///
    /// Internally:
    /// 1. Auto-fit price range
    /// 2. Compute ticks + measure text → dynamic Y-axis width
    /// 3. Grid canvas — grid lines
    /// 4. Chart canvas — candles + volume
    /// 5. Overlay canvas — axes, crosshair, watermark
    pub fn render(&mut self) {
        if !self.engine.viewport.price_locked {
            self.engine.viewport.auto_fit_price(self.engine.bars.as_slice());
        }

        let dpr = self.engine.dpr;
        let style = &self.engine.style;

        // Step 1: preliminary layout with previous frame's y_axis_css_w
        let prelim_layout = ChartLayout::from_physical(
            self.engine.viewport.width,
            self.engine.viewport.height,
            dpr,
            style,
            if self.y_axis_css_w > 0.0 { self.y_axis_css_w } else { 34.0 },
        );

        // Step 2: compute ticks to measure text widths
        let (_, prelim_y_ticks, _) = geometry_generator::generate(
            self.engine.bars.as_slice(),
            &self.engine.viewport,
            style,
            &prelim_layout,
        );

        // Step 3: measure max Y-axis label width → compute dynamic axis width
        let max_text_w_phys = self.overlay.measure_max_tick_width(style, &prelim_y_ticks);
        let max_text_w_css = max_text_w_phys / dpr;
        let y_axis_css_w = style.price_axis_width(max_text_w_css);
        self.y_axis_css_w = y_axis_css_w;

        // Step 4: final layout with measured y-axis width
        let layout = ChartLayout::from_physical(
            self.engine.viewport.width,
            self.engine.viewport.height,
            dpr,
            style,
            y_axis_css_w,
        );

        // Step 5: grid canvas (background grid lines + ticks)
        let (y_ticks, x_ticks) = self.grid.render(
            self.engine.bars.as_slice(),
            &self.engine.viewport,
            style,
            &layout,
        );

        // Step 6: main chart canvas (candles + volume)
        self.engine.y_axis_css_w = y_axis_css_w;
        if let Err(e) = self.engine.render() {
            log::warn!("render error: {}", e);
        }

        // Step 7: overlay (axes, crosshair, watermark)
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
}
