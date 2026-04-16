//! Theme system — single source of truth for all chart colors, fonts, and sizes.
//!
//! ## Architecture
//!
//! - [`ThemeConfig`] is the **public-facing** configuration object.
//!   It organizes all visual settings into logical groups and provides
//!   `dark()` and `light()` presets plus full custom support.
//!
//! - [`ChartStyle`] (in traits.rs) is the **internal rendering** struct.
//!   Renderers consume `ChartStyle` directly. `ThemeConfig` converts to
//!   `ChartStyle` via [`ThemeConfig::to_chart_style()`].
//!
//! - The legacy `default_style()` function and `pub const` values remain
//!   for backward compatibility. `ChartStyle::default()` still delegates here.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use axiuscharts::core::renderer::theme::{ThemeConfig, ThemePreset};
//!
//! // Use a preset
//! let dark = ThemeConfig::dark();
//! let light = ThemeConfig::light();
//!
//! // Custom theme based on dark
//! let mut custom = ThemeConfig::dark();
//! custom.colors.background = [0.1, 0.1, 0.12, 1.0];
//! custom.colors.bullish = [0.0, 0.8, 0.4, 1.0];
//!
//! // Convert to internal rendering style
//! let style = custom.to_chart_style();
//! ```

use super::traits::{ChartStyle, CrosshairLineStyle, LastPriceLineStyle};
use crate::core::constants::DEFAULT_PRICE_SCALE_TICK_MARK_DENSITY;
use crate::core::series::LineStyle;

// ═════════════════════════════════════════════════════════════════════════════
// Hex helper (kept for backward compat with const values below)
// ═════════════════════════════════════════════════════════════════════════════

/// Hex helper: converts a u8 channel (0-255) to the 0.0-1.0 range.
pub const fn ch(v: u8) -> f32 {
    v as f32 / 255.0
}

// ═════════════════════════════════════════════════════════════════════════════
// Legacy constants (kept for backward compatibility)
// ═════════════════════════════════════════════════════════════════════════════

/// Main chart & axis background.
pub const BG: [f32; 4] = [ch(0x13), ch(0x13), ch(0x15), 1.0];
/// Bullish candle body (#00B562).
pub const BULLISH: [f32; 4] = [ch(0x00), ch(0xB5), ch(0x62), 1.0];
/// Bearish candle body (#F20751).
pub const BEARISH: [f32; 4] = [ch(0xF2), ch(0x07), ch(0x51), 1.0];
/// Bullish volume (same hue, lower alpha).
pub const BULLISH_VOLUME: [f32; 4] = [ch(0x00), ch(0xB5), ch(0x62), 0.35];
/// Bearish volume.
pub const BEARISH_VOLUME: [f32; 4] = [ch(0xF2), ch(0x07), ch(0x51), 0.35];
/// Bullish wick (#00B562), matching the body fill.
pub const WICK_BULLISH: [f32; 4] = [ch(0x00), ch(0xB5), ch(0x62), 1.0];
/// Bearish wick (#F20751), matching the body fill.
pub const WICK_BEARISH: [f32; 4] = [ch(0xF2), ch(0x07), ch(0x51), 1.0];
/// Bullish candle border (#008045).
pub const BORDER_BULLISH: [f32; 4] = [ch(0x00), ch(0x80), ch(0x45), 1.0];
/// Bearish candle border (#89002B).
pub const BORDER_BEARISH: [f32; 4] = [ch(0x89), ch(0x00), ch(0x2B), 1.0];
/// Grid line color.
pub const GRID: [f32; 4] = [0.2, 0.2, 0.24, 0.4];
/// Axis border / tick color.
pub const AXIS_BORDER: [f32; 4] = [ch(0x2A), ch(0x2A), ch(0x2A), 1.0];
/// Axis label text color.
pub const AXIS_TEXT: [f32; 4] = [ch(0xE7), ch(0xE7), ch(0xE7), 1.0];
/// Crosshair line color for the dark theme.
pub const CROSSHAIR: [f32; 4] = [ch(0xEB), ch(0xEB), ch(0xEB), 1.0];
/// Crosshair label background for the dark theme.
pub const CROSSHAIR_LABEL_BG: [f32; 4] = [ch(0xEB), ch(0xEB), ch(0xEB), 1.0];
/// Crosshair label text for the dark theme.
pub const CROSSHAIR_LABEL_TEXT: [f32; 4] = [ch(0x13), ch(0x13), ch(0x15), 1.0];
/// Default font family.
///
/// This matches the Axiusflow app chart stack so chart text (axes, overlays,
/// drawing labels) stays visually aligned with the host UI by default.
pub const FONT_FAMILY: &str =
    "'Geist Sans', 'Noto Sans SC', 'Noto Sans', Roboto, 'Helvetica Neue', Arial, 'Liberation Sans', sans-serif";
