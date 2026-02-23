//! Renderer subsystem — unified geometry + dumb renderers.
//!
//! Architecture:
//! - transforms.rs: coordinate conversion utilities (bar↔x, price↔y)
//! - draw_list.rs: ColoredRect, ColoredLine, DrawText structs
//! - tick_marks.rs: shared tick computation (single source of truth)
//! - geometry_generator.rs: single source of truth for candle/volume visual math
//! - canvas2d.rs: dumb DrawList consumer (Canvas2D)
//! - wgpu_backend.rs: dumb DrawList consumer (WebGPU instanced quads)
//! - pipeline_manager.rs: single rect pipeline
//! - wgpu_context.rs: GPU device/surface management
//! - price_axis.rs: dedicated PriceAxisRenderer
//! - time_axis.rs: dedicated TimeAxisRenderer
//! - overlay.rs: crosshair lines + watermark on pane top canvas

pub mod theme;
pub mod traits;
pub mod transforms;

/// Shared Canvas2D color helper — converts `[f32; 4]` RGBA to CSS string.
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn rgba_str(c: &[f32; 4]) -> String {
    format!(
        "rgba({},{},{},{})",
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        c[3]
    )
}
pub mod draw_list;
pub mod geometry_generator;
pub mod line_generator;
pub mod pipeline_manager;
pub mod series;
pub mod tick_marks;
pub mod wgpu_backend;
pub mod wgpu_context;

#[cfg(target_arch = "wasm32")]
pub mod canvas2d;

#[cfg(target_arch = "wasm32")]
pub mod overlay;

#[cfg(target_arch = "wasm32")]
pub mod text_cache;

#[cfg(target_arch = "wasm32")]
pub mod price_axis;

#[cfg(target_arch = "wasm32")]
pub mod time_axis;

pub mod value_projection;
