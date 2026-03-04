//! Footprint Geometry Generator — produces ColoredRects and DrawTexts
//! for footprint chart rendering.
//!
//! Takes footprint data, viewport, style → produces pixel-perfect geometry
//! that both Canvas2D and WebGPU renderers consume.
//!
//! # Rendering Layout (per bar)
//!
//! ```text
//! ┌─────────────────────────────────┐ ← bar slot width
//! │  ┌──────────┬──────────┐        │
//! │  │  Bid Vol  │  Ask Vol  │ ← POC │  ← price level N+2
//! │  ├──────────┼──────────┤        │
//! │  │  Bid Vol  │  Ask Vol  │       │  ← price level N+1 (value area)
//! │  ├──────────┼──────────┤        │
//! │  │  Bid Vol  │  Ask Vol  │       │  ← price level N
//! │  └──────────┴──────────┘        │
//! │  ┌─────────────────────┐        │
//! │  │   Delta Bar (+125)   │        │  ← cumulative delta summary
//! │  └─────────────────────┘        │
//! └─────────────────────────────────┘
//! ```

use crate::core::data::BarArray;
use crate::core::footprint::{
    FootprintBar, FootprintData, FootprintDisplayMode, FootprintOptions, ImbalanceType,
    VolumeColorIntensity,
};
use crate::core::renderer::draw_list::{ColoredRect, DrawText, TextAlign};
use crate::core::renderer::series::CandleSizing;
use crate::core::renderer::traits::ChartStyle;
use crate::core::renderer::transforms::{bar_to_x, color4, price_to_y};
use crate::core::viewport::Viewport;

