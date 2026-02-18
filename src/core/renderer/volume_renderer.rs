//! VolumeRenderer — instanced histogram rendering for volume bars.
//!
//! Each volume bar is a single rectangle drawn as a triangle strip (4 verts).
//! Color is determined by bullish/bearish in the fragment shader.

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use super::pipeline_manager::VolumeInstance;

pub struct VolumeRenderer {
    instance_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    capacity: usize,
    count: u32,
}

impl VolumeRenderer {
    const INITIAL_CAPACITY: usize = 8192;

    pub fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let capacity = Self::INITIAL_CAPACITY;

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume_instance_buffer"),
            size: (capacity * std::mem::size_of::<VolumeInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volume_uniform_buffer"),
            size: std::mem::size_of::<crate::core::viewport::ViewportUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("volume_bind_group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            instance_buffer,
            uniform_buffer,
            bind_group,
            capacity,
            count: 0,
        }
    }

    /// Upload visible volume bars.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bars: &[Bar],
        viewport: &Viewport,
    ) {
        let start = (viewport.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
        let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());
        if start >= end {
            self.count = 0;
            return;
        }

        let visible = &bars[start..end];
        let bar_width = 0.8;

        // Find max volume for normalization (passed via uniform projection)
        let max_vol = visible.iter().map(|b| b.volume).fold(0.0f32, f32::max);

        let instances: Vec<VolumeInstance> = visible
            .iter()
            .enumerate()
            .map(|(i, bar)| VolumeInstance {
                x: (start + i) as f32 + 0.5,
                volume: bar.volume,
                bar_width,
                is_bullish: if bar.is_bullish() { 1.0 } else { 0.0 },
            })
            .collect();

        self.count = instances.len() as u32;

        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("volume_instance_buffer"),
                size: (self.capacity * std::mem::size_of::<VolumeInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));

        // Upload volume uniforms (projection maps 0..max_vol)
        let uniforms = viewport.volume_uniforms(max_vol);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, pipeline: &'a wgpu::RenderPipeline) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        // 4 vertices per instance (triangle strip rectangle)
        pass.draw(0..4, 0..self.count);
    }
}