/// Axis label font size in CSS px.
pub const FONT_SIZE: f32 = 12.0;
/// Bar width as fraction of bar slot (0.0-1.0).
pub const BAR_WIDTH_RATIO: f32 = 0.8;
/// Axis border width in CSS px.
pub const AXIS_BORDER_SIZE: f32 = 1.0;
/// Axis tick length in CSS px.
pub const AXIS_TICK_LENGTH: f32 = 5.0;

/// Match lightweight-charts contrast selection for solid label backgrounds.
///
/// LWC converts the background to grayscale and chooses black text for bright
/// labels, otherwise white text. AxiusCharts stores colors normalized to 0.0-1.0,
/// so the grayscale threshold is normalized from 160/255.
pub fn contrast_text_color(background: [f32; 4]) -> [f32; 4] {
    let grayscale =
        0.199 * background[0] as f64 + 0.687 * background[1] as f64 + 0.114 * background[2] as f64;
    if grayscale > (160.0 / 255.0) {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Legacy builder (unchanged — ChartStyle::default() delegates here)
// ═════════════════════════════════════════════════════════════════════════════

/// Build the default `ChartStyle` from the dark theme constants.
/// This is the same output as `ThemeConfig::dark().to_chart_style()`.
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
            style: LineStyle::Dotted,
            label_visible: true,
        },
        font_family: FONT_FAMILY.into(),
        font_size: FONT_SIZE,
        bar_width_ratio: BAR_WIDTH_RATIO,
        axis_border_size: AXIS_BORDER_SIZE,
        axis_tick_length: AXIS_TICK_LENGTH,
        price_scale_tick_mark_density: DEFAULT_PRICE_SCALE_TICK_MARK_DENSITY as f32,
        axis_ticks_visible: true,
        axis_border_visible: false,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// New ThemeConfig System
// ═════════════════════════════════════════════════════════════════════════════

/// Theme preset selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreset {
    /// Dark theme (default) — dark background, light text.
    Dark,
    /// Light theme — white background, dark text. Based on TradingView Light.
    Light,
}

// ── Color sub-groups ────────────────────────────────────────────────────────

/// Core chart colors: backgrounds, candle body/wick/volume, grid, axis.
#[derive(Debug, Clone)]
pub struct ThemeColors {
    /// Main chart and axis background.
    pub background: [f32; 4],
    /// Bullish (up) candle body color.
    pub bullish: [f32; 4],
    /// Bearish (down) candle body color.
    pub bearish: [f32; 4],
    /// Bullish volume bar color (typically same hue, lower alpha).
    pub bullish_volume: [f32; 4],
    /// Bearish volume bar color.
    pub bearish_volume: [f32; 4],
    /// Bullish candle wick color.
    pub wick_bullish: [f32; 4],
    /// Bearish candle wick color.
    pub wick_bearish: [f32; 4],
    /// Grid line color.
    pub grid: [f32; 4],
    /// Axis border and tick mark color.
    pub axis_border: [f32; 4],
    /// Axis label text color.
    pub axis_text: [f32; 4],
}

/// Crosshair appearance configuration.
#[derive(Debug, Clone)]
pub struct ThemeCrosshair {
    /// Crosshair line color.
    pub line_color: [f32; 4],
    /// Crosshair axis label background.
    pub label_bg: [f32; 4],
    /// Crosshair axis label text color.
    pub label_text: [f32; 4],
    /// Crosshair line width in CSS px.
    pub line_width: f64,
    /// Crosshair line dash style.
    pub line_style: LineStyle,
    /// Whether the vertical crosshair line is visible.
    pub vert_visible: bool,
    /// Whether the horizontal crosshair line is visible.
    pub horz_visible: bool,
    /// Whether the vertical axis label (time) is visible.
    pub vert_label_visible: bool,
    /// Whether the horizontal axis label (price) is visible.
    pub horz_label_visible: bool,
}

/// Typography settings.
#[derive(Debug, Clone)]
pub struct ThemeTypography {
    /// Font family for all chart text (axis labels, tooltips, etc.).
    pub font_family: String,
    /// Base font size for axis labels in CSS px.
    pub font_size: f32,
}