// ═══════════════════════════════════════════════════════════════════════════════
// Output Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Complete geometry output for footprint chart rendering.
/// Canvas2D renders rects via fill_rect and texts via fillText.
/// WebGPU renders rects via instanced quads; texts via overlay Canvas2D.
pub struct FootprintGeometry {
    /// All colored rectangles (cell backgrounds, POC markers, value area fills, etc.).
    pub rects: Vec<ColoredRect>,
    /// Text labels (volume numbers, delta values).
    pub texts: Vec<DrawText>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main Generator
// ═══════════════════════════════════════════════════════════════════════════════

/// Generate complete footprint chart geometry for one frame.
///
/// This is the SINGLE SOURCE OF TRUTH for footprint rendering — both
/// Canvas2D and WebGPU use this output identically.
pub fn generate_footprint_geometry(
    bars: &BarArray,
    viewport: &Viewport,
    style: &ChartStyle,
    fp_data: &FootprintData,
    fp_opts: &FootprintOptions,
    pane_w: f64,
    pane_h: f64,
    h_ratio: f64,
    v_ratio: f64,
) -> FootprintGeometry {
    let sizing = CandleSizing::compute_from_pane(pane_w, viewport, h_ratio, v_ratio);

    // Footprint doesn't use volume sub-pane — it integrates volume directly
    let candle_h = pane_h;

    let start = (viewport.start_bar.floor() as usize)
        .saturating_sub(1)
        .min(bars.len());
    let end = ((viewport.end_bar.ceil() as usize) + 1).min(bars.len());

    if start >= end {
        return FootprintGeometry {
            rects: Vec::new(),
            texts: Vec::new(),
        };
    }

    let visible_bars = end - start;
    // Pre-allocate conservatively: ~20 rects per bar (cells + decorations)
    let mut rects = Vec::with_capacity(visible_bars * 20);
    let mut texts = Vec::with_capacity(visible_bars * 10);

    let font_size = fp_opts.font_size * v_ratio as f32;

    for i in start..end {
        let bar = bars.get_unchecked(i);
        let fp_bar = match fp_data.get_bar(i) {
            Some(b) => b,
            None => {
                // No footprint data — render as a simple candlestick fallback
                generate_fallback_candle(
                    i, bar.open, bar.high, bar.low, bar.close, viewport, style, pane_w, candle_h,
                    &sizing, &mut rects,
                );
                continue;
            }
        };

        if fp_bar.levels.is_empty() {
            continue;
        }

        // ── Compute bar geometry ──
        let center_x = bar_to_x(i as f64 + 0.5, viewport, pane_w);
        let half_bar = (sizing.bar_width * 0.5).floor();
        let bar_left = (center_x - half_bar).round();
        let bar_width = sizing.bar_width;

        // ── Compute price-level geometry ──
        let tick_size = if fp_opts.tick_size > 0.0 {
            fp_opts.tick_size
        } else {
            // Auto-detect tick size from levels
            auto_tick_size(fp_bar)
        };

        // Precompute analytics
        let poc = fp_bar.poc();
        let _value_area = if fp_opts.show_value_area {
            fp_bar.value_area(fp_opts.value_area_pct)
        } else {
            None
        };
        let _stacked = if fp_opts.show_stacked_imbalances {
            fp_bar.stacked_imbalances(fp_opts.imbalance_ratio, fp_opts.stacked_imbalance_min)
        } else {
            Vec::new()
        };
        let (unfinished_low, unfinished_high) = if fp_opts.show_unfinished_auction {
            fp_bar.unfinished_auction()
        } else {
            (false, false)
        };
        let max_side_vol = fp_bar.max_side_volume().max(1.0);
        let max_total_vol = fp_bar.max_level_volume().max(1.0);
        let max_delta_abs = fp_bar.max_level_delta_abs().max(1.0);

        // ── Single background rect for the entire ladder ──
        // Spans from the highest price level top to the lowest price level bottom.
        {
            let first_level = &fp_bar.levels[0];
            let last_level = &fp_bar.levels[fp_bar.levels.len() - 1];
            let ladder_top = price_to_y(
                last_level.price as f64 + tick_size as f64,
                viewport,
                candle_h,
            )
            .round();
            let ladder_bottom = price_to_y(first_level.price as f64, viewport, candle_h).round();
            let ladder_h = (ladder_bottom - ladder_top).max(1.0);

            let (cbr, cbg, cbb, _) = color4(&fp_opts.cell_bg_color);
            rects.push(ColoredRect {
                x: bar_left as f32,
                y: ladder_top as f32,
                w: bar_width as f32,
                h: ladder_h as f32,
                r: cbr,
                g: cbg,
                b: cbb,
                a: fp_opts.cell_bg_color[3],
            });
        }

        // ── Render each price level ──
        for (level_idx, level) in fp_bar.levels.iter().enumerate() {
            let price_top = level.price as f64 + tick_size as f64;
            let price_bottom = level.price as f64;

            let y_top = price_to_y(price_top, viewport, candle_h).round();
            let y_bottom = price_to_y(price_bottom, viewport, candle_h).round();
            let cell_h = (y_bottom - y_top).max(1.0);

            // Skip cells outside visible area
            if y_bottom < 0.0 || y_top > candle_h {
                continue;
            }

            let cell_y = y_top;

            // Check if this level is the POC
            let is_poc = poc.map(|(idx, _)| idx == level_idx).unwrap_or(false);

            // Check for imbalances (used by BidAsk cell renderer for color)
            let imbalance = if fp_opts.show_imbalances {
                level.imbalance(fp_opts.imbalance_ratio)
            } else {
                ImbalanceType::None
            };

            // ── Cell dividers — horizontal between rows, vertical between bid/ask ──
            let (dr, dg, db, da) = color4(&style.grid_color);
            // Horizontal divider at top of cell
            rects.push(ColoredRect {
                x: bar_left as f32,
                y: cell_y as f32,
                w: bar_width as f32,
                h: 1.0,
                r: dr,
                g: dg,
                b: db,
                a: da * 0.5,
            });
            // Vertical divider between bid and ask
            let mid_x = bar_left + bar_width * 0.5;
            rects.push(ColoredRect {
                x: mid_x as f32,
                y: cell_y as f32,
                w: 1.0,
                h: cell_h as f32,
                r: dr,
                g: dg,
                b: db,
                a: da * 0.5,
            });

            // ── Render volume bars / text based on display mode ──
            match fp_opts.display_mode {
                FootprintDisplayMode::BidAsk => {
                    render_bid_ask_cell(
                        &mut rects,
                        &mut texts,
                        fp_opts,
                        level,
                        bar_left,
                        cell_y,
                        bar_width,
                        cell_h,
                        max_side_vol,
                        font_size,
                        imbalance,
                    );
                }
                FootprintDisplayMode::Delta => {
                    render_delta_cell(
                        &mut rects,
                        &mut texts,
                        fp_opts,
                        level,
                        bar_left,
                        cell_y,
                        bar_width,
                        cell_h,
                        max_delta_abs,
                        font_size,
                    );
                }
                FootprintDisplayMode::Volume => {
                    render_volume_cell(
                        &mut rects,
                        &mut texts,
                        fp_opts,
                        level,
                        bar_left,
                        cell_y,
                        bar_width,
                        cell_h,
                        max_total_vol,
                        font_size,
                    );
                }
                FootprintDisplayMode::DeltaProfile => {
                    render_delta_profile_cell(
                        &mut rects,
                        fp_opts,
                        level,
                        bar_left,
                        cell_y,
                        bar_width,
                        cell_h,
                        max_delta_abs,
                    );
                }
                FootprintDisplayMode::VolumeProfile => {
                    render_volume_profile_cell(
                        &mut rects,
                        fp_opts,
                        level,
                        bar_left,
                        cell_y,
                        bar_width,
                        cell_h,
                        max_total_vol,
                    );
                }
            }

            // ── POC marker ──
            if is_poc && fp_opts.show_poc {
                let poc_w = (bar_width * fp_opts.poc_width as f64).max(2.0);
                let (pr, pg, pb, pa) = color4(&fp_opts.poc_color);
                // Left POC marker
                rects.push(ColoredRect {
                    x: bar_left as f32,
                    y: cell_y as f32,
                    w: poc_w as f32,
                    h: cell_h as f32,
                    r: pr,
                    g: pg,
                    b: pb,
                    a: pa,
                });
                // Right POC marker
                rects.push(ColoredRect {
                    x: (bar_left + bar_width - poc_w) as f32,
                    y: cell_y as f32,
                    w: poc_w as f32,
                    h: cell_h as f32,
                    r: pr,
                    g: pg,
                    b: pb,
                    a: pa,
                });
            }

            // ── Unfinished auction markers ──
            if unfinished_low && level_idx == 0 {
                let (ur, ug, ub, ua) = color4(&fp_opts.unfinished_auction_color);
                rects.push(ColoredRect {
                    x: bar_left as f32,
                    y: (cell_y + cell_h - 2.0) as f32,
                    w: bar_width as f32,
                    h: 2.0,
                    r: ur,
                    g: ug,
                    b: ub,
                    a: ua,
                });
            }
            if unfinished_high && level_idx == fp_bar.levels.len() - 1 {
                let (ur, ug, ub, ua) = color4(&fp_opts.unfinished_auction_color);
                rects.push(ColoredRect {
                    x: bar_left as f32,
                    y: cell_y as f32,
                    w: bar_width as f32,
                    h: 2.0,
                    r: ur,
                    g: ug,
                    b: ub,
                    a: ua,
                });
            }
        }

        // ── Delta summary bar at bar bottom ──
        if fp_opts.show_delta_bar {
            render_delta_bar(
                &mut rects, &mut texts, fp_opts, fp_bar, bar_left, candle_h, bar_width, font_size,
                v_ratio,
            );
        }
    }

    FootprintGeometry { rects, texts }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cell Renderers (per display mode)
// ═══════════════════════════════════════════════════════════════════════════════

/// BidAsk mode: bid volume bar + text on left, ask volume bar + text on right.
fn render_bid_ask_cell(
    rects: &mut Vec<ColoredRect>,
    texts: &mut Vec<DrawText>,
    opts: &FootprintOptions,
    level: &crate::core::footprint::FootprintLevel,
    bar_left: f64,
    cell_y: f64,
    bar_width: f64,
    cell_h: f64,
    max_side_vol: f32,
    font_size: f32,
    imbalance: ImbalanceType,
) {
    let half_w = bar_width * 0.5;
    let padding = opts.cell_padding as f64;

    // Bid (sell) side — left half, bar grows from center to left
    let bid_frac = (level.bid_volume / max_side_vol).clamp(0.0, 1.0);
    let bid_bar_w = (half_w - padding * 2.0) * bid_frac as f64;
    let bid_color = match imbalance {
        ImbalanceType::SellImbalance => opts.sell_imbalance_color,
        _ => intensity_blend(
            opts.sell_color,
            opts.cell_bg_color,
            bid_frac,
            &opts.volume_color_intensity,
        ),
    };
    if bid_bar_w > 0.5 {
        rects.push(ColoredRect {
            x: (bar_left + half_w - padding - bid_bar_w) as f32,
            y: (cell_y + 1.0) as f32,
            w: bid_bar_w as f32,
            h: (cell_h - 2.0).max(1.0) as f32,
            r: bid_color[0],
            g: bid_color[1],
            b: bid_color[2],
            a: bid_color[3],
        });
    }

    // Ask (buy) side — right half, bar grows from center to right
    let ask_frac = (level.ask_volume / max_side_vol).clamp(0.0, 1.0);
    let ask_bar_w = (half_w - padding * 2.0) * ask_frac as f64;
    let ask_color = match imbalance {
        ImbalanceType::BuyImbalance => opts.buy_imbalance_color,
        _ => intensity_blend(
            opts.buy_color,
            opts.cell_bg_color,
            ask_frac,
            &opts.volume_color_intensity,
        ),
    };
    if ask_bar_w > 0.5 {
        rects.push(ColoredRect {
            x: (bar_left + half_w + padding) as f32,
            y: (cell_y + 1.0) as f32,
            w: ask_bar_w as f32,
            h: (cell_h - 2.0).max(1.0) as f32,
            r: ask_color[0],
            g: ask_color[1],
            b: ask_color[2],
            a: ask_color[3],
        });
    }

    // Volume text — adaptive font size to fit cell
    let effective_font = adaptive_font_size(font_size, cell_h);
    if opts.show_volume_text && effective_font > 0.0 {
        let text_y = cell_y + cell_h * 0.5;

        // Bid text (right-aligned within left half)
        if level.bid_volume > 0.0 {
            texts.push(DrawText {
                text: format_volume(level.bid_volume),
                x: (bar_left + half_w - padding - 2.0) as f32,
                y: text_y as f32,
                font_size: effective_font,
                r: opts.text_color[0],
                g: opts.text_color[1],
                b: opts.text_color[2],
                a: opts.text_color[3],
                align: TextAlign::Right,
            });
        }

        // Ask text (left-aligned within right half)
        if level.ask_volume > 0.0 {
            texts.push(DrawText {
                text: format_volume(level.ask_volume),
                x: (bar_left + half_w + padding + 2.0) as f32,
                y: text_y as f32,
                font_size: effective_font,
                r: opts.text_color[0],
                g: opts.text_color[1],
                b: opts.text_color[2],
                a: opts.text_color[3],
                align: TextAlign::Left,
            });
        }
    }
}

/// Delta mode: single delta value per level with color coding.
fn render_delta_cell(
    rects: &mut Vec<ColoredRect>,
    texts: &mut Vec<DrawText>,
    opts: &FootprintOptions,
    level: &crate::core::footprint::FootprintLevel,
    bar_left: f64,
    cell_y: f64,
    bar_width: f64,
    cell_h: f64,
    max_delta_abs: f32,
    font_size: f32,
) {
    let delta = level.delta();
    let delta_frac = (delta.abs() / max_delta_abs).clamp(0.0, 1.0);
    let padding = opts.cell_padding as f64;

    let color = if delta >= 0.0 {
        intensity_blend(
            opts.positive_delta_color,
            opts.cell_bg_color,
            delta_frac,
            &opts.volume_color_intensity,
        )
    } else {
        intensity_blend(
            opts.negative_delta_color,
            opts.cell_bg_color,
            delta_frac,
            &opts.volume_color_intensity,
        )
    };

    // Delta bar filling from center
    let fill_w = (bar_width - padding * 2.0) * delta_frac as f64;
    if fill_w > 0.5 {
        let x = if delta >= 0.0 {
            bar_left + bar_width * 0.5
        } else {
            bar_left + bar_width * 0.5 - fill_w
        };
        rects.push(ColoredRect {
            x: x as f32,
            y: (cell_y + 1.0) as f32,
            w: fill_w as f32,
            h: (cell_h - 2.0).max(1.0) as f32,
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        });
    }

    // Delta text
    let effective_font = adaptive_font_size(font_size, cell_h);
    if opts.show_volume_text && effective_font > 0.0 {
        let text_color = if delta >= 0.0 {
            opts.positive_delta_color
        } else {
            opts.negative_delta_color
        };
        texts.push(DrawText {
            text: format_delta(delta),
            x: (bar_left + bar_width * 0.5) as f32,
            y: (cell_y + cell_h * 0.5) as f32,
            font_size: effective_font,
            r: text_color[0],
            g: text_color[1],
            b: text_color[2],
            a: text_color[3],
            align: TextAlign::Center,
        });
    }
}

/// Volume mode: single total volume per level.
fn render_volume_cell(
    rects: &mut Vec<ColoredRect>,
    texts: &mut Vec<DrawText>,
    opts: &FootprintOptions,
    level: &crate::core::footprint::FootprintLevel,
    bar_left: f64,
    cell_y: f64,
    bar_width: f64,
    cell_h: f64,
    max_total_vol: f32,
    font_size: f32,
) {
    let vol = level.total_volume();
    let vol_frac = (vol / max_total_vol).clamp(0.0, 1.0);
    let padding = opts.cell_padding as f64;

    let is_bullish = level.ask_volume >= level.bid_volume;
    let base_color = if is_bullish {
        opts.buy_color
    } else {
        opts.sell_color
    };
    let color = intensity_blend(
        base_color,
        opts.cell_bg_color,
        vol_frac,
        &opts.volume_color_intensity,
    );

    // Volume fill bar
    let fill_w = (bar_width - padding * 2.0) * vol_frac as f64;
    if fill_w > 0.5 {
        rects.push(ColoredRect {
            x: (bar_left + padding) as f32,
            y: (cell_y + 1.0) as f32,
            w: fill_w as f32,
            h: (cell_h - 2.0).max(1.0) as f32,
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        });
    }

    // Volume text
    let effective_font = adaptive_font_size(font_size, cell_h);
    if opts.show_volume_text && effective_font > 0.0 {
        texts.push(DrawText {
            text: format_volume(vol),
            x: (bar_left + bar_width * 0.5) as f32,
            y: (cell_y + cell_h * 0.5) as f32,
            font_size: effective_font,
            r: opts.text_color[0],
            g: opts.text_color[1],
            b: opts.text_color[2],
            a: opts.text_color[3],
            align: TextAlign::Center,
        });
    }
}

/// Delta Profile mode: horizontal bar showing delta magnitude.
fn render_delta_profile_cell(
    rects: &mut Vec<ColoredRect>,
    opts: &FootprintOptions,
    level: &crate::core::footprint::FootprintLevel,
    bar_left: f64,
    cell_y: f64,
    bar_width: f64,
    cell_h: f64,
    max_delta_abs: f32,
) {
    let delta = level.delta();
    let delta_frac = (delta.abs() / max_delta_abs).clamp(0.0, 1.0);
    let padding = opts.cell_padding as f64;
    let max_w = bar_width - padding * 2.0;
    let fill_w = max_w * delta_frac as f64;

    let color = if delta >= 0.0 {
        opts.positive_delta_color
    } else {
        opts.negative_delta_color
    };

    if fill_w > 0.5 {
        // Bar extends from center
        let x = if delta >= 0.0 {
            bar_left + bar_width * 0.5
        } else {
            bar_left + bar_width * 0.5 - fill_w
        };
        rects.push(ColoredRect {
            x: x as f32,
            y: (cell_y + 1.0) as f32,
            w: fill_w as f32,
            h: (cell_h - 2.0).max(1.0) as f32,
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        });
    }
}

/// Volume Profile mode: horizontal bar showing total volume.
fn render_volume_profile_cell(
    rects: &mut Vec<ColoredRect>,
    opts: &FootprintOptions,
    level: &crate::core::footprint::FootprintLevel,
    bar_left: f64,
    cell_y: f64,
    bar_width: f64,
    cell_h: f64,
    max_total_vol: f32,
) {
    let vol = level.total_volume();
    let vol_frac = (vol / max_total_vol).clamp(0.0, 1.0);
    let padding = opts.cell_padding as f64;
    let max_w = bar_width - padding * 2.0;
    let fill_w = max_w * vol_frac as f64;

    let is_bullish = level.ask_volume >= level.bid_volume;
    let color = if is_bullish {
        opts.buy_color
    } else {
        opts.sell_color
    };

    if fill_w > 0.5 {
        rects.push(ColoredRect {
            x: (bar_left + padding) as f32,
            y: (cell_y + 1.0) as f32,
            w: fill_w as f32,
            h: (cell_h - 2.0).max(1.0) as f32,
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        });
    }
}

/// Render the delta summary bar at the bottom of a footprint bar.
fn render_delta_bar(
    rects: &mut Vec<ColoredRect>,
    texts: &mut Vec<DrawText>,
    opts: &FootprintOptions,
    fp_bar: &FootprintBar,
    bar_left: f64,
    candle_h: f64,
    bar_width: f64,
    font_size: f32,
    _v_ratio: f64,
) {
    let delta = fp_bar.net_delta();
    let bar_h = opts.delta_bar_height as f64;
    let y = candle_h - bar_h;

    let color = if delta >= 0.0 {
        opts.cum_delta_positive_color
    } else {
        opts.cum_delta_negative_color
    };

    // Background
    rects.push(ColoredRect {
        x: bar_left as f32,
        y: y as f32,
        w: bar_width as f32,
        h: bar_h as f32,
        r: color[0],
        g: color[1],
        b: color[2],
        a: color[3] * 0.4,
    });

    // Delta text
    if opts.show_volume_text {
        texts.push(DrawText {
            text: format_delta(delta),
            x: (bar_left + bar_width * 0.5) as f32,
            y: (y + bar_h * 0.5) as f32,
            font_size: font_size * 0.9,
            r: color[0],
            g: color[1],
            b: color[2],
            a: 1.0,
            align: TextAlign::Center,
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Fallback Candle (when a bar lacks footprint data)
// ═══════════════════════════════════════════════════════════════════════════════

/// Render a simple candlestick for bars without footprint data.
fn generate_fallback_candle(
    bar_idx: usize,
    open: f32,
    high: f32,
    low: f32,
    close: f32,
    vp: &Viewport,
    style: &ChartStyle,
    chart_w: f64,
    candle_h: f64,
    sizing: &CandleSizing,
    rects: &mut Vec<ColoredRect>,
) {
    let bull = close >= open;
    let (cr, cg, cb, ca) = if bull {
        color4(&style.bullish_color)
    } else {
        color4(&style.bearish_color)
    };
    let (wr, wg, wb, wa) = if bull {
        color4(&style.wick_bullish_color)
    } else {
        color4(&style.wick_bearish_color)
    };

    let center_x = bar_to_x(bar_idx as f64 + 0.5, vp, chart_w).round();
    let body_top = price_to_y(open.max(close) as f64, vp, candle_h).round();
    let body_bottom = price_to_y(open.min(close) as f64, vp, candle_h).round();
    let high_y = price_to_y(high as f64, vp, candle_h).round();
    let low_y = price_to_y(low as f64, vp, candle_h).round();

    let wick_offset = (sizing.wick_width * 0.5).floor();
    let half_bar = (sizing.bar_width * 0.5).floor();

    // Wick
    if body_top > high_y {
        rects.push(ColoredRect {
            x: (center_x - wick_offset) as f32,
            y: high_y as f32,
            w: sizing.wick_width as f32,
            h: (body_top - high_y) as f32,
            r: wr,
            g: wg,
            b: wb,
            a: wa,
        });
    }
    if low_y > body_bottom {
        rects.push(ColoredRect {
            x: (center_x - wick_offset) as f32,
            y: (body_bottom + 1.0) as f32,
            w: sizing.wick_width as f32,
            h: (low_y - body_bottom) as f32,
            r: wr,
            g: wg,
            b: wb,
            a: wa,
        });
    }

    // Body
    let left = center_x - half_bar;
    let h = (body_bottom - body_top + 1.0).max(1.0);
    rects.push(ColoredRect {
        x: left as f32,
        y: body_top as f32,
        w: sizing.bar_width as f32,
        h: h as f32,
        r: cr,
        g: cg,
        b: cb,
        a: ca,
    });
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Compute adaptive font size that fits within a cell.
/// Returns the font size in physical pixels, or 0.0 if the cell is too small for any text.
/// Minimum readable text is ~5px physical; below that we skip text entirely.
#[inline]
fn adaptive_font_size(max_font: f32, cell_h: f64) -> f32 {
    let min_font: f32 = 5.0;
    // Leave 2px padding (1px top + 1px bottom)
    let available = (cell_h - 2.0) as f32;
    if available < min_font {
        return 0.0;
    }
    available.min(max_font)
}

/// Auto-detect tick size from footprint levels.
fn auto_tick_size(bar: &FootprintBar) -> f32 {
    if bar.levels.len() < 2 {
        return 1.0;
    }
    let mut min_diff = f32::MAX;
    for i in 1..bar.levels.len() {
        let diff = (bar.levels[i].price - bar.levels[i - 1].price).abs();
        if diff > 0.0 && diff < min_diff {
            min_diff = diff;
        }
    }
    if min_diff == f32::MAX || min_diff <= 0.0 {
        1.0
    } else {
        min_diff
    }
}

/// Blend color `over` on top of `base` using standard alpha compositing.
#[allow(dead_code)]
fn blend_over(base: [f32; 4], over: [f32; 4]) -> [f32; 4] {
    let oa = over[3];
    let ba = base[3];
    let out_a = oa + ba * (1.0 - oa);
    if out_a <= 0.0 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    [
        (over[0] * oa + base[0] * ba * (1.0 - oa)) / out_a,
        (over[1] * oa + base[1] * ba * (1.0 - oa)) / out_a,
        (over[2] * oa + base[2] * ba * (1.0 - oa)) / out_a,
        out_a,
    ]
}

/// Blend between cell_bg and volume_color based on intensity mode.
fn intensity_blend(
    color: [f32; 4],
    bg: [f32; 4],
    frac: f32,
    mode: &VolumeColorIntensity,
) -> [f32; 4] {
    let t = match mode {
        VolumeColorIntensity::None => 0.6, // Fixed opacity
        VolumeColorIntensity::Linear => 0.2 + 0.8 * frac,
        VolumeColorIntensity::Logarithmic => {
            if frac <= 0.0 {
                0.2
            } else {
                0.2 + 0.8 * (1.0 + frac.ln() / 4.6).clamp(0.0, 1.0) // ln(100)/4.6 ≈ 1.0
            }
        }
    };

    [
        bg[0] + (color[0] - bg[0]) * t,
        bg[1] + (color[1] - bg[1]) * t,
        bg[2] + (color[2] - bg[2]) * t,
        (color[3] * t).clamp(0.0, 1.0),
    ]
}

/// Format volume for display (compact notation).
fn format_volume(vol: f32) -> String {
    if vol >= 1_000_000.0 {
        format!("{:.1}M", vol / 1_000_000.0)
    } else if vol >= 10_000.0 {
        format!("{:.1}K", vol / 1_000.0)
    } else if vol >= 1_000.0 {
        format!("{:.0}", vol)
    } else if vol >= 1.0 {
        format!("{:.0}", vol)
    } else if vol > 0.0 {
        format!("{:.2}", vol)
    } else {
        String::new()
    }
}

/// Format delta for display (with sign).
fn format_delta(delta: f32) -> String {
    let abs = delta.abs();
    let sign = if delta >= 0.0 { "+" } else { "-" };
    if abs >= 1_000_000.0 {
        format!("{}{:.1}M", sign, abs / 1_000_000.0)
    } else if abs >= 10_000.0 {
        format!("{}{:.1}K", sign, abs / 1_000.0)
    } else if abs >= 1.0 {
        format!("{}{:.0}", sign, abs)
    } else if abs > 0.0 {
        format!("{}{:.2}", sign, abs)
    } else {
        "0".to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_volume() {
        assert_eq!(format_volume(0.0), "");
        assert_eq!(format_volume(0.5), "0.50");
        assert_eq!(format_volume(42.0), "42");
        assert_eq!(format_volume(1234.0), "1234");
        assert_eq!(format_volume(15000.0), "15.0K");
        assert_eq!(format_volume(2_500_000.0), "2.5M");
    }

    #[test]
    fn test_format_delta() {
        assert_eq!(format_delta(0.0), "0");
        assert_eq!(format_delta(50.0), "+50");
        assert_eq!(format_delta(-30.0), "-30");
        assert_eq!(format_delta(15000.0), "+15.0K");
        assert_eq!(format_delta(-2_500_000.0), "-2.5M");
    }

    #[test]
    fn test_blend_over() {
        let base = [0.0, 0.0, 0.0, 1.0]; // black
        let over = [1.0, 1.0, 1.0, 0.5]; // 50% white
        let result = blend_over(base, over);
        // Should be gray-ish
        assert!(result[0] > 0.4 && result[0] < 0.6);
        assert!((result[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_auto_tick_size() {
        let bar = crate::core::footprint::FootprintBar {
            levels: vec![
                crate::core::footprint::FootprintLevel {
                    price: 100.0,
                    bid_volume: 100.0,
                    ask_volume: 100.0,
                },
                crate::core::footprint::FootprintLevel {
                    price: 100.5,
                    bid_volume: 100.0,
                    ask_volume: 100.0,
                },
                crate::core::footprint::FootprintLevel {
                    price: 101.0,
                    bid_volume: 100.0,
                    ask_volume: 100.0,
                },
            ],
        };
        assert_eq!(auto_tick_size(&bar), 0.5);
    }
}
