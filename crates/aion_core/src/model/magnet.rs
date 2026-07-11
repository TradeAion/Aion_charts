//! Crosshair magnet. Port of `src/model/magnet.ts` (RENDERING_SPEC.md §8).
//!
//! In Magnet mode the crosshair's horizontal line snaps to the close price of the bar under
//! the cursor; in MagnetOHLC it snaps to whichever of open/high/low/close is nearest the
//! cursor in *pixel* space. The comparison is done on coordinates (not prices) so it behaves
//! correctly on log scales.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrosshairMode {
    /// Crosshair moves freely with the cursor.
    Normal,
    /// Horizontal line sticks to the close price of the hovered bar.
    Magnet,
    /// Hidden (rendering suppressed).
    Hidden,
    /// Horizontal line sticks to the nearest of O/H/L/C of the hovered bar.
    MagnetOhlc,
}

/// Returns the candidate whose coordinate is nearest `cursor_coord`. Candidates are
/// `(price, coordinate)` pairs (already converted for the current price-scale mode).
/// Returns `None` when there are no candidates (caller keeps the raw cursor price).
pub fn magnet_snap(cursor_coord: f64, candidates: &[(f64, f64)]) -> Option<f64> {
    candidates
        .iter()
        .min_by(|a, b| {
            (a.1 - cursor_coord)
                .abs()
                .partial_cmp(&(b.1 - cursor_coord).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|&(price, _)| price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_to_nearest_coordinate() {
        // cursor at y=100; candidates: close@105, high@40, low@160, open@98
        let candidates = [(10.0, 105.0), (20.0, 40.0), (5.0, 160.0), (12.0, 98.0)];
        // nearest coord to 100 is 98 (open, price 12)
        assert_eq!(magnet_snap(100.0, &candidates), Some(12.0));
    }

    #[test]
    fn single_candidate_close() {
        // Magnet mode passes only the close: always snaps to it
        assert_eq!(magnet_snap(50.0, &[(123.0, 200.0)]), Some(123.0));
    }

    #[test]
    fn empty_candidates() {
        assert_eq!(magnet_snap(50.0, &[]), None);
    }
}
