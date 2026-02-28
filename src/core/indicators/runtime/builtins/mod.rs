pub mod array;
pub mod color;
pub mod introspection;
pub mod line;
pub mod map;
pub mod math;
pub mod na_ops;
pub mod str;
pub mod ta;

use crate::core::indicators::runtime::value::RayValue;

/// Registry for builtin function dispatch.
pub struct BuiltinRegistry;

impl BuiltinRegistry {
    /// Evaluate a builtin function call by name.
    /// For simple builtins that don't need bar context.
    pub fn call(name: &str, args: &[RayValue]) -> Option<RayValue> {
        Self::call_with_context(name, args, None, None)
    }

    /// Evaluate a builtin function call with optional TA and introspection context.
    /// TA functions require bar data access for series operations.
    /// Introspection functions require symbol/bar/timeframe metadata.
    pub fn call_with_context(
        name: &str,
        args: &[RayValue],
        ta_ctx: Option<&ta::TaContext>,
        intro_ctx: Option<&introspection::IntrospectionContext>,
    ) -> Option<RayValue> {
        let name_lower = name.to_ascii_lowercase();

        // Na-handling functions
        match name_lower.as_str() {
            "nz" => return Some(na_ops::nz(args)),
            "na" => return Some(na_ops::na(args)),
            "fixnan" => return Some(na_ops::fixnan(args)),
            _ => {}
        }

        // syminfo.* properties
        if let Some(prop) = name_lower.strip_prefix("syminfo.") {
            if let Some(ctx) = intro_ctx {
                return introspection::call_syminfo(prop, ctx);
            }
            return Some(RayValue::Na); // No context available
        }

        // barstate.* properties
        if let Some(prop) = name_lower.strip_prefix("barstate.") {
            if let Some(ctx) = intro_ctx {
                return introspection::call_barstate(prop, ctx);
            }
            return Some(RayValue::Na);
        }

        // timeframe.* properties
        if let Some(prop) = name_lower.strip_prefix("timeframe.") {
            if let Some(ctx) = intro_ctx {
                return introspection::call_timeframe(prop, ctx);
            }
            return Some(RayValue::Na);
        }

        // array.* functions
        if let Some(array_fn) = name_lower.strip_prefix("array.") {
            return array::call(array_fn, args);
        }

        // str.* functions
        if let Some(str_fn) = name_lower.strip_prefix("str.") {
            return str::call(str_fn, args);
        }

        // map.* functions
        if let Some(map_fn) = name_lower.strip_prefix("map.") {
            return map::call(map_fn, args);
        }

        // line.* functions (computed properties; line.new/set/delete are handled by compiler)
        if let Some(line_fn) = name_lower.strip_prefix("line.") {
            return line::call(line_fn, args);
        }

        // color.* functions
        if let Some(color_fn) = name_lower.strip_prefix("color.") {
            return color::call(color_fn, args);
        }

        // ta.* functions (require bar context)
        if let Some(ta_fn) = name_lower.strip_prefix("ta.") {
            return ta::call(ta_fn, args, ta_ctx);
        }

        // math.* functions
        if let Some(math_fn) = name_lower.strip_prefix("math.") {
            return math::call(math_fn, args);
        }

        // Direct math function names (without prefix)
        if let Some(result) = math::call(&name_lower, args) {
            return Some(result);
        }

        None
    }
}
