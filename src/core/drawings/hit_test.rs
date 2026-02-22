//! Hit-test math for drawing tools.
//!
//! All functions operate in CSS pixel coordinates.

/// Distance from point (px, py) to line segment (x0,y0)→(x1,y1) in CSS px.
pub fn point_to_segment_distance(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-12 {
        // Degenerate segment — just distance to point
        return ((px - x0).powi(2) + (py - y0).powi(2)).sqrt();
    }

    // Project point onto line, clamp to [0, 1]
    let t = ((px - x0) * dx + (py - y0) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = x0 + t * dx;
    let proj_y = y0 + t * dy;

    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

/// Check if point (px, py) is inside axis-aligned rectangle defined by two corners.
/// Corners can be in any order (will be min/max'd).
pub fn point_in_rect(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);
    px >= min_x && px <= max_x && py >= min_y && py <= max_y
}

/// Distance from point to the nearest edge of an axis-aligned rectangle.
/// Returns 0.0 if the point is inside the rect.
pub fn point_to_rect_edge_distance(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);

    // Distance to each edge segment
    let top = point_to_segment_distance(px, py, min_x, min_y, max_x, min_y);
    let bottom = point_to_segment_distance(px, py, min_x, max_y, max_x, max_y);
    let left = point_to_segment_distance(px, py, min_x, min_y, min_x, max_y);
    let right = point_to_segment_distance(px, py, max_x, min_y, max_x, max_y);

    top.min(bottom).min(left).min(right)
}

/// Distance from point to a circle center.
pub fn point_to_circle_distance(px: f64, py: f64, cx: f64, cy: f64) -> f64 {
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Hit-test threshold in CSS pixels (how close cursor must be to "hit" a line/edge).
pub const HIT_THRESHOLD_CSS: f64 = 5.0;

/// Hit-test threshold for anchor points (slightly larger than the anchor radius).
pub const ANCHOR_HIT_THRESHOLD_CSS: f64 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_to_segment() {
        // Point on the segment
        let d = point_to_segment_distance(5.0, 0.0, 0.0, 0.0, 10.0, 0.0);
        assert!((d - 0.0).abs() < 1e-10);

        // Point perpendicular to midpoint
        let d = point_to_segment_distance(5.0, 3.0, 0.0, 0.0, 10.0, 0.0);
        assert!((d - 3.0).abs() < 1e-10);

        // Point past the end
        let d = point_to_segment_distance(12.0, 0.0, 0.0, 0.0, 10.0, 0.0);
        assert!((d - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_in_rect() {
        assert!(point_in_rect(5.0, 5.0, 0.0, 0.0, 10.0, 10.0));
        assert!(!point_in_rect(15.0, 5.0, 0.0, 0.0, 10.0, 10.0));
        // Inverted corners
        assert!(point_in_rect(5.0, 5.0, 10.0, 10.0, 0.0, 0.0));
    }
}
