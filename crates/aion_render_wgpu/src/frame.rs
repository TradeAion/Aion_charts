//! Frame composition: one render pass drawing scissored groups of solid + textured quads.
//!
//! Groups replicate LWC's pane/axis canvas separation: the pane group is scissored to the
//! pane rect so candle geometry never bleeds into the axes; axis groups draw unscissored
//! (or with their own rects). Draw order within a group: solid quads, then text.

use wgpu::util::DeviceExt;

use crate::quad_pipeline::{QuadInstance, QuadRenderer};
use crate::tex_quad_pipeline::{TexQuadInstance, TexQuadRenderer};

#[derive(Default)]
pub struct DrawGroup {
    /// x, y, w, h in bitmap px; None = full target.
    pub scissor: Option<[u32; 4]>,
    pub quads: Vec<QuadInstance>,
    pub tex_quads: Vec<TexQuadInstance>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    width_px: u32,
    height_px: u32,
    clear_color: wgpu::Color,
    quad: &QuadRenderer,
    tex: &TexQuadRenderer,
    groups: &[DrawGroup],
) {
    quad.write_globals(queue, width_px, height_px);
    tex.write_globals(queue, width_px, height_px);

    // buffers must outlive the pass
    let buffers: Vec<(Option<wgpu::Buffer>, u32, Option<wgpu::Buffer>, u32)> = groups
        .iter()
        .map(|g| {
            let qb = (!g.quads.is_empty()).then(|| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("group_quads"),
                    contents: bytemuck::cast_slice(&g.quads),
                    usage: wgpu::BufferUsages::VERTEX,
                })
            });
            let tb = (!g.tex_quads.is_empty()).then(|| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("group_tex_quads"),
                    contents: bytemuck::cast_slice(&g.tex_quads),
                    usage: wgpu::BufferUsages::VERTEX,
                })
            });
            (qb, g.quads.len() as u32, tb, g.tex_quads.len() as u32)
        })
        .collect();

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        for (group, (qb, q_count, tb, t_count)) in groups.iter().zip(&buffers) {
            let [sx, sy, sw, sh] = match group.scissor {
                Some([x, y, w, h]) => {
                    // clamp inside the attachment
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

            if let Some(qb) = qb {
                quad.draw(&mut pass, qb, *q_count);
            }
            if let Some(tb) = tb {
                tex.draw(&mut pass, tb, *t_count);
            }
        }
    }

    queue.submit(Some(encoder.finish()));
}
