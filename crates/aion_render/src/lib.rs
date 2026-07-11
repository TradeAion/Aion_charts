//! aion_render — backend-agnostic draw-list IR and rendering math.
//!
//! Views in `aion_core` (once wired) emit [`draw_list::DrawList`]s; backends
//! (`aion_render_wgpu`, and later a Canvas2D fallback executor) consume them.
//! Pixel math is specified in `docs/RENDERING_SPEC.md`.

pub mod bar_width;
pub mod bars;
pub mod candles;
pub mod color;
pub mod draw_list;
pub mod histogram;
pub mod line;