/// Layout sizing constants.
#[derive(Debug, Clone)]
pub struct ThemeLayout {
    /// Bar width as fraction of bar slot (0.0-1.0). 0.8 = 80%.
    pub bar_width_ratio: f32,
    /// Axis border width in CSS px.
    pub axis_border_size: f32,
    /// Axis tick length in CSS px.
    pub axis_tick_length: f32,
    /// Price scale tick mark density multiplier.
    pub price_scale_tick_mark_density: f32,
}

/// Last-price line appearance.
#[derive(Debug, Clone)]
pub struct ThemeLastPriceLine {
    /// Whether the live price line is rendered.
    pub visible: bool,
    /// Line width in CSS px.
    pub width: f64,
    /// Dash style.
    pub style: LineStyle,
    /// Whether the price label on the axis is visible.
    pub label_visible: bool,
}

/// Default colors for overlay series (line, area, histogram, bar, baseline).
#[derive(Debug, Clone)]
pub struct ThemeSeriesDefaults {
    /// Default line series color.
    pub line_color: [f32; 4],
    /// Default area series line color.
    pub area_line_color: [f32; 4],
    /// Area top fill gradient color.
    pub area_top_fill: [f32; 4],
    /// Area bottom fill gradient color (typically transparent).
    pub area_bottom_fill: [f32; 4],
    /// Default histogram bar color.
    pub histogram_color: [f32; 4],
    /// OHLC bar up color.
    pub bar_up_color: [f32; 4],
    /// OHLC bar down color.
    pub bar_down_color: [f32; 4],
    /// Baseline series top line color.
    pub baseline_top_line: [f32; 4],
    /// Baseline series bottom line color.
    pub baseline_bottom_line: [f32; 4],
    /// Baseline top fill color 1 (near line).
    pub baseline_top_fill_1: [f32; 4],
    /// Baseline top fill color 2 (fade to zero).
    pub baseline_top_fill_2: [f32; 4],
    /// Baseline bottom fill color 1 (fade from zero).
    pub baseline_bottom_fill_1: [f32; 4],
    /// Baseline bottom fill color 2 (near line).
    pub baseline_bottom_fill_2: [f32; 4],
    /// Default marker color.
    pub marker_color: [f32; 4],
    /// Default marker text color.
    pub marker_text_color: [f32; 4],
    /// Default price line color.
    pub price_line_color: [f32; 4],
    /// Default price line label text color.
    pub price_line_text_color: [f32; 4],
}

/// Default colors for drawing tools.
#[derive(Debug, Clone)]
pub struct ThemeDrawingDefaults {
    /// Default drawing tool color (trend line, rectangle stroke, etc.).
    pub color: [f32; 4],
    /// Fibonacci retracement line color.
    pub fibonacci_color: [f32; 4],
    /// Fibonacci zone fill color.
    pub fibonacci_fill: [f32; 4],
    /// Scale/measurement tool color.
    pub scale_color: [f32; 4],
    /// Scale/measurement fill color.
    pub scale_fill: [f32; 4],
    /// Anchor handle color (drag points on drawings).
    pub anchor_color: [f32; 4],
    /// Default label font size for drawings in CSS px.
    pub font_size: f64,
    /// Fibonacci label font size in CSS px.
    pub fibonacci_font_size: f64,
}

/// Indicator/study color palette — colors assigned to indicators in order.
#[derive(Debug, Clone)]
pub struct ThemeIndicatorPalette {
    /// Ordered color palette for indicator lines.
    /// Indicators are assigned colors from this list in order.
    pub colors: Vec<[f32; 4]>,
    /// Fallback color when palette is exhausted.
    pub fallback: [f32; 4],
}

/// Workspace (multi-pane split layout) styling.
#[derive(Debug, Clone)]
pub struct ThemeWorkspace {
    /// Divider line color between workspace panes.
    pub divider_color: [f32; 4],
    /// Divider color when hovered/active.
    pub divider_active_color: [f32; 4],
    /// Workspace pane background color.
    pub pane_background: [f32; 4],
    /// Active pane border highlight color.
    pub pane_active_border: [f32; 4],
}

/// Subpane separator styling (between indicator panes).
#[derive(Debug, Clone)]
pub struct ThemeSubpaneSeparator {
    /// Separator line color.
    pub color: [f32; 4],
    /// Separator hover color.
    pub hover_color: [f32; 4],
}

