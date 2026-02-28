use crate::core::indicators::runtime::value::RayValue;
use std::f64::consts::{E, PI};

/// Math namespace builtin functions.
pub fn call(name: &str, args: &[RayValue]) -> Option<RayValue> {
    // Constants
    match name {
        "pi" => return Some(RayValue::Number(PI)),
        "e" => return Some(RayValue::Number(E)),
        _ => {}
    }

    // Single-argument functions
    if let Some(x) = args.first().and_then(RayValue::as_number) {
        let result = match name {
            "abs" => x.abs(),
            "sign" => {
                if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }
            "ceil" => x.ceil(),
            "floor" => x.floor(),
            "round" => x.round(),
            "sqrt" => {
                if x >= 0.0 {
                    x.sqrt()
                } else {
                    return Some(RayValue::Na);
                }
            }
            "log" | "ln" => {
                if x > 0.0 {
                    x.ln()
                } else {
                    return Some(RayValue::Na);
                }
            }
            "log10" => {
                if x > 0.0 {
                    x.log10()
                } else {
                    return Some(RayValue::Na);
                }
            }
            "log2" => {
                if x > 0.0 {
                    x.log2()
                } else {
                    return Some(RayValue::Na);
                }
            }
            "exp" => x.exp(),
            "sin" => x.sin(),
            "cos" => x.cos(),
            "tan" => x.tan(),
            "asin" => {
                if (-1.0..=1.0).contains(&x) {
                    x.asin()
                } else {
                    return Some(RayValue::Na);
                }
            }
            "acos" => {
                if (-1.0..=1.0).contains(&x) {
                    x.acos()
                } else {
                    return Some(RayValue::Na);
                }
            }
            "atan" => x.atan(),
            "sinh" => x.sinh(),
            "cosh" => x.cosh(),
            "tanh" => x.tanh(),
            "todegrees" | "degrees" => x.to_degrees(),
            "toradians" | "radians" => x.to_radians(),
            _ => return try_multi_arg(name, args),
        };
        return Some(RayValue::Number(result));
    }

    // Try multi-argument functions even if first arg is na
    try_multi_arg(name, args)
}

fn try_multi_arg(name: &str, args: &[RayValue]) -> Option<RayValue> {
    match name {
        "max" => {
            let numbers: Vec<f64> = args.iter().filter_map(RayValue::as_number).collect();
            if numbers.is_empty() {
                Some(RayValue::Na)
            } else {
                Some(RayValue::Number(
                    numbers.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                ))
            }
        }
        "min" => {
            let numbers: Vec<f64> = args.iter().filter_map(RayValue::as_number).collect();
            if numbers.is_empty() {
                Some(RayValue::Na)
            } else {
                Some(RayValue::Number(
                    numbers.iter().cloned().fold(f64::INFINITY, f64::min),
                ))
            }
        }
        "avg" | "average" => {
            let numbers: Vec<f64> = args.iter().filter_map(RayValue::as_number).collect();
            if numbers.is_empty() {
                Some(RayValue::Na)
            } else {
                Some(RayValue::Number(
                    numbers.iter().sum::<f64>() / numbers.len() as f64,
                ))
            }
        }
        "sum" => {
            let numbers: Vec<f64> = args.iter().filter_map(RayValue::as_number).collect();
            if numbers.is_empty() {
                Some(RayValue::Na)
            } else {
                Some(RayValue::Number(numbers.iter().sum()))
            }
        }
        "pow" => {
            let base = args.first().and_then(RayValue::as_number)?;
            let exp = args.get(1).and_then(RayValue::as_number)?;
            let result = base.powf(exp);
            if result.is_finite() {
                Some(RayValue::Number(result))
            } else {
                Some(RayValue::Na)
            }
        }
        "atan2" => {
            let y = args.first().and_then(RayValue::as_number)?;
            let x = args.get(1).and_then(RayValue::as_number)?;
            Some(RayValue::Number(y.atan2(x)))
        }
        "random" => {
            // Deterministic pseudo-random based on bar index if provided
            // For simplicity, return a constant for now (real impl needs seed)
            Some(RayValue::Number(0.5))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abs_returns_absolute_value() {
        let result = call("abs", &[RayValue::Number(-42.0)]);
        assert_eq!(result, Some(RayValue::Number(42.0)));
    }

    #[test]
    fn sign_returns_sign() {
        assert_eq!(
            call("sign", &[RayValue::Number(5.0)]),
            Some(RayValue::Number(1.0))
        );
        assert_eq!(
            call("sign", &[RayValue::Number(-5.0)]),
            Some(RayValue::Number(-1.0))
        );
        assert_eq!(
            call("sign", &[RayValue::Number(0.0)]),
            Some(RayValue::Number(0.0))
        );
    }

    #[test]
    fn sqrt_returns_na_for_negative() {
        let result = call("sqrt", &[RayValue::Number(-1.0)]);
        assert_eq!(result, Some(RayValue::Na));
    }

    #[test]
    fn sqrt_returns_correct_value() {
        let result = call("sqrt", &[RayValue::Number(9.0)]);
        assert_eq!(result, Some(RayValue::Number(3.0)));
    }

    #[test]
    fn pi_constant() {
        let result = call("pi", &[]);
        assert_eq!(result, Some(RayValue::Number(PI)));
    }

    #[test]
    fn max_returns_maximum() {
        let result = call(
            "max",
            &[
                RayValue::Number(1.0),
                RayValue::Number(5.0),
                RayValue::Number(3.0),
            ],
        );
        assert_eq!(result, Some(RayValue::Number(5.0)));
    }

    #[test]
    fn min_returns_minimum() {
        let result = call(
            "min",
            &[
                RayValue::Number(1.0),
                RayValue::Number(5.0),
                RayValue::Number(3.0),
            ],
        );
        assert_eq!(result, Some(RayValue::Number(1.0)));
    }

    #[test]
    fn avg_returns_average() {
        let result = call(
            "avg",
            &[
                RayValue::Number(2.0),
                RayValue::Number(4.0),
                RayValue::Number(6.0),
            ],
        );
        assert_eq!(result, Some(RayValue::Number(4.0)));
    }

    #[test]
    fn pow_returns_power() {
        let result = call("pow", &[RayValue::Number(2.0), RayValue::Number(3.0)]);
        assert_eq!(result, Some(RayValue::Number(8.0)));
    }

    #[test]
    fn trig_functions_work() {
        let result = call("sin", &[RayValue::Number(0.0)]);
        assert_eq!(result, Some(RayValue::Number(0.0)));

        let result = call("cos", &[RayValue::Number(0.0)]);
        assert_eq!(result, Some(RayValue::Number(1.0)));
    }

    #[test]
    fn todegrees_converts() {
        let result = call("todegrees", &[RayValue::Number(PI)]);
        if let Some(RayValue::Number(v)) = result {
            assert!((v - 180.0).abs() < 1e-10);
        } else {
            panic!("expected number");
        }
    }

    #[test]
    fn toradians_converts() {
        let result = call("toradians", &[RayValue::Number(180.0)]);
        if let Some(RayValue::Number(v)) = result {
            assert!((v - PI).abs() < 1e-10);
        } else {
            panic!("expected number");
        }
    }
}
