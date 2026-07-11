//! Time scale layout state machine. Port of the coordinate/zoom/scroll math from
//! `src/model/time-scale.ts` (tick mark generation and formatting live elsewhere).
//!
//! State: `(width, bar_spacing, right_offset, base_index)` over a point list of length
//! `points_len`. All coordinates are media (CSS) pixels; indices are integer bar positions,
//! logical values are float bar positions (integers at bar centers).

use crate::helpers::mathex::clamp;
use crate::model::range::{LogicalRange, StrictRange};
use crate::{Coordinate, TimePointIndex};

/// `Constants.MinVisibleBarsCount` in LWC.
const MIN_VISIBLE_BARS_COUNT: f64 = 2.0;

#[derive(Clone, Debug)]
pub struct TimeScaleOptions {
    pub right_offset: f64,
    pub bar_spacing: f64,
    pub min_bar_spacing: f64,
    /// 0 disables the option (default max = half the width).
    pub max_bar_spacing: f64,
    pub fix_left_edge: bool,
    pub fix_right_edge: bool,
    pub lock_visible_time_range_on_resize: bool,
    pub right_bar_stays_on_scroll: bool,
    /// When set, overrides `right_offset` and is preserved in pixels across zoom.
    pub right_offset_pixels: Option<f64>,
}