// ── ThemeConfig (top-level) ─────────────────────────────────────────────────

/// Complete theme configuration for a AxiusCharts chart.
///
/// Organizes all visual settings into logical groups.
/// Use [`ThemeConfig::dark()`] or [`ThemeConfig::light()`] for presets,
/// then customize individual fields as needed.
///
/// Convert to the internal rendering struct with [`ThemeConfig::to_chart_style()`].
#[derive(Debug, Clone)]
pub struct ThemeConfig {
    /// Core chart colors (background, candles, grid, axis).
    pub colors: ThemeColors,
    /// Crosshair appearance.
    pub crosshair: ThemeCrosshair,
    /// Typography (fonts, sizes).
    pub typography: ThemeTypography,
    /// Layout sizing.
    pub layout: ThemeLayout,
    /// Last-price line options.
    pub last_price_line: ThemeLastPriceLine,
    /// Default colors for overlay series types.
    pub series_defaults: ThemeSeriesDefaults,
    /// Default colors for drawing tools.
    pub drawing_defaults: ThemeDrawingDefaults,
    /// Indicator color palette.
    pub indicator_palette: ThemeIndicatorPalette,
    /// Workspace (multi-pane) styling.
    pub workspace: ThemeWorkspace,
    /// Subpane separator styling.
    pub subpane_separator: ThemeSubpaneSeparator,
}

impl ThemeConfig {
    // ── Presets ──────────────────────────────────────────────────────────

    /// Dark theme preset — matches the existing AxiusCharts default (TradingView dark-inspired).
    pub fn dark() -> Self {
        Self {
            colors: ThemeColors {
                background: BG,
                bullish: BULLISH,
                bearish: BEARISH,
                bullish_volume: BULLISH_VOLUME,
                bearish_volume: BEARISH_VOLUME,
                wick_bullish: WICK_BULLISH,
                wick_bearish: WICK_BEARISH,
                grid: GRID,
                axis_border: AXIS_BORDER,
                axis_text: AXIS_TEXT,
            },
            crosshair: ThemeCrosshair {
                line_color: CROSSHAIR,
                label_bg: CROSSHAIR_LABEL_BG,
                label_text: CROSSHAIR_LABEL_TEXT,
                line_width: 1.0,
                line_style: LineStyle::LargeDashed,
                vert_visible: true,
                horz_visible: true,
                vert_label_visible: true,
                horz_label_visible: true,
            },
            typography: ThemeTypography {
                font_family: FONT_FAMILY.into(),
                font_size: FONT_SIZE,
            },
            layout: ThemeLayout {
                bar_width_ratio: BAR_WIDTH_RATIO,
                axis_border_size: AXIS_BORDER_SIZE,
                axis_tick_length: AXIS_TICK_LENGTH,
                price_scale_tick_mark_density: DEFAULT_PRICE_SCALE_TICK_MARK_DENSITY as f32,
            },
            last_price_line: ThemeLastPriceLine {
                visible: true,
                width: 1.0,
                style: LineStyle::Dotted,
                label_visible: true,
            },
            series_defaults: ThemeSeriesDefaults {
                // Vibrant blue #3B82F6 (Tailwind blue-500)
                line_color: [0.231, 0.510, 0.965, 1.0],
                area_line_color: [0.231, 0.510, 0.965, 1.0],
                area_top_fill: [0.231, 0.510, 0.965, 0.28],
                area_bottom_fill: [0.231, 0.510, 0.965, 0.0],
                histogram_color: BULLISH,
                bar_up_color: BULLISH,
                bar_down_color: BEARISH,
                // Baseline
                baseline_top_line: BULLISH,
                baseline_bottom_line: BEARISH,
                baseline_top_fill_1: [ch(0x35), ch(0x59), ch(0xE9), 0.28],
                baseline_top_fill_2: [ch(0x35), ch(0x59), ch(0xE9), 0.05],
                baseline_bottom_fill_1: [ch(0xFB), ch(0x37), ch(0x48), 0.05],
                baseline_bottom_fill_2: [ch(0xFB), ch(0x37), ch(0x48), 0.28],
                // Markers & price lines
                marker_color: [0.231, 0.510, 0.965, 1.0],
                marker_text_color: [1.0, 1.0, 1.0, 0.9],
                price_line_color: [0.5, 0.5, 0.5, 1.0],
                price_line_text_color: [1.0, 1.0, 1.0, 0.9],
            },
            drawing_defaults: ThemeDrawingDefaults {
                color: [0.35, 0.55, 0.95, 1.0],
                fibonacci_color: [0.95, 0.75, 0.25, 1.0],
                fibonacci_fill: [0.95, 0.75, 0.25, 0.05],
                scale_color: [0.6, 0.8, 0.4, 1.0],
                scale_fill: [0.6, 0.8, 0.4, 0.1],
                anchor_color: [1.0, 1.0, 1.0, 1.0],
                font_size: 11.0,
                fibonacci_font_size: 10.0,
            },
            indicator_palette: ThemeIndicatorPalette {
                colors: vec![
                    [0.161, 0.384, 1.0, 1.0],   // Blue (#2962FF)
                    [1.0, 0.627, 0.0, 1.0],     // Amber (#FFA000)
                    [0.608, 0.349, 0.714, 1.0], // Purple (#9B59B6)
                    [0.9, 0.3, 0.3, 1.0],       // Red
                    [0.18, 0.8, 0.44, 1.0],     // Green (#2ECC71)
                    [0.5, 0.5, 0.5, 0.6],       // Grey (muted)
                ],
                fallback: [0.5, 0.5, 0.5, 1.0],
            },
            workspace: ThemeWorkspace {
                divider_color: [0.18, 0.18, 0.22, 1.0],
                divider_active_color: [0.102, 0.737, 0.612, 0.55],
                pane_background: [0.09, 0.09, 0.09, 1.0],
                pane_active_border: [0.102, 0.737, 0.612, 0.45],
            },
            subpane_separator: ThemeSubpaneSeparator {
                color: [0.2, 0.2, 0.24, 1.0],
                hover_color: [0.102, 0.737, 0.612, 0.55],
            },
        }
    }

