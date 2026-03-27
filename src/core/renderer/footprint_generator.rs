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
    FootprintBar, FootprintData, FootprintDisplayMode, FootprintGradientStyle, FootprintOptions,
    ImbalanceType, VolumeColorIntensity,
};
use crate::core::renderer::draw_list::{ColoredRect, DrawText, HorizontalGradientRect, TextAlign};
use crate::core::renderer::series::CandleSizing;
use crate::core::renderer::traits::ChartStyle;
use crate::core::renderer::transforms::{bar_to_x, color4, price_to_y};
use crate::core::renderer::value_projection::TimeScaleIndex;
use crate::core::viewport::Viewport;

#[inline]
fn visible_main_bar_range(
    bars: &BarArray,
    viewport: &Viewport,
    time_scale: &TimeScaleIndex,
) -> Option<(usize, usize)> {
    if bars.is_empty() {
        return None;
    }
    time_scale.visible_main_bar_range(viewport.start_bar - 1.0, viewport.end_bar + 1.0)
}

#[inline]
fn main_bar_center_x(
    bar_index: usize,
    viewport: &Viewport,
    time_scale: &TimeScaleIndex,
    chart_w: f64,
) -> Option<f64> {
    time_scale
        .logical_index_for_main_bar(bar_index)
        .map(|logical_index| bar_to_x(logical_index + 0.5, viewport, chart_w))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Output Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Complete geometry output for footprint chart rendering.
/// Canvas2D renders rects via fill_rect and texts via fillText.
/// WebGPU renders rects via instanced quads; texts via overlay Canvas2D.
pub struct FootprintGeometry {
    /// Flat geometry rendered beneath gradient-filled cells.
    pub base_rects: Vec<ColoredRect>,
    /// Smooth horizontal cell fills.
    pub gradient_rects: Vec<HorizontalGradientRect>,
    /// Flat geometry rendered above gradient-filled cells.
    pub overlay_rects: Vec<ColoredRect>,
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
    time_scale: &TimeScaleIndex,
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

    // Standard candle area height — same formula as every other chart type.
    // In Footprint mode the engine sets volume_height_ratio = 0, so this
    // evaluates to pane_h (full pane).  No special-casing needed.
    let vol_h = pane_h * viewport.volume_height_ratio as f64;
    let candle_h = pane_h - vol_h;

    let Some((start, end)) = visible_main_bar_range(bars, viewport, time_scale) else {
        return FootprintGeometry {
            base_rects: Vec::new(),
            gradient_rects: Vec::new(),
            overlay_rects: Vec::new(),
            texts: Vec::new(),
        };
    };

    if start >= end {
        return FootprintGeometry {
            base_rects: Vec::new(),
            gradient_rects: Vec::new(),
            overlay_rects: Vec::new(),
            texts: Vec::new(),
        };
    }

    let visible_bars = end - start;
    // Pre-allocate conservatively: ~20 rects per bar (cells + decorations)
    let mut base_rects = Vec::with_capacity(visible_bars * 10);
    let mut gradient_rects = Vec::with_capacity(visible_bars * 12);
    let mut overlay_rects = Vec::with_capacity(visible_bars * 14);
    let mut texts = Vec::with_capacity(visible_bars * 10);

    let font_size = fp_opts.font_size * v_ratio as f32;
    // Dynamic aggregation threshold:
    // keep rows readable when text is shown, otherwise honor raw min_cell_height.
    let min_cell_px = fp_opts.aggregation_min_cell_height_css() * v_ratio;

    for i in start..end {
        let bar = bars.get_unchecked(i);
        let fp_bar = match fp_data.get_bar(i) {
            Some(b) => b,
            None => {
                // No footprint data — render as a simple candlestick fallback
                generate_fallback_candle(
                    i,
                    time_scale,
                    bar.open,
                    bar.high,
                    bar.low,
                    bar.close,
                    viewport,
                    style,
                    pane_w,
                    candle_h,
                    &sizing,
                    &mut overlay_rects,
                );
                continue;
            }
        };

        if fp_bar.levels.is_empty() {
            generate_fallback_candle(
                i,
                time_scale,
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                viewport,
                style,
                pane_w,
                candle_h,
                &sizing,
                &mut overlay_rects,
            );
            continue;
        }

        // ── Compute bar geometry ──
        // Layout: [candle | gap | ladder]
        // The candle occupies the left portion, the ladder the right.
        let Some(center_x) = main_bar_center_x(i, viewport, time_scale, pane_w) else {
            continue;
        };
        let half_bar = (sizing.bar_width * 0.5).floor();
        let slot_left = (center_x - half_bar).round();
        let slot_width = sizing.bar_width;

        // Candle takes ~15% of slot, gap ~2%, ladder gets the rest (~83%).
        // Matches ATAS/Sierra Chart proportions — the ladder is the dominant
        // visual element, the candle provides OHLC context on the side.
        let candle_frac = 0.15_f64;
        let gap_frac = 0.02_f64;
        let candle_w = (slot_width * candle_frac).round().max(3.0);
        let gap_w = (slot_width * gap_frac).round().max(1.0);
        let ladder_left = slot_left + candle_w + gap_w;
        let bar_left = ladder_left;
        let bar_width = (slot_width - candle_w - gap_w).max(1.0);

        // ── Compute tick size BEFORE candle rendering so the body can snap ──
        let base_tick = if fp_opts.tick_size > 0.0 {
            fp_opts.tick_size
        } else {
            auto_tick_size(fp_bar)
        };

        // ── Dynamic aggregation ──
        let natural_cell_h = {
            let y0 = price_to_y(fp_bar.levels[0].price as f64, viewport, candle_h);
            let y1 = price_to_y(
                fp_bar.levels[0].price as f64 + base_tick as f64,
                viewport,
                candle_h,
            );
            (y0 - y1).abs()
        };
        let agg_factor =
            if min_cell_px > 0.0 && natural_cell_h > 0.0 && natural_cell_h < min_cell_px {
                ((min_cell_px / natural_cell_h).ceil() as usize).max(1)
            } else {
                1
            };
        let effective_tick = base_tick * agg_factor as f32;

        // ── Render candlestick on the left side ──
        // The candle body is snapped to the tick grid so it aligns perfectly
        // with the ladder cell boundaries.  The live price line (at raw close)
        // sits INSIDE the snapped body because ceil/floor expand the body to
        // encompass the raw OHLC range.  Both use the same Y coordinate space
        // (volume_height_ratio = 0 → candle_area == full pane) so they are
        // properly connected.
        //
        // Keep candle geometry in the later overlay layer instead of the flat
        // background layer. That guarantees the candle stays visible over the
        // ladder background/fills in both Canvas2D and WebGPU.
        {
            let bull = bar.close >= bar.open;
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

            let candle_center = slot_left + candle_w * 0.5;

            // Snap open/close to tick grid boundaries so the candle body
            // aligns exactly with the ladder cell edges.
            let et = effective_tick as f64;
            let body_high_price = bar.open.max(bar.close) as f64;
            let body_low_price = bar.open.min(bar.close) as f64;
            let snapped_high = (body_high_price / et).ceil() * et;
            let snapped_low = (body_low_price / et).floor() * et;

            let body_top = price_to_y(snapped_high, viewport, candle_h).round();
            let body_bottom = price_to_y(snapped_low, viewport, candle_h).round();
            let high_y = price_to_y(bar.high as f64, viewport, candle_h).round();
            let low_y = price_to_y(bar.low as f64, viewport, candle_h).round();

            // Wick (1px wide, centered in candle area)
            let wick_w = 1.0_f64.max(sizing.wick_width.min(candle_w * 0.4));
            let wick_x = (candle_center - wick_w * 0.5).round();

            // Upper wick
            if body_top > high_y {
                overlay_rects.push(ColoredRect {
                    x: wick_x as f32,
                    y: high_y as f32,
                    w: wick_w as f32,
                    h: (body_top - high_y) as f32,
                    r: wr,
                    g: wg,
                    b: wb,
                    a: wa,
                });
            }
            // Lower wick
            if low_y > body_bottom {
                overlay_rects.push(ColoredRect {
                    x: wick_x as f32,
                    y: (body_bottom + 1.0) as f32,
                    w: wick_w as f32,
                    h: (low_y - body_bottom) as f32,
                    r: wr,
                    g: wg,
                    b: wb,
                    a: wa,
                });
            }

            // Body
            let body_w = candle_w.max(1.0);
            let body_h = (body_bottom - body_top + 1.0).max(1.0);
            overlay_rects.push(ColoredRect {
                x: slot_left as f32,
                y: body_top as f32,
                w: body_w as f32,
                h: body_h as f32,
                r: cr,
                g: cg,
                b: cb,
                a: ca,
            });
        }

        // ── Aggregate levels using the already-computed factor ──
        let levels = fp_bar.aggregate_levels(agg_factor);

        // Precompute analytics (on original bar for correctness)
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

        // Max volumes are recomputed from aggregated levels so color
        // intensity scales correctly at the merged granularity.
        let max_side_vol = levels
            .iter()
            .map(|l| l.bid_volume.max(l.ask_volume))
            .fold(0.0f32, f32::max)
            .max(1.0);
        let max_total_vol = levels
            .iter()
            .map(|l| l.bid_volume + l.ask_volume)
            .fold(0.0f32, f32::max)
            .max(1.0);
        let max_delta_abs = levels
            .iter()
            .map(|l| (l.ask_volume - l.bid_volume).abs())
            .fold(0.0f32, f32::max)
            .max(1.0);

        // ── Single background rect for the entire ladder ──
        {
            let first_level = &levels[0];
            let last_level = &levels[levels.len() - 1];
            let ladder_top = price_to_y(
                last_level.price as f64 + effective_tick as f64,
                viewport,
                candle_h,
            )
            .round();
            let ladder_bottom = price_to_y(first_level.price as f64, viewport, candle_h).round();
            let ladder_h = (ladder_bottom - ladder_top).max(1.0);

            let (cbr, cbg, cbb, _) = color4(&fp_opts.cell_bg_color);
            base_rects.push(ColoredRect {
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

        // ── Render each (possibly aggregated) price level ──
        for (level_idx, level) in levels.iter().enumerate() {
            let price_top = level.price as f64 + effective_tick as f64;
            let price_bottom = level.price as f64;

            let y_top = price_to_y(price_top, viewport, candle_h).round();
            let y_bottom = price_to_y(price_bottom, viewport, candle_h).round();
            let cell_h = (y_bottom - y_top).max(1.0);

            // Skip cells outside visible area
            if y_bottom < 0.0 || y_top > candle_h {
                continue;
            }

            let cell_y = y_top;

            // Check if this level contains the POC price (works across aggregation)
            let is_poc = poc
                .map(|(_, poc_lvl)| {
                    let poc_p = poc_lvl.price;
                    poc_p >= level.price && (poc_p as f64) < price_top
                })
                .unwrap_or(false);

            // Check for imbalances (used by BidAsk cell renderer for color)
            let imbalance = if fp_opts.show_imbalances {
                level.imbalance(fp_opts.imbalance_ratio)
            } else {
                ImbalanceType::None
            };

            // ── Cell outline — proper row border, no internal bid/ask divider ──
            let (dr, dg, db, da) = color4(&fp_opts.cell_border_color);
            // Top border
            overlay_rects.push(ColoredRect {
                x: bar_left as f32,
                y: cell_y as f32,
                w: bar_width as f32,
                h: 1.0,
                r: dr,
                g: dg,
                b: db,
                a: da,
            });
            // Left border
            overlay_rects.push(ColoredRect {
                x: bar_left as f32,
                y: cell_y as f32,
                w: 1.0,
                h: cell_h as f32,
                r: dr,
                g: dg,
                b: db,
                a: da,
            });
            // Right border
            overlay_rects.push(ColoredRect {
                x: (bar_left + bar_width - 1.0).max(bar_left) as f32,
                y: cell_y as f32,
                w: 1.0,
                h: cell_h as f32,
                r: dr,
                g: dg,
                b: db,
                a: da,
            });
            // Bottom border only for the last row so shared row separators stay 1px.
            if level_idx == 0 {
                overlay_rects.push(ColoredRect {
                    x: bar_left as f32,
                    y: (cell_y + cell_h - 1.0).max(cell_y) as f32,
                    w: bar_width as f32,
                    h: 1.0,
                    r: dr,
                    g: dg,
                    b: db,
                    a: da,
                });
            }

            // ── Render volume bars / text based on display mode ──
            match fp_opts.display_mode {
                FootprintDisplayMode::BidAsk => {
                    render_bid_ask_cell(
                        &mut gradient_rects,
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
                        &mut gradient_rects,
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
                        &mut gradient_rects,
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
                        &mut gradient_rects,
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
                        &mut gradient_rects,
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
                overlay_rects.push(ColoredRect {
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
                overlay_rects.push(ColoredRect {
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
                overlay_rects.push(ColoredRect {
                    x: bar_left as f32,
                    y: (cell_y + cell_h - 2.0).max(cell_y) as f32,
                    w: bar_width as f32,
                    h: 2.0,
                    r: ur,
                    g: ug,
                    b: ub,
                    a: ua,
                });
            }
            if unfinished_high && level_idx == levels.len() - 1 {
                let (ur, ug, ub, ua) = color4(&fp_opts.unfinished_auction_color);
                overlay_rects.push(ColoredRect {
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
                &mut overlay_rects,
                &mut texts,
                fp_opts,
                fp_bar,
                bar_left,
                candle_h,
                bar_width,
                font_size,
                v_ratio,
            );
        }
    }

    FootprintGeometry {
        base_rects,
        gradient_rects,
        overlay_rects,
        texts,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cell Renderers (per display mode)
// ═══════════════════════════════════════════════════════════════════════════════

/// BidAsk mode: bid volume bar + text on left, ask volume bar + text on right.
fn render_bid_ask_cell(
    gradient_rects: &mut Vec<HorizontalGradientRect>,
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
        _ => opts.sell_color,
    };
    if bid_bar_w > 0.5 {
        push_horizontal_gradient_rects(
            gradient_rects,
            bar_left + half_w - padding - bid_bar_w,
            cell_y + 1.0,
            bid_bar_w,
            (cell_h - 2.0).max(1.0),
            bid_color,
            opts.cell_bg_color,
            bid_frac,
            &opts.volume_color_intensity,
            opts.gradient_style,
            HorizontalGradientDirection::RightToLeft,
        );
    }

    // Ask (buy) side — right half, bar grows from center to right
    let ask_frac = (level.ask_volume / max_side_vol).clamp(0.0, 1.0);
    let ask_bar_w = (half_w - padding * 2.0) * ask_frac as f64;
    let ask_color = match imbalance {
        ImbalanceType::BuyImbalance => opts.buy_imbalance_color,
        _ => opts.buy_color,
    };
    if ask_bar_w > 0.5 {
        push_horizontal_gradient_rects(
            gradient_rects,
            bar_left + half_w + padding,
            cell_y + 1.0,
            ask_bar_w,
            (cell_h - 2.0).max(1.0),
            ask_color,
            opts.cell_bg_color,
            ask_frac,
            &opts.volume_color_intensity,
            opts.gradient_style,
            HorizontalGradientDirection::LeftToRight,
        );
    }

    // Volume text — adaptive font size to fit cell
    let effective_font = adaptive_font_size(font_size, cell_h);
    let avail_half_w = half_w - padding * 2.0 - 2.0; // usable text width per side
    let show_half_text = opts.show_volume_text
        && footprint_text_slot_allows_text(
            effective_font,
            cell_h,
            avail_half_w,
            FootprintTextSlot::HalfVolume,
        );
    if show_half_text {
        let text_y = cell_y + cell_h * 0.5;
        let bid_text_x = bar_left + half_w * 0.5;
        let ask_text_x = bar_left + half_w * 1.5;

        // Bid text centered within the left half-cell.
        if level.bid_volume > 0.0 {
            let txt = format_volume(level.bid_volume);
            texts.push(DrawText {
                text: txt,
                x: bid_text_x as f32,
                y: text_y as f32,
                font_size: effective_font,
                r: opts.text_color[0],
                g: opts.text_color[1],
                b: opts.text_color[2],
                a: opts.text_color[3],
                align: TextAlign::Center,
            });
        }

        // Ask text centered within the right half-cell.
        if level.ask_volume > 0.0 {
            let txt = format_volume(level.ask_volume);
            texts.push(DrawText {
                text: txt,
                x: ask_text_x as f32,
                y: text_y as f32,
                font_size: effective_font,
                r: opts.text_color[0],
                g: opts.text_color[1],
                b: opts.text_color[2],
                a: opts.text_color[3],
                align: TextAlign::Center,
            });
        }
    }
}

/// Delta mode: single delta value per level with color coding.
fn render_delta_cell(
    gradient_rects: &mut Vec<HorizontalGradientRect>,
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
        opts.positive_delta_color
    } else {
        opts.negative_delta_color
    };

    // Delta bar filling from center
    let fill_w = (bar_width - padding * 2.0) * delta_frac as f64;
    if fill_w > 0.5 {
        let x = if delta >= 0.0 {
            bar_left + bar_width * 0.5
        } else {
            bar_left + bar_width * 0.5 - fill_w
        };
        push_horizontal_gradient_rects(
            gradient_rects,
            x,
            cell_y + 1.0,
            fill_w,
            (cell_h - 2.0).max(1.0),
            color,
            opts.cell_bg_color,
            delta_frac,
            &opts.volume_color_intensity,
            opts.gradient_style,
            if delta >= 0.0 {
                HorizontalGradientDirection::LeftToRight
            } else {
                HorizontalGradientDirection::RightToLeft
            },
        );
    }

    // Delta text
    let effective_font = adaptive_font_size(font_size, cell_h);
    let avail_w = bar_width - opts.cell_padding as f64 * 2.0;
    if opts.show_volume_text
        && footprint_text_slot_allows_text(
            effective_font,
            cell_h,
            avail_w,
            FootprintTextSlot::Delta,
        )
    {
        let txt = format_delta(delta);
        let text_color = if delta >= 0.0 {
            opts.positive_delta_color
        } else {
            opts.negative_delta_color
        };
        texts.push(DrawText {
            text: txt,
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
    gradient_rects: &mut Vec<HorizontalGradientRect>,
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
    // Volume fill bar
    let fill_w = (bar_width - padding * 2.0) * vol_frac as f64;
    if fill_w > 0.5 {
        push_horizontal_gradient_rects(
            gradient_rects,
            bar_left + padding,
            cell_y + 1.0,
            fill_w,
            (cell_h - 2.0).max(1.0),
            base_color,
            opts.cell_bg_color,
            vol_frac,
            &opts.volume_color_intensity,
            opts.gradient_style,
            if is_bullish {
                HorizontalGradientDirection::LeftToRight
            } else {
                HorizontalGradientDirection::RightToLeft
            },
        );
    }

    // Volume text
    let effective_font = adaptive_font_size(font_size, cell_h);
    let avail_w = bar_width - opts.cell_padding as f64 * 2.0;
    if opts.show_volume_text
        && footprint_text_slot_allows_text(
            effective_font,
            cell_h,
            avail_w,
            FootprintTextSlot::FullVolume,
        )
    {
        let txt = format_volume(vol);
        texts.push(DrawText {
            text: txt,
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
    gradient_rects: &mut Vec<HorizontalGradientRect>,
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
        push_horizontal_gradient_rects(
            gradient_rects,
            x,
            cell_y + 1.0,
            fill_w,
            (cell_h - 2.0).max(1.0),
            color,
            opts.cell_bg_color,
            delta_frac,
            &opts.volume_color_intensity,
            opts.gradient_style,
            if delta >= 0.0 {
                HorizontalGradientDirection::LeftToRight
            } else {
                HorizontalGradientDirection::RightToLeft
            },
        );
    }
}

/// Volume Profile mode: horizontal bar showing total volume.
fn render_volume_profile_cell(
    gradient_rects: &mut Vec<HorizontalGradientRect>,
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
        push_horizontal_gradient_rects(
            gradient_rects,
            bar_left + padding,
            cell_y + 1.0,
            fill_w,
            (cell_h - 2.0).max(1.0),
            color,
            opts.cell_bg_color,
            vol_frac,
            &opts.volume_color_intensity,
            opts.gradient_style,
            if is_bullish {
                HorizontalGradientDirection::LeftToRight
            } else {
                HorizontalGradientDirection::RightToLeft
            },
        );
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
    let fs = font_size * 0.9;
    let avail_w = (bar_width - opts.cell_padding as f64 * 2.0).max(0.0);
    if opts.show_volume_text
        && footprint_text_slot_allows_text(fs, bar_h, avail_w, FootprintTextSlot::Delta)
    {
        let txt = format_delta(delta);
        texts.push(DrawText {
            text: txt,
            x: (bar_left + bar_width * 0.5) as f32,
            y: (y + bar_h * 0.5) as f32,
            font_size: fs,
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
    time_scale: &TimeScaleIndex,
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

    let Some(center_x) = main_bar_center_x(bar_idx, vp, time_scale, chart_w).map(f64::round) else {
        return;
    };
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

#[derive(Clone, Copy)]
enum FootprintTextSlot {
    HalfVolume,
    FullVolume,
    Delta,
}

/// Use a slot-based threshold so labels flip on/off together at a given zoom
/// instead of varying by formatted string length.
#[inline]
fn footprint_text_slot_allows_text(
    font_size: f32,
    cell_h: f64,
    available_width: f64,
    slot: FootprintTextSlot,
) -> bool {
    if font_size <= 0.0 || available_width <= 0.0 || cell_h <= 0.0 {
        return false;
    }
    let required_chars = match slot {
        FootprintTextSlot::HalfVolume => 6.0,
        FootprintTextSlot::FullVolume => 6.0,
        FootprintTextSlot::Delta => 7.0,
    };
    let char_w = font_size as f64 * 0.55;
    let required_width = char_w * required_chars;
    let required_height = font_size as f64 + 1.0;
    available_width >= required_width && cell_h >= required_height
}

/// Compute adaptive font size that fits within a cell.
/// Returns the font size in physical pixels, or 0.0 if the cell is too small
/// for any text.  The minimum readable threshold is 4px physical; below that
/// we skip text entirely.
#[inline]
fn adaptive_font_size(max_font: f32, cell_h: f64) -> f32 {
    let min_font: f32 = 4.0;
    // Leave 1px padding (top + bottom combined)
    let available = (cell_h - 1.0) as f32;
    if available < min_font {
        return 0.0;
    }
    available.min(max_font)
}

/// Auto-detect tick size from footprint levels.
fn auto_tick_size(bar: &FootprintBar) -> f32 {
    bar.inferred_tick_size()
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
    gradient_style: FootprintGradientStyle,
) -> [f32; 4] {
    let base_t = match mode {
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
    let (min_t, curve, alpha_floor, glow_mix) = match gradient_style {
        FootprintGradientStyle::SoftGlow => (0.20, 1.15_f32, 0.18, 0.10),
        FootprintGradientStyle::StrongGlow => (0.24, 1.35_f32, 0.22, 0.18),
        FootprintGradientStyle::NoGlow => (0.16, 1.00_f32, 0.12, 0.00),
    };
    let t = (min_t + (1.0 - min_t) * base_t.powf(curve)).clamp(0.0, 1.0);
    let tint = [
        color[0] + (1.0 - color[0]) * glow_mix,
        color[1] + (1.0 - color[1]) * glow_mix,
        color[2] + (1.0 - color[2]) * glow_mix,
        color[3],
    ];

    [
        bg[0] + (tint[0] - bg[0]) * t,
        bg[1] + (tint[1] - bg[1]) * t,
        bg[2] + (tint[2] - bg[2]) * t,
        (alpha_floor + color[3] * t * (1.0 - alpha_floor)).clamp(0.0, 1.0),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HorizontalGradientDirection {
    LeftToRight,
    RightToLeft,
}

fn push_horizontal_gradient_rects(
    rects: &mut Vec<HorizontalGradientRect>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    base_color: [f32; 4],
    bg: [f32; 4],
    frac: f32,
    mode: &VolumeColorIntensity,
    gradient_style: FootprintGradientStyle,
    direction: HorizontalGradientDirection,
) {
    if w <= 0.5 || h <= 0.5 {
        return;
    }

    let flat = intensity_blend(base_color, bg, frac, mode, gradient_style);
    let (origin_mix, edge_glow_mix, origin_alpha) = match gradient_style {
        FootprintGradientStyle::SoftGlow => (0.44_f32, 0.12_f32, 0.62_f32),
        FootprintGradientStyle::StrongGlow => (0.34_f32, 0.20_f32, 0.56_f32),
        FootprintGradientStyle::NoGlow => (1.0_f32, 0.0_f32, 1.0_f32),
    };

    let origin = [
        bg[0] + (flat[0] - bg[0]) * origin_mix,
        bg[1] + (flat[1] - bg[1]) * origin_mix,
        bg[2] + (flat[2] - bg[2]) * origin_mix,
        (flat[3] * origin_alpha).clamp(0.0, 1.0),
    ];
    let edge = [
        flat[0] + (1.0 - flat[0]) * edge_glow_mix,
        flat[1] + (1.0 - flat[1]) * edge_glow_mix,
        flat[2] + (1.0 - flat[2]) * edge_glow_mix,
        flat[3],
    ];
    let (left, right) = match direction {
        HorizontalGradientDirection::LeftToRight => (origin, edge),
        HorizontalGradientDirection::RightToLeft => (edge, origin),
    };

    rects.push(HorizontalGradientRect {
        x: x as f32,
        y: y as f32,
        w: w as f32,
        h: h as f32,
        left_r: left[0],
        left_g: left[1],
        left_b: left[2],
        left_a: left[3],
        right_r: right[0],
        right_g: right[1],
        right_b: right[2],
        right_a: right[3],
    });
}

/// Format volume for display (compact notation).
fn format_volume(vol: f32) -> String {
    if vol >= 1_000_000.0 {
        format!("{:.1}M", vol / 1_000_000.0)
    } else if vol >= 1_000.0 {
        format!("{:.1}K", vol / 1_000.0)
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
    } else if abs >= 1_000.0 {
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
    use crate::core::data::{Bar, BarArray};
    use crate::core::footprint::{FootprintBar, FootprintData, FootprintLevel, FootprintOptions};
    use crate::core::renderer::traits::ChartStyle;
    use crate::core::viewport::Viewport;

    fn sample_viewport() -> Viewport {
        let mut viewport = Viewport::new(400, 300);
        viewport.start_bar = -0.5;
        viewport.end_bar = 1.5;
        viewport.price_min = 99.0;
        viewport.price_max = 102.0;
        viewport.volume_height_ratio = 0.0;
        viewport
    }

    fn sample_bars() -> BarArray {
        let mut bars = BarArray::new();
        bars.set(vec![Bar {
            timestamp: 1,
            open: 100.2,
            high: 101.4,
            low: 99.8,
            close: 100.9,
            volume: 50.0,
            _pad: 0.0,
        }])
        .unwrap();
        bars
    }

    #[test]
    fn test_format_volume() {
        assert_eq!(format_volume(0.0), "");
        assert_eq!(format_volume(0.5), "0.50");
        assert_eq!(format_volume(42.0), "42");
        assert_eq!(format_volume(1234.0), "1.2K");
        assert_eq!(format_volume(15000.0), "15.0K");
        assert_eq!(format_volume(2_500_000.0), "2.5M");
    }

    #[test]
    fn test_format_delta() {
        assert_eq!(format_delta(0.0), "0");
        assert_eq!(format_delta(50.0), "+50");
        assert_eq!(format_delta(-30.0), "-30");
        assert_eq!(format_delta(1234.0), "+1.2K");
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
    fn soft_glow_gradient_emits_single_smooth_gradient_rect() {
        let mut rects = Vec::new();
        push_horizontal_gradient_rects(
            &mut rects,
            10.0,
            20.0,
            24.0,
            8.0,
            [0.2, 0.4, 0.9, 1.0],
            [0.08, 0.09, 0.12, 1.0],
            0.9,
            &crate::core::footprint::VolumeColorIntensity::Linear,
            crate::core::footprint::FootprintGradientStyle::SoftGlow,
            HorizontalGradientDirection::LeftToRight,
        );

        assert_eq!(
            rects.len(),
            1,
            "soft glow should emit a single smooth gradient rect"
        );
        let rect = rects.first().unwrap();
        assert!(rect.left_r < rect.right_r);
        assert!(rect.left_a < rect.right_a);
    }

    #[test]
    fn no_glow_gradient_emits_single_flat_rect() {
        let mut rects = Vec::new();
        push_horizontal_gradient_rects(
            &mut rects,
            10.0,
            20.0,
            24.0,
            8.0,
            [0.9, 0.2, 0.3, 1.0],
            [0.08, 0.09, 0.12, 1.0],
            0.9,
            &crate::core::footprint::VolumeColorIntensity::Linear,
            crate::core::footprint::FootprintGradientStyle::NoGlow,
            HorizontalGradientDirection::RightToLeft,
        );

        assert_eq!(rects.len(), 1, "no glow should stay flat");
        let rect = rects.first().unwrap();
        assert!((rect.left_r - rect.right_r).abs() < 1e-6);
        assert!((rect.left_a - rect.right_a).abs() < 1e-6);
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

    #[test]
    fn populated_footprint_bar_keeps_candle_in_overlay_layer() {
        let bars = sample_bars();
        let mut fp_data = FootprintData::new();
        fp_data.set_bar(
            0,
            FootprintBar {
                levels: vec![
                    FootprintLevel {
                        price: 100.0,
                        bid_volume: 15.0,
                        ask_volume: 20.0,
                    },
                    FootprintLevel {
                        price: 100.5,
                        bid_volume: 25.0,
                        ask_volume: 18.0,
                    },
                    FootprintLevel {
                        price: 101.0,
                        bid_volume: 11.0,
                        ask_volume: 30.0,
                    },
                ],
            },
        );
        let viewport = sample_viewport();
        let style = ChartStyle::default();
        let opts = FootprintOptions::default();
        let time_scale = TimeScaleIndex::from_bars(&bars);

        let geom = generate_footprint_geometry(
            &bars,
            &time_scale,
            &viewport,
            &style,
            &fp_data,
            &opts,
            400.0,
            300.0,
            1.0,
            1.0,
        );

        let ladder_left = geom
            .base_rects
            .iter()
            .map(|rect| rect.x)
            .fold(f32::INFINITY, f32::min);
        assert!(ladder_left.is_finite(), "ladder background should exist");
        assert!(
            geom.overlay_rects
                .iter()
                .any(|rect| rect.x + rect.w <= ladder_left),
            "candle geometry should render in the later overlay layer left of the ladder"
        );
    }

    #[test]
    fn empty_level_footprint_bar_falls_back_to_candle() {
        let bars = sample_bars();
        let mut fp_data = FootprintData::new();
        fp_data.set_bar(0, FootprintBar::new());
        let viewport = sample_viewport();
        let style = ChartStyle::default();
        let opts = FootprintOptions::default();
        let time_scale = TimeScaleIndex::from_bars(&bars);

        let geom = generate_footprint_geometry(
            &bars,
            &time_scale,
            &viewport,
            &style,
            &fp_data,
            &opts,
            400.0,
            300.0,
            1.0,
            1.0,
        );

        assert!(
            geom.gradient_rects.is_empty(),
            "bars with no footprint levels should not emit ladder gradients"
        );
        assert!(
            geom.overlay_rects
                .iter()
                .any(|rect| rect.w >= 1.0 && rect.h >= 1.0),
            "bars with empty footprint levels should still show a fallback candle"
        );
    }

    #[test]
    fn bid_ask_cells_use_outer_border_without_center_divider() {
        let bars = sample_bars();
        let mut fp_data = FootprintData::new();
        fp_data.set_bar(
            0,
            FootprintBar {
                levels: vec![FootprintLevel {
                    price: 100.0,
                    bid_volume: 15.0,
                    ask_volume: 20.0,
                }],
            },
        );
        let viewport = sample_viewport();
        let style = ChartStyle::default();
        let mut opts = FootprintOptions::default();
        opts.show_poc = false;
        opts.show_delta_bar = false;
        opts.show_unfinished_auction = false;
        opts.show_volume_text = false;
        let time_scale = TimeScaleIndex::from_bars(&bars);

        let geom = generate_footprint_geometry(
            &bars,
            &time_scale,
            &viewport,
            &style,
            &fp_data,
            &opts,
            400.0,
            300.0,
            1.0,
            1.0,
        );

        let ladder = geom
            .base_rects
            .iter()
            .max_by(|a, b| a.w.partial_cmp(&b.w).unwrap_or(std::cmp::Ordering::Equal))
            .expect("ladder background should exist");
        let mid_x = ladder.x + ladder.w * 0.5;
        let right_x = ladder.x + ladder.w - 1.0;

        assert!(
            geom.overlay_rects
                .iter()
                .any(|rect| (rect.x - ladder.x).abs() < 0.1 && rect.w <= 1.0 && rect.h >= 1.0),
            "row should include a left border"
        );
        assert!(
            geom.overlay_rects
                .iter()
                .any(|rect| (rect.x - right_x).abs() < 0.1 && rect.w <= 1.0 && rect.h >= 1.0),
            "row should include a right border"
        );
        assert!(
            !geom
                .overlay_rects
                .iter()
                .any(|rect| (rect.x - mid_x).abs() < 0.1 && rect.w <= 1.0 && rect.h >= 1.0),
            "row should not emit an internal bid/ask divider"
        );
    }

    #[test]
    fn bid_ask_text_is_centered_within_each_half_cell() {
        let bars = sample_bars();
        let mut fp_data = FootprintData::new();
        fp_data.set_bar(
            0,
            FootprintBar {
                levels: vec![FootprintLevel {
                    price: 100.0,
                    bid_volume: 338.0,
                    ask_volume: 407.0,
                }],
            },
        );
        let viewport = sample_viewport();
        let style = ChartStyle::default();
        let mut opts = FootprintOptions::default();
        opts.show_poc = false;
        opts.show_delta_bar = false;
        opts.show_unfinished_auction = false;
        opts.show_volume_text = true;
        let time_scale = TimeScaleIndex::from_bars(&bars);

        let geom = generate_footprint_geometry(
            &bars,
            &time_scale,
            &viewport,
            &style,
            &fp_data,
            &opts,
            400.0,
            300.0,
            1.0,
            1.0,
        );

        let left_label = format_volume(338.0);
        let right_label = format_volume(407.0);
        let left_text = geom
            .texts
            .iter()
            .find(|text| text.text == left_label)
            .expect("bid-side text should render");
        let right_text = geom
            .texts
            .iter()
            .find(|text| text.text == right_label)
            .expect("ask-side text should render");

        let ladder = geom
            .base_rects
            .iter()
            .find(|rect| {
                rect.w > rect.h
                    && left_text.y >= rect.y
                    && left_text.y <= rect.y + rect.h
                    && right_text.y >= rect.y
                    && right_text.y <= rect.y + rect.h
            })
            .expect("row background should exist");

        let left_center = ladder.x + ladder.w * 0.25;
        let right_center = ladder.x + ladder.w * 0.75;

        assert_eq!(left_text.align, TextAlign::Center);
        assert_eq!(right_text.align, TextAlign::Center);
        assert!(
            (left_text.x - left_center).abs() < 0.1,
            "bid text should be centered in left half-cell"
        );
        assert!(
            (right_text.x - right_center).abs() < 0.1,
            "ask text should be centered in right half-cell"
        );
    }

    #[test]
    fn bid_ask_text_visibility_uses_slot_threshold_not_label_length() {
        let opts = FootprintOptions::default();
        let short = FootprintLevel {
            price: 100.0,
            bid_volume: 5.0,
            ask_volume: 8.0,
        };
        let long = FootprintLevel {
            price: 100.0,
            bid_volume: 100_000.0,
            ask_volume: 250_000.0,
        };

        let mut short_texts = Vec::new();
        let mut short_gradients = Vec::new();
        render_bid_ask_cell(
            &mut short_gradients,
            &mut short_texts,
            &opts,
            &short,
            0.0,
            0.0,
            42.0,
            20.0,
            250_000.0,
            10.0,
            ImbalanceType::None,
        );

        let mut long_texts = Vec::new();
        let mut long_gradients = Vec::new();
        render_bid_ask_cell(
            &mut long_gradients,
            &mut long_texts,
            &opts,
            &long,
            0.0,
            0.0,
            42.0,
            20.0,
            250_000.0,
            10.0,
            ImbalanceType::None,
        );

        assert!(
            short_texts.is_empty() && long_texts.is_empty(),
            "narrow slots should hide all bid/ask text regardless of label length"
        );

        short_texts.clear();
        short_gradients.clear();
        render_bid_ask_cell(
            &mut short_gradients,
            &mut short_texts,
            &opts,
            &short,
            0.0,
            0.0,
            74.0,
            20.0,
            250_000.0,
            10.0,
            ImbalanceType::None,
        );

        long_texts.clear();
        long_gradients.clear();
        render_bid_ask_cell(
            &mut long_gradients,
            &mut long_texts,
            &opts,
            &long,
            0.0,
            0.0,
            74.0,
            20.0,
            250_000.0,
            10.0,
            ImbalanceType::None,
        );

        assert_eq!(short_texts.len(), 2, "wide slots should show short labels");
        assert_eq!(
            long_texts.len(),
            2,
            "wide slots should show long labels too"
        );
    }
}
