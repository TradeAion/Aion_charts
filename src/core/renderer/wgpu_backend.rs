//! WgpuRenderer — phased WebGPU renderer implementing ChartRenderer.
//!
//! Architecture:
//! - `begin_frame` acquires SurfaceTexture + creates CommandEncoder (stored in self).
//! - Each `draw_*` method creates a short-lived RenderPass (borrows encoder, draws, drops).
//! - `end_frame` submits the encoder and presents.
//! - This avoids the self-referential borrow trap with wgpu::RenderPass.
//!
//! Rendering pipelines:
//! - **Rect pipeline** (rect.wgsl): for grid lines, background, volume bars —
//!   uses the existing ColoredRect instance format (pixel coords).
//! - **Candle pipeline** (candles.wgsl): instanced OHLCV rendering —
//!   CPU maps f64 world data to f32 pixel coords, shader generates geometry.
//!   24 verts/instance (upper wick + lower wick + border + body fill).

use crate::core::renderer::draw_list::ColoredRect;
use crate::core::renderer::geometry_generator;
use crate::core::renderer::pipeline_manager::{PipelineManager, RectViewportUniform};
use crate::core::renderer::series::CandleSizing;
use crate::core::renderer::traits::{ChartRenderer, RenderContext};
use crate::core::renderer::wgpu_context::GpuContext;
use crate::core::viewport::Viewport;
use bytemuck::{Pod, Zeroable};

// ── GPU data types ───────────────────────────────────────────────────────────

/// Per-instance candle data uploaded to GPU. All values in physical pixels.
/// The CPU converts f64 world coords → f32 pixel coords relative to the
/// current viewport origin, solving the f64→f32 precision trap.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CandleInstance {
    pub center_x: f32,
    pub open_y: f32,
    pub high_y: f32,
    pub low_y: f32,
    pub close_y: f32,
    /// 1.0 = bullish, 0.0 = bearish
    pub state: f32,
}

/// Uniform block for the candle pipeline.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CandleUniforms {
    pub width: f32,
    pub height: f32,
    /// Full bar width in physical pixels (NOT half). Shader computes edges asymmetrically.
    pub bar_width: f32,
    /// Full wick width in physical pixels (NOT half). Shader computes edges asymmetrically.
    pub wick_width: f32,
    pub border_width: f32,
    pub draw_body: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    // Colors — vec4 each (16 bytes)
    pub bullish_body: [f32; 4],
    pub bearish_body: [f32; 4],
    pub bullish_wick: [f32; 4],
    pub bearish_wick: [f32; 4],
}

// ── Frame state (borrow-safe) ────────────────────────────────────────────────

/// Transient per-frame GPU state. Stored in self between begin_frame/end_frame.
/// The encoder lives here; each draw_* method creates a short-lived RenderPass.
struct FrameState {
    output: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
    /// True if the first render pass has already cleared the surface.
    cleared: bool,
}

// ── Buffer pool helpers ──────────────────────────────────────────────────────

const INITIAL_CANDLE_CAPACITY: usize = 8192;
const INITIAL_RECT_CAPACITY: usize = 16384;

/// Maximum number of surface recovery attempts before returning an error.
const MAX_SURFACE_RECOVERY_ATTEMPTS: u32 = 3;

/// Error type for render failures that may be recoverable.
#[derive(Debug, Clone)]
pub enum RenderError {
    /// Surface was lost and could not be recovered after retries.
    SurfaceLost(String),
    /// Transient error that may succeed on retry.
    Transient(String),
    /// Permanent error that won't be fixed by retrying.
    Permanent(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::SurfaceLost(msg) => write!(f, "Surface lost: {}", msg),
            RenderError::Transient(msg) => write!(f, "Transient error: {}", msg),
            RenderError::Permanent(msg) => write!(f, "Permanent error: {}", msg),
        }
    }
}

// ── WgpuRenderer ─────────────────────────────────────────────────────────────

const INITIAL_LINE_CAPACITY: usize = 4096;
const INITIAL_AREA_CAPACITY: usize = 4096;

pub struct WgpuRenderer {
    pub gpu: GpuContext,