    /// Light theme preset — TradingView Light reference colors.
    /// White background, dark text, same bullish/bearish accent colors.
    pub fn light() -> Self {
        Self {
            colors: ThemeColors {
                background: [1.0, 1.0, 1.0, 1.0], // #FFFFFF
                bullish: BULLISH,
                bearish: BEARISH,
                bullish_volume: BULLISH_VOLUME,
                bearish_volume: BEARISH_VOLUME,
                wick_bullish: WICK_BULLISH,
                wick_bearish: WICK_BEARISH,
                grid: [ch(0xE0), ch(0xE3), ch(0xEB), 0.4], // #E0E3EB @ 40%
                axis_border: [ch(0xF5), ch(0xF5), ch(0xF5), 1.0], // #F5F5F5
                axis_text: [ch(0x13), ch(0x13), ch(0x15), 1.0], // #131315
            },
            crosshair: ThemeCrosshair {
                line_color: [ch(0x13), ch(0x13), ch(0x15), 1.0], // #131315
                label_bg: [ch(0x13), ch(0x13), ch(0x15), 1.0],   // #131315
                label_text: [ch(0xF5), ch(0xF5), ch(0xF5), 1.0], // #F5F5F5
                line_width: 1.0,
                line_style: LineStyle::LargeDashed,
                vert_visible: true,
                horz_visible: true,
                vert_label_visible: true,
                horz_label_visible: true,
            },
            typography: ThemeTypography {
                font_family: FONT_FAMILY.into(),
                font_size: FONT_SIZE,
            },
            layout: ThemeLayout {
                bar_width_ratio: BAR_WIDTH_RATIO,
                axis_border_size: AXIS_BORDER_SIZE,
                axis_tick_length: AXIS_TICK_LENGTH,
                price_scale_tick_mark_density: DEFAULT_PRICE_SCALE_TICK_MARK_DENSITY as f32,
            },
            last_price_line: ThemeLastPriceLine {
                visible: true,
                width: 1.0,
                style: LineStyle::Dotted,
                label_visible: true,
            },
            series_defaults: ThemeSeriesDefaults {
                line_color: [0.161, 0.384, 1.0, 1.0],
                area_line_color: [0.161, 0.384, 1.0, 1.0],
                area_top_fill: [0.161, 0.384, 1.0, 0.28],
                area_bottom_fill: [0.161, 0.384, 1.0, 0.0],
                histogram_color: BULLISH,
                bar_up_color: BULLISH,
                bar_down_color: BEARISH,
                baseline_top_line: BULLISH,
                baseline_bottom_line: BEARISH,
                baseline_top_fill_1: [ch(0x35), ch(0x59), ch(0xE9), 0.28],
                baseline_top_fill_2: [ch(0x35), ch(0x59), ch(0xE9), 0.05],
                baseline_bottom_fill_1: [ch(0xFB), ch(0x37), ch(0x48), 0.05],
                baseline_bottom_fill_2: [ch(0xFB), ch(0x37), ch(0x48), 0.28],
                marker_color: [0.161, 0.384, 1.0, 1.0],
                marker_text_color: [ch(0x13), ch(0x17), ch(0x22), 0.9], // Dark text on light
                price_line_color: [0.5, 0.5, 0.5, 1.0],
                price_line_text_color: [1.0, 1.0, 1.0, 0.9],
            },
            drawing_defaults: ThemeDrawingDefaults {
                color: [0.35, 0.55, 0.95, 1.0],
                fibonacci_color: [0.95, 0.75, 0.25, 1.0],
                fibonacci_fill: [0.95, 0.75, 0.25, 0.05],
                scale_color: [0.6, 0.8, 0.4, 1.0],
                scale_fill: [0.6, 0.8, 0.4, 0.1],
                anchor_color: [ch(0x13), ch(0x17), ch(0x22), 1.0], // Dark anchors on light bg
                font_size: 11.0,
                fibonacci_font_size: 10.0,
            },
            indicator_palette: ThemeIndicatorPalette {
                colors: vec![
                    [0.161, 0.384, 1.0, 1.0],
                    [1.0, 0.627, 0.0, 1.0],
                    [0.608, 0.349, 0.714, 1.0],
                    [0.9, 0.3, 0.3, 1.0],
                    [0.18, 0.8, 0.44, 1.0],
                    [0.5, 0.5, 0.5, 0.6],
                ],
                fallback: [0.5, 0.5, 0.5, 1.0],
            },
            workspace: ThemeWorkspace {
                divider_color: [ch(0xE0), ch(0xE3), ch(0xEB), 1.0],
                divider_active_color: [0.161, 0.384, 1.0, 0.55],
                pane_background: [ch(0xF8), ch(0xF9), ch(0xFD), 1.0], // #F8F9FD
                pane_active_border: [0.161, 0.384, 1.0, 0.45],
            },
            subpane_separator: ThemeSubpaneSeparator {
                color: [ch(0xE0), ch(0xE3), ch(0xEB), 1.0],
                hover_color: [0.161, 0.384, 1.0, 0.55],
            },
        }
    }

