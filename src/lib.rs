//! RayCore — high-performance charting engine.
//!
//! Supports multiple rendering backends:
//! - WebGPU (wgpu 28) — primary, highest performance
//! - Canvas 2D — fallback for browsers without WebGPU

pub mod core;

// Re-export key public types at crate root.
pub use crate::core::data::{Bar, BarArray};
pub use crate::core::viewport::Viewport;
pub use crate::core::engine::ChartEngine;
pub use crate::core::renderer::traits::{
    Renderer, RendererBackend, RenderContext, ChartStyle, CrosshairState,
};
pub use crate::core::renderer::series::ChartLayout;
pub use crate::core::renderer::wgpu_context::GpuContext;
pub use crate::core::renderer::wgpu_backend::WgpuRenderer;
pub use crate::core::renderer::pipeline_manager::PipelineManager;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::canvas2d::Canvas2DRenderer;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::overlay::OverlayRenderer;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::grid::GridRenderer;
