//! Baseline helpers shared by main + overlay generators.
//!
//! Centralizes baseline-crossing split logic so both render paths produce
//! consistent two-tone line segments.

/// Emit one or two colored segments for a line crossing a baseline.
///
/// - If segment does not cross the baseline, emits one segment.
/// - If it crosses, emits two segments split at the intersection point.
/// - Points exactly on baseline are treated as the "top" side.
pub fn emit_split_segment_by_baseline<F>(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    baseline_y: f64,
    top_color: [f32; 4],
    bottom_color: [f32; 4],
    mut emit: F,
) where
    F: FnMut(f64, f64, f64, f64, [f32; 4]),
{
    const EPS: f64 = 1e-6;

    let color_for = |y: f64| {
        if y <= baseline_y {
            top_color
        } else {
            bottom_color
        }
    };

    let d1 = y1 - baseline_y;
    let d2 = y2 - baseline_y;

    // Strict crossing between endpoints.
    if d1.abs() > EPS && d2.abs() > EPS && d1.signum() != d2.signum() {
        let t = (baseline_y - y1) / (y2 - y1);
        if t > 0.0 && t < 1.0 {
            let xc = x1 + (x2 - x1) * t;

            emit(x1, y1, xc, baseline_y, color_for(y1));
            emit(xc, baseline_y, x2, y2, color_for(y2));
            return;
        }
    }

    emit(x1, y1, x2, y2, color_for((y1 + y2) * 0.5));
}

#[cfg(test)]
mod tests {
    use super::emit_split_segment_by_baseline;

    #[test]
    fn non_crossing_segment_emits_one_piece() {
        let mut out = Vec::new();
        emit_split_segment_by_baseline(
            0.0,
            10.0,
            10.0,
            20.0,
            50.0,
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            |x1, y1, x2, y2, c| out.push((x1, y1, x2, y2, c)),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].4, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn crossing_segment_emits_two_pieces() {
        let mut out = Vec::new();
        emit_split_segment_by_baseline(
            0.0,
            10.0,
            10.0,
            30.0,
            20.0,
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            |x1, y1, x2, y2, c| out.push((x1, y1, x2, y2, c)),
        );
        assert_eq!(out.len(), 2);
        assert!((out[0].3 - 20.0).abs() < 1e-6);
        assert!((out[1].1 - 20.0).abs() < 1e-6);
        assert_eq!(out[0].4, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(out[1].4, [0.0, 1.0, 0.0, 1.0]);
    }
}
