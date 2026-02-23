//! Default theme — single source of truth for all chart colors and sizes.
//!
//! Edit this file to change the look of the chart.
//! `ChartStyle::default()` in traits.rs delegates here.

use super::traits::{ChartStyle, CrosshairLineStyle, LastPriceLineStyle};
use crate::core::series::LineStyle;

/// Hex helper: converts a u8 channel (0–255) to the 0.0–1.0 range.
const fn ch(v: u8) -> f32 {
    v as f32 / 255.0
}

// ── Colors ──────────────────────────────────────────────────────────────────

/// Main chart & axis background.
pub const BG: [f32; 4] = [ch(0x17), ch(0x17), ch(0x17), 1.0];

/// Bullish candle body.
pub const BULLISH: [f32; 4] = [ch(0x1A), ch(0xBC), ch(0x9C), 1.0];
/// Bearish candle body.
pub const BEARISH: [f32; 4] = [ch(0xE7), ch(0x4C), ch(0x3C), 1.0];

/// Bullish volume (same hue, lower alpha).
pub const BULLISH_VOLUME: [f32; 4] = [ch(0x1A), ch(0xBC), ch(0x9C), 0.35];
/// Bearish volume.
pub const BEARISH_VOLUME: [f32; 4] = [ch(0xE7), ch(0x4C), ch(0x3C), 0.35];

/// Bullish wick.
pub const WICK_BULLISH: [f32; 4] = [ch(0x1A), ch(0xBC), ch(0x9C), 0.9];
/// Bearish wick.
pub const WICK_BEARISH: [f32; 4] = [ch(0xE7), ch(0x4C), ch(0x3C), 0.9];

/// Grid line color (currently unused — grid disabled).
pub const GRID: [f32; 4] = [0.2, 0.2, 0.24, 0.4];

/// Axis border / tick color.
pub const AXIS_BORDER: [f32; 4] = [0.2, 0.2, 0.24, 1.0];
/// Axis label text color.
pub const AXIS_TEXT: [f32; 4] = [0.55, 0.55, 0.6, 1.0];

/// Crosshair line color (LWC: #9598A1).
pub const CROSSHAIR: [f32; 4] = [ch(0x95), ch(0x98), ch(0xA1), 1.0];
/// Crosshair label background (LWC: #131722).
pub const CROSSHAIR_LABEL_BG: [f32; 4] = [ch(0x13), ch(0x17), ch(0x22), 1.0];
/// Crosshair label text.
pub const CROSSHAIR_LABEL_TEXT: [f32; 4] = [0.9, 0.9, 0.9, 1.0];

/// Watermark text color.
pub const WATERMARK: [f32; 4] = [0.15, 0.16, 0.18, 1.0];

// ── Sizes ───────────────────────────────────────────────────────────────────

/// Font family (LWC default).
pub const FONT_FAMILY: &str =
    "-apple-system, BlinkMacSystemFont, 'Trebuchet MS', Roboto, Ubuntu, sans-serif";
/// Axis label font size in CSS px.
pub const FONT_SIZE: f32 = 11.0;
/// Watermark font size in CSS px.
pub const FONT_SIZE_WATERMARK: f32 = 48.0;
/// Bar width as fraction of bar slot (0.0–1.0).
pub const BAR_WIDTH_RATIO: f32 = 0.8;
/// Axis border width in CSS px.
pub const AXIS_BORDER_SIZE: f32 = 1.0;
/// Axis tick length in CSS px.
pub const AXIS_TICK_LENGTH: f32 = 5.0;

// ── Builder ─────────────────────────────────────────────────────────────────

/// Build the default `ChartStyle` from the constants above.
pub fn default_style() -> ChartStyle {
    ChartStyle {
        bg_color: BG,
        bullish_color: BULLISH,
        bearish_color: BEARISH,
        bullish_volume_color: BULLISH_VOLUME,
        bearish_volume_color: BEARISH_VOLUME,
        wick_bullish_color: WICK_BULLISH,
        wick_bearish_color: WICK_BEARISH,
        grid_color: GRID,
        axis_border_color: AXIS_BORDER,
        axis_text_color: AXIS_TEXT,
        axis_bg_color: BG,
        crosshair_vert_line: CrosshairLineStyle {
            color: CROSSHAIR,
            width: 1.0,
            style: LineStyle::LargeDashed,
            visible: true,
            label_visible: true,
            label_bg_color: CROSSHAIR_LABEL_BG,
        },
        crosshair_horz_line: CrosshairLineStyle {
            color: CROSSHAIR,
            width: 1.0,
            style: LineStyle::LargeDashed,
            visible: true,
            label_visible: true,
            label_bg_color: CROSSHAIR_LABEL_BG,
        },
        crosshair_label_text: CROSSHAIR_LABEL_TEXT,
        last_price_line: LastPriceLineStyle {
            visible: true,
            width: 1.0,
            style: LineStyle::Dashed,
            label_visible: true,
        },
        watermark_color: WATERMARK,
        watermark_text: String::new(),
        font_family: FONT_FAMILY.into(),
        font_size: FONT_SIZE,
        font_size_watermark: FONT_SIZE_WATERMARK,
        bar_width_ratio: BAR_WIDTH_RATIO,
        axis_border_size: AXIS_BORDER_SIZE,
        axis_tick_length: AXIS_TICK_LENGTH,
    }
}
