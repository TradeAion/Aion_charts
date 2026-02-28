//! Table namespace builtins for RayDSL.
//!
//! Provides table positioning constants and cell configuration helpers.
//! Note: table.new/cell/delete are handled by the compiler as object mutations,
//! but computed properties and constants are dispatched here.

use crate::core::indicators::runtime::value::RayValue;

/// Dispatch table.* function calls.
pub fn call(name: &str, args: &[RayValue]) -> Option<RayValue> {
    match name {
        // Position constants (used in table.new position parameter)
        "position_top_left" | "position.top_left" => {
            Some(RayValue::String("position.top_left".to_string()))
        }
        "position_top_center" | "position.top_center" => {
            Some(RayValue::String("position.top_center".to_string()))
        }
        "position_top_right" | "position.top_right" => {
            Some(RayValue::String("position.top_right".to_string()))
        }
        "position_middle_left" | "position.middle_left" => {
            Some(RayValue::String("position.middle_left".to_string()))
        }
        "position_middle_center" | "position.middle_center" => {
            Some(RayValue::String("position.middle_center".to_string()))
        }
        "position_middle_right" | "position.middle_right" => {
            Some(RayValue::String("position.middle_right".to_string()))
        }
        "position_bottom_left" | "position.bottom_left" => {
            Some(RayValue::String("position.bottom_left".to_string()))
        }
        "position_bottom_center" | "position.bottom_center" => {
            Some(RayValue::String("position.bottom_center".to_string()))
        }
        "position_bottom_right" | "position.bottom_right" => {
            Some(RayValue::String("position.bottom_right".to_string()))
        }

        // Text alignment constants
        "align_left" | "text.align_left" => Some(RayValue::String("text.align_left".to_string())),
        "align_center" | "text.align_center" => {
            Some(RayValue::String("text.align_center".to_string()))
        }
        "align_right" | "text.align_right" => {
            Some(RayValue::String("text.align_right".to_string()))
        }
        "align_top" | "text.align_top" => Some(RayValue::String("text.align_top".to_string())),
        "align_bottom" | "text.align_bottom" => {
            Some(RayValue::String("text.align_bottom".to_string()))
        }

        // Text size constants
        "size_auto" | "size.auto" => Some(RayValue::String("size.auto".to_string())),
        "size_tiny" | "size.tiny" => Some(RayValue::String("size.tiny".to_string())),
        "size_small" | "size.small" => Some(RayValue::String("size.small".to_string())),
        "size_normal" | "size.normal" => Some(RayValue::String("size.normal".to_string())),
        "size_large" | "size.large" => Some(RayValue::String("size.large".to_string())),
        "size_huge" | "size.huge" => Some(RayValue::String("size.huge".to_string())),

        // Computed properties for table references
        "get_rows" | "rows" => get_rows(args),
        "get_columns" | "columns" => get_columns(args),

        _ => None,
    }
}

/// Get the number of rows in a table.
/// Usage: table.rows(table_id)
fn get_rows(args: &[RayValue]) -> Option<RayValue> {
    // For now, return Na - table metadata needs to be stored in object registry
    // In full implementation, would look up table object and return its row count
    if args.is_empty() {
        return Some(RayValue::Na);
    }
    // Return Na for now; actual implementation requires object registry access
    Some(RayValue::Na)
}

/// Get the number of columns in a table.
/// Usage: table.columns(table_id)
fn get_columns(args: &[RayValue]) -> Option<RayValue> {
    if args.is_empty() {
        return Some(RayValue::Na);
    }
    // Return Na for now; actual implementation requires object registry access
    Some(RayValue::Na)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_constants() {
        assert_eq!(
            call("position_top_left", &[]),
            Some(RayValue::String("position.top_left".to_string()))
        );
        assert_eq!(
            call("position_bottom_right", &[]),
            Some(RayValue::String("position.bottom_right".to_string()))
        );
        assert_eq!(
            call("position_middle_center", &[]),
            Some(RayValue::String("position.middle_center".to_string()))
        );
    }

    #[test]
    fn test_text_alignment_constants() {
        assert_eq!(
            call("align_left", &[]),
            Some(RayValue::String("text.align_left".to_string()))
        );
        assert_eq!(
            call("align_center", &[]),
            Some(RayValue::String("text.align_center".to_string()))
        );
        assert_eq!(
            call("align_top", &[]),
            Some(RayValue::String("text.align_top".to_string()))
        );
    }

    #[test]
    fn test_size_constants() {
        assert_eq!(
            call("size_auto", &[]),
            Some(RayValue::String("size.auto".to_string()))
        );
        assert_eq!(
            call("size_normal", &[]),
            Some(RayValue::String("size.normal".to_string()))
        );
        assert_eq!(
            call("size_huge", &[]),
            Some(RayValue::String("size.huge".to_string()))
        );
    }

    #[test]
    fn test_unknown_function() {
        assert_eq!(call("unknown_function", &[]), None);
    }

    #[test]
    fn test_get_rows_empty() {
        assert_eq!(get_rows(&[]), Some(RayValue::Na));
    }

    #[test]
    fn test_get_columns_empty() {
        assert_eq!(get_columns(&[]), Some(RayValue::Na));
    }
}