    // Rect pipeline (grid, bg, volume) — existing path
    rect_pipelines: PipelineManager,
    rect_instance_buf: wgpu::Buffer,
    rect_uniform_buf: wgpu::Buffer,
    rect_bind_group: wgpu::BindGroup,
    rect_capacity: usize,

    // Line pipeline — for smooth anti-aliased line charts
    line_instance_buf: wgpu::Buffer,
    line_capacity: usize,

    // Area pipeline — for smooth area chart fills
    area_instance_buf: wgpu::Buffer,
    area_capacity: usize,

    // Candle pipeline — new instanced OHLCV path
    candle_pipeline: wgpu::RenderPipeline,
    candle_instance_buf: wgpu::Buffer,
    candle_uniform_buf: wgpu::Buffer,
    candle_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    candle_bind_group_layout: wgpu::BindGroupLayout,
    candle_capacity: usize,

    // Per-frame state (Some between begin_frame..end_frame)
    frame: Option<FrameState>,
}

impl WgpuRenderer {
    pub fn new(gpu: GpuContext) -> Self {
        // ── Rect pipeline (existing) ──
        let rect_pipelines = PipelineManager::new(&gpu.device, gpu.format);

        let rect_instance_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect_instances"),
            size: (INITIAL_RECT_CAPACITY * std::mem::size_of::<ColoredRect>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let rect_uniform_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect_viewport_uniform"),
            size: std::mem::size_of::<RectViewportUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let rect_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect_bind_group"),
            layout: &rect_pipelines.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: rect_uniform_buf.as_entire_binding(),
            }],
        });

        // ── Candle pipeline (new) ──
        let candle_bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("candle_uniform_bgl"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let candle_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("candle_pipeline_layout"),
                    bind_group_layouts: &[&candle_bind_group_layout],
                    immediate_size: 0,
                });

        let candle_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("candle_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../../../shaders/candles.wgsl").into(),
                ),
            });

        let candle_pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("candle_pipeline"),
                layout: Some(&candle_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &candle_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<CandleInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32,   // center_x
                            1 => Float32,   // open_y
                            2 => Float32,   // high_y
                            3 => Float32,   // low_y
                            4 => Float32,   // close_y
                            5 => Float32,   // state
                        ],
                    }],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &candle_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            });

        let candle_instance_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("candle_instances"),
            size: (INITIAL_CANDLE_CAPACITY * std::mem::size_of::<CandleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let candle_uniform_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("candle_uniforms"),
            size: std::mem::size_of::<CandleUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let candle_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("candle_bind_group"),
            layout: &candle_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: candle_uniform_buf.as_entire_binding(),
            }],
        });

        // ── Line buffer (for smooth line/area charts) ──
        let line_instance_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line_instances"),
            size: (INITIAL_LINE_CAPACITY
                * std::mem::size_of::<crate::core::renderer::draw_list::LineSegment>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Area buffer (for smooth area chart fills) ──
        let area_instance_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("area_instances"),
            size: (INITIAL_AREA_CAPACITY
                * std::mem::size_of::<crate::core::renderer::draw_list::AreaSegment>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            gpu,
            rect_pipelines,
            rect_instance_buf,
            rect_uniform_buf,
            rect_bind_group,
            rect_capacity: INITIAL_RECT_CAPACITY,
            line_instance_buf,
            line_capacity: INITIAL_LINE_CAPACITY,
            area_instance_buf,
            area_capacity: INITIAL_AREA_CAPACITY,
            candle_pipeline,
            candle_instance_buf,
            candle_uniform_buf,
            candle_bind_group,
            candle_bind_group_layout,
            candle_capacity: INITIAL_CANDLE_CAPACITY,
            frame: None,
        }
    }

    // ── Rect helpers ─────────────────────────────────────────────────────────

    /// Upload ColoredRects to the rect instance buffer. Returns count.
    fn upload_rects(&mut self, rects: &[ColoredRect], phys_w: u32, phys_h: u32) -> u32 {
        let count = rects.len() as u32;
        if count == 0 {
            return 0;
        }

        // Grow if needed
        if rects.len() > self.rect_capacity {
            self.rect_capacity = rects.len().next_power_of_two();
            self.rect_instance_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_instances"),
                size: (self.rect_capacity * std::mem::size_of::<ColoredRect>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        self.gpu
            .queue
            .write_buffer(&self.rect_instance_buf, 0, bytemuck::cast_slice(rects));

        let uniform = RectViewportUniform {
            width: phys_w as f32,
            height: phys_h as f32,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        self.gpu
            .queue
            .write_buffer(&self.rect_uniform_buf, 0, bytemuck::bytes_of(&uniform));

        count
    }

    /// Issue a render pass that draws rect instances.
    fn draw_rect_pass(&mut self, rect_count: u32) {
        if rect_count == 0 {
            return;
        }
        let frame = self
            .frame
            .as_mut()
            .expect("draw_rect_pass called outside begin/end_frame");
        let load_op = if frame.cleared {
            wgpu::LoadOp::Load
        } else {
            frame.cleared = true;
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.09020,
                g: 0.09020,
                b: 0.09020,
                a: 1.0,
            })
        };
        {
            let mut pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("rect_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load_op,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            pass.set_pipeline(&self.rect_pipelines.rect_pipeline);
            pass.set_bind_group(0, &self.rect_bind_group, &[]);
            pass.set_vertex_buffer(0, self.rect_instance_buf.slice(..));
            pass.draw(0..6, 0..rect_count);
        } // RenderPass drops here — encoder borrow released
    }

    // ── Line helpers ─────────────────────────────────────────────────────────

    /// Upload LineSegments to the line instance buffer. Returns count.
    fn upload_lines(
        &mut self,
        lines: &[crate::core::renderer::draw_list::LineSegment],
        phys_w: u32,
        phys_h: u32,
    ) -> u32 {
        let count = lines.len() as u32;
        if count == 0 {
            return 0;
        }

        // Grow if needed
        if lines.len() > self.line_capacity {
            self.line_capacity = lines.len().next_power_of_two();
            self.line_instance_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("line_instances"),
                size: (self.line_capacity
                    * std::mem::size_of::<crate::core::renderer::draw_list::LineSegment>())
                    as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        self.gpu
            .queue
            .write_buffer(&self.line_instance_buf, 0, bytemuck::cast_slice(lines));

        // Reuse the rect uniform buffer for viewport dimensions
        let uniform = RectViewportUniform {
            width: phys_w as f32,
            height: phys_h as f32,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        self.gpu
            .queue
            .write_buffer(&self.rect_uniform_buf, 0, bytemuck::bytes_of(&uniform));

        count
    }

    /// Issue a render pass that draws line segment instances.
    fn draw_line_pass(&mut self, line_count: u32) {
        if line_count == 0 {
            return;
        }
        let frame = self
            .frame
            .as_mut()
            .expect("draw_line_pass called outside begin/end_frame");
        let load_op = if frame.cleared {
            wgpu::LoadOp::Load
        } else {
            frame.cleared = true;
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.09020,
                g: 0.09020,
                b: 0.09020,
                a: 1.0,
            })
        };
        {
            let mut pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("line_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load_op,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            pass.set_pipeline(&self.rect_pipelines.line_pipeline);
            pass.set_bind_group(0, &self.rect_bind_group, &[]);
            pass.set_vertex_buffer(0, self.line_instance_buf.slice(..));
            pass.draw(0..6, 0..line_count); // 6 verts per line segment (2 triangles)
        }
    }

    // ── Area helpers ─────────────────────────────────────────────────────────

    /// Upload AreaSegments to the area instance buffer. Returns count.
    fn upload_areas(
        &mut self,
        areas: &[crate::core::renderer::draw_list::AreaSegment],
        phys_w: u32,
        phys_h: u32,
    ) -> u32 {
        let count = areas.len() as u32;
        if count == 0 {
            return 0;
        }

        // Grow if needed
        if areas.len() > self.area_capacity {
            self.area_capacity = areas.len().next_power_of_two();
            self.area_instance_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("area_instances"),
                size: (self.area_capacity
                    * std::mem::size_of::<crate::core::renderer::draw_list::AreaSegment>())
                    as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        self.gpu
            .queue
            .write_buffer(&self.area_instance_buf, 0, bytemuck::cast_slice(areas));

        // Reuse the rect uniform buffer for viewport dimensions
        let uniform = RectViewportUniform {
            width: phys_w as f32,
            height: phys_h as f32,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        self.gpu
            .queue
            .write_buffer(&self.rect_uniform_buf, 0, bytemuck::bytes_of(&uniform));

        count
    }

    /// Issue a render pass that draws area segment instances (trapezoids).
    fn draw_area_pass(&mut self, area_count: u32) {
        if area_count == 0 {
            return;
        }
        let frame = self
            .frame
            .as_mut()
            .expect("draw_area_pass called outside begin/end_frame");
        let load_op = if frame.cleared {
            wgpu::LoadOp::Load
        } else {
            frame.cleared = true;
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.09020,
                g: 0.09020,
                b: 0.09020,
                a: 1.0,
            })
        };
        {
            let mut pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("area_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load_op,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            pass.set_pipeline(&self.rect_pipelines.area_pipeline);
            pass.set_bind_group(0, &self.rect_bind_group, &[]);
            pass.set_vertex_buffer(0, self.area_instance_buf.slice(..));
            pass.draw(0..6, 0..area_count); // 6 verts per area segment (2 triangles)
        }
    }

    // ── Candle helpers ───────────────────────────────────────────────────────

    /// Build CandleInstance array from visible bars. Maps f64 world coords
    /// to f32 pixel coords relative to viewport origin.
    fn build_candle_instances(
        bars: &crate::core::data::BarArray,
        viewport: &Viewport,
        pane_w: f64,
        candle_h: f64,
    ) -> Vec<CandleInstance> {
        let start = (viewport.start_bar.floor() as usize)
            .saturating_sub(1)
            .min(bars.len());
        let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());
        if start >= end {
            return Vec::new();
        }

        let bar_range = viewport.end_bar - viewport.start_bar;
        let price_range = viewport.price_max - viewport.price_min;

        let mut instances = Vec::with_capacity(end - start);
        for i in start..end {
            // SAFETY: i is bounded by start..end which are clamped to bars.len()
            let b = bars.get_unchecked(i);
            // f64 world → f32 pixel, relative to viewport origin
            let center_x =
                ((i as f64 + 0.5 - viewport.start_bar) / bar_range * pane_w).round() as f32;
            let open_y = (candle_h * (1.0 - (b.open as f64 - viewport.price_min) / price_range))
                .round() as f32;
            let high_y = (candle_h * (1.0 - (b.high as f64 - viewport.price_min) / price_range))
                .round() as f32;
            let low_y = (candle_h * (1.0 - (b.low as f64 - viewport.price_min) / price_range))
                .round() as f32;
            let close_y = (candle_h * (1.0 - (b.close as f64 - viewport.price_min) / price_range))
                .round() as f32;
            let state = if b.close >= b.open { 1.0f32 } else { 0.0f32 };

            instances.push(CandleInstance {
                center_x,
                open_y,
                high_y,
                low_y,
                close_y,
                state,
            });
        }
        instances
    }

    /// Upload candle instances and uniforms. Returns instance count.
    fn upload_candles(&mut self, instances: &[CandleInstance], uniforms: &CandleUniforms) -> u32 {
        let count = instances.len() as u32;
        if count == 0 {
            return 0;
        }

        // Grow buffer if needed
        if instances.len() > self.candle_capacity {
            self.candle_capacity = instances.len().next_power_of_two();
            self.candle_instance_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("candle_instances"),
                size: (self.candle_capacity * std::mem::size_of::<CandleInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        self.gpu.queue.write_buffer(
            &self.candle_instance_buf,
            0,
            bytemuck::cast_slice(instances),
        );
        self.gpu
            .queue
            .write_buffer(&self.candle_uniform_buf, 0, bytemuck::bytes_of(uniforms));

        count
    }

    /// Issue a render pass that draws candle instances.
    fn draw_candle_pass(&mut self, candle_count: u32) {
        if candle_count == 0 {
            return;
        }
        let frame = self
            .frame
            .as_mut()
            .expect("draw_candle_pass called outside begin/end_frame");
        let load_op = if frame.cleared {
            wgpu::LoadOp::Load
        } else {
            frame.cleared = true;
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.09020,
                g: 0.09020,
                b: 0.09020,
                a: 1.0,
            })
        };
        {
            let mut pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("candle_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: load_op,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            pass.set_pipeline(&self.candle_pipeline);
            pass.set_bind_group(0, &self.candle_bind_group, &[]);
            pass.set_vertex_buffer(0, self.candle_instance_buf.slice(..));
            // 24 vertices per candle (4 quads × 6 verts)
            pass.draw(0..24, 0..candle_count);
        } // RenderPass drops here
    }

    /// Acquire a surface texture with retry logic and exponential backoff.
    ///
    /// Handles transient GPU errors by:
    /// 1. Reconfiguring the surface on Lost/Outdated errors
    /// 2. Retrying up to MAX_SURFACE_RECOVERY_ATTEMPTS times
    fn acquire_surface_texture(&mut self) -> Result<wgpu::SurfaceTexture, String> {
        let mut last_error = None;

        for attempt in 0..MAX_SURFACE_RECOVERY_ATTEMPTS {
            match self.gpu.surface.get_current_texture() {
                Ok(tex) => {
                    if attempt > 0 {
                        log::info!("Surface recovered after {} attempt(s)", attempt);
                    }
                    return Ok(tex);
                }
                Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                    log::warn!(
                        "Surface lost/outdated (attempt {}/{}), reconfiguring...",
                        attempt + 1,
                        MAX_SURFACE_RECOVERY_ATTEMPTS
                    );

                    // Reconfigure the surface
                    self.gpu
                        .surface
                        .configure(&self.gpu.device, &self.gpu.config);

                    last_error = Some("Surface lost after reconfigure".to_string());
                }
                Err(wgpu::SurfaceError::Timeout) => {
                    log::warn!(
                        "Surface timeout (attempt {}/{}), retrying...",
                        attempt + 1,
                        MAX_SURFACE_RECOVERY_ATTEMPTS
                    );
                    last_error = Some("Surface timeout".to_string());
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    // Out of memory is likely unrecoverable without freeing resources
                    return Err(
                        "GPU out of memory - cannot recover. Try reducing chart size or data."
                            .to_string(),
                    );
                }
                Err(e) => {
                    // Other/unknown errors - log and retry
                    log::warn!(
                        "Surface error {:?} (attempt {}/{}), retrying...",
                        e,
                        attempt + 1,
                        MAX_SURFACE_RECOVERY_ATTEMPTS
                    );
                    last_error = Some(format!("{:?}", e));
                }
            }
        }

        // All retries exhausted
        Err(format!(
            "Surface error after {} attempts: {}. Consider falling back to Canvas2D.",
            MAX_SURFACE_RECOVERY_ATTEMPTS,
            last_error.unwrap_or_else(|| "unknown".to_string())
        ))
    }
}

// ── ChartRenderer implementation ─────────────────────────────────────────────

impl ChartRenderer for WgpuRenderer {
    fn name(&self) -> &str {
        "webgpu"
    }

    fn resize(&mut self, physical_width: u32, physical_height: u32, _dpr: f64) {
        self.gpu.resize(physical_width, physical_height);
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn begin_frame(&mut self, _ctx: &RenderContext) -> Result<(), String> {
        let output = self.acquire_surface_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("raycore_encoder"),
            });
        self.frame = Some(FrameState {
            output,
            view,
            encoder,
            cleared: false,
        });
        Ok(())
    }

    fn draw_grid(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let pane_w = ctx.viewport.width as f64;
        let pane_h = ctx.viewport.height as f64;

        // Background fill
        let bg = &ctx.style.bg_color;
        let mut rects = vec![ColoredRect {
            x: 0.0,
            y: 0.0,
            w: pane_w as f32,
            h: pane_h as f32,
            r: bg[0],
            g: bg[1],
            b: bg[2],
            a: bg[3],
        }];

        // Grid lines (same as Canvas2D path)
        let grid_rects = geometry_generator::generate_grid_rects(
            ctx.style,
            ctx.y_ticks,
            ctx.x_ticks,
            pane_w,
            pane_h,
        );
        rects.extend(grid_rects);

        let count = self.upload_rects(&rects, ctx.viewport.width, ctx.viewport.height);
        self.draw_rect_pass(count);
        Ok(())
    }

    fn draw_candles(&mut self, ctx: &RenderContext) -> Result<(), String> {
        use crate::core::chart_type::MainChartType;

        let pane_w = ctx.viewport.width as f64;
        let pane_h = ctx.viewport.height as f64;
        let vol_h = pane_h * ctx.viewport.volume_height_ratio as f64;
        let candle_h = pane_h - vol_h;

        // For non-candlestick types, use rect-based rendering
        match ctx.main_chart_type {
            MainChartType::Candlestick | MainChartType::HeikinAshi => {
                // Use the optimized instanced candle pipeline
                let sizing = CandleSizing::compute_from_pane(
                    pane_w,
                    ctx.viewport,
                    ctx.h_pixel_ratio,
                    ctx.v_pixel_ratio,
                );

                let instances = if ctx.main_chart_type == MainChartType::HeikinAshi {
                    // For Heikin-Ashi, we'd need to transform the data
                    // For now, fall back to rect-based rendering
                    let rects = geometry_generator::generate_heikin_ashi_rects(
                        ctx.bars,
                        ctx.viewport,
                        ctx.style,
                        pane_w,
                        pane_h,
                        ctx.h_pixel_ratio,
                        ctx.v_pixel_ratio,
                    );
                    let count = self.upload_rects(&rects, ctx.viewport.width, ctx.viewport.height);
                    self.draw_rect_pass(count);
                    return Ok(());
                } else {
                    Self::build_candle_instances(ctx.bars, ctx.viewport, pane_w, candle_h)
                };

                let uniforms = CandleUniforms {
                    width: pane_w as f32,
                    height: pane_h as f32,
                    bar_width: sizing.bar_width as f32,
                    wick_width: sizing.wick_width as f32,
                    border_width: sizing.border_width as f32,
                    draw_body: if sizing.draw_body { 1.0 } else { 0.0 },
                    _pad0: 0.0,
                    _pad1: 0.0,
                    bullish_body: ctx.style.bullish_color,
                    bearish_body: ctx.style.bearish_color,
                    bullish_wick: ctx.style.wick_bullish_color,
                    bearish_wick: ctx.style.wick_bearish_color,
                };

                let count = self.upload_candles(&instances, &uniforms);
                self.draw_candle_pass(count);
            }
            MainChartType::OhlcBars => {
                let rects = geometry_generator::generate_ohlc_bar_rects(
                    ctx.bars,
                    ctx.viewport,
                    ctx.style,
                    pane_w,
                    pane_h,
                    ctx.h_pixel_ratio,
                    ctx.v_pixel_ratio,
                );
                let count = self.upload_rects(&rects, ctx.viewport.width, ctx.viewport.height);
                self.draw_rect_pass(count);
            }
            MainChartType::Line => {
                // Use the line pipeline for smooth anti-aliased lines
                let line_width = ctx.main_chart_options.line_width * ctx.v_pixel_ratio as f32;
                let segments = geometry_generator::generate_line_segments(
                    ctx.bars,
                    ctx.viewport,
                    ctx.main_chart_options.line_color,
                    line_width,
                    pane_w,
                    pane_h,
                );
                let count = self.upload_lines(&segments, ctx.viewport.width, ctx.viewport.height);
                self.draw_line_pass(count);
            }
            MainChartType::Area | MainChartType::Baseline => {
                // Use area pipeline for smooth trapezoid fills + line pipeline for smooth top edge
                let fill_color = if ctx.main_chart_type == MainChartType::Baseline {
                    ctx.main_chart_options.baseline_top_fill_color
                } else {
                    ctx.main_chart_options.area_top_color
                };
                let line_color = if ctx.main_chart_type == MainChartType::Baseline {
                    ctx.main_chart_options.baseline_top_line_color
                } else {
                    ctx.main_chart_options.line_color
                };
                let line_width = ctx.main_chart_options.line_width * ctx.v_pixel_ratio as f32;

                // Generate area fill segments (trapezoids)
                let area_segments = geometry_generator::generate_area_segments(
                    ctx.bars,
                    ctx.viewport,
                    fill_color,
                    pane_w,
                    pane_h,
                );

                // Generate line segments for smooth top edge
                let line_segments = geometry_generator::generate_line_segments(
                    ctx.bars,
                    ctx.viewport,
                    line_color,
                    line_width,
                    pane_w,
                    pane_h,
                );

                // Draw fill first (trapezoids)
                let area_count =
                    self.upload_areas(&area_segments, ctx.viewport.width, ctx.viewport.height);
                self.draw_area_pass(area_count);

                // Then draw the line on top
                let line_count =
                    self.upload_lines(&line_segments, ctx.viewport.width, ctx.viewport.height);
                self.draw_line_pass(line_count);
            }
        }
        Ok(())
    }

    fn draw_volume(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let pane_w = ctx.viewport.width as f64;
        let pane_h = ctx.viewport.height as f64;
        let vol_rects = geometry_generator::generate_volume_rects(
            ctx.bars,
            ctx.viewport,
            ctx.style,
            pane_w,
            pane_h,
            ctx.h_pixel_ratio,
            ctx.v_pixel_ratio,
        );
        let count = self.upload_rects(&vol_rects, ctx.viewport.width, ctx.viewport.height);
        self.draw_rect_pass(count);
        Ok(())
    }

    fn draw_lines(&mut self, ctx: &RenderContext) -> Result<(), String> {
        let pane_w = ctx.viewport.width as f64;
        let pane_h = ctx.viewport.height as f64;

        // Build timestamps slice for bar-index lookup
        let ts: Vec<u64> = (0..ctx.bars.len())
            .map(|i| ctx.bars.timestamps.value(i))
            .collect();

        // Generate smooth line segments + fill rects for overlays
        let (line_segments, fill_rects) =
            crate::core::renderer::line_generator::generate_all_overlay_geometry(
                ctx.series,
                ctx.viewport,
                &ts,
                pane_w,
                pane_h,
                ctx.h_pixel_ratio,
                ctx.v_pixel_ratio,
            );

        // Draw fill rects first (area/baseline fills behind lines)
        if !fill_rects.is_empty() {
            let count = self.upload_rects(&fill_rects, ctx.viewport.width, ctx.viewport.height);
            self.draw_rect_pass(count);
        }

        // Draw smooth anti-aliased line segments on top
        if !line_segments.is_empty() {
            let count = self.upload_lines(&line_segments, ctx.viewport.width, ctx.viewport.height);
            self.draw_line_pass(count);
        }

        Ok(())
    }

    fn draw_text(&mut self, _ctx: &RenderContext) -> Result<(), String> {
        // Text is handled by Canvas2D overlay/axis renderers — not GPU-rendered
        Ok(())
    }

    fn draw_crosshair(&mut self, _ctx: &RenderContext) -> Result<(), String> {
        // Crosshair is handled by Canvas2D overlay renderer
        Ok(())
    }

    fn end_frame(&mut self) -> Result<(), String> {
        let frame = self
            .frame
            .take()
            .ok_or_else(|| "end_frame called without begin_frame".to_string())?;

        // If nothing was drawn, issue a clear pass so the surface is valid
        if !frame.cleared {
            let mut encoder = frame.encoder;
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("clear_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.09020,
                                g: 0.09020,
                                b: 0.09020,
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
            }
            self.gpu.queue.submit(std::iter::once(encoder.finish()));
            frame.output.present();
        } else {
            self.gpu
                .queue
                .submit(std::iter::once(frame.encoder.finish()));
            frame.output.present();
        }
        Ok(())
    }
}
