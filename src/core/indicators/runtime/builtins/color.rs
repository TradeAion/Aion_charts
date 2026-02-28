use crate::core::indicators::runtime::value::{RayColor, RayValue};

/// Dispatch color.* function calls.
pub fn call(fn_name: &str, args: &[RayValue]) -> Option<RayValue> {
    match fn_name {
        "new" => Some(color_new(args)),
        "rgb" => Some(color_rgb(args)),
        "r" => Some(color_r(args)),
        "g" => Some(color_g(args)),
        "b" => Some(color_b(args)),
        "t" => Some(color_t(args)),
        "a" => Some(color_a(args)),
        _ => None,
    }
}

/// color.new(color, transp) - Create a new color with transparency
/// transp: 0 = opaque, 100 = fully transparent
fn color_new(args: &[RayValue]) -> RayValue {
    let Some(base_color) = args.first().and_then(|v| v.as_color()) else {
        return RayValue::Na;
    };
    let transparency = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.clamp(0.0, 100.0) as u8)
        .unwrap_or(0);

    RayValue::Color(base_color.with_transparency(transparency))
}

/// color.rgb(r, g, b) or color.rgb(r, g, b, transp) - Create color from RGB components
fn color_rgb(args: &[RayValue]) -> RayValue {
    let r = args
        .first()
        .and_then(|v| v.as_number())
        .map(|n| n.clamp(0.0, 255.0) as u8)
        .unwrap_or(0);
    let g = args
        .get(1)
        .and_then(|v| v.as_number())
        .map(|n| n.clamp(0.0, 255.0) as u8)
        .unwrap_or(0);
    let b = args
        .get(2)
        .and_then(|v| v.as_number())
        .map(|n| n.clamp(0.0, 255.0) as u8)
        .unwrap_or(0);
    let transp = args
        .get(3)
        .and_then(|v| v.as_number())
        .map(|n| n.clamp(0.0, 100.0) as u8)
        .unwrap_or(0);

    let color = RayColor::rgb(r, g, b);
    RayValue::Color(color.with_transparency(transp))
}

/// color.r(color) - Get red component (0-255)
fn color_r(args: &[RayValue]) -> RayValue {
    args.first()
        .and_then(|v| v.as_color())
        .map(|c| RayValue::Number(c.r as f64))
        .unwrap_or(RayValue::Na)
}

/// color.g(color) - Get green component (0-255)
fn color_g(args: &[RayValue]) -> RayValue {
    args.first()
        .and_then(|v| v.as_color())
        .map(|c| RayValue::Number(c.g as f64))
        .unwrap_or(RayValue::Na)
}

/// color.b(color) - Get blue component (0-255)
fn color_b(args: &[RayValue]) -> RayValue {
    args.first()
        .and_then(|v| v.as_color())
        .map(|c| RayValue::Number(c.b as f64))
        .unwrap_or(RayValue::Na)
}

/// color.t(color) - Get transparency (0-100)
fn color_t(args: &[RayValue]) -> RayValue {
    args.first()
        .and_then(|v| v.as_color())
        .map(|c| {
            // Convert alpha (0-255) to transparency (0-100)
            // alpha 255 = transp 0, alpha 0 = transp 100
            let transp = 100.0 - (c.a as f64 * 100.0 / 255.0);
            RayValue::Number(transp.round())
        })
        .unwrap_or(RayValue::Na)
}

/// color.a(color) - Get alpha component (0-255) - extension to Pine
fn color_a(args: &[RayValue]) -> RayValue {
    args.first()
        .and_then(|v| v.as_color())
        .map(|c| RayValue::Number(c.a as f64))
        .unwrap_or(RayValue::Na)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_rgb_creates_opaque_color() {
        let result = color_rgb(&[
            RayValue::Number(255.0),
            RayValue::Number(128.0),
            RayValue::Number(0.0),
        ]);
        let color = result.as_color().unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 0);
        assert_eq!(color.a, 255); // opaque
    }

    #[test]
    fn color_rgb_with_transparency() {
        let result = color_rgb(&[
            RayValue::Number(255.0),
            RayValue::Number(0.0),
            RayValue::Number(0.0),
            RayValue::Number(50.0), // 50% transparent
        ]);
        let color = result.as_color().unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
        // 50% transparency = ~127 alpha
        assert!((color.a as i32 - 127).abs() <= 1);
    }

    #[test]
    fn color_new_applies_transparency() {
        let base = RayValue::Color(RayColor::rgb(100, 150, 200));
        let result = color_new(&[base, RayValue::Number(25.0)]);
        let color = result.as_color().unwrap();
        assert_eq!(color.r, 100);
        assert_eq!(color.g, 150);
        assert_eq!(color.b, 200);
        // 25% transparency = ~191 alpha
        assert!((color.a as i32 - 191).abs() <= 1);
    }

    #[test]
    fn color_r_extracts_red() {
        let color = RayValue::Color(RayColor::rgb(128, 64, 32));
        let result = color_r(&[color]);
        assert_eq!(result.as_number(), Some(128.0));
    }

    #[test]
    fn color_g_extracts_green() {
        let color = RayValue::Color(RayColor::rgb(128, 64, 32));
        let result = color_g(&[color]);
        assert_eq!(result.as_number(), Some(64.0));
    }

    #[test]
    fn color_b_extracts_blue() {
        let color = RayValue::Color(RayColor::rgb(128, 64, 32));
        let result = color_b(&[color]);
        assert_eq!(result.as_number(), Some(32.0));
    }

    #[test]
    fn color_t_extracts_transparency() {
        let color = RayValue::Color(RayColor::new(100, 100, 100, 255));
        let result = color_t(&[color]);
        assert_eq!(result.as_number(), Some(0.0)); // fully opaque = 0% transparent
    }
}
