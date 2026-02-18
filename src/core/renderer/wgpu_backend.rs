//! WgpuRenderer — wraps the existing wgpu rendering pipeline behind the Renderer trait.
//!
//! This is the high-performance WebGPU path. It owns GpuContext, PipelineManager,
//! CandleRenderer, and VolumeRenderer — the same components from Phase 1,
//! now encapsulated behind a unified interface.

use crate::core::data::Bar;
use crate::core::viewport::Viewport;
use crate::core::renderer::wgpu_context::GpuContext;
use crate::core::renderer::pipeline_manager::{PipelineManager, CandleInstance, VolumeInstance};
use crate::core::renderer::traits::{Renderer, RenderContext, ChartStyle};
use crate::core::renderer::series::{ChartLayout, CandleSizing};

pub struct WgpuRenderer {
    pub gpu: GpuContext,
    pipelines: PipelineManager,
    // Candle rendering state
    candle_instance_buf: wgpu::Buffer,
    candle_uniform_buf: wgpu::Buffer,
    candle_bind_group: wgpu::BindGroup,
    candle_capacity: usize,
    candle_count: u32,
    // Volume rendering state
    vol_instance_buf: wgpu::Buffer,
    vol_uniform_buf: wgpu::Buffer,
    vol_bind_group: wgpu::BindGroup,
    vol_capacity: usize,
    vol_count: u32,
}

const INITIAL_CAPACITY: usize = 8192;

impl WgpuRenderer {
    pub fn new(gpu: GpuContext) -> Self {
        let pipelines = PipelineManager::new(&gpu.device, gpu.format);

        // Candle buffers
        let candle_instance_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wgpu_candle_instances"),
            size: (INITIAL_CAPACITY * std::mem::size_of::<CandleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let candle_uniform_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wgpu_candle_uniforms"),
            size: std::mem::size_of::<crate::core::viewport::ViewportUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let candle_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wgpu_candle_bg"),
            layout: &pipelines.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: candle_uniform_buf.as_entire_binding(),
            }],
        });

        // Volume buffers
        let vol_instance_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wgpu_vol_instances"),
            size: (INITIAL_CAPACITY * std::mem::size_of::<VolumeInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vol_uniform_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wgpu_vol_uniforms"),
            size: std::mem::size_of::<crate::core::viewport::ViewportUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vol_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wgpu_vol_bg"),
            layout: &pipelines.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: vol_uniform_buf.as_entire_binding(),
            }],
        });

        Self {
            gpu,
            pipelines,
            candle_instance_buf,
            candle_uniform_buf,
            candle_bind_group,
            candle_capacity: INITIAL_CAPACITY,
            candle_count: 0,
            vol_instance_buf,
            vol_uniform_buf,
            vol_bind_group,
            vol_capacity: INITIAL_CAPACITY,
            vol_count: 0,
        }
    }

    fn upload_candles(
        &mut self, bars: &[Bar], viewport: &Viewport, style: &ChartStyle,
        layout: &ChartLayout, sizing: &CandleSizing,
    ) {
        let start = (viewport.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
        let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());
        if start >= end {
            self.candle_count = 0;
            return;
        }

        let visible = &bars[start..end];
        let instances: Vec<CandleInstance> = visible
            .iter()
            .enumerate()
            .map(|(i, bar)| CandleInstance {
                x: (start + i) as f32 + 0.5,
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
                bar_width: style.bar_width_ratio, // legacy field, unused by shader
            })
            .collect();

        self.candle_count = instances.len() as u32;

        if instances.len() > self.candle_capacity {
            self.candle_capacity = instances.len().next_power_of_two();
            self.candle_instance_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wgpu_candle_instances"),
                size: (self.candle_capacity * std::mem::size_of::<CandleInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        self.gpu.queue.write_buffer(&self.candle_instance_buf, 0, bytemuck::cast_slice(&instances));

        // height_px = candle_h so the shader can convert border_width_px to price units
        let uniforms = viewport.candle_uniforms(
            layout.chart_w as f32,
            layout.candle_h as f32,
            sizing.bar_width as f32,
            sizing.wick_width as f32,
            sizing.border_width as f32,
            sizing.draw_body,
        );
        self.gpu.queue.write_buffer(&self.candle_uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    }

    fn upload_volume(
        &mut self, bars: &[Bar], viewport: &Viewport, _style: &ChartStyle,
        layout: &ChartLayout, sizing: &CandleSizing,
    ) {
        let start = (viewport.start_bar.floor() as usize).saturating_sub(1).min(bars.len());
        let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());
        if start >= end {
            self.vol_count = 0;
            return;
        }

        let visible = &bars[start..end];
        let max_vol = visible.iter().map(|b| b.volume).fold(0.0f32, f32::max);

        let instances: Vec<VolumeInstance> = visible
            .iter()
            .enumerate()
            .map(|(i, bar)| VolumeInstance {
                x: (start + i) as f32 + 0.5,
                volume: bar.volume,
                bar_width: 0.0, // legacy field, unused by shader
                is_bullish: if bar.is_bullish() { 1.0 } else { 0.0 },
            })
            .collect();

        self.vol_count = instances.len() as u32;

        if instances.len() > self.vol_capacity {
            self.vol_capacity = instances.len().next_power_of_two();
            self.vol_instance_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wgpu_vol_instances"),
                size: (self.vol_capacity * std::mem::size_of::<VolumeInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        self.gpu.queue.write_buffer(&self.vol_instance_buf, 0, bytemuck::cast_slice(&instances));
        let uniforms = viewport.volume_uniforms(max_vol, layout.chart_w as f32, sizing.bar_width as f32);
        self.gpu.queue.write_buffer(&self.vol_uniform_buf, 0, bytemuck::bytes_of(&uniforms));
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
        let sizing = CandleSizing::compute(&layout, ctx.viewport);

        self.upload_candles(ctx.bars, ctx.viewport, ctx.style, &layout, &sizing);
        self.upload_volume(ctx.bars, ctx.viewport, ctx.style, &layout, &sizing);

        let output = self.gpu.surface.get_current_texture()
            .map_err(|e| format!("Surface error: {:?}", e))?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.gpu.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("raycore_encoder") }
        );

        // Clear transparent so the grid canvas behind shows through
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
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

            let chart_w = layout.chart_w as f32;
            let candle_h = layout.candle_h as f32;
            let vol_h = layout.vol_h as f32;

            // Draw candles — restricted to chart area (excludes axis regions)
            if self.candle_count > 0 {
                pass.set_viewport(0.0, 0.0, chart_w, candle_h, 0.0, 1.0);
                pass.set_pipeline(&self.pipelines.candle_pipeline);
                pass.set_bind_group(0, &self.candle_bind_group, &[]);
                pass.set_vertex_buffer(0, self.candle_instance_buf.slice(..));
                // 6 vertices per quad × 4 quads (upper_wick, lower_wick, border, body) = 24
                pass.draw(0..24, 0..self.candle_count);
            }

            // Draw volume — below candles, same chart_w
            if self.vol_count > 0 {
                pass.set_viewport(0.0, candle_h, chart_w, vol_h, 0.0, 1.0);
                pass.set_pipeline(&self.pipelines.volume_pipeline);
                pass.set_bind_group(0, &self.vol_bind_group, &[]);
                pass.set_vertex_buffer(0, self.vol_instance_buf.slice(..));
                // 6 vertices per quad (TriangleList)
                pass.draw(0..6, 0..self.vol_count);
            }
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    fn is_valid(&self) -> bool { true }
}
