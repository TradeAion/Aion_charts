//! RayCore — high-performance charting engine.
//!
//! Unified Geometry architecture: all visual math computed once in
//! geometry_generator.rs → DrawList consumed identically by Canvas2D
//! and WebGPU renderers. Pixel-perfect consistency guaranteed.

pub mod core;

// Re-export key public types at crate root.
pub use crate::core::data::{Bar, BarArray};
pub use crate::core::viewport::Viewport;
pub use crate::core::engine::ChartEngine;
pub use crate::core::renderer::traits::{
    Renderer, RendererBackend, RenderContext, ChartStyle, CrosshairState,
};
pub use crate::core::renderer::series::ChartLayout;
pub use crate::core::renderer::draw_list::{DrawList, ColoredRect};
pub use crate::core::renderer::geometry_generator;
pub use crate::core::renderer::wgpu_context::GpuContext;
pub use crate::core::renderer::wgpu_backend::WgpuRenderer;
pub use crate::core::renderer::pipeline_manager::PipelineManager;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::canvas2d::Canvas2DRenderer;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::overlay::OverlayRenderer;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::grid::GridRenderer;
