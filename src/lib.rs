//! RayCore — high-performance GPU-accelerated charting engine.
//!
//! This is the core library crate. It is target-agnostic: all wgpu types
//! are created from caller-supplied device/queue/surface so the same code
//! compiles for native (Tauri) and wasm32 (browser WebGPU).

pub mod core;

// Re-export key public types at crate root for ergonomics.
pub use crate::core::data::{Bar, BarArray};
pub use crate::core::viewport::Viewport;
pub use crate::core::renderer::wgpu_context::GpuContext;
pub use crate::core::renderer::candle_renderer::CandleRenderer;
pub use crate::core::renderer::volume_renderer::VolumeRenderer;
pub use crate::core::renderer::pipeline_manager::PipelineManager;
pub use crate::core::engine::ChartEngine;