    /// Create a theme config from a preset enum value.
    pub fn from_preset(preset: ThemePreset) -> Self {
        match preset {
            ThemePreset::Dark => Self::dark(),
            ThemePreset::Light => Self::light(),
        }
    }

    // ── Conversion ──────────────────────────────────────────────────────

    /// Convert this theme config to the internal [`ChartStyle`] used by renderers.
    ///
    /// This is the bridge between the public-facing ThemeConfig and the
    /// internal rendering struct. Renderers never see ThemeConfig directly.
    pub fn to_chart_style(&self) -> ChartStyle {
        ChartStyle {
            bg_color: self.colors.background,
            bullish_color: self.colors.bullish,
            bearish_color: self.colors.bearish,
            bullish_volume_color: self.colors.bullish_volume,
            bearish_volume_color: self.colors.bearish_volume,
            wick_bullish_color: self.colors.wick_bullish,
            wick_bearish_color: self.colors.wick_bearish,
            grid_color: self.colors.grid,
            axis_border_color: self.colors.axis_border,
            axis_text_color: self.colors.axis_text,
            axis_bg_color: self.colors.background,
            crosshair_vert_line: CrosshairLineStyle {
                color: self.crosshair.line_color,
                width: self.crosshair.line_width,
                style: self.crosshair.line_style,
                visible: self.crosshair.vert_visible,
                label_visible: self.crosshair.vert_label_visible,
                label_bg_color: self.crosshair.label_bg,
            },
            crosshair_horz_line: CrosshairLineStyle {
                color: self.crosshair.line_color,
                width: self.crosshair.line_width,
                style: self.crosshair.line_style,
                visible: self.crosshair.horz_visible,
                label_visible: self.crosshair.horz_label_visible,
                label_bg_color: self.crosshair.label_bg,
            },
            crosshair_label_text: self.crosshair.label_text,
            last_price_line: LastPriceLineStyle {
                visible: self.last_price_line.visible,
                width: self.last_price_line.width,
                style: self.last_price_line.style,
                label_visible: self.last_price_line.label_visible,
            },
            font_family: self.typography.font_family.clone(),
            font_size: self.typography.font_size,
            bar_width_ratio: self.layout.bar_width_ratio,
            axis_border_size: self.layout.axis_border_size,
            axis_tick_length: self.layout.axis_tick_length,
            price_scale_tick_mark_density: self.layout.price_scale_tick_mark_density,
            axis_ticks_visible: true,
            axis_border_visible: false,
        }
    }

