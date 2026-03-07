//! PipelineManager — rect/line/area pipelines for the unified geometry architecture.
//!
//! Three pipelines, three shaders:
//! - rect.wgsl: draws colored axis-aligned quads at pixel positions
//! - line.wgsl: draws anti-aliased line segments as rotated quads
//! - area.wgsl: draws filled trapezoids for smooth area charts
use crate::core::renderer::draw_list::{
    AreaSegment, ColoredRect, HorizontalGradientRect, LineSegment,
};

/// Simple viewport uniform for pixel→NDC conversion in the shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectViewportUniform {
    pub width: f32,
    pub height: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

pub struct PipelineManager {
    pub rect_pipeline: wgpu::RenderPipeline,
    pub gradient_rect_pipeline: wgpu::RenderPipeline,
    pub line_pipeline: wgpu::RenderPipeline,
    pub area_pipeline: wgpu::RenderPipeline,
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,
}

impl PipelineManager {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("rect_uniform_bgl"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            immediate_size: 0,
        });

        // ── Rect Pipeline ──
        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/rect.wgsl").into()),
        });

        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ColoredRect>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32,   // x
                        1 => Float32,   // y
                        2 => Float32,   // w
                        3 => Float32,   // h
                        4 => Float32,   // r
                        5 => Float32,   // g
                        6 => Float32,   // b
                        7 => Float32,   // a
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
                module: &rect_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        let gradient_rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gradient_rect_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/gradient_rect.wgsl").into(),
            ),
        });

        let gradient_rect_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("gradient_rect_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &gradient_rect_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<HorizontalGradientRect>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32,
                            1 => Float32,
                            2 => Float32,
                            3 => Float32,
                            4 => Float32,
                            5 => Float32,
                            6 => Float32,
                            7 => Float32,
                            8 => Float32,
                            9 => Float32,
                            10 => Float32,
                            11 => Float32,
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
                    module: &gradient_rect_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            });

        // ── Line Pipeline ──
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("line_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/line.wgsl").into()),
        });

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LineSegment>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32,   // x1
                        1 => Float32,   // y1
                        2 => Float32,   // x2
                        3 => Float32,   // y2
                        4 => Float32,   // width
                        5 => Float32,   // r
                        6 => Float32,   // g
                        7 => Float32,   // b
                        8 => Float32,   // a
                        9 => Float32,   // _pad
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
                module: &line_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        // ── Area Pipeline ──
        let area_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("area_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/area.wgsl").into()),
        });

        let area_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("area_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &area_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<AreaSegment>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32,   // x1
                        1 => Float32,   // y1
                        2 => Float32,   // x2
                        3 => Float32,   // y2
                        4 => Float32,   // bottom
                        5 => Float32,   // top_r
                        6 => Float32,   // top_g
                        7 => Float32,   // top_b
                        8 => Float32,   // top_a
                        9 => Float32,   // bottom_r
                        10 => Float32,  // bottom_g
                        11 => Float32,  // bottom_b
                        12 => Float32,  // bottom_a
                        13 => Float32,  // gradient_top
                        14 => Float32,  // _pad1
                        15 => Float32,  // _pad2
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
                module: &area_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            rect_pipeline,
            gradient_rect_pipeline,
            line_pipeline,
            area_pipeline,
            uniform_bind_group_layout,
        }
    }
}
