//! Footprint Chart — professional order-flow visualization.
//!
//! Footprint charts display bid/ask volume at each price level within a
//! candlestick, providing detailed order flow analysis for professional traders.
//!
//! # Pro Features
//!
//! - **Multiple display modes**: Bid×Ask, Delta, Volume, Delta Profile
//! - **Imbalance detection**: Configurable bid/ask ratio thresholds
//! - **Stacked imbalances**: Consecutive imbalance highlighting (absorption/exhaustion)
//! - **Diagonal imbalances**: Compare bid at one level with ask at adjacent level
//! - **Point of Control (POC)**: Price level with highest total volume
//! - **Value Area**: Price range containing 70% of total volume (configurable)
//! - **Unfinished auction**: Detect incomplete auction patterns at high/low
//! - **Cumulative delta**: Running buy−sell volume difference
//! - **Color gradient**: Volume/delta-based cell coloring
//! - **Configurable tick size**: Price granularity per footprint row
//!
//! # Data Flow
//!
//! ```text
//! JS: chart.set_data_with_footprint_arrays(...)
//!   → ChartEngine::set_data_with_footprint(...)
//!     → Geometry generator reads FootprintData during draw_candles
//!       → Renders bid/ask cells, POC marker, value area, imbalances
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use axiuscharts::footprint::*;
//!
//! let mut fp_data = FootprintData::new();
//!
//! let bar = FootprintBar {
//!     levels: vec![
//!         FootprintLevel { price: 100.0, bid_volume: 150.0, ask_volume: 200.0 },
//!         FootprintLevel { price: 100.5, bid_volume: 300.0, ask_volume: 100.0 },
//!         FootprintLevel { price: 101.0, bid_volume: 50.0, ask_volume: 400.0 },
//!     ],
//! };
//! fp_data.set_bar(0, bar);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
// Display Mode
// ═══════════════════════════════════════════════════════════════════════════════

/// How the footprint cells are displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintDisplayMode {
    /// Show bid volume on left, ask volume on right (e.g., "150 × 200").
    #[default]
    BidAsk,
    /// Show delta (ask − bid) per level. Positive = buying pressure.
    Delta,
    /// Show total volume (bid + ask) per level.
    Volume,
    /// Show delta as a horizontal profile bar.
    DeltaProfile,
    /// Show total volume as a horizontal profile bar.
    VolumeProfile,
}

impl FootprintDisplayMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bid_ask" | "bidask" | "bid_x_ask" => Self::BidAsk,
            "delta" => Self::Delta,
            "volume" | "vol" => Self::Volume,
            "delta_profile" | "deltaprofile" => Self::DeltaProfile,
            "volume_profile" | "volumeprofile" => Self::VolumeProfile,
            _ => Self::BidAsk,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BidAsk => "bid_ask",
            Self::Delta => "delta",
            Self::Volume => "volume",
            Self::DeltaProfile => "delta_profile",
            Self::VolumeProfile => "volume_profile",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Imbalance Type
// ═══════════════════════════════════════════════════════════════════════════════

/// Type of imbalance detected at a price level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImbalanceType {
    /// No imbalance detected.
    None,
    /// Buy imbalance: ask volume significantly exceeds bid at this level.
    BuyImbalance,
    /// Sell imbalance: bid volume significantly exceeds ask at this level.
    SellImbalance,
}

/// Diagonal imbalance — comparing bid at one level with ask at adjacent level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagonalImbalanceType {
    None,
    /// Ask at level N vs Bid at level N+1 shows buying absorption.
    BuyAbsorption,
    /// Bid at level N vs Ask at level N-1 shows selling absorption.
    SellAbsorption,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Footprint Level — a single price row within a bar
// ═══════════════════════════════════════════════════════════════════════════════

/// A single price level within a footprint bar.
///
/// Stores bid (sell-side) and ask (buy-side) volume traded at this price.
/// The price represents the lower bound of the tick range:
/// e.g., price=100.0 with tick_size=0.5 covers [100.0, 100.5).
#[derive(Debug, Clone, Copy)]
pub struct FootprintLevel {
    /// Price at this level (lower bound of tick range).
    pub price: f64,
    /// Volume traded on the bid side (sellers hitting bids).
    pub bid_volume: f64,
    /// Volume traded on the ask side (buyers lifting asks).
    pub ask_volume: f64,
}

impl FootprintLevel {
    /// Total volume at this level.
    #[inline]
    pub fn total_volume(&self) -> f64 {
        self.bid_volume + self.ask_volume
    }

    /// Delta at this level (positive = buying pressure).
    #[inline]
    pub fn delta(&self) -> f64 {
        self.ask_volume - self.bid_volume
    }

