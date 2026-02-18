//! ChartEngine — the top-level orchestrator that owns all subsystems.
//!
//! This is the main struct that the WASM layer (or native app) interacts with.
//! It owns the GPU context, viewport, data, and all renderers.

use crate::core::data::{Bar, BarArray};
use crate::core::viewport::Viewport;
use crate::core::renderer::wgpu_context::GpuContext;
use crate::core::renderer::pipeline_manager::PipelineManager;
use crate::core::renderer::candle_renderer::CandleRenderer;
use crate::core::renderer::volume_renderer::VolumeRenderer;

/// The main chart engine. Owns everything needed to render a chart.
pub struct ChartEngine {
    pub gpu: GpuContext,
    pub viewport: Viewport,
    pub bars: BarArray,
    pub pipelines: PipelineManager,
    pub candle_renderer: CandleRenderer,
    pub volume_renderer: VolumeRenderer,
}

impl ChartEngine {
    /// Create a new engine from an already-initialized GPU context.
    pub fn new(gpu: GpuContext, width: u32, height: u32) -> Self {
        let viewport = Viewport::new(width, height);
        let bars = BarArray::new();

        let pipelines = PipelineManager::new(&gpu.device, gpu.format);
        let candle_renderer =
            CandleRenderer::new(&gpu.device, &pipelines.uniform_bind_group_layout);
        let volume_renderer =
            VolumeRenderer::new(&gpu.device, &pipelines.uniform_bind_group_layout);

        Self {
            gpu,
            viewport,
            bars,
            pipelines,
            candle_renderer,
            volume_renderer,
        }
    }

    /// Replace all bar data.
    pub fn set_data(&mut self, bars: Vec<Bar>) {
        let len = bars.len();
        self.bars.set(bars);

        // Auto-fit viewport to show last N bars
        let visible = (len as f64).min(200.0);
        self.viewport.set_range((len as f64) - visible, len as f64);

        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(self.bars.as_slice());
        }
    }

    /// Resize the canvas / surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
        self.viewport.resize(width, height);
    }

    /// Set visible bar range (zoom to range).
    pub fn zoom_to_range(&mut self, start: u64, end: u64) {
        self.viewport.set_range(start as f64, end as f64);
        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(self.bars.as_slice());
        }
    }

    /// Main render loop — called once per frame.
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Auto-fit price if not locked
        if !self.viewport.price_locked {
            self.viewport.auto_fit_price(self.bars.as_slice());
        }

        // Upload instance data for visible bars
        self.candle_renderer.update(
            &self.gpu.device,
            &self.gpu.queue,
            self.bars.as_slice(),
            &self.viewport,
        );
        self.volume_renderer.update(
            &self.gpu.device,
            &self.gpu.queue,
            self.bars.as_slice(),
            &self.viewport,
        );

        // Acquire frame
        let output = self.gpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("raycore_encoder"),
                });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("candle_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Dark background
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.067,
                            g: 0.075,
                            b: 0.094,
                            a: 1.0,
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

            // Set viewport for candle area (top portion)
            let vol_h = (self.viewport.height as f32 * self.viewport.volume_height_ratio) as u32;
            let candle_h = self.viewport.height.saturating_sub(vol_h);
            pass.set_viewport(
                0.0,
                0.0,
                self.viewport.width as f32,
                candle_h as f32,
                0.0,
                1.0,
            );

            self.candle_renderer
                .draw(&mut pass, &self.pipelines.candle_pipeline);

            // Set viewport for volume area (bottom portion)
            pass.set_viewport(
                0.0,
                candle_h as f32,
                self.viewport.width as f32,
                vol_h as f32,
                0.0,
                1.0,
            );

            self.volume_renderer
                .draw(&mut pass, &self.pipelines.volume_pipeline);
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
