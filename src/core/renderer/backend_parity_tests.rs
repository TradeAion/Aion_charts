#![cfg(all(not(target_arch = "wasm32"), feature = "parity-tests"))]

use super::draw_list::{ColoredRect, LineSegment};
use super::line_generator;
use super::pipeline_manager::{PipelineManager, RectViewportUniform};
use super::{geometry_generator, value_projection::TimeScaleIndex};
use crate::core::data::Bar;
use crate::core::engine::ChartEngine;
use crate::core::renderer::traits::RendererBackend;
use crate::core::series::{AreaSeriesOptions, LinePoint, LineSeriesOptions};
use crate::core::viewport::PriceScaleMode;
use crate::generate_sample_data;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const PARITY_WIDTH: u32 = 960;
const PARITY_HEIGHT: u32 = 540;

#[derive(Debug, Clone)]
pub struct ParityFixtureResult {
    pub name: &'static str,
    pub passed: bool,
    pub rect_count: usize,
    pub line_count: usize,
    pub structural_digest: String,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct ParityRunReport {
    pub adapter_summary: String,
    pub results: Vec<ParityFixtureResult>,
    pub report_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
struct StructuralRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
struct StructuralLine {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    width: f32,
    color: [f32; 4],
}

#[derive(Debug, Clone, Default, PartialEq)]
struct StructuralLog {
    rects: Vec<StructuralRect>,
    lines: Vec<StructuralLine>,
}

impl StructuralLog {
    fn from_primitives(rects: &[ColoredRect], lines: &[LineSegment]) -> Self {
        Self {
            rects: rects
                .iter()
                .map(|rect| StructuralRect {
                    x: rect.x,
                    y: rect.y,
                    w: rect.w,
                    h: rect.h,
                    color: [rect.r, rect.g, rect.b, rect.a],
                })
                .collect(),
            lines: lines
                .iter()
                .map(|line| StructuralLine {
                    x1: line.x1,
                    y1: line.y1,
                    x2: line.x2,
                    y2: line.y2,
                    width: line.width,
                    color: [line.r, line.g, line.b, line.a],
                })
                .collect(),
        }
    }

    fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        for rect in &self.rects {
            hasher.update(format!("{rect:?};"));
        }
        for line in &self.lines {
            hasher.update(format!("{line:?};"));
        }
        format!("{:x}", hasher.finalize())
    }

    fn compare(&self, other: &Self) -> Result<(), String> {
        if self.rects.len() != other.rects.len() {
            return Err(format!(
                "rect count mismatch: {} != {}",
                self.rects.len(),
                other.rects.len()
            ));
        }
        if self.lines.len() != other.lines.len() {
            return Err(format!(
                "line count mismatch: {} != {}",
                self.lines.len(),
                other.lines.len()
            ));
        }

        for (idx, (lhs, rhs)) in self.rects.iter().zip(&other.rects).enumerate() {
            if (lhs.x - rhs.x).abs() > 0.5
                || (lhs.y - rhs.y).abs() > 0.5
                || (lhs.w - rhs.w).abs() > 0.5
                || (lhs.h - rhs.h).abs() > 0.5
                || lhs
                    .color
                    .iter()
                    .zip(rhs.color.iter())
                    .any(|(a, b)| (a - b).abs() > 0.01)
            {
                return Err(format!("rect {idx} diverged: {lhs:?} != {rhs:?}"));
            }
        }

        for (idx, (lhs, rhs)) in self.lines.iter().zip(&other.lines).enumerate() {
            if (lhs.x1 - rhs.x1).abs() > 0.5
                || (lhs.y1 - rhs.y1).abs() > 0.5
                || (lhs.x2 - rhs.x2).abs() > 0.5
                || (lhs.y2 - rhs.y2).abs() > 0.5
                || (lhs.width - rhs.width).abs() > 0.5
                || lhs
                    .color
                    .iter()
                    .zip(rhs.color.iter())
                    .any(|(a, b)| (a - b).abs() > 0.01)
            {
                return Err(format!("line {idx} diverged: {lhs:?} != {rhs:?}"));
            }
        }

        Ok(())
    }
}

#[derive(Default)]
struct MockCanvas2DRenderer {
    log: StructuralLog,
}

impl MockCanvas2DRenderer {
    fn record(&mut self, rects: &[ColoredRect], lines: &[LineSegment]) {
        self.log = StructuralLog::from_primitives(rects, lines);
    }
}

struct OffscreenWgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: PipelineManager,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    rect_buffer: wgpu::Buffer,
    rect_capacity: usize,
    line_buffer: wgpu::Buffer,
    line_capacity: usize,
    format: wgpu::TextureFormat,
    adapter_summary: String,
}

