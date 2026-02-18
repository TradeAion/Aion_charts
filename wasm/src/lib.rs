//! WASM entry point — thin JS interop layer for RayCore.
//!
//! This crate re-exports the core engine behind `#[wasm_bindgen]` functions.
//! All heavy logic lives in the `raycore` crate; this is just the bridge.

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use raycore::{Bar, ChartEngine, GpuContext};

/// Initialize panic hook and logger (called once).
fn init_logging() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
}

/// The JS-facing handle to the chart engine.
#[wasm_bindgen]
pub struct RayCore {
    engine: ChartEngine,
}

#[wasm_bindgen]
impl RayCore {
    /// Create a new RayCore instance attached to a `<canvas>` element by ID.
    ///
    /// This is async because WebGPU adapter/device requests are async.
    /// Call from JS: `const core = await RayCore.create("my-canvas");`
    pub async fn create(canvas_id: &str) -> Result<RayCore, JsValue> {
        init_logging();

        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| format!("canvas '{}' not found", canvas_id))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "element is not a canvas")?;

        let width = canvas.client_width() as u32;
        let height = canvas.client_height() as u32;

        // wgpu 28 accepts HtmlCanvasElement as a SurfaceTarget
        let gpu = GpuContext::new(
            wgpu::SurfaceTarget::Canvas(canvas),
            width.max(1),
            height.max(1),
        )
        .await;

        let engine = ChartEngine::new(gpu, width, height);

        log::info!(
            "RayCore initialized: {}x{}, format={:?}",
            width,
            height,
            engine.gpu.format
        );

        Ok(RayCore { engine })
    }

    /// Resize the rendering surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.engine.resize(width, height);
    }

    /// Set bar data from a Float32Array.
    ///
    /// The array must be a flat sequence of [timestamp_lo, timestamp_hi, open, high, low, close, volume, _pad]
    /// where timestamp is split into two f32 values (lo and hi 32 bits of u64).
    ///
    /// For simplicity in Phase 1, we accept a flat f32 array with 8 floats per bar:
    ///   [index_as_f32, 0.0, open, high, low, close, volume, 0.0]
    /// The timestamp field is reconstructed from the index.
    pub fn set_data(&mut self, data: &[f32]) {
        const FLOATS_PER_BAR: usize = 8;
        if data.len() % FLOATS_PER_BAR != 0 {
            log::error!("set_data: array length must be multiple of 8");
            return;
        }

        let bar_count = data.len() / FLOATS_PER_BAR;
        let mut bars = Vec::with_capacity(bar_count);

        for i in 0..bar_count {
            let base = i * FLOATS_PER_BAR;
            // Reconstruct timestamp from first two f32s (lo + hi bits of u64)
            let ts_lo = data[base] as u32;
            let ts_hi = data[base + 1] as u32;
            let timestamp = ((ts_hi as u64) << 32) | (ts_lo as u64);

            bars.push(Bar {
                timestamp,
                open: data[base + 2],
                high: data[base + 3],
                low: data[base + 4],
                close: data[base + 5],
                volume: data[base + 6],
                _pad: 0.0,
            });
        }

        self.engine.set_data(bars);
        log::info!("set_data: loaded {} bars", bar_count);
    }

    /// Convenience: set bar data from simple arrays (easier from JS).
    /// Each array has `count` elements.
    pub fn set_data_arrays(
        &mut self,
        open: &[f32],
        high: &[f32],
        low: &[f32],
        close: &[f32],
        volume: &[f32],
    ) {
        let count = open.len().min(high.len()).min(low.len()).min(close.len()).min(volume.len());
        let mut bars = Vec::with_capacity(count);
        for i in 0..count {
            bars.push(Bar {
                timestamp: i as u64,
                open: open[i],
                high: high[i],
                low: low[i],
                close: close[i],
                volume: volume[i],
                _pad: 0.0,
            });
        }
        self.engine.set_data(bars);
        log::info!("set_data_arrays: loaded {} bars", count);
    }

    /// Zoom to a specific bar range.
    pub fn zoom_to_range(&mut self, start: u64, end: u64) {
        self.engine.zoom_to_range(start, end);
    }

    /// Render one frame.
    pub fn render(&mut self) {
        match self.engine.render() {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let w = self.engine.gpu.config.width;
                let h = self.engine.gpu.config.height;
                self.engine.gpu.resize(w, h);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Out of GPU memory!");
            }
            Err(e) => {
                log::warn!("Surface error: {:?}", e);
            }
        }
    }

    /// Get the current visible bar range as [start, end].
    pub fn visible_range(&self) -> Vec<f64> {
        vec![
            self.engine.viewport.start_bar,
            self.engine.viewport.end_bar,
        ]
    }

    /// Pan by a number of bars.
    pub fn pan(&mut self, delta_bars: f64) {
        self.engine.viewport.pan(delta_bars);
    }

    /// Zoom around a focal bar. factor > 1 = zoom out, < 1 = zoom in.
    pub fn zoom(&mut self, focal_bar: f64, factor: f64) {
        self.engine.viewport.zoom(focal_bar, factor);
    }
}