    // ── Color helpers ───────────────────────────────────────────────────

    /// Get the indicator color at the given index, wrapping around the palette.
    pub fn indicator_color(&self, index: usize) -> [f32; 4] {
        if self.indicator_palette.colors.is_empty() {
            return self.indicator_palette.fallback;
        }
        self.indicator_palette.colors[index % self.indicator_palette.colors.len()]
    }

    /// Get the main chart up/down colors for candlestick/OHLC chart types.
    /// Returns `(up_color, down_color)`.
    pub fn chart_type_colors(&self) -> ([f32; 4], [f32; 4]) {
        (self.colors.bullish, self.colors.bearish)
    }

    /// Get the main chart border colors for candlestick/OHLC chart types.
    /// Returns `(up_border_color, down_border_color)`.
    pub fn chart_type_border_colors(&self) -> ([f32; 4], [f32; 4]) {
        (BORDER_BULLISH, BORDER_BEARISH)
    }

    /// Convert an RGBA [f32; 4] color to a CSS `rgba(...)` string.
    pub fn color_to_css(color: &[f32; 4]) -> String {
        format!(
            "rgba({},{},{},{})",
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            color[3]
        )
    }

    /// Convert an RGBA [f32; 4] color to a CSS hex string (#RRGGBB or #RRGGBBAA).
    pub fn color_to_hex(color: &[f32; 4]) -> String {
        let r = (color[0] * 255.0) as u8;
        let g = (color[1] * 255.0) as u8;
        let b = (color[2] * 255.0) as u8;
        let a = (color[3] * 255.0) as u8;
        if a == 255 {
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)
        }
    }

    /// Generate CSS custom properties (variables) from this theme.
    ///
    /// Returns a list of `(name, value)` pairs suitable for setting on a
    /// DOM element's style. Variable names use the `--axiuscharts-` prefix.
    pub fn to_css_variables(&self) -> Vec<(String, String)> {
        let mut vars = Vec::with_capacity(24);

        vars.push((
            "--axiuscharts-bg".into(),
            Self::color_to_css(&self.colors.background),
        ));
        vars.push((
            "--axiuscharts-text".into(),
            Self::color_to_css(&self.colors.axis_text),
        ));
        vars.push((
            "--axiuscharts-bullish".into(),
            Self::color_to_css(&self.colors.bullish),
        ));
        vars.push((
            "--axiuscharts-bearish".into(),
            Self::color_to_css(&self.colors.bearish),
        ));
        vars.push((
            "--axiuscharts-grid".into(),
            Self::color_to_css(&self.colors.grid),
        ));
        vars.push((
            "--axiuscharts-border".into(),
            Self::color_to_css(&self.colors.axis_border),
        ));
        vars.push((
            "--axiuscharts-crosshair".into(),
            Self::color_to_css(&self.crosshair.line_color),
        ));
        vars.push((
            "--axiuscharts-crosshair-label-bg".into(),
            Self::color_to_css(&self.crosshair.label_bg),
        ));
        vars.push((
            "--axiuscharts-crosshair-label-text".into(),
            Self::color_to_css(&self.crosshair.label_text),
        ));
        vars.push((
            "--axiuscharts-font-family".into(),
            self.typography.font_family.clone(),
        ));
        vars.push((
            "--axiuscharts-font-size".into(),
            format!("{}px", self.typography.font_size),
        ));

        vars
    }
}

