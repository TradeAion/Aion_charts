//! aion_render_wgpu — WebGPU backend.
//!
//! Phase 0 scope: the solid-quad pipeline (instanced integer rects). This alone renders
//! candles, wicks, histograms, grid lines and the crosshair — everything that Canvas2D
//! `fillRect` covers in lightweight-charts. Polylines, round-rects and glyphs follow.

mod quad_executor;
mod quad_pipeline;

pub use quad_executor::prims_to_instances;
pub use quad_pipeline::{QuadInstance, QuadRenderer};

pub use aion_render::draw_list::DrawList;
