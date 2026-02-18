//! WgpuRenderer — dumb DrawList consumer (WebGPU path).
//!
//! Receives a DrawList from GeometryGenerator and uploads all ColoredRects
//! as instances to a single GPU buffer. One draw call renders everything.
//! The shader trivially converts pixel coords to NDC.

use crate::core::renderer::wgpu_context::GpuContext;
use crate::core::renderer::pipeline_manager::{PipelineManager, RectViewportUniform};
use crate::core::renderer::traits::{Renderer, RenderContext};
use crate::core::renderer::series::ChartLayout;
use crate::core::renderer::draw_list::{DrawList, ColoredRect};
use crate::core::renderer::geometry_generator;

pub struct WgpuRenderer {
    pub gpu: GpuContext,
    pipelines: PipelineManager,
    instance_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    capacity: usize,
    rect_count: u32,
}

const INITIAL_CAPACITY: usize = 16384;

impl WgpuRenderer {
    pub fn new(gpu: GpuContext) -> Self {
        let pipelines = PipelineManager::new(&gpu.device, gpu.format);

        let instance_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect_instances"),
            size: (INITIAL_CAPACITY * std::mem::size_of::<ColoredRect>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect_viewport_uniform"),
            size: std::mem::size_of::<RectViewportUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect_bind_group"),
            layout: &pipelines.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        Self {
            gpu,
            pipelines,
            instance_buf,
            uniform_buf,
            bind_group,
            capacity: INITIAL_CAPACITY,
            rect_count: 0,
        }
    }

    fn upload(&mut self, dl: &DrawList, phys_w: u32, phys_h: u32) {
        self.rect_count = dl.rects.len() as u32;
        if self.rect_count == 0 { return; }

        // Grow buffer if needed
        if dl.rects.len() > self.capacity {
            self.capacity = dl.rects.len().next_power_of_two();
            self.instance_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_instances"),
                size: (self.capacity * std::mem::size_of::<ColoredRect>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        self.gpu.queue.write_buffer(
            &self.instance_buf, 0,
            bytemuck::cast_slice(&dl.rects),
        );

        let uniform = RectViewportUniform {
            width: phys_w as f32,
            height: phys_h as f32,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        self.gpu.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniform));
    }
}

impl Renderer for WgpuRenderer {
    fn name(&self) -> &str { "webgpu" }

    fn resize(&mut self, physical_width: u32, physical_height: u32, _dpr: f64) {
        self.gpu.resize(physical_width, physical_height);
    }

    fn render_frame(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let layout = ChartLayout::from_physical(
            ctx.viewport.width, ctx.viewport.height, ctx.dpr, ctx.style,
        );

        // Generate geometry — SAME code path as Canvas2D
        let (dl, _, _) = geometry_generator::generate(ctx.bars, ctx.viewport, ctx.style, &layout);

        self.upload(&dl, ctx.viewport.width, ctx.viewport.height);

        let output = self.gpu.surface.get_current_texture()
            .map_err(|e| format!("Surface error: {:?}", e))?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.gpu.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("raycore_encoder") }
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rect_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0, g: 0.0, b: 0.0, a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if self.rect_count > 0 {
                pass.set_pipeline(&self.pipelines.rect_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instance_buf.slice(..));
                // 6 vertices per quad (TriangleList)
                pass.draw(0..6, 0..self.rect_count);
            }
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    fn is_valid(&self) -> bool { true }
}
