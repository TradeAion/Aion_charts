//! Line namespace builtin functions for AionDSL.
//!
//! Provides Pine Script-compatible line drawing operations including:
//! - Creation: `line.new` (creates a line between two points)
//! - Property access: `line.get_x1`, `line.get_y1`, `line.get_x2`, `line.get_y2`
//! - Property modification: `line.set_x1`, `line.set_y1`, `line.set_x2`, `line.set_y2`
//! - Utility: `line.get_price`, `line.copy`, `line.delete`
//!
//! Note: Line objects are implemented as 2-point polylines internally.
//! The actual drawing object creation is handled by the compiler and VM.
//! This module provides helper functions for line calculations and property access.

use crate::core::indicators::runtime::value::RayValue;

/// Line namespace builtin function dispatch.
/// Note: `line.new`, `line.set`, and `line.delete` are handled by the compiler
/// as they create/modify drawing objects. This module handles computed properties.
pub fn call(name: &str, args: &[RayValue]) -> Option<RayValue> {
    match name {
        // Coordinate extraction (for when line is stored as array/tuple)
        "get_x1" => line_get_x1(args),
        "get_y1" => line_get_y1(args),
        "get_x2" => line_get_x2(args),
        "get_y2" => line_get_y2(args),

        // Computed properties
        "get_price" => line_get_price(args),
        "get_slope" => line_get_slope(args),
        "get_length" => line_get_length(args),
        "get_midpoint_x" => line_get_midpoint_x(args),
        "get_midpoint_y" => line_get_midpoint_y(args),

        // Line-point relationships
        "is_point_on_line" => line_is_point_on_line(args),
        "get_y_at_x" => line_get_y_at_x(args),
        "get_x_at_y" => line_get_x_at_y(args),

        // Line-line relationships
        "get_intersection" => line_get_intersection(args),
        "is_parallel" => line_is_parallel(args),
        "is_perpendicular" => line_is_perpendicular(args),

        // Utility - create line representation as tuple/array
        "from_coords" => line_from_coords(args),

        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Coordinate Extraction
// ═══════════════════════════════════════════════════════════════════════════════

/// Helper to extract line coordinates from a tuple [x1, y1, x2, y2]
fn extract_line_coords(args: &[RayValue]) -> Option<(f64, f64, f64, f64)> {
    match args.first() {
        Some(RayValue::Tuple(coords)) | Some(RayValue::Array(coords)) if coords.len() >= 4 => {
            let x1 = coords.first().and_then(RayValue::as_number)?;
            let y1 = coords.get(1).and_then(RayValue::as_number)?;
            let x2 = coords.get(2).and_then(RayValue::as_number)?;
            let y2 = coords.get(3).and_then(RayValue::as_number)?;
            Some((x1, y1, x2, y2))
        }
        _ => None,
    }
}

/// line.get_x1(line) - Get x1 coordinate
fn line_get_x1(args: &[RayValue]) -> Option<RayValue> {
    let (x1, _, _, _) = extract_line_coords(args)?;
    Some(RayValue::Number(x1))
}

/// line.get_y1(line) - Get y1 coordinate
fn line_get_y1(args: &[RayValue]) -> Option<RayValue> {
    let (_, y1, _, _) = extract_line_coords(args)?;
    Some(RayValue::Number(y1))
}

/// line.get_x2(line) - Get x2 coordinate
fn line_get_x2(args: &[RayValue]) -> Option<RayValue> {
    let (_, _, x2, _) = extract_line_coords(args)?;
    Some(RayValue::Number(x2))
}

/// line.get_y2(line) - Get y2 coordinate
fn line_get_y2(args: &[RayValue]) -> Option<RayValue> {
    let (_, _, _, y2) = extract_line_coords(args)?;
    Some(RayValue::Number(y2))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Computed Properties
// ═══════════════════════════════════════════════════════════════════════════════

/// line.get_price(line, bar_index) - Get y value at given x (bar index)
/// Interpolates/extrapolates the line to find the price at the given bar
fn line_get_price(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;
    let bar_index = args.get(1).and_then(RayValue::as_number)?;

    // Handle vertical line
    if (x2 - x1).abs() < f64::EPSILON {
        return Some(RayValue::Na);
    }

    // Linear interpolation: y = y1 + (y2 - y1) * (x - x1) / (x2 - x1)
    let slope = (y2 - y1) / (x2 - x1);
    let price = y1 + slope * (bar_index - x1);

    Some(RayValue::Number(price))
}

/// line.get_slope(line) - Get slope of line (change in y per unit x)
fn line_get_slope(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;

    // Handle vertical line
    if (x2 - x1).abs() < f64::EPSILON {
        return Some(RayValue::Na);
    }

    let slope = (y2 - y1) / (x2 - x1);
    Some(RayValue::Number(slope))
}

/// line.get_length(line) - Get Euclidean length of line segment
fn line_get_length(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;

    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = (dx * dx + dy * dy).sqrt();

    Some(RayValue::Number(length))
}

/// line.get_midpoint_x(line) - Get x coordinate of line midpoint
fn line_get_midpoint_x(args: &[RayValue]) -> Option<RayValue> {
    let (x1, _, x2, _) = extract_line_coords(args)?;
    Some(RayValue::Number((x1 + x2) / 2.0))
}

/// line.get_midpoint_y(line) - Get y coordinate of line midpoint
fn line_get_midpoint_y(args: &[RayValue]) -> Option<RayValue> {
    let (_, y1, _, y2) = extract_line_coords(args)?;
    Some(RayValue::Number((y1 + y2) / 2.0))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Line-Point Relationships
// ═══════════════════════════════════════════════════════════════════════════════

/// line.is_point_on_line(line, x, y, tolerance) - Check if point is on line
fn line_is_point_on_line(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;
    let px = args.get(1).and_then(RayValue::as_number)?;
    let py = args.get(2).and_then(RayValue::as_number)?;
    let tolerance = args.get(3).and_then(RayValue::as_number).unwrap_or(1e-9);

    // Use cross product to check collinearity
    // (py - y1) * (x2 - x1) - (px - x1) * (y2 - y1) should be ~0
    let cross = (py - y1) * (x2 - x1) - (px - x1) * (y2 - y1);

    // Normalize by line length for consistent tolerance
    let length = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    let distance = if length > f64::EPSILON {
        cross.abs() / length
    } else {
        // Degenerate line (point)
        ((px - x1).powi(2) + (py - y1).powi(2)).sqrt()
    };

    Some(RayValue::Bool(distance <= tolerance))
}

/// line.get_y_at_x(line, x) - Get y coordinate at given x (interpolate/extrapolate)
fn line_get_y_at_x(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;
    let x = args.get(1).and_then(RayValue::as_number)?;

    if (x2 - x1).abs() < f64::EPSILON {
        return Some(RayValue::Na);
    }

    let slope = (y2 - y1) / (x2 - x1);
    let y = y1 + slope * (x - x1);

    Some(RayValue::Number(y))
}

/// line.get_x_at_y(line, y) - Get x coordinate at given y (interpolate/extrapolate)
fn line_get_x_at_y(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;
    let y = args.get(1).and_then(RayValue::as_number)?;

    if (y2 - y1).abs() < f64::EPSILON {
        return Some(RayValue::Na);
    }

    let inv_slope = (x2 - x1) / (y2 - y1);
    let x = x1 + inv_slope * (y - y1);

    Some(RayValue::Number(x))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Line-Line Relationships
// ═══════════════════════════════════════════════════════════════════════════════

/// line.get_intersection(line1, line2) - Get intersection point as [x, y] or na
fn line_get_intersection(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;

    // Extract second line from args[1]
    let (x3, y3, x4, y4) = match args.get(1) {
        Some(RayValue::Tuple(coords)) | Some(RayValue::Array(coords)) if coords.len() >= 4 => {
            let a = coords.first().and_then(RayValue::as_number)?;
            let b = coords.get(1).and_then(RayValue::as_number)?;
            let c = coords.get(2).and_then(RayValue::as_number)?;
            let d = coords.get(3).and_then(RayValue::as_number)?;
            (a, b, c, d)
        }
        _ => return Some(RayValue::Na),
    };

    // Line 1: from (x1, y1) to (x2, y2)
    // Line 2: from (x3, y3) to (x4, y4)
    // Using parametric line intersection formula

    let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);

    if denom.abs() < f64::EPSILON {
        // Lines are parallel
        return Some(RayValue::Na);
    }

    let t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / denom;

    let ix = x1 + t * (x2 - x1);
    let iy = y1 + t * (y2 - y1);

    Some(RayValue::Tuple(vec![
        RayValue::Number(ix),
        RayValue::Number(iy),
    ]))
}

/// line.is_parallel(line1, line2) - Check if two lines are parallel
fn line_is_parallel(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;

    let (x3, y3, x4, y4) = match args.get(1) {
        Some(RayValue::Tuple(coords)) | Some(RayValue::Array(coords)) if coords.len() >= 4 => {
            let a = coords.first().and_then(RayValue::as_number)?;
            let b = coords.get(1).and_then(RayValue::as_number)?;
            let c = coords.get(2).and_then(RayValue::as_number)?;
            let d = coords.get(3).and_then(RayValue::as_number)?;
            (a, b, c, d)
        }
        _ => return Some(RayValue::Bool(false)),
    };

    // Check if cross product of direction vectors is ~0
    let dx1 = x2 - x1;
    let dy1 = y2 - y1;
    let dx2 = x4 - x3;
    let dy2 = y4 - y3;

    let cross = dx1 * dy2 - dy1 * dx2;

    Some(RayValue::Bool(cross.abs() < f64::EPSILON))
}

/// line.is_perpendicular(line1, line2) - Check if two lines are perpendicular
fn line_is_perpendicular(args: &[RayValue]) -> Option<RayValue> {
    let (x1, y1, x2, y2) = extract_line_coords(args)?;

    let (x3, y3, x4, y4) = match args.get(1) {
        Some(RayValue::Tuple(coords)) | Some(RayValue::Array(coords)) if coords.len() >= 4 => {
            let a = coords.first().and_then(RayValue::as_number)?;
            let b = coords.get(1).and_then(RayValue::as_number)?;
            let c = coords.get(2).and_then(RayValue::as_number)?;
            let d = coords.get(3).and_then(RayValue::as_number)?;
            (a, b, c, d)
        }
        _ => return Some(RayValue::Bool(false)),
    };

    // Check if dot product of direction vectors is ~0
    let dx1 = x2 - x1;
    let dy1 = y2 - y1;
    let dx2 = x4 - x3;
    let dy2 = y4 - y3;

    let dot = dx1 * dx2 + dy1 * dy2;

    // Normalize by product of magnitudes for scale-invariant check
    let mag1 = (dx1 * dx1 + dy1 * dy1).sqrt();
    let mag2 = (dx2 * dx2 + dy2 * dy2).sqrt();

    if mag1 < f64::EPSILON || mag2 < f64::EPSILON {
        return Some(RayValue::Bool(false));
    }

    let cos_angle = dot / (mag1 * mag2);
    Some(RayValue::Bool(cos_angle.abs() < 1e-9))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Utility Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// line.from_coords(x1, y1, x2, y2) - Create line representation as tuple
fn line_from_coords(args: &[RayValue]) -> Option<RayValue> {
    let x1 = args.first().and_then(RayValue::as_number).unwrap_or(0.0);
    let y1 = args.get(1).and_then(RayValue::as_number).unwrap_or(0.0);
    let x2 = args.get(2).and_then(RayValue::as_number).unwrap_or(0.0);
    let y2 = args.get(3).and_then(RayValue::as_number).unwrap_or(0.0);

    Some(RayValue::Tuple(vec![
        RayValue::Number(x1),
        RayValue::Number(y1),
        RayValue::Number(x2),
        RayValue::Number(y2),
    ]))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(x1: f64, y1: f64, x2: f64, y2: f64) -> RayValue {
        RayValue::Tuple(vec![
            RayValue::Number(x1),
            RayValue::Number(y1),
            RayValue::Number(x2),
            RayValue::Number(y2),
        ])
    }

    #[test]
    fn line_get_coordinates() {
        let line = make_line(10.0, 100.0, 20.0, 200.0);

        assert_eq!(
            call("get_x1", &[line.clone()]),
            Some(RayValue::Number(10.0))
        );
        assert_eq!(
            call("get_y1", &[line.clone()]),
            Some(RayValue::Number(100.0))
        );
        assert_eq!(
            call("get_x2", &[line.clone()]),
            Some(RayValue::Number(20.0))
        );
        assert_eq!(call("get_y2", &[line]), Some(RayValue::Number(200.0)));
    }

    #[test]
    fn line_get_slope_computes_correctly() {
        // Line from (0, 0) to (10, 20) has slope 2.0
        let line = make_line(0.0, 0.0, 10.0, 20.0);
        assert_eq!(call("get_slope", &[line]), Some(RayValue::Number(2.0)));
    }

    #[test]
    fn line_get_slope_vertical_returns_na() {
        // Vertical line has undefined slope
        let line = make_line(5.0, 0.0, 5.0, 100.0);
        assert_eq!(call("get_slope", &[line]), Some(RayValue::Na));
    }

    #[test]
    fn line_get_length_computes_correctly() {
        // Line from (0, 0) to (3, 4) has length 5 (3-4-5 triangle)
        let line = make_line(0.0, 0.0, 3.0, 4.0);
        assert_eq!(call("get_length", &[line]), Some(RayValue::Number(5.0)));
    }

    #[test]
    fn line_get_price_interpolates() {
        // Line from (0, 100) to (10, 200), price at x=5 should be 150
        let line = make_line(0.0, 100.0, 10.0, 200.0);
        let result = call("get_price", &[line, RayValue::Number(5.0)]);
        assert_eq!(result, Some(RayValue::Number(150.0)));
    }

    #[test]
    fn line_get_price_extrapolates() {
        // Line from (0, 100) to (10, 200), price at x=20 should be 300
        let line = make_line(0.0, 100.0, 10.0, 200.0);
        let result = call("get_price", &[line, RayValue::Number(20.0)]);
        assert_eq!(result, Some(RayValue::Number(300.0)));
    }

    #[test]
    fn line_get_midpoint() {
        let line = make_line(0.0, 0.0, 10.0, 20.0);
        assert_eq!(
            call("get_midpoint_x", &[line.clone()]),
            Some(RayValue::Number(5.0))
        );
        assert_eq!(
            call("get_midpoint_y", &[line]),
            Some(RayValue::Number(10.0))
        );
    }

    #[test]
    fn line_get_y_at_x() {
        let line = make_line(0.0, 0.0, 10.0, 100.0);
        let result = call("get_y_at_x", &[line, RayValue::Number(5.0)]);
        assert_eq!(result, Some(RayValue::Number(50.0)));
    }

    #[test]
    fn line_get_x_at_y() {
        let line = make_line(0.0, 0.0, 100.0, 10.0);
        let result = call("get_x_at_y", &[line, RayValue::Number(5.0)]);
        assert_eq!(result, Some(RayValue::Number(50.0)));
    }

    #[test]
    fn line_is_point_on_line_true() {
        let line = make_line(0.0, 0.0, 10.0, 10.0);
        // Point (5, 5) is on the line y = x
        let result = call(
            "is_point_on_line",
            &[line, RayValue::Number(5.0), RayValue::Number(5.0)],
        );
        assert_eq!(result, Some(RayValue::Bool(true)));
    }

    #[test]
    fn line_is_point_on_line_false() {
        let line = make_line(0.0, 0.0, 10.0, 10.0);
        // Point (5, 6) is NOT on the line y = x
        let result = call(
            "is_point_on_line",
            &[line, RayValue::Number(5.0), RayValue::Number(6.0)],
        );
        assert_eq!(result, Some(RayValue::Bool(false)));
    }

    #[test]
    fn line_get_intersection_finds_point() {
        // Line 1: from (0, 0) to (10, 10) - diagonal
        // Line 2: from (0, 10) to (10, 0) - other diagonal
        // They intersect at (5, 5)
        let line1 = make_line(0.0, 0.0, 10.0, 10.0);
        let line2 = make_line(0.0, 10.0, 10.0, 0.0);
        let result = call("get_intersection", &[line1, line2]);

        if let Some(RayValue::Tuple(coords)) = result {
            assert_eq!(coords.len(), 2);
            if let (RayValue::Number(x), RayValue::Number(y)) = (&coords[0], &coords[1]) {
                assert!((x - 5.0).abs() < 1e-9);
                assert!((y - 5.0).abs() < 1e-9);
            } else {
                panic!("expected numbers");
            }
        } else {
            panic!("expected tuple");
        }
    }

    #[test]
    fn line_get_intersection_parallel_returns_na() {
        // Two parallel lines
        let line1 = make_line(0.0, 0.0, 10.0, 10.0);
        let line2 = make_line(0.0, 1.0, 10.0, 11.0);
        let result = call("get_intersection", &[line1, line2]);
        assert_eq!(result, Some(RayValue::Na));
    }

    #[test]
    fn line_is_parallel_true() {
        let line1 = make_line(0.0, 0.0, 10.0, 10.0);
        let line2 = make_line(5.0, 0.0, 15.0, 10.0);
        let result = call("is_parallel", &[line1, line2]);
        assert_eq!(result, Some(RayValue::Bool(true)));
    }

    #[test]
    fn line_is_parallel_false() {
        let line1 = make_line(0.0, 0.0, 10.0, 10.0);
        let line2 = make_line(0.0, 0.0, 10.0, 0.0);
        let result = call("is_parallel", &[line1, line2]);
        assert_eq!(result, Some(RayValue::Bool(false)));
    }

    #[test]
    fn line_is_perpendicular_true() {
        // Horizontal and vertical lines
        let line1 = make_line(0.0, 0.0, 10.0, 0.0); // horizontal
        let line2 = make_line(0.0, 0.0, 0.0, 10.0); // vertical
        let result = call("is_perpendicular", &[line1, line2]);
        assert_eq!(result, Some(RayValue::Bool(true)));
    }

    #[test]
    fn line_is_perpendicular_false() {
        let line1 = make_line(0.0, 0.0, 10.0, 10.0);
        let line2 = make_line(0.0, 0.0, 10.0, 5.0);
        let result = call("is_perpendicular", &[line1, line2]);
        assert_eq!(result, Some(RayValue::Bool(false)));
    }

    #[test]
    fn line_from_coords_creates_tuple() {
        let result = call(
            "from_coords",
            &[
                RayValue::Number(1.0),
                RayValue::Number(2.0),
                RayValue::Number(3.0),
                RayValue::Number(4.0),
            ],
        );
        if let Some(RayValue::Tuple(coords)) = result {
            assert_eq!(coords.len(), 4);
            assert_eq!(coords[0], RayValue::Number(1.0));
            assert_eq!(coords[1], RayValue::Number(2.0));
            assert_eq!(coords[2], RayValue::Number(3.0));
            assert_eq!(coords[3], RayValue::Number(4.0));
        } else {
            panic!("expected tuple");
        }
    }
}
