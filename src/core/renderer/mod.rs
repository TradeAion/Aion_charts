//! Renderer subsystem — trait, backends, GPU context, pipelines, and composable series.

pub mod traits;
pub mod series;
pub mod wgpu_context;
pub mod pipeline_manager;
pub mod wgpu_backend;

#[cfg(target_arch = "wasm32")]
pub mod canvas2d;

#[cfg(target_arch = "wasm32")]
pub mod overlay;

#[cfg(target_arch = "wasm32")]
pub mod grid;

#[cfg(target_arch = "wasm32")]
pub mod candle_series;

#[cfg(target_arch = "wasm32")]
pub mod volume_series;

// Phase 1 sub-renderers (WebGPU-specific standalone components).
pub mod candle_renderer;
pub mod volume_renderer;
pub mod study_renderer;
