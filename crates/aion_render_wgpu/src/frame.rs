//! Frame composition: one MSAA render pass drawing scissored groups of triangle meshes
//! (area fills, then line strokes), solid quads, then textured quads (text).
//!
//! Groups replicate LWC's pane/axis canvas separation: the pane group is scissored to the
//! pane rect. 4x MSAA smooths diagonal line edges while leaving pixel-aligned rects and
//! texture-alpha text bit-identical (their edges never straddle a pixel boundary), so
//! candles and labels stay crisp.

use wgpu::util::DeviceExt;

use crate::quad_pipeline::{QuadInstance, QuadRenderer};
use crate::tex_quad_pipeline::{TexQuadInstance, TexQuadRenderer};
use crate::tri_pipeline::{TriRenderer, TriVertex};

pub const SAMPLE_COUNT: u32 = 4;

#[derive(Default)]
pub struct DrawGroup {
    /// x, y, w, h in bitmap px; None = full target.
    pub scissor: Option<[u32; 4]>,
    /// Background rects drawn *below* the series (grid lines). Kept separate from `quads` because
    /// the fixed bucket order draws all tris (area fills / line strokes) between them and the
    /// foreground quads — so grid must live in its own under-layer or it paints over line/area.
    pub under_quads: Vec<QuadInstance>,
    /// Drawn after the under-layer (area fills, below strokes).
    pub fill_tris: Vec<TriVertex>,
    /// Drawn after fills (line strokes).
    pub stroke_tris: Vec<TriVertex>,
    /// Foreground rects drawn *above* the series (candles, price lines, last-value, crosshair).
    pub quads: Vec<QuadInstance>,
    pub tex_quads: Vec<TexQuadInstance>,
}

/// The MSAA color target; recreated on resize.
pub struct MsaaTarget {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl MsaaTarget {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("msaa_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            view,
            width,
            height,
        }
    }

    /// Recreates the target if the size changed.
    pub fn ensure(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        if self.width != width || self.height != height {
            *self = Self::new(device, format, width, height);
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

struct GroupBuffers {
    under_quad: Option<(wgpu::Buffer, u32)>,
    fill: Option<(wgpu::Buffer, u32)>,
    stroke: Option<(wgpu::Buffer, u32)>,
    quad: Option<(wgpu::Buffer, u32)>,
    tex: Option<(wgpu::Buffer, u32)>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    msaa_view: &wgpu::TextureView,
    resolve_view: &wgpu::TextureView,
    width_px: u32,
    height_px: u32,
    clear_color: wgpu::Color,
    quad: &QuadRenderer,
    tex: &TexQuadRenderer,
    tri: &TriRenderer,
    groups: &[DrawGroup],
) {
    quad.write_globals(queue, width_px, height_px);
    tex.write_globals(queue, width_px, height_px);
    tri.write_globals(queue, width_px, height_px);

    let vbuf = |contents: &[u8], label| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: wgpu::BufferUsages::VERTEX,
        })
    };

    // buffers must outlive the pass
    let buffers: Vec<GroupBuffers> = groups
        .iter()
        .map(|g| GroupBuffers {
            under_quad: (!g.under_quads.is_empty()).then(|| {
                (
                    vbuf(bytemuck::cast_slice(&g.under_quads), "under_quads"),
                    g.under_quads.len() as u32,
                )
            }),
            fill: (!g.fill_tris.is_empty()).then(|| {
                (
                    vbuf(bytemuck::cast_slice(&g.fill_tris), "fill"),
                    g.fill_tris.len() as u32,
                )
            }),
            stroke: (!g.stroke_tris.is_empty()).then(|| {
                (
                    vbuf(bytemuck::cast_slice(&g.stroke_tris), "stroke"),
                    g.stroke_tris.len() as u32,
                )
            }),
            quad: (!g.quads.is_empty()).then(|| {
                (
                    vbuf(bytemuck::cast_slice(&g.quads), "quads"),
                    g.quads.len() as u32,
                )
            }),
            tex: (!g.tex_quads.is_empty()).then(|| {
                (
                    vbuf(bytemuck::cast_slice(&g.tex_quads), "tex"),
                    g.tex_quads.len() as u32,
                )
            }),
        })
        .collect();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("frame"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: msaa_view,
                resolve_target: Some(resolve_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    // resolve target holds the result; MSAA buffer itself can be discarded
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        for (group, bufs) in groups.iter().zip(&buffers) {
            let [sx, sy, sw, sh] = match group.scissor {
                Some([x, y, w, h]) => {
                    let x = x.min(width_px);
                    let y = y.min(height_px);
                    [x, y, w.min(width_px - x), h.min(height_px - y)]
                }
                None => [0, 0, width_px, height_px],
            };
            if sw == 0 || sh == 0 {
                continue;
            }
            pass.set_scissor_rect(sx, sy, sw, sh);

            if let Some((b, n)) = &bufs.under_quad {
                quad.draw(&mut pass, b, *n);
            }
            if let Some((b, n)) = &bufs.fill {
                tri.draw(&mut pass, b, *n);
            }
            if let Some((b, n)) = &bufs.stroke {
                tri.draw(&mut pass, b, *n);
            }
            if let Some((b, n)) = &bufs.quad {
                quad.draw(&mut pass, b, *n);
            }
            if let Some((b, n)) = &bufs.tex {
                tex.draw(&mut pass, b, *n);
            }
        }
    }

    queue.submit(Some(encoder.finish()));
}