    /// Check for imbalance at this level given a ratio threshold.
    /// Typical threshold: 3.0 (300%).
    pub fn imbalance(&self, ratio: f64) -> ImbalanceType {
        if self.bid_volume <= 0.0 && self.ask_volume <= 0.0 {
            return ImbalanceType::None;
        }
        if self.bid_volume <= 0.0 {
            return ImbalanceType::BuyImbalance;
        }
        if self.ask_volume <= 0.0 {
            return ImbalanceType::SellImbalance;
        }
        let ask_bid_ratio = self.ask_volume / self.bid_volume;
        let bid_ask_ratio = self.bid_volume / self.ask_volume;
        if ask_bid_ratio >= ratio {
            ImbalanceType::BuyImbalance
        } else if bid_ask_ratio >= ratio {
            ImbalanceType::SellImbalance
        } else {
            ImbalanceType::None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Footprint Bar — all levels for one candlestick
// ═══════════════════════════════════════════════════════════════════════════════

/// Complete footprint data for a single bar (candlestick).
///
/// Contains all price levels with bid/ask volume, plus precomputed
/// analytics (POC, value area, cumulative delta).
#[derive(Debug, Clone)]
pub struct FootprintBar {
    /// Price levels, sorted by price ascending.
    pub levels: Vec<FootprintLevel>,
}

impl FootprintBar {
    pub fn new() -> Self {
        Self { levels: Vec::new() }
    }

    /// Total volume across all levels.
    pub fn total_volume(&self) -> f64 {
        self.levels.iter().map(|l| l.total_volume()).sum()
    }

    /// Total bid volume.
    pub fn total_bid(&self) -> f64 {
        self.levels.iter().map(|l| l.bid_volume).sum()
    }

    /// Total ask volume.
    pub fn total_ask(&self) -> f64 {
        self.levels.iter().map(|l| l.ask_volume).sum()
    }

    /// Net delta (total ask - total bid). Positive = net buying.
    pub fn net_delta(&self) -> f64 {
        self.total_ask() - self.total_bid()
    }

    /// Infer a representative tick size from adjacent price levels.
    ///
    /// Uses the median positive level-to-level diff to avoid overreacting to
    /// occasional outlier spacing or floating-point noise.
    pub fn inferred_tick_size(&self) -> f64 {
        if self.levels.len() < 2 {
            return 1.0;
        }
        let mut diffs: Vec<f64> = self
            .levels
            .windows(2)
            .filter_map(|w| {
                let d = (w[1].price - w[0].price).abs();
                if d.is_finite() && d > f64::EPSILON {
                    Some(d)
                } else {
                    None
                }
            })
            .collect();
        if diffs.is_empty() {
            return 1.0;
        }
        diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = diffs.len() / 2;
        if diffs.len() % 2 == 0 {
            (diffs[mid - 1] + diffs[mid]) * 0.5
        } else {
            diffs[mid]
        }
    }

    /// Find the Point of Control — the price level with the highest total volume.
    /// Returns (level_index, &FootprintLevel) or None if empty.
    pub fn poc(&self) -> Option<(usize, &FootprintLevel)> {
        if self.levels.is_empty() {
            return None;
        }
        let mut max_vol = 0.0f64;
        let mut max_idx = 0;
        for (i, level) in self.levels.iter().enumerate() {
            let vol = level.total_volume();
            if vol > max_vol {
                max_vol = vol;
                max_idx = i;
            }
        }
        Some((max_idx, &self.levels[max_idx]))
    }

    /// Compute the Value Area — the range of prices containing `pct` (0.0-1.0)
    /// of the total volume, centered on the POC.
    ///
    /// Returns (va_low_idx, va_high_idx) inclusive indices into `self.levels`.
    /// Standard value: 0.70 (70%).
    pub fn value_area(&self, pct: f64) -> Option<(usize, usize)> {
        if self.levels.is_empty() {
            return None;
        }

        let total = self.total_volume();
        if total <= 0.0 {
            return None;
        }

        let target = total * pct.clamp(0.0, 1.0);
        let (poc_idx, _) = self.poc()?;

        let mut va_low = poc_idx;
        let mut va_high = poc_idx;
        let mut va_vol = self.levels[poc_idx].total_volume();

        // Expand outward from POC until we capture enough volume
        while va_vol < target {
            let can_go_down = va_low > 0;
            let can_go_up = va_high < self.levels.len() - 1;

            if !can_go_down && !can_go_up {
                break;
            }

            let vol_below = if can_go_down {
                self.levels[va_low - 1].total_volume()
            } else {
                0.0
            };
            let vol_above = if can_go_up {
                self.levels[va_high + 1].total_volume()
            } else {
                0.0
            };

            if vol_below >= vol_above {
                va_low -= 1;
                va_vol += vol_below;
            } else {
                va_high += 1;
                va_vol += vol_above;
            }
        }

        Some((va_low, va_high))
    }

    /// Detect stacked imbalances — consecutive levels with the same imbalance type.
    /// Returns a list of (start_idx, end_idx, ImbalanceType) for runs of
    /// `min_stack` or more consecutive imbalances.
    pub fn stacked_imbalances(
        &self,
        ratio: f64,
        min_stack: usize,
    ) -> Vec<(usize, usize, ImbalanceType)> {
        if self.levels.len() < min_stack {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut run_start = 0;
        let mut run_type = ImbalanceType::None;
        let mut run_len = 0;

        for (i, level) in self.levels.iter().enumerate() {
            let imb = level.imbalance(ratio);
            if imb == run_type && imb != ImbalanceType::None {
                run_len += 1;
            } else {
                if run_len >= min_stack && run_type != ImbalanceType::None {
                    result.push((run_start, i - 1, run_type));
                }
                run_start = i;
                run_type = imb;
                run_len = if imb != ImbalanceType::None { 1 } else { 0 };
            }
        }

        if run_len >= min_stack && run_type != ImbalanceType::None {
            result.push((run_start, self.levels.len() - 1, run_type));
        }

        result
    }

    /// Detect diagonal imbalances between adjacent levels.
    /// Compares ask at level N with bid at level N+1 (and vice versa).
    pub fn diagonal_imbalances(&self, ratio: f64) -> Vec<(usize, DiagonalImbalanceType)> {
        let mut result = Vec::new();
        if self.levels.len() < 2 {
            return result;
        }

        for i in 0..self.levels.len() - 1 {
            let current = &self.levels[i];
            let above = &self.levels[i + 1];

            // Buy absorption: ask at current level vs bid at level above
            if above.bid_volume > 0.0 && current.ask_volume / above.bid_volume >= ratio {
                result.push((i, DiagonalImbalanceType::BuyAbsorption));
            } else if current.ask_volume > 0.0 && above.bid_volume / current.ask_volume >= ratio {
                result.push((i, DiagonalImbalanceType::SellAbsorption));
            }
        }

        result
    }

    /// Detect unfinished auction — zero volume at high or low of the bar.
    /// An unfinished auction at the high means potential for further upside;
    /// at the low, potential for further downside.
    pub fn unfinished_auction(&self) -> (bool, bool) {
        if self.levels.is_empty() {
            return (false, false);
        }

        let first = &self.levels[0];
        let last = &self.levels[self.levels.len() - 1];

        // Unfinished at low: ask volume at the lowest level is 0
        let unfinished_low = first.ask_volume <= 0.0 && first.bid_volume > 0.0;
        // Unfinished at high: bid volume at the highest level is 0
        let unfinished_high = last.bid_volume <= 0.0 && last.ask_volume > 0.0;

        (unfinished_low, unfinished_high)
    }

    /// Maximum volume at any single level (for scaling).
    pub fn max_level_volume(&self) -> f64 {
        self.levels
            .iter()
            .map(|l| l.total_volume())
            .fold(0.0f64, f64::max)
    }

    /// Maximum delta absolute value at any level (for scaling).
    pub fn max_level_delta_abs(&self) -> f64 {
        self.levels
            .iter()
            .map(|l| l.delta().abs())
            .fold(0.0f64, f64::max)
    }

    /// Maximum bid or ask volume at any level (for BidAsk mode scaling).
    pub fn max_side_volume(&self) -> f64 {
        self.levels
            .iter()
            .map(|l| l.bid_volume.max(l.ask_volume))
            .fold(0.0f64, f64::max)
    }

    /// Merge every `factor` adjacent levels into one, summing volumes.
    ///
    /// The resulting levels use the lowest price in each group as the level
    /// price and have an effective tick size of `original_tick * factor`.
    /// This is the core of dynamic aggregation: when the Y-axis is squeezed
    /// so individual tick-rows become too small, the caller passes a factor >1
    /// to combine rows into readable cells.
    ///
    /// `factor == 1` returns a clone of the original levels unchanged.
    pub fn aggregate_levels(&self, factor: usize) -> Vec<FootprintLevel> {
        if factor <= 1 || self.levels.is_empty() {
            return self.levels.clone();
        }
        let mut out = Vec::with_capacity(self.levels.len() / factor + 1);
        for chunk in self.levels.chunks(factor) {
            let price = chunk[0].price; // lowest price in group (levels sorted asc)
            let bid: f64 = chunk.iter().map(|l| l.bid_volume).sum();
            let ask: f64 = chunk.iter().map(|l| l.ask_volume).sum();
            out.push(FootprintLevel {
                price,
                bid_volume: bid,
                ask_volume: ask,
            });
        }
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Footprint Data — collection of footprint bars
// ═══════════════════════════════════════════════════════════════════════════════

/// Collection of footprint bars indexed by bar index.
///
/// Not all bars need footprint data — bars without footprint data
/// are rendered as normal candlesticks.
#[derive(Debug, Clone)]
pub struct FootprintData {
    /// Map from bar index → FootprintBar.
    bars: HashMap<usize, FootprintBar>,
}

impl FootprintData {
    pub fn new() -> Self {
        Self {
            bars: HashMap::new(),
        }
    }

    /// Set footprint data for a specific bar index.
    /// Levels should be sorted by price ascending.
    pub fn set_bar(&mut self, bar_idx: usize, mut bar: FootprintBar) {
        // Ensure levels are sorted by price
        bar.levels.sort_by(|a, b| {
            a.price
                .partial_cmp(&b.price)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.bars.insert(bar_idx, bar);
    }

    /// Get footprint data for a bar index.
    pub fn get_bar(&self, bar_idx: usize) -> Option<&FootprintBar> {
        self.bars.get(&bar_idx)
    }

    /// Remove footprint data for a bar index.
    pub fn remove_bar(&mut self, bar_idx: usize) -> Option<FootprintBar> {
        self.bars.remove(&bar_idx)
    }

    /// Clear all footprint data.
    pub fn clear(&mut self) {
        self.bars.clear();
    }

    /// Set footprint data for multiple bars at once (bulk load).
    pub fn set_bars(&mut self, bars: Vec<(usize, FootprintBar)>) {
        self.bars.clear();
        for (idx, bar) in bars {
            self.set_bar(idx, bar);
        }
    }

    /// Number of bars with footprint data.
    pub fn len(&self) -> usize {
        self.bars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bars.is_empty()
    }

    /// Check if a bar has footprint data.
    pub fn has_bar(&self, bar_idx: usize) -> bool {
        self.bars.contains_key(&bar_idx)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Footprint Theme
// ═══════════════════════════════════════════════════════════════════════════════

/// High-level footprint directional palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintPalette {
    /// Blue for buying / positive states, red for selling / negative states.
    #[default]
    BlueRed,
    /// Green for buying / positive states, red for selling / negative states.
    GreenRed,
}

impl FootprintPalette {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "green_red" | "green-red" | "greenred" => Self::GreenRed,
            _ => Self::BlueRed,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BlueRed => "blue_red",
            Self::GreenRed => "green_red",
        }
    }
}

/// How aggressively footprint fills should glow/intensify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintGradientStyle {
    /// Subtle production-safe glow and opacity ramp.
    SoftGlow,
    /// Stronger emphasis with brighter highlights.
    StrongGlow,
    /// Flat fills with no extra glow emphasis.
    #[default]
    NoGlow,
}

impl FootprintGradientStyle {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "soft_glow" | "soft-glow" | "softglow" => Self::SoftGlow,
            "strong_glow" | "strong-glow" | "strongglow" => Self::StrongGlow,
            "no_glow" | "no-glow" | "noglow" | "none" | "flat" => Self::NoGlow,
            _ => Self::NoGlow,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SoftGlow => "soft_glow",
            Self::StrongGlow => "strong_glow",
            Self::NoGlow => "no_glow",
        }
    }
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn with_alpha(mut color: [f32; 4], alpha: f32) -> [f32; 4] {
    color[3] = clamp01(alpha);
    color
}

fn mix_rgb(a: [f32; 4], b: [f32; 4], t: f32, alpha: f32) -> [f32; 4] {
    let t = clamp01(t);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        clamp01(alpha),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════════
// Footprint Options — pro configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Complete configuration for footprint chart rendering.
#[derive(Debug, Clone)]
pub struct FootprintOptions {
    /// Display mode (BidAsk, Delta, Volume, etc.).
    pub display_mode: FootprintDisplayMode,

    /// Directional palette family used by all footprint states.
    pub palette: FootprintPalette,

    /// Intensity/glow treatment for footprint fills.
    pub gradient_style: FootprintGradientStyle,

    /// Tick size — price granularity per row.
    /// E.g., 0.5 means each row covers a $0.50 range.
    /// If 0.0 or negative, auto-calculated from data.
    pub tick_size: f64,

    // ── Imbalance settings ──
    /// Imbalance ratio threshold (e.g., 3.0 = 300%).
    /// A level is flagged as imbalanced when one side exceeds the other by this ratio.
    pub imbalance_ratio: f64,
    /// Whether to highlight imbalances.
    pub show_imbalances: bool,
    /// Minimum consecutive levels for stacked imbalance detection.
    pub stacked_imbalance_min: usize,
    /// Whether to show stacked imbalances.
    pub show_stacked_imbalances: bool,
    /// Whether to show diagonal imbalances.
    pub show_diagonal_imbalances: bool,

    // ── POC & Value Area ──
    /// Whether to show the Point of Control marker.
    pub show_poc: bool,
    /// POC marker color [R, G, B, A].
    pub poc_color: [f32; 4],
    /// POC marker width (fraction of bar width, 0.0-1.0).
    pub poc_width: f32,
    /// Whether to show the Value Area.
    pub show_value_area: bool,
    /// Value Area percentage (0.0-1.0, default 0.70).
    pub value_area_pct: f64,
    /// Value Area fill color [R, G, B, A].
    pub value_area_color: [f32; 4],

    // ── Unfinished auction ──
    /// Whether to show unfinished auction markers.
    pub show_unfinished_auction: bool,
    /// Unfinished auction marker color.
    pub unfinished_auction_color: [f32; 4],

    // ── Colors ──
    /// Buy (ask) volume text/bar color.
    pub buy_color: [f32; 4],
    /// Sell (bid) volume text/bar color.
    pub sell_color: [f32; 4],
    /// Buy imbalance highlight color.
    pub buy_imbalance_color: [f32; 4],
    /// Sell imbalance highlight color.
    pub sell_imbalance_color: [f32; 4],
    /// Stacked buy imbalance highlight color.
    pub stacked_buy_imbalance_color: [f32; 4],
    /// Stacked sell imbalance highlight color.
    pub stacked_sell_imbalance_color: [f32; 4],
    /// Diagonal imbalance indicator color.
    pub diagonal_imbalance_color: [f32; 4],
    /// Positive delta color (for Delta mode).
    pub positive_delta_color: [f32; 4],
    /// Negative delta color (for Delta mode).
    pub negative_delta_color: [f32; 4],
    /// Cell background color (base).
    pub cell_bg_color: [f32; 4],
    /// Cell border/outline color.
    pub cell_border_color: [f32; 4],
    /// Text color for volume numbers.
    pub text_color: [f32; 4],
    /// High volume cell background tint.
    pub high_volume_color: [f32; 4],
    /// Cumulative delta bar positive color.
    pub cum_delta_positive_color: [f32; 4],
    /// Cumulative delta bar negative color.
    pub cum_delta_negative_color: [f32; 4],

    // ── Layout ──
    /// Minimum cell height in CSS pixels (default 1).
    pub min_cell_height: f32,
    /// Cell inset in CSS pixels.
    pub cell_inset: f32,
    /// Font size for volume text in CSS pixels.
    pub font_size: f32,
    /// Whether to show the volume text within cells.
    pub show_volume_text: bool,
    /// Whether to show the cumulative delta column on the right.
    pub show_cumulative_delta: bool,
    /// Cumulative delta column width in CSS pixels.
    pub cumulative_delta_width: f32,
    /// Whether to show the total delta row at the bottom.
    pub show_delta_bar: bool,
    /// Delta bar height in CSS pixels.
    pub delta_bar_height: f32,
    /// Volume color intensity scaling mode.
    pub volume_color_intensity: VolumeColorIntensity,
    /// Whether pane wheel/pinch zoom should scale both time and price axes.
    pub zoom_price_with_time: bool,
}

impl FootprintOptions {
    /// Effective minimum row height (CSS px) used for dynamic aggregation.
    ///
    /// When volume text is visible, require enough height for readable text so
    /// rows are merged before labels become tiny/illegible.
    pub fn aggregation_min_cell_height_css(&self) -> f64 {
        let base = self.min_cell_height.max(0.0) as f64;
        if !self.show_volume_text {
            return base.max(1.0);
        }
        // Keep the merge threshold compact by default; text renderer already
        // adapts font size down to 4px when space is tight.
        let text_target = (self.font_size.max(0.0) as f64 * 0.45 + 0.5).clamp(5.0, 7.0);
        base.max(text_target)
    }

    /// Refresh derived directional/neutral colors from the semantic palette.
    pub fn apply_semantic_theme(&mut self) {
        use crate::core::renderer::theme::{ch, BEARISH};

        let blue = [ch(0x35), ch(0x59), ch(0xE9), 1.0];
        let red = [ch(0xFB), ch(0x37), ch(0x48), 1.0];
        let green = [ch(0x00), ch(0xC8), ch(0x72), 1.0];
        let bearish = with_alpha(BEARISH, 1.0);
        let neutral_bg = [0.10, 0.11, 0.14, 0.92];
        let neutral_border = [0.57, 0.61, 0.72, 0.14];
        let neutral_text = [0.88, 0.90, 0.96, 1.0];
        let neutral_va = [0.52, 0.56, 0.67, 0.08];
        let neutral_high = [0.95, 0.97, 1.0, 0.12];

        let (buy_base, sell_base) = match self.palette {
            FootprintPalette::BlueRed => (blue, red),
            FootprintPalette::GreenRed => (green, bearish),
        };

        let (base_alpha, emphasis_alpha, stacked_alpha, border_alpha, va_alpha, high_alpha) =
            match self.gradient_style {
                FootprintGradientStyle::SoftGlow => (0.90, 0.38, 0.52, 0.14, 0.08, 0.12),
                FootprintGradientStyle::StrongGlow => (0.96, 0.50, 0.66, 0.18, 0.10, 0.16),
                FootprintGradientStyle::NoGlow => (0.84, 0.26, 0.34, 0.10, 0.06, 0.08),
            };

        self.buy_color = with_alpha(buy_base, base_alpha);
        self.sell_color = with_alpha(sell_base, base_alpha);
        self.buy_imbalance_color = mix_rgb(buy_base, [1.0, 1.0, 1.0, 1.0], 0.10, emphasis_alpha);
        self.sell_imbalance_color = mix_rgb(sell_base, [1.0, 1.0, 1.0, 1.0], 0.08, emphasis_alpha);
        self.stacked_buy_imbalance_color =
            mix_rgb(buy_base, [1.0, 1.0, 1.0, 1.0], 0.16, stacked_alpha);
        self.stacked_sell_imbalance_color =
            mix_rgb(sell_base, [1.0, 1.0, 1.0, 1.0], 0.12, stacked_alpha);
        self.diagonal_imbalance_color = mix_rgb(
            self.poc_color,
            [1.0, 1.0, 1.0, 1.0],
            0.06,
            emphasis_alpha * 0.9,
        );
        self.positive_delta_color = with_alpha(buy_base, 0.92);
        self.negative_delta_color = with_alpha(sell_base, 0.92);
        self.cell_bg_color = neutral_bg;
        self.cell_border_color = with_alpha(neutral_border, border_alpha);
        self.text_color = neutral_text;
        self.high_volume_color = with_alpha(neutral_high, high_alpha);
        self.cum_delta_positive_color = with_alpha(buy_base, base_alpha * 0.82);
        self.cum_delta_negative_color = with_alpha(sell_base, base_alpha * 0.82);
        self.value_area_color = with_alpha(neutral_va, va_alpha);
        self.unfinished_auction_color = mix_rgb(
            self.poc_color,
            [0.92, 0.94, 1.0, 1.0],
            0.08,
            emphasis_alpha * 0.9,
        );
    }
}

/// How volume magnitude affects cell color intensity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VolumeColorIntensity {
    /// No color scaling — all cells same opacity.
    None,
    /// Linear scaling from 0 to max volume.
    #[default]
    Linear,
    /// Logarithmic scaling (better for high-volume outliers).
    Logarithmic,
}

impl Default for FootprintOptions {
    fn default() -> Self {
        use crate::core::renderer::theme::ch;

        let mut opts = Self {
            display_mode: FootprintDisplayMode::BidAsk,
            palette: FootprintPalette::BlueRed,
            gradient_style: FootprintGradientStyle::NoGlow,
            tick_size: 0.0, // auto

            // Imbalance settings
            imbalance_ratio: 3.0,
            show_imbalances: true,
            stacked_imbalance_min: 3,
            show_stacked_imbalances: true,
            show_diagonal_imbalances: false,

            // POC & Value Area
            show_poc: true,
            poc_color: [ch(0xFF), ch(0xD7), ch(0x00), 0.9], // Gold
            poc_width: 0.06,
            show_value_area: true,
            value_area_pct: 0.70,
            value_area_color: [0.0, 0.0, 0.0, 0.0],

            // Unfinished auction
            show_unfinished_auction: true,
            unfinished_auction_color: [0.0, 0.0, 0.0, 0.0],

            // Colors
            buy_color: [0.0, 0.0, 0.0, 0.0],
            sell_color: [0.0, 0.0, 0.0, 0.0],
            buy_imbalance_color: [0.0, 0.0, 0.0, 0.0],
            sell_imbalance_color: [0.0, 0.0, 0.0, 0.0],
            stacked_buy_imbalance_color: [0.0, 0.0, 0.0, 0.0],
            stacked_sell_imbalance_color: [0.0, 0.0, 0.0, 0.0],
            diagonal_imbalance_color: [0.0, 0.0, 0.0, 0.0],
            positive_delta_color: [0.0, 0.0, 0.0, 0.0],
            negative_delta_color: [0.0, 0.0, 0.0, 0.0],
            cell_bg_color: [0.0, 0.0, 0.0, 0.0],
            cell_border_color: [0.0, 0.0, 0.0, 0.0],
            text_color: [0.0, 0.0, 0.0, 0.0],
            high_volume_color: [0.0, 0.0, 0.0, 0.0],
            cum_delta_positive_color: [0.0, 0.0, 0.0, 0.0],
            cum_delta_negative_color: [0.0, 0.0, 0.0, 0.0],

            // Layout
            min_cell_height: 1.0,
            cell_inset: 1.0,
            font_size: 10.0,
            show_volume_text: true,
            show_cumulative_delta: false,
            cumulative_delta_width: 40.0,
            show_delta_bar: true,
            delta_bar_height: 16.0,
            volume_color_intensity: VolumeColorIntensity::Linear,
            zoom_price_with_time: true,
        };
        opts.apply_semantic_theme();
        opts
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bar() -> FootprintBar {
        FootprintBar {
            levels: vec![
                FootprintLevel {
                    price: 100.0,
                    bid_volume: 500.0,
                    ask_volume: 100.0,
                },
                FootprintLevel {
                    price: 100.5,
                    bid_volume: 200.0,
                    ask_volume: 800.0,
                },
                FootprintLevel {
                    price: 101.0,
                    bid_volume: 300.0,
                    ask_volume: 300.0,
                },
                FootprintLevel {
                    price: 101.5,
                    bid_volume: 100.0,
                    ask_volume: 600.0,
                },
                FootprintLevel {
                    price: 102.0,
                    bid_volume: 50.0,
                    ask_volume: 400.0,
                },
            ],
        }
    }

    #[test]
    fn test_level_delta() {
        let level = FootprintLevel {
            price: 100.0,
            bid_volume: 150.0,
            ask_volume: 200.0,
        };
        assert_eq!(level.delta(), 50.0);
        assert_eq!(level.total_volume(), 350.0);
    }

    #[test]
    fn test_level_imbalance() {
        let buy_imb = FootprintLevel {
            price: 100.0,
            bid_volume: 100.0,
            ask_volume: 400.0,
        };
        assert_eq!(buy_imb.imbalance(3.0), ImbalanceType::BuyImbalance);

        let sell_imb = FootprintLevel {
            price: 100.0,
            bid_volume: 400.0,
            ask_volume: 100.0,
        };
        assert_eq!(sell_imb.imbalance(3.0), ImbalanceType::SellImbalance);

        let no_imb = FootprintLevel {
            price: 100.0,
            bid_volume: 200.0,
            ask_volume: 250.0,
        };
        assert_eq!(no_imb.imbalance(3.0), ImbalanceType::None);
    }

    #[test]
    fn test_poc() {
        let bar = sample_bar();
        let (idx, level) = bar.poc().unwrap();
        assert_eq!(idx, 1); // 200 + 800 = 1000, highest
        assert_eq!(level.price, 100.5);
    }

    #[test]
    fn test_inferred_tick_size_median() {
        let bar = sample_bar();
        assert!((bar.inferred_tick_size() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_inferred_tick_size_ignores_tiny_outlier() {
        let bar = FootprintBar {
            levels: vec![
                FootprintLevel {
                    price: 100.0,
                    bid_volume: 1.0,
                    ask_volume: 1.0,
                },
                FootprintLevel {
                    price: 100.000001,
                    bid_volume: 1.0,
                    ask_volume: 1.0,
                },
                FootprintLevel {
                    price: 100.5,
                    bid_volume: 1.0,
                    ask_volume: 1.0,
                },
                FootprintLevel {
                    price: 101.0,
                    bid_volume: 1.0,
                    ask_volume: 1.0,
                },
            ],
        };
        // diffs are [~1e-6, ~0.499999, 0.5] -> median should stay around 0.5
        assert!((bar.inferred_tick_size() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn test_value_area() {
        let bar = sample_bar();
        let (va_low, va_high) = bar.value_area(0.70).unwrap();
        // Total volume = 600 + 1000 + 600 + 700 + 450 = 3350
        // 70% = 2345. POC at idx 1 (1000).
        // Expand: idx 0 (600) and idx 2 (600) → 2200
        // Next: idx 3 (700) vs nothing left below → 2900 > 2345
        assert!(va_low <= 1);
        assert!(va_high >= 1);
    }

    #[test]
    fn test_net_delta() {
        let bar = sample_bar();
        let delta = bar.net_delta();
        // Total ask = 100+800+300+600+400 = 2200
        // Total bid = 500+200+300+100+50 = 1150
        assert_eq!(delta, 1050.0);
    }

    #[test]
    fn test_unfinished_auction() {
        let bar = FootprintBar {
            levels: vec![
                FootprintLevel {
                    price: 100.0,
                    bid_volume: 200.0,
                    ask_volume: 0.0,
                }, // unfinished low
                FootprintLevel {
                    price: 101.0,
                    bid_volume: 100.0,
                    ask_volume: 100.0,
                },
                FootprintLevel {
                    price: 102.0,
                    bid_volume: 0.0,
                    ask_volume: 300.0,
                }, // unfinished high
            ],
        };
        let (low, high) = bar.unfinished_auction();
        assert!(low);
        assert!(high);
    }

    #[test]
    fn test_stacked_imbalances() {
        let bar = FootprintBar {
            levels: vec![
                FootprintLevel {
                    price: 100.0,
                    bid_volume: 100.0,
                    ask_volume: 400.0,
                }, // buy
                FootprintLevel {
                    price: 100.5,
                    bid_volume: 50.0,
                    ask_volume: 500.0,
                }, // buy
                FootprintLevel {
                    price: 101.0,
                    bid_volume: 80.0,
                    ask_volume: 400.0,
                }, // buy
                FootprintLevel {
                    price: 101.5,
                    bid_volume: 200.0,
                    ask_volume: 200.0,
                }, // none
            ],
        };
        let stacked = bar.stacked_imbalances(3.0, 3);
        assert_eq!(stacked.len(), 1);
        assert_eq!(stacked[0].0, 0); // start
        assert_eq!(stacked[0].1, 2); // end
        assert_eq!(stacked[0].2, ImbalanceType::BuyImbalance);
    }

    #[test]
    fn test_footprint_data() {
        let mut data = FootprintData::new();
        assert!(data.is_empty());

        data.set_bar(0, sample_bar());
        assert_eq!(data.len(), 1);
        assert!(data.has_bar(0));
        assert!(!data.has_bar(1));

        let bar = data.get_bar(0).unwrap();
        assert_eq!(bar.levels.len(), 5);
    }

    #[test]
    fn test_display_mode_roundtrip() {
        for mode in &[
            FootprintDisplayMode::BidAsk,
            FootprintDisplayMode::Delta,
            FootprintDisplayMode::Volume,
            FootprintDisplayMode::DeltaProfile,
            FootprintDisplayMode::VolumeProfile,
        ] {
            let s = mode.as_str();
            let parsed = FootprintDisplayMode::from_str(s);
            assert_eq!(*mode, parsed);
        }
    }

    #[test]
    fn test_palette_roundtrip() {
        for palette in &[FootprintPalette::BlueRed, FootprintPalette::GreenRed] {
            let parsed = FootprintPalette::from_str(palette.as_str());
            assert_eq!(*palette, parsed);
        }
    }

    #[test]
    fn test_gradient_style_roundtrip() {
        for style in &[
            FootprintGradientStyle::SoftGlow,
            FootprintGradientStyle::StrongGlow,
            FootprintGradientStyle::NoGlow,
        ] {
            let parsed = FootprintGradientStyle::from_str(style.as_str());
            assert_eq!(*style, parsed);
        }
        assert_eq!(
            FootprintGradientStyle::default(),
            FootprintGradientStyle::NoGlow
        );
        assert_eq!(
            FootprintGradientStyle::from_str("unknown"),
            FootprintGradientStyle::NoGlow
        );
    }

    #[test]
    fn default_palette_is_blue_red_without_extra_green_or_orange_states() {
        let opts = FootprintOptions::default();
        assert_eq!(opts.palette, FootprintPalette::BlueRed);
        assert_eq!(opts.gradient_style, FootprintGradientStyle::NoGlow);

        assert!(
            opts.buy_color[2] > opts.buy_color[1] && opts.buy_color[2] > opts.buy_color[0],
            "default buy color should be blue-dominant"
        );
        assert!(
            opts.sell_color[0] > opts.sell_color[1] && opts.sell_color[0] > opts.sell_color[2],
            "default sell color should be red-dominant"
        );
        assert!(
            opts.positive_delta_color[2] > opts.positive_delta_color[1],
            "positive delta should stay in the positive palette family"
        );
        assert!(
            opts.negative_delta_color[0] > opts.negative_delta_color[2],
            "negative delta should stay in the negative palette family"
        );
        assert!(
            opts.unfinished_auction_color[0] >= 0.75 && opts.unfinished_auction_color[1] >= 0.65,
            "unfinished auction should use POC/neutral accenting instead of orange"
        );
    }

    #[test]
    fn green_red_palette_updates_all_directional_states() {
        let mut opts = FootprintOptions::default();
        opts.palette = FootprintPalette::GreenRed;
        opts.apply_semantic_theme();

        assert!(
            opts.buy_color[1] > opts.buy_color[0] && opts.buy_color[1] > opts.buy_color[2],
            "green_red buy state should be green-dominant"
        );
        assert!(
            opts.positive_delta_color[1] > opts.positive_delta_color[0]
                && opts.positive_delta_color[1] > opts.positive_delta_color[2],
            "green_red positive delta should be green-dominant"
        );
        assert!(
            opts.cum_delta_positive_color[1] > opts.cum_delta_positive_color[0],
            "green_red cumulative positive delta should stay green"
        );
        assert!(
            opts.sell_color[0] > opts.sell_color[1] && opts.sell_color[0] > opts.sell_color[2],
            "green_red sell state should remain red-dominant"
        );
    }

    #[test]
    fn test_aggregate_levels_factor_1_returns_clone() {
        let bar = sample_bar();
        let agg = bar.aggregate_levels(1);
        assert_eq!(agg.len(), bar.levels.len());
        for (a, b) in agg.iter().zip(bar.levels.iter()) {
            assert_eq!(a.price, b.price);
            assert_eq!(a.bid_volume, b.bid_volume);
            assert_eq!(a.ask_volume, b.ask_volume);
        }
    }

    #[test]
    fn test_aggregate_levels_factor_2() {
        let bar = sample_bar(); // 5 levels
        let agg = bar.aggregate_levels(2);
        // 5 levels / 2 = 2 full chunks + 1 remainder = 3 aggregated levels
        assert_eq!(agg.len(), 3);
        // First chunk: levels 0+1
        assert_eq!(agg[0].price, 100.0); // lowest price in chunk
        assert_eq!(agg[0].bid_volume, 500.0 + 200.0);
        assert_eq!(agg[0].ask_volume, 100.0 + 800.0);
        // Second chunk: levels 2+3
        assert_eq!(agg[1].price, 101.0);
        assert_eq!(agg[1].bid_volume, 300.0 + 100.0);
        assert_eq!(agg[1].ask_volume, 300.0 + 600.0);
        // Third chunk: level 4 (remainder)
        assert_eq!(agg[2].price, 102.0);
        assert_eq!(agg[2].bid_volume, 50.0);
        assert_eq!(agg[2].ask_volume, 400.0);
    }

    #[test]
    fn test_aggregate_levels_factor_larger_than_levels() {
        let bar = sample_bar(); // 5 levels
        let agg = bar.aggregate_levels(10);
        // All levels merge into one
        assert_eq!(agg.len(), 1);
        assert_eq!(agg[0].price, 100.0);
        let total_bid: f64 = bar.levels.iter().map(|l| l.bid_volume).sum();
        let total_ask: f64 = bar.levels.iter().map(|l| l.ask_volume).sum();
        assert_eq!(agg[0].bid_volume, total_bid);
        assert_eq!(agg[0].ask_volume, total_ask);
    }
}