impl Default for TimeScaleOptions {
    fn default() -> Self {
        Self {
            right_offset: 0.0,
            bar_spacing: 6.0,
            min_bar_spacing: 0.5,
            max_bar_spacing: 0.0,
            fix_left_edge: false,
            fix_right_edge: false,
            lock_visible_time_range_on_resize: false,
            right_bar_stays_on_scroll: false,
            right_offset_pixels: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TransitionState {
    bar_spacing: f64,
    right_offset: f64,
}

#[derive(Clone, Debug)]
pub struct TimeScaleCore {
    options: TimeScaleOptions,
    width: f64,
    base_index: Option<TimePointIndex>,
    right_offset: f64,
    bar_spacing: f64,
    points_len: usize,
    scroll_start_point: Option<Coordinate>,
    scale_start_point: Option<Coordinate>,
    common_transition_start_state: Option<TransitionState>,
}

impl TimeScaleCore {
    pub fn new(options: TimeScaleOptions) -> Self {
        let right_offset = options.right_offset;
        let bar_spacing = options.bar_spacing;
        Self {
            options,
            width: 0.0,
            base_index: None,
            right_offset,
            bar_spacing,
            points_len: 0,
            scroll_start_point: None,
            scale_start_point: None,
            common_transition_start_state: None,
        }
    }

    pub fn options(&self) -> &TimeScaleOptions {
        &self.options
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn bar_spacing(&self) -> f64 {
        self.bar_spacing
    }

    pub fn right_offset(&self) -> f64 {
        self.right_offset
    }

    pub fn points_len(&self) -> usize {
        self.points_len
    }

    /// Base index or 0 when unset (matches LWC's `baseIndex()` getter).
    pub fn base_index(&self) -> TimePointIndex {
        self.base_index.unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0.0 || self.points_len == 0 || self.base_index.is_none()
    }

    pub fn set_points_len(&mut self, len: usize) {
        self.points_len = len;
        self.correct_offset();
    }

    pub fn set_base_index(&mut self, base_index: Option<TimePointIndex>) {
        self.base_index = base_index;
        self.correct_offset();
        self.do_fix_left_edge();
    }

    fn first_index(&self) -> Option<TimePointIndex> {
        if self.points_len == 0 { None } else { Some(0) }
    }

    fn last_index(&self) -> Option<TimePointIndex> {
        if self.points_len == 0 { None } else { Some(self.points_len as i64 - 1) }
    }

    // --- coordinate conversion (RENDERING_SPEC.md §1.1) ---

    pub fn index_to_coordinate(&self, index: TimePointIndex) -> Coordinate {
        if self.is_empty() {
            return 0.0;
        }
        let base_index = self.base_index();
        let delta_from_right = base_index as f64 + self.right_offset - index as f64;
        self.width - (delta_from_right + 0.5) * self.bar_spacing - 1.0
    }

    /// Batch conversion: writes x for each index. The hot path for series rendering.
    pub fn indexes_to_coordinates(&self, indices: &[TimePointIndex], out_x: &mut [Coordinate]) {
        debug_assert_eq!(indices.len(), out_x.len());
        let base = self.base_index() as f64 + self.right_offset;
        for (i, &index) in indices.iter().enumerate() {
            let delta_from_right = base - index as f64;
            out_x[i] = self.width - (delta_from_right + 0.5) * self.bar_spacing - 1.0;
        }
    }

    fn right_offset_for_coordinate(&self, x: Coordinate) -> f64 {
        (self.width - 1.0 - x) / self.bar_spacing
    }

    pub fn coordinate_to_float_index(&self, x: Coordinate) -> f64 {
        let delta_from_right = self.right_offset_for_coordinate(x);
        let base_index = self.base_index();
        let index = base_index as f64 + self.right_offset - delta_from_right;
        // JS-compatible fp-noise cleanup
        (index * 1_000_000.0).round() / 1_000_000.0
    }

    pub fn coordinate_to_index(&self, x: Coordinate) -> TimePointIndex {
        self.coordinate_to_float_index(x).ceil() as TimePointIndex
    }

    // --- visible range ---

    pub fn visible_logical_range(&self) -> Option<LogicalRange> {
        if self.is_empty() {
            return None;
        }

        let base_index = self.base_index();
        let new_bars_length = self.width / self.bar_spacing;
        let right_border = self.right_offset + base_index as f64;
        let left_border = right_border - new_bars_length + 1.0;

        Some(LogicalRange::new(left_border, right_border))
    }

    pub fn visible_strict_range(&self) -> Option<StrictRange> {
        self.visible_logical_range().map(|r| r.to_strict())
    }

    // --- sizing ---

    pub fn set_width(&mut self, new_width: f64) {
        if !new_width.is_finite() || new_width <= 0.0 || self.width == new_width {
            return;
        }

        // capture the previous visible range before mutating (needed for fix_left_edge)
        let previous_visible_range = self.visible_logical_range();

        let old_width = self.width;
        self.width = new_width;

        if self.options.lock_visible_time_range_on_resize && old_width != 0.0 {
            self.bar_spacing = self.bar_spacing * new_width / old_width;
        }

        if self.options.fix_left_edge {
            if let Some(prev) = previous_visible_range {
                if prev.left() <= 0.0 {
                    let delta = old_width - new_width;
                    // reducing right_offset means moving right
                    self.right_offset -= (delta / self.bar_spacing).round() + 1.0;
                }
            }
        }

        // bar spacing first: right offset correction depends on it
        self.correct_bar_spacing();
        self.correct_offset();
    }

    // --- bar spacing / offset mutation ---

    pub fn set_bar_spacing(&mut self, new_bar_spacing: f64) {
        let old_bar_spacing = self.bar_spacing;
        self.set_bar_spacing_internal(new_bar_spacing);
        if self.options.right_offset_pixels.is_some() && old_bar_spacing != 0.0 {
            // pixel mode: keep the pixel offset by rescaling the bar offset
            self.right_offset = self.right_offset * old_bar_spacing / self.bar_spacing;
        }
        self.correct_offset();
    }

    fn set_bar_spacing_internal(&mut self, new_bar_spacing: f64) {
        self.bar_spacing = new_bar_spacing;
        self.correct_bar_spacing();
    }

    pub fn set_right_offset(&mut self, offset: f64) {
        self.right_offset = offset;
        self.correct_offset();
    }

    fn max_bar_spacing(&self) -> f64 {
        if self.options.max_bar_spacing > 0.0 {
            self.options.max_bar_spacing
        } else {
            self.width * 0.5
        }
    }

    fn min_bar_spacing(&self) -> f64 {
        if self.options.fix_left_edge && self.options.fix_right_edge && self.points_len != 0 {
            self.width / self.points_len as f64
        } else {
            self.options.min_bar_spacing
        }
    }

    fn correct_bar_spacing(&mut self) {
        let bar_spacing = clamp(self.bar_spacing, self.min_bar_spacing(), self.max_bar_spacing());
        if self.bar_spacing != bar_spacing {
            self.bar_spacing = bar_spacing;
        }
    }

    fn min_right_offset(&self) -> Option<f64> {
        let first_index = self.first_index()?;
        let base_index = self.base_index?;

        let bars_estimation = if self.options.fix_left_edge {
            self.width / self.bar_spacing
        } else {
            MIN_VISIBLE_BARS_COUNT.min(self.points_len as f64)
        };

        Some(first_index as f64 - base_index as f64 - 1.0 + bars_estimation)
    }

    fn max_right_offset(&self) -> f64 {
        if self.options.fix_right_edge {
            0.0
        } else {
            (self.width / self.bar_spacing) - MIN_VISIBLE_BARS_COUNT.min(self.points_len as f64)
        }
    }

    fn correct_offset(&mut self) {
        // block scrolling into the past
        if let Some(min_right_offset) = self.min_right_offset() {
            if self.right_offset < min_right_offset {
                self.right_offset = min_right_offset;
            }
        }

        // block scrolling into the future
        let max_right_offset = self.max_right_offset();
        if self.right_offset > max_right_offset {
            self.right_offset = max_right_offset;
        }
    }

    fn do_fix_left_edge(&mut self) {
        if !self.options.fix_left_edge {
            return;
        }
        let Some(first_index) = self.first_index() else { return };
        let Some(visible_range) = self.visible_strict_range() else { return };

        let delta = visible_range.left() - first_index;
        if delta < 0 {
            let left_edge_offset = self.right_offset - delta as f64 - 1.0;
            self.set_right_offset(left_edge_offset);
        }
        self.correct_bar_spacing();
    }

    // --- zoom (RENDERING_SPEC.md §1.1) ---

    /// `scale` is in 1/10 parts of the current bar spacing; negative zooms out.
    pub fn zoom(&mut self, zoom_point: Coordinate, scale: f64) {
        let float_index_at_zoom_point = self.coordinate_to_float_index(zoom_point);

        let bar_spacing = self.bar_spacing;
        let new_bar_spacing = bar_spacing + scale * (bar_spacing / 10.0);

        self.set_bar_spacing(new_bar_spacing);

        if !self.options.right_bar_stays_on_scroll {
            // move the index under zoom_point back to its coordinate
            let new_offset =
                self.right_offset + (float_index_at_zoom_point - self.coordinate_to_float_index(zoom_point));
            self.set_right_offset(new_offset);
        }
    }

    // --- axis-drag scale ---

    pub fn start_scale(&mut self, x: Coordinate) {
        if self.scroll_start_point.is_some() {
            self.end_scroll();
        }
        if self.scale_start_point.is_some() || self.common_transition_start_state.is_some() {
            return;
        }
        if self.is_empty() {
            return;
        }
        self.scale_start_point = Some(x);
        self.save_common_transitions_start_state();
    }

    pub fn scale_to(&mut self, x: Coordinate) {
        let Some(start_state) = self.common_transition_start_state else { return };

        let start_length_from_right = clamp(self.width - x, 0.0, self.width);
        let current_length_from_right =
            clamp(self.width - self.scale_start_point.expect("scale started"), 0.0, self.width);
        if start_length_from_right == 0.0 || current_length_from_right == 0.0 {
            return;
        }

        self.set_bar_spacing(start_state.bar_spacing * start_length_from_right / current_length_from_right);
    }

    pub fn end_scale(&mut self) {
        if self.scale_start_point.is_none() {
            return;
        }
        self.scale_start_point = None;
        self.clear_common_transitions_start_state();
    }

    // --- drag scroll ---

    pub fn start_scroll(&mut self, x: Coordinate) {
        if self.scroll_start_point.is_some() || self.common_transition_start_state.is_some() {
            return;
        }
        if self.is_empty() {
            return;
        }
        self.scroll_start_point = Some(x);
        self.save_common_transitions_start_state();
    }

    pub fn scroll_to(&mut self, x: Coordinate) {
        let Some(scroll_start_point) = self.scroll_start_point else { return };

        let shift_in_logical = (scroll_start_point - x) / self.bar_spacing;
        self.right_offset = self
            .common_transition_start_state
            .expect("scroll started")
            .right_offset
            + shift_in_logical;

        self.correct_offset();
    }

    pub fn end_scroll(&mut self) {
        if self.scroll_start_point.is_none() {
            return;
        }
        self.scroll_start_point = None;
        self.clear_common_transitions_start_state();
    }

    // --- range setting ---

    /// Port of `setVisibleRange` (without the invalidation side effects).
    pub fn set_visible_range(&mut self, range: StrictRange, apply_default_offset: bool) {
        let length = range.count() as f64;
        let pixel_offset = if apply_default_offset {
            self.options.right_offset_pixels.unwrap_or(0.0)
        } else {
            0.0
        };
        self.set_bar_spacing_internal((self.width - pixel_offset) / length);
        self.right_offset = range.right() as f64 - self.base_index() as f64;
        if apply_default_offset {
            self.right_offset = if pixel_offset != 0.0 {
                pixel_offset / self.bar_spacing
            } else {
                self.options.right_offset
            };
        }
        self.correct_offset();
    }

    pub fn fit_content(&mut self) {
        let (Some(first), Some(last)) = (self.first_index(), self.last_index()) else {
            return;
        };

        // include the user-defined right offset in the range so scaling reserves space for it
        let right_offset_bars = if self.options.right_offset_pixels.is_none() {
            self.options.right_offset
        } else {
            0.0
        };
        self.set_visible_range(
            StrictRange::new(first, last + right_offset_bars as i64),
            true,
        );
    }

    pub fn set_logical_range(&mut self, range: LogicalRange) {
        self.set_visible_range(
            StrictRange::new(range.left().round() as i64, range.right().round() as i64),
            false,
        );
    }

    pub fn restore_default(&mut self) {
        self.set_bar_spacing(self.options.bar_spacing);
        let new_offset = match self.options.right_offset_pixels {
            Some(px) => px / self.bar_spacing,
            None => self.options.right_offset,
        };
        self.set_right_offset(new_offset);
    }

    fn save_common_transitions_start_state(&mut self) {
        self.common_transition_start_state = Some(TransitionState {
            bar_spacing: self.bar_spacing,
            right_offset: self.right_offset,
        });
    }

    fn clear_common_transitions_start_state(&mut self) {
        self.common_transition_start_state = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(width: f64, bar_spacing: f64, right_offset: f64, points: usize, base: i64) -> TimeScaleCore {
        let mut s = TimeScaleCore::new(TimeScaleOptions {
            bar_spacing,
            right_offset,
            ..Default::default()
        });
        s.set_width(width);
        s.points_len = points;
        s.base_index = Some(base);
        s
    }

    #[test]
    fn index_to_coordinate_formula() {
        // x = width - (base + rightOffset - index + 0.5) * barSpacing - 1
        let s = scale(100.0, 6.0, 0.0, 20, 10);
        assert_eq!(s.index_to_coordinate(10), 100.0 - 0.5 * 6.0 - 1.0); // 96
        assert_eq!(s.index_to_coordinate(9), 100.0 - 1.5 * 6.0 - 1.0); // 90
    }

    #[test]
    fn coordinate_roundtrip() {
        // coordinate_to_float_index(index_to_coordinate(i)) == i - 0.5 by construction
        // (float index measures the logical cell boundary; ceil snaps back to the bar index)
        let s = scale(100.0, 6.0, 2.5, 50, 30);
        for index in [0i64, 5, 29, 30] {
            let x = s.index_to_coordinate(index);
            let f = s.coordinate_to_float_index(x);
            assert!((f - (index as f64 - 0.5)).abs() < 1e-6, "index {index} -> x {x} -> {f}");
            assert_eq!(s.coordinate_to_index(x), index);
        }
    }

    #[test]
    fn coordinate_to_index_cell_boundaries() {
        let s = scale(100.0, 6.0, 0.0, 20, 10);
        let x_of_9 = s.index_to_coordinate(9);
        // bar 9 owns roughly (center - spacing/2, center + spacing/2]
        assert_eq!(s.coordinate_to_index(x_of_9), 9);
        assert_eq!(s.coordinate_to_index(x_of_9 + 3.0), 9); // exactly on the boundary
        assert_eq!(s.coordinate_to_index(x_of_9 + 3.1), 10); // past it
    }

    #[test]
    fn visible_logical_range_formula() {
        let s = scale(120.0, 6.0, 0.0, 100, 50);
        let r = s.visible_logical_range().unwrap();
        // rightBorder = rightOffset + baseIndex = 50; barsLength = 120/6 = 20
        assert_eq!(r.right(), 50.0);
        assert_eq!(r.left(), 50.0 - 20.0 + 1.0);
        let strict = r.to_strict();
        assert_eq!(strict.left(), 31);
        assert_eq!(strict.right(), 50);
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let mut s = scale(400.0, 6.0, 0.0, 500, 300);
        let cursor = 250.0;
        let before = s.coordinate_to_float_index(cursor);
        s.zoom(cursor, 1.0); // zoom in 10%
        assert!((s.bar_spacing() - 6.6).abs() < 1e-12);
        let after = s.coordinate_to_float_index(cursor);
        assert!((before - after).abs() < 1e-6, "point drifted: {before} -> {after}");
    }

    #[test]
    fn zoom_out_clamps_to_min_bar_spacing() {
        let mut s = scale(400.0, 0.55, 0.0, 500, 300);
        s.zoom(200.0, -5.0); // massive zoom out
        assert!(s.bar_spacing() >= 0.5);
    }

    #[test]
    fn scroll_moves_by_bars() {
        let mut s = scale(400.0, 8.0, 10.0, 500, 300);
        s.start_scroll(200.0);
        s.scroll_to(160.0); // dragged 40px left -> content moves left -> offset += 40/8
        assert!((s.right_offset() - 15.0).abs() < 1e-12);
        s.end_scroll();
    }

    #[test]
    fn scroll_clamped_to_future_limit() {
        let mut s = scale(400.0, 8.0, 0.0, 500, 300);
        s.start_scroll(400.0);
        s.scroll_to(0.0); // drag 400px left -> +50 bars, but max = width/spacing - 2 = 48
        assert!((s.right_offset() - 48.0).abs() < 1e-12);
    }

    #[test]
    fn scroll_clamped_to_past_limit() {
        let mut s = scale(400.0, 8.0, 0.0, 100, 99);
        s.start_scroll(0.0);
        s.scroll_to(4000.0); // drag far right -> past; min = 0 - 99 - 1 + 2 = -98
        assert!((s.right_offset() - -98.0).abs() < 1e-12);
    }

    #[test]
    fn axis_drag_scale_formula() {
        // new = start_spacing * (width - x) / (width - start_x)
        let mut s = scale(400.0, 6.0, 0.0, 500, 300);
        s.start_scale(300.0); // 100px from right
        s.scale_to(350.0); // 50px from right -> 6 * 50/100 = 3 (zoom out)
        assert!((s.bar_spacing() - 3.0).abs() < 1e-12);
        s.end_scale();

        // dragging away from the right edge zooms in
        let mut s2 = scale(400.0, 6.0, 0.0, 500, 300);
        s2.start_scale(300.0);
        s2.scale_to(200.0); // 200px from right -> 6 * 200/100 = 12
        assert!((s2.bar_spacing() - 12.0).abs() < 1e-12);
    }

    #[test]
    fn fit_content_shows_all_bars() {
        let mut s = scale(500.0, 6.0, 0.0, 100, 99);
        s.fit_content();
        let r = s.visible_strict_range().unwrap();
        assert!(r.left() <= 0);
        assert!(r.right() >= 99);
    }

    #[test]
    fn max_bar_spacing_defaults_to_half_width() {
        let mut s = scale(400.0, 6.0, 0.0, 500, 300);
        s.set_bar_spacing(10_000.0);
        assert_eq!(s.bar_spacing(), 200.0);
    }
}
