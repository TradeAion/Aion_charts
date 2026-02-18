//! PipelineManager — creates and caches render pipelines and bind group layouts.
//!
//! Design: pipelines are created lazily on first use and cached by name.
//! This avoids creating pipelines we might not need and keeps init fast.


/// Manages all render/compute pipelines.
pub struct PipelineManager {
    pub candle_pipeline: wgpu::RenderPipeline,
    pub volume_pipeline: wgpu::RenderPipeline,
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,
}

impl PipelineManager {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        // Shared uniform bind group layout (projection matrix etc.)
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform_bind_group_layout"),
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

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("chart_pipeline_layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                immediate_size: 0,
            });

        // --- Candle shader ---
        let candle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("candle_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/candle.wgsl").into(),
            ),
        });

        let candle_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("candle_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &candle_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        // Per-instance bar data
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<CandleInstance>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32,   // x (bar index)
                                1 => Float32,   // open
                                2 => Float32,   // high
                                3 => Float32,   // low
                                4 => Float32,   // close
                                5 => Float32,   // bar_width
                            ],
                        },
                    ],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
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
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            });

        // --- Volume shader ---
        let volume_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("volume_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/volume.wgsl").into(),
            ),
        });

        let volume_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("volume_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &volume_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<VolumeInstance>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32,   // x
                                1 => Float32,   // volume
                                2 => Float32,   // bar_width
                                3 => Float32,   // is_bullish (0 or 1)
                            ],
                        },
                    ],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &volume_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            });

        Self {
            candle_pipeline,
            volume_pipeline,
            uniform_bind_group_layout,
        }
    }
}

// --- Instance data structs for vertex buffers ---

/// Per-instance data uploaded for each visible candle.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CandleInstance {
    pub x: f32,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub bar_width: f32,
}

/// Per-instance data for each volume bar.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VolumeInstance {
    pub x: f32,
    pub volume: f32,
    pub bar_width: f32,
    pub is_bullish: f32,
}
