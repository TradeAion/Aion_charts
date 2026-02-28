use crate::core::indicators::runtime::value::RayValue;

/// nz(x) - Replace na with 0, or nz(x, replacement) - replace na with replacement
pub fn nz(args: &[RayValue]) -> RayValue {
    let value = args.first().cloned().unwrap_or(RayValue::Na);
    let replacement = args.get(1).cloned().unwrap_or(RayValue::Number(0.0));

    if value.is_na() {
        replacement
    } else {
        value
    }
}

/// na(x) - Returns true if x is na, false otherwise
pub fn na(args: &[RayValue]) -> RayValue {
    let value = args.first().unwrap_or(&RayValue::Na);
    RayValue::Bool(value.is_na())
}

/// fixnan(x) - Replaces NaN/na values by propagating the last valid value
/// Note: This is a simplified version that requires series history.
/// For expression-level use, it just returns the value or Na.
pub fn fixnan(args: &[RayValue]) -> RayValue {
    let value = args.first().cloned().unwrap_or(RayValue::Na);
    // In full implementation, this would track previous non-na value
    // For now, just return the value (na passthrough)
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nz_replaces_na_with_zero() {
        let result = nz(&[RayValue::Na]);
        assert_eq!(result, RayValue::Number(0.0));
    }

    #[test]
    fn nz_preserves_non_na_value() {
        let result = nz(&[RayValue::Number(42.0)]);
        assert_eq!(result, RayValue::Number(42.0));
    }

    #[test]
    fn nz_uses_custom_replacement() {
        let result = nz(&[RayValue::Na, RayValue::Number(99.0)]);
        assert_eq!(result, RayValue::Number(99.0));
    }

    #[test]
    fn na_returns_true_for_na() {
        let result = na(&[RayValue::Na]);
        assert_eq!(result, RayValue::Bool(true));
    }

    #[test]
    fn na_returns_false_for_number() {
        let result = na(&[RayValue::Number(42.0)]);
        assert_eq!(result, RayValue::Bool(false));
    }
}
