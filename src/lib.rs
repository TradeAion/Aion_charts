//! RayCore — high-performance charting engine.
//!
//! Widget-based architecture matching LWC:
//! - Separate DOM elements for pane, price axis, time axis
//! - Per-widget canvases (base + top layers)
//! - Shared tick computation, per-widget rendering

pub mod core;

// Re-export key public types at crate root.
pub use crate::core::data::{Bar, BarArray};
pub use crate::core::viewport::Viewport;
pub use crate::core::engine::ChartEngine;
pub use crate::core::renderer::traits::{
    ChartRenderer, Renderer, RendererBackend, RenderContext, ChartStyle,
    CrosshairState, CrosshairMode, TickMark,
};
pub use crate::core::renderer::series::CandleSizing;
pub use crate::core::renderer::draw_list::{DrawList, ColoredRect};
pub use crate::core::renderer::geometry_generator;
pub use crate::core::renderer::tick_marks;
pub use crate::core::renderer::wgpu_context::GpuContext;
pub use crate::core::renderer::wgpu_backend::{WgpuRenderer, CandleInstance, CandleUniforms};
pub use crate::core::renderer::pipeline_manager::PipelineManager;
pub use crate::core::interaction::{InteractionHandler, HitZone, TouchCrosshairMode};
pub use crate::core::drawings::DrawingManager;
pub use crate::core::drawings::types::DrawingTool;
pub use crate::core::demo_data::generate_sample_data;
pub use crate::core::series::{
    SeriesId, SeriesCollection, Series, SeriesType,
    LinePoint, LineDataArray, LineSeriesOptions, LineStyle,
};

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::canvas2d::Canvas2DRenderer;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::overlay::OverlayRenderer;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::price_axis::PriceAxisRenderer;

#[cfg(target_arch = "wasm32")]
pub use crate::core::renderer::time_axis::TimeAxisRenderer;
