//! Renderer subsystem — unified geometry + dumb renderers.
//!
//! Architecture:
//! - draw_list.rs: ColoredRect, ColoredLine, DrawText structs
//! - geometry_generator.rs: single source of truth for all visual math
//! - canvas2d.rs: dumb DrawList consumer (Canvas2D)
//! - wgpu_backend.rs: dumb DrawList consumer (WebGPU instanced quads)
//! - pipeline_manager.rs: single rect pipeline
//! - wgpu_context.rs: GPU device/surface management

pub mod traits;
pub mod series;
pub mod draw_list;
pub mod geometry_generator;
pub mod wgpu_context;
pub mod pipeline_manager;
pub mod wgpu_backend;

#[cfg(target_arch = "wasm32")]
pub mod canvas2d;

#[cfg(target_arch = "wasm32")]
pub mod overlay;

#[cfg(target_arch = "wasm32")]
pub mod grid;
