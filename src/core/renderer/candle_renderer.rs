//! CandleRenderer — instanced rendering of OHLC candle bodies + wicks.
//!
//! Architecture:
//! - Each visible bar becomes one `CandleInstance` uploaded to a vertex buffer.
//! - The vertex shader generates 4 vertices per instance for the body (triangle strip)
//!   and 2 vertices for each wick line, all from the instance data.
//! - We use a single draw call with instancing, keeping draw calls O(1).

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use super::pipeline_manager::CandleInstance;

pub struct CandleRenderer {
    instance_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    capacity: usize,
    count: u32,
}

impl CandleRenderer {
    /// Initial capacity in instances. Will grow if needed.
    const INITIAL_CAPACITY: usize = 8192;

    pub fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let capacity = Self::INITIAL_CAPACITY;

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("candle_instance_buffer"),
            size: (capacity * std::mem::size_of::<CandleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("candle_uniform_buffer"),
            size: std::mem::size_of::<crate::core::viewport::ViewportUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("candle_bind_group"),
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

    /// Upload visible bars to the GPU instance buffer.
    /// Only uploads the bars in the viewport range — not all bars.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bars: &[Bar],
        viewport: &Viewport,
    ) {
        // Determine visible range
        let start = (viewport.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
        let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());
        if start >= end {
            self.count = 0;
            return;
        }

        let visible = &bars[start..end];
        let bar_width = 0.8; // 80% of 1.0 bar slot

        // Build instance data
        let instances: Vec<CandleInstance> = visible
            .iter()
            .enumerate()
            .map(|(i, bar)| CandleInstance {
                x: (start + i) as f32 + 0.5, // center of bar slot
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
                bar_width,
            })
            .collect();

        self.count = instances.len() as u32;

        // Grow buffer if needed
        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("candle_instance_buffer"),
                size: (self.capacity * std::mem::size_of::<CandleInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));

        // Upload uniforms (legacy — pass defaults for new sizing fields)
        let uniforms = viewport.candle_uniforms(
            viewport.width as f32,   // chart_w
            viewport.height as f32,  // candle_h
            8.0,                     // bar_width_px
            1.0,                     // wick_width_px
            1.0,                     // border_width_px
            true,                    // draw_body
        );
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Record draw commands into a render pass.
    /// The candle shader generates 12 vertices per instance:
    /// - 4 for the body (triangle strip)
    /// - 4 for the upper wick (thin quad)
    /// - 4 for the lower wick (thin quad)
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, pipeline: &'a wgpu::RenderPipeline) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        // 12 vertices per instance: body(4) + upper_wick(4) + lower_wick(4)
        pass.draw(0..12, 0..self.count);
    }
}