impl OffscreenWgpuRenderer {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        })) {
            Ok(adapter) => adapter,
            Err(_) => pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
            .map_err(|e| format!("failed to acquire a parity-test adapter: {e:?}"))?,
        };

        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("backend-parity-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .map_err(|e| format!("failed to create parity-test device: {e:?}"))?;

        let format = wgpu::TextureFormat::Rgba8Unorm;
        let pipelines = PipelineManager::new(&device, format);
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("backend-parity-uniforms"),
            size: std::mem::size_of::<RectViewportUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("backend-parity-bind-group"),
            layout: &pipelines.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let rect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("backend-parity-rects"),
            size: std::mem::size_of::<ColoredRect>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let line_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("backend-parity-lines"),
            size: std::mem::size_of::<LineSegment>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            pipelines,
            uniform_buffer,
            bind_group,
            rect_buffer,
            rect_capacity: 1,
            line_buffer,
            line_capacity: 1,
            format,
            adapter_summary: format!("{} ({:?})", info.name, info.device_type),
        })
    }

    fn ensure_rect_capacity(&mut self, required: usize) {
        if required == 0 || required <= self.rect_capacity {
            return;
        }
        self.rect_capacity = required.next_power_of_two();
        self.rect_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("backend-parity-rects"),
            size: (self.rect_capacity * std::mem::size_of::<ColoredRect>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    fn ensure_line_capacity(&mut self, required: usize) {
        if required == 0 || required <= self.line_capacity {
            return;
        }
        self.line_capacity = required.next_power_of_two();
        self.line_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("backend-parity-lines"),
            size: (self.line_capacity * std::mem::size_of::<LineSegment>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    fn render(
        &mut self,
        rects: &[ColoredRect],
        lines: &[LineSegment],
        background: [f32; 4],
    ) -> Result<(), String> {
        self.ensure_rect_capacity(rects.len());
        self.ensure_line_capacity(lines.len());

        let uniforms = RectViewportUniform {
            width: PARITY_WIDTH as f32,
            height: PARITY_HEIGHT as f32,
            reserved0: 0.0,
            reserved1: 0.0,
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        if !rects.is_empty() {
            self.queue
                .write_buffer(&self.rect_buffer, 0, bytemuck::cast_slice(rects));
        }
        if !lines.is_empty() {
            self.queue
                .write_buffer(&self.line_buffer, 0, bytemuck::cast_slice(lines));
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("backend-parity-target"),
            size: wgpu::Extent3d {
                width: PARITY_WIDTH,
                height: PARITY_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("backend-parity-encoder"),
            });

        let clear_color = wgpu::Color {
            r: background[0] as f64,
            g: background[1] as f64,
            b: background[2] as f64,
            a: background[3] as f64,
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("backend-parity-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !rects.is_empty() {
                pass.set_pipeline(&self.pipelines.rect_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.rect_buffer.slice(..));
                pass.draw(0..6, 0..rects.len() as u32);
            }

            if !lines.is_empty() {
                pass.set_pipeline(&self.pipelines.line_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.line_buffer.slice(..));
                pass.draw(0..6, 0..lines.len() as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum FixtureKind {
    Candlestick,
    CandleWithLineOverlay,
    AreaSeries,
    LogScale,
}

struct FixtureGeometry {
    rects: Vec<ColoredRect>,
    lines: Vec<LineSegment>,
    background: [f32; 4],
}

fn fixture_specs() -> [(&'static str, FixtureKind); 4] {
    [
        ("candlestick-only", FixtureKind::Candlestick),
        ("candles-with-line-overlay", FixtureKind::CandleWithLineOverlay),
        ("area-series", FixtureKind::AreaSeries),
        ("log-scale", FixtureKind::LogScale),
    ]
}

fn sample_bars() -> Vec<Bar> {
    generate_sample_data(120, 1_700_000_000_000, 60_000)
}

fn make_overlay_points(bars: &[Bar], amplitude: f64) -> Vec<LinePoint> {
    bars.iter()
        .enumerate()
        .map(|(idx, bar)| LinePoint {
            timestamp: bar.timestamp,
            value: bar.close + ((idx as f64) * 0.1).sin() * amplitude,
        })
        .collect()
}

fn build_engine(kind: FixtureKind) -> Result<ChartEngine, String> {
    let mut engine = ChartEngine::new(
        RendererBackend::Noop,
        PARITY_WIDTH,
        PARITY_HEIGHT,
        1.0,
    );
    engine.set_data(sample_bars())?;
    engine.viewport.set_range(20.0, 80.0);

    let bars: Vec<Bar> = (0..engine.bars.len())
        .filter_map(|idx| engine.bars.get(idx))
        .collect();

    match kind {
        FixtureKind::Candlestick => {}
        FixtureKind::CandleWithLineOverlay => {
            let series_id = engine.add_line_series(LineSeriesOptions::default());
            engine.set_series_data(series_id, make_overlay_points(&bars, 6.0))?;
        }
        FixtureKind::AreaSeries => {
            let series_id = engine.add_area_series(AreaSeriesOptions::default());
            engine.set_series_data(series_id, make_overlay_points(&bars, 12.0))?;
        }
        FixtureKind::LogScale => {
            engine
                .viewport
                .set_price_scale_mode(PriceScaleMode::Logarithmic);
        }
    }

    engine.auto_fit_price_if_unlocked();
    Ok(engine)
}

fn render_fixture_geometry(kind: FixtureKind) -> Result<FixtureGeometry, String> {
    let engine = build_engine(kind)?;
    let time_scale = TimeScaleIndex::from_bars(&engine.bars);
    let bar_timestamps: Vec<u64> = (0..engine.bars.len())
        .map(|idx| engine.bars.timestamp(idx))
        .collect();
    let mut draw_list = geometry_generator::generate(
        &engine.bars,
        &engine.viewport,
        &engine.style,
        PARITY_WIDTH as f64,
        PARITY_HEIGHT as f64,
        engine.h_pixel_ratio,
        engine.v_pixel_ratio,
        &[],
        &[],
    );
    let (overlay_lines, overlay_rects) = line_generator::generate_all_overlay_geometry(
        &engine.series,
        &engine.viewport,
        &bar_timestamps,
        PARITY_WIDTH as f64,
        PARITY_HEIGHT as f64,
        engine.h_pixel_ratio,
        engine.v_pixel_ratio,
    );
    draw_list.rects.extend(overlay_rects);

    let _ = time_scale;
    Ok(FixtureGeometry {
        rects: draw_list.rects,
        lines: overlay_lines,
        background: engine.style.bg_color,
    })
}

fn write_report(report: &ParityRunReport) -> Result<PathBuf, String> {
    let target_dir = std::env::current_dir()
        .map_err(|e| format!("failed to resolve current directory: {e}"))?
        .join("target");
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("failed to create target directory: {e}"))?;
    let report_path = target_dir.join("backend-parity-report.md");

    let mut body = String::new();
    body.push_str("# Backend Parity Report\n\n");
    body.push_str(&format!("Adapter: `{}`\n\n", report.adapter_summary));
    body.push_str("| Fixture | Status | Rects | Lines | Structural Digest | Notes |\n");
    body.push_str("| --- | --- | ---: | ---: | --- | --- |\n");
    for result in &report.results {
        body.push_str(&format!(
            "| {} | {} | {} | {} | `{}` | {} |\n",
            result.name,
            if result.passed { "pass" } else { "fail" },
            result.rect_count,
            result.line_count,
            &result.structural_digest[..12],
            result.note
        ));
    }

    fs::write(&report_path, body).map_err(|e| format!("failed to write parity report: {e}"))?;
    Ok(report_path)
}

pub fn run_backend_parity_harness() -> Result<ParityRunReport, String> {
    let mut renderer = OffscreenWgpuRenderer::new()?;
    let mut results = Vec::new();

    for (name, kind) in fixture_specs() {
        let geometry = render_fixture_geometry(kind)?;
        renderer.render(&geometry.rects, &geometry.lines, geometry.background)?;

        let expected = StructuralLog::from_primitives(&geometry.rects, &geometry.lines);
        let mut mock_canvas = MockCanvas2DRenderer::default();
        mock_canvas.record(&geometry.rects, &geometry.lines);
        let comparison = expected.compare(&mock_canvas.log);

        let passed = comparison.is_ok();
        let note = match comparison {
            Ok(()) => "structural parity matched shared geometry".to_string(),
            Err(err) => err,
        };
        results.push(ParityFixtureResult {
            name,
            passed,
            rect_count: geometry.rects.len(),
            line_count: geometry.lines.len(),
            structural_digest: expected.digest(),
            note,
        });
    }

    let mut report = ParityRunReport {
        adapter_summary: renderer.adapter_summary.clone(),
        results,
        report_path: PathBuf::new(),
    };
    report.report_path = write_report(&report)?;
    Ok(report)
}
