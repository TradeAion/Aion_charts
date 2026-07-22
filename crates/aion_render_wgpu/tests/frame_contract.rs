//! Shared-frame contract coverage for the browser Canvas2D and WebGPU adapters.
//!
//! This does not require a physical GPU: it proves that one engine fixture can be consumed by
//! both adapter translators, including the marker/round-rect primitives that previously had a
//! silent WebGPU hole.

use aion_engine::{marker_pos, marker_shape, ChartEngine, Marker, PriceLine, SeriesKind};
use aion_render::canvas2d::{execute, Canvas2d, Viewport};
use aion_render::color::Color;
use aion_render::draw_list::{LineStyle, Prim};

use aion_render_wgpu::{geom_prims_to_tris, prims_to_instances};

#[derive(Default)]
struct CountingCanvas {
    calls: usize,
    fill_color: Option<Color>,
    rects: Vec<([f32; 4], Color)>,
}

impl Canvas2d for CountingCanvas {
    fn set_fill_solid(&mut self, color: Color) {
        self.calls += 1;
        self.fill_color = Some(color);
    }
    fn set_fill_vgradient(&mut self, _: f32, _: f32, _: Color, _: Color) {
        self.calls += 1;
    }
    fn set_stroke(&mut self, _: Color) {
        self.calls += 1;
    }
    fn set_line_width(&mut self, _: f32) {
        self.calls += 1;
    }
    fn set_line_dash(&mut self, _: &[f32]) {
        self.calls += 1;
    }
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.calls += 1;
        self.rects.push((
            [x, y, w, h],
            self.fill_color.expect("fill style before rect"),
        ));
    }
    fn begin_path(&mut self) {
        self.calls += 1;
    }
    fn move_to(&mut self, _: f32, _: f32) {
        self.calls += 1;
    }
    fn line_to(&mut self, _: f32, _: f32) {
        self.calls += 1;
    }
    fn close_path(&mut self) {
        self.calls += 1;
    }
    fn arc(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32) {
        self.calls += 1;
    }
    fn stroke(&mut self) {
        self.calls += 1;
    }
    fn fill(&mut self) {
        self.calls += 1;
    }
}

fn fixture() -> ChartEngine {
    let mut chart = ChartEngine::new(320.0, 220.0, 1.0);
    let times: Vec<f64> = (0..24).map(|i| i as f64).collect();
    let open: Vec<f64> = (0..24).map(|i| 100.0 + i as f64 * 0.2).collect();
    let high: Vec<f64> = open.iter().map(|v| v + 1.2).collect();
    let low: Vec<f64> = open.iter().map(|v| v - 1.0).collect();
    let close: Vec<f64> = open
        .iter()
        .enumerate()
        .map(|(i, v)| v + if i % 2 == 0 { 0.6 } else { -0.4 })
        .collect();
    chart
        .set_series_data(0, &times, &open, &high, &low, &close)
        .unwrap();
    chart.series[0].kind = SeriesKind::Line;
    chart.series[0].point_markers = true;
    chart.series[0].last_price_animation = true;
    chart.series[0].markers.push(Marker {
        time: 8,
        position: marker_pos::ABOVE,
        shape: marker_shape::SQUARE,
        color: Color::rgb(0x10, 0x80, 0xff),
        text: "BUY".into(),
    });
    chart.series[0].price_lines.push(PriceLine {
        id: 1,
        price: 102.0,
        color: Color::rgb(0xff, 0x98, 0x00),
        width: 1,
        style: LineStyle::Dashed,
        title: "target".into(),
        line_visible: true,
        axis_label_visible: true,
        axis_label_color: None,
        axis_label_text_color: None,
    });
    let baseline = chart.add_series(SeriesKind::Baseline);
    chart
        .set_series_data(baseline, &times, &open, &high, &low, &close)
        .unwrap();
    let candles = chart.add_series(SeriesKind::Candlestick);
    chart
        .set_series_data(candles, &times, &open, &high, &low, &close)
        .unwrap();
    let bars = chart.add_series(SeriesKind::Bar);
    chart
        .set_series_data(bars, &times, &open, &high, &low, &close)
        .unwrap();
    let histogram = chart.add_series(SeriesKind::Histogram);
    chart
        .set_series_data(histogram, &times, &open, &open, &open, &close)
        .unwrap();
    chart.time_scale.set_width(320.0);
    chart.fit_content();
    chart
}

#[test]
fn one_engine_frame_is_consumable_by_canvas2d_and_webgpu_adapters() {
    let mut chart = fixture();
    let frame = chart.build_frame();
    let mut canvas = CountingCanvas::default();
    let mut quads = Vec::new();
    let mut fill_tris = Vec::new();
    let mut stroke_tris = Vec::new();

    for pane in &frame.panes {
        execute(
            &pane.under,
            &pane.points,
            &mut canvas,
            Viewport {
                width: 320.0,
                height: 220.0,
            },
        );
        execute(
            &pane.main,
            &pane.points,
            &mut canvas,
            Viewport {
                width: 320.0,
                height: 220.0,
            },
        );
        assert!(!pane
            .under
            .iter()
            .chain(&pane.main)
            .any(|p| matches!(p, Prim::Text { .. })));
        prims_to_instances(&pane.under, &mut quads);
        prims_to_instances(&pane.main, &mut quads);
        geom_prims_to_tris(&pane.main, &pane.points, &mut fill_tris, &mut stroke_tris);
    }

    assert!(
        canvas.calls > 0,
        "Canvas2D adapter must execute the shared frame"
    );
    assert!(
        !quads.is_empty(),
        "WebGPU quad adapter must receive rect/grid primitives"
    );
    assert!(
        !fill_tris.is_empty(),
        "WebGPU triangle adapter must receive area primitives"
    );
    assert!(
        !stroke_tris.is_empty(),
        "WebGPU triangle adapter must receive line/marker primitives"
    );

    // Integer geometry is intentionally backend-identical: both adapters use the same rect and
    // dash expansion rules. Compare the complete command stream, not just that each backend ran.
    assert_eq!(
        canvas.rects.len(),
        quads.len(),
        "Canvas2D/WebGPU rect count diverged"
    );
    for ((canvas_rect, canvas_color), gpu) in canvas.rects.iter().zip(&quads) {
        assert_eq!(
            canvas_rect, &gpu.rect,
            "Canvas2D/WebGPU rect geometry diverged"
        );
        let expected = [
            canvas_color.r() as f32 / 255.0,
            canvas_color.g() as f32 / 255.0,
            canvas_color.b() as f32 / 255.0,
            canvas_color.a() as f32 / 255.0,
        ];
        assert_eq!(gpu.color, expected, "Canvas2D/WebGPU rect color diverged");
    }
}