impl Default for ThemeConfig {
    /// Default theme is dark (matches existing behavior).
    fn default() -> Self {
        Self::dark()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_matches_legacy_default() {
        let legacy = default_style();
        let theme_style = ThemeConfig::dark().to_chart_style();

        // Core colors must match exactly
        assert_eq!(legacy.bg_color, theme_style.bg_color);
        assert_eq!(legacy.bullish_color, theme_style.bullish_color);
        assert_eq!(legacy.bearish_color, theme_style.bearish_color);
        assert_eq!(
            legacy.bullish_volume_color,
            theme_style.bullish_volume_color
        );
        assert_eq!(
            legacy.bearish_volume_color,
            theme_style.bearish_volume_color
        );
        assert_eq!(legacy.wick_bullish_color, theme_style.wick_bullish_color);
        assert_eq!(legacy.wick_bearish_color, theme_style.wick_bearish_color);
        assert_eq!(legacy.grid_color, theme_style.grid_color);
        assert_eq!(legacy.axis_border_color, theme_style.axis_border_color);
        assert_eq!(legacy.axis_text_color, theme_style.axis_text_color);
        assert_eq!(
            legacy.crosshair_label_text,
            theme_style.crosshair_label_text
        );
        assert_eq!(legacy.font_family, theme_style.font_family);
        assert_eq!(legacy.font_size, theme_style.font_size);
        assert_eq!(legacy.bar_width_ratio, theme_style.bar_width_ratio);
    }

    #[test]
    fn light_theme_has_white_background() {
        let light = ThemeConfig::light();
        assert_eq!(light.colors.background, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn preset_round_trip() {
        let dark = ThemeConfig::from_preset(ThemePreset::Dark);
        let light = ThemeConfig::from_preset(ThemePreset::Light);
        assert_ne!(dark.colors.background, light.colors.background);
    }

    #[test]
    fn indicator_color_wraps() {
        let theme = ThemeConfig::dark();
        let len = theme.indicator_palette.colors.len();
        assert_eq!(theme.indicator_color(0), theme.indicator_color(len));
    }

    #[test]
    fn css_variable_generation() {
        let theme = ThemeConfig::dark();
        let vars = theme.to_css_variables();
        assert!(vars.iter().any(|(k, _)| k == "--axiuscharts-bg"));
        assert!(vars.iter().any(|(k, _)| k == "--axiuscharts-bullish"));
        assert!(vars.iter().any(|(k, _)| k == "--axiuscharts-font-family"));
    }

    #[test]
    fn color_to_hex_opaque() {
        let hex = ThemeConfig::color_to_hex(&[1.0, 0.0, 0.0, 1.0]);
        assert_eq!(hex, "#ff0000");
    }

    #[test]
    fn color_to_hex_alpha() {
        let hex = ThemeConfig::color_to_hex(&[1.0, 0.0, 0.0, 0.5]);
        assert_eq!(hex, "#ff00007f");
    }

    #[test]
    fn default_axis_borders_are_disabled() {
        let legacy = default_style();
        let dark = ThemeConfig::dark().to_chart_style();
        let light = ThemeConfig::light().to_chart_style();

        assert!(
            !legacy.axis_border_visible,
            "legacy default style should ship with axis borders hidden"
        );
        assert!(
            !dark.axis_border_visible,
            "dark theme default should ship with axis borders hidden"
        );
        assert!(
            !light.axis_border_visible,
            "light theme default should ship with axis borders hidden"
        );
        assert!(
            legacy.axis_ticks_visible,
            "tick marks should stay enabled by default"
        );
    }

    #[test]
    fn axis_palette_matches_requested_presets() {
        let dark = ThemeConfig::dark();
        let light = ThemeConfig::light();

        assert_eq!(dark.colors.axis_border, [ch(0x2A), ch(0x2A), ch(0x2A), 1.0]);
        assert_eq!(dark.colors.axis_text, [ch(0xE7), ch(0xE7), ch(0xE7), 1.0]);
        assert_eq!(
            light.colors.axis_border,
            [ch(0xF5), ch(0xF5), ch(0xF5), 1.0]
        );
        assert_eq!(light.colors.axis_text, [ch(0x13), ch(0x13), ch(0x15), 1.0]);
        assert_eq!(
            dark.crosshair.line_color,
            [ch(0xEB), ch(0xEB), ch(0xEB), 1.0]
        );
        assert_eq!(dark.crosshair.label_bg, [ch(0xEB), ch(0xEB), ch(0xEB), 1.0]);
        assert_eq!(
            dark.crosshair.label_text,
            [ch(0x13), ch(0x13), ch(0x15), 1.0]
        );
        assert_eq!(
            light.crosshair.line_color,
            [ch(0x13), ch(0x13), ch(0x15), 1.0]
        );
        assert_eq!(
            light.crosshair.label_bg,
            [ch(0x13), ch(0x13), ch(0x15), 1.0]
        );
        assert_eq!(
            light.crosshair.label_text,
            [ch(0xF5), ch(0xF5), ch(0xF5), 1.0]
        );
    }
}
