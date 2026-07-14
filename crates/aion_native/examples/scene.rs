//! Renders a representative chart-like scene through the Canvas2D executor + tiny-skia target and
//! saves a PNG. Proves the Prim IR rasterizes correctly off-GPU (golden/SSR path).
//!
//! Run: `cargo run -p aion_native --example scene -- out.png`

use aion_native::render_prims;
use aion_render::color::Color;
use aion_render::draw_list::{Gradient, IRect, LineStyle, LineType, Prim};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "scene.png".to_string());
    let (w, h) = (480u32, 300u32);

    let grid = Color::rgb(0xe1, 0xe3, 0xea);
    let up = Color::rgb(0x26, 0xa6, 0x9a);
    let down = Color::rgb(0xef, 0x53, 0x50);
    let line_c = Color::rgb(0x21, 0x96, 0xf3);

    let mut prims: Vec<Prim> = Vec::new();

    // background gradient (white -> very light blue)
    prims.push(Prim::Background {
        gradient: Gradient { top: Color::rgb(0xff, 0xff, 0xff), bottom: Color::rgb(0xf0, 0xf4, 0xff) },
    });

    // horizontal + vertical grid
    for i in 1..6 {
        let y = i * 50;
        prims.push(Prim::HLine { y, x0: 0, x1: w as i32, width: 1, style: LineStyle::Solid, color: grid });
    }
    for i in 1..9 {
        let x = i * 50;
        prims.push(Prim::VLine { x, y0: 0, y1: h as i32, width: 1, style: LineStyle::Solid, color: grid });
    }

    // a row of candlesticks (wick VLine + body Rect), heights vary
    let bodies = [
        (40, 120, 60, true),
        (90, 150, 40, false),
        (140, 100, 70, true),
        (190, 130, 50, false),
        (240, 90, 55, true),
        (290, 140, 45, false),
    ];
    for (cx, top, body_h, is_up) in bodies {
        let color = if is_up { up } else { down };
        prims.push(Prim::VLine { x: cx, y0: top - 25, y1: top + body_h + 25, width: 1, style: LineStyle::Solid, color });
        prims.push(Prim::Rect { rect: IRect { x: cx - 8, y: top, w: 16, h: body_h }, color });
    }

    // an area fill + line over the right portion
    let pts: Vec<[f32; 2]> = (0..8)
        .map(|i| [350.0 + i as f32 * 16.0, 180.0 - (i as f32 * 1.3).sin() * 30.0])
        .collect();
    let first = 0u32;
    let count = pts.len() as u32;
    prims.push(Prim::AreaFill {
        first_point: first,
        point_count: count,
        base_y: 260.0,
        gradient: Gradient { top: Color::rgba(0x21, 0x96, 0xf3, 0x80), bottom: Color::rgba(0x21, 0x96, 0xf3, 0x08) },
    });
    prims.push(Prim::Polyline {
        first_point: first,
        point_count: count,
        width: 2.0,
        style: LineStyle::Solid,
        line_type: LineType::Simple,
        color: line_c,
    });

    // a dashed price line and a marker circle
    prims.push(Prim::HLine { y: 210, x0: 0, x1: w as i32, width: 1, style: LineStyle::Dashed, color: down });
    prims.push(Prim::Circle { cx: 240.0, cy: 90.0, radius: 6.0, fill: line_c, stroke_width: 2.0, stroke: Color::rgb(0xff, 0xff, 0xff) });

    let canvas = render_prims(w, h, Color::rgb(0xff, 0xff, 0xff), &prims, &pts);
    canvas.save_png(&out).expect("save png");
    println!("wrote {out} ({w}x{h}, {} prims)", prims.len());
}
