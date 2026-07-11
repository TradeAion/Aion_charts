//! aion_render_wgpu — WebGPU backend.
//!
//! Pipelines: solid quads (integer rects — candles, wicks, grid, crosshair) and textured
//! quads (label atlas). Frames are composed as scissored draw groups, replicating LWC's
//! pane/axis canvas separation. Polylines and SDF round-rects follow.

mod atlas;
mod frame;
mod quad_executor;
mod quad_pipeline;
mod tex_quad_pipeline;

pub use atlas::{AtlasSlot, LabelAtlas, ATLAS_SIZE};
pub use frame::{render_frame, DrawGroup};
pub use quad_executor::prims_to_instances;
pub use quad_pipeline::{QuadInstance, QuadRenderer};
pub use tex_quad_pipeline::{TexQuadInstance, TexQuadRenderer};

pub use aion_render::draw_list::DrawList;
