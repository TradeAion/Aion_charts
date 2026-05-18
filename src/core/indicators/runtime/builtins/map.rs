//! Map namespace builtin functions for AionDSL.
//!
//! Provides Pine Script-compatible map operations including:
//! - Creation: `map.new`, `map.from`
//! - Access: `map.get`, `map.contains`, `map.size`
//! - Mutation: `map.put`, `map.remove`, `map.clear`
//! - Iteration: `map.keys`, `map.values`, `map.entries`

use crate::core::indicators::runtime::value::RayValue;

/// Map namespace builtin function dispatch.
pub fn call(name: &str, args: &[RayValue]) -> Option<RayValue> {
    match name {
        // Creation functions
        "new" | "new_float" | "new_int" | "new_bool" | "new_string" | "new_color" => map_new(args),

        // Access functions
        "size" => map_size(args),
        "get" => map_get(args),
        "contains" => map_contains(args),

        // Mutation functions (return new map for immutable semantics)
        "put" => map_put(args),
        "remove" => map_remove(args),
        "clear" => map_clear(args),

        // Iteration functions
        "keys" => map_keys(args),
        "values" => map_values(args),
        "entries" | "to_array" => map_entries(args),

        // Utility functions
        "copy" => map_copy(args),
        "put_all" | "merge" => map_put_all(args),

        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Creation Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// map.new<type>() - Create empty map
fn map_new(_args: &[RayValue]) -> Option<RayValue> {
    Some(RayValue::Map(vec![]))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Access Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// map.size(m) - Returns number of entries in map
fn map_size(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Map(entries)) => Some(RayValue::Number(entries.len() as f64)),
        _ => Some(RayValue::Na),
    }
}

/// map.get(m, key) - Get value by key, returns na if not found
fn map_get(args: &[RayValue]) -> Option<RayValue> {
    let entries = match args.first() {
        Some(RayValue::Map(e)) => e,
        _ => return Some(RayValue::Na),
    };
    let key = args.get(1)?;
    let default = args.get(2).cloned().unwrap_or(RayValue::Na);

    for (k, v) in entries {
        if values_equal(k, key) {
            return Some(v.clone());
        }
    }

    Some(default)
}

/// map.contains(m, key) - Check if key exists in map
fn map_contains(args: &[RayValue]) -> Option<RayValue> {
    let entries = match args.first() {
        Some(RayValue::Map(e)) => e,
        _ => return Some(RayValue::Bool(false)),
    };
    let key = args.get(1)?;

    for (k, _) in entries {
        if values_equal(k, key) {
            return Some(RayValue::Bool(true));
        }
    }

    Some(RayValue::Bool(false))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Mutation Functions (returning new maps for immutable semantics)
// ═══════════════════════════════════════════════════════════════════════════════

/// map.put(m, key, value) - Returns new map with key-value pair added/updated
fn map_put(args: &[RayValue]) -> Option<RayValue> {
    let entries = match args.first() {
        Some(RayValue::Map(e)) => e.clone(),
        _ => return Some(RayValue::Na),
    };
    let key = args.get(1)?.clone();
    let value = args.get(2).cloned().unwrap_or(RayValue::Na);

    let mut new_entries = entries;
    let mut found = false;

    // Update existing key if found
    for (k, v) in &mut new_entries {
        if values_equal(k, &key) {
            *v = value.clone();
            found = true;
            break;
        }
    }

    // Add new entry if not found
    if !found {
        new_entries.push((key, value));
    }

    Some(RayValue::Map(new_entries))
}

/// map.remove(m, key) - Returns tuple [new_map, removed_value]
fn map_remove(args: &[RayValue]) -> Option<RayValue> {
    let entries = match args.first() {
        Some(RayValue::Map(e)) => e.clone(),
        _ => return Some(RayValue::Na),
    };
    let key = args.get(1)?;

    let mut new_entries = Vec::new();
    let mut removed = RayValue::Na;

    for (k, v) in entries {
        if values_equal(&k, key) {
            removed = v;
        } else {
            new_entries.push((k, v));
        }
    }

    Some(RayValue::Tuple(vec![RayValue::Map(new_entries), removed]))
}

/// map.clear(m) - Returns empty map
fn map_clear(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Map(_)) => Some(RayValue::Map(vec![])),
        _ => Some(RayValue::Na),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Iteration Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// map.keys(m) - Returns array of all keys
fn map_keys(args: &[RayValue]) -> Option<RayValue> {
    let entries = match args.first() {
        Some(RayValue::Map(e)) => e,
        _ => return Some(RayValue::Array(vec![])),
    };

    let keys: Vec<RayValue> = entries.iter().map(|(k, _)| k.clone()).collect();
    Some(RayValue::Array(keys))
}

/// map.values(m) - Returns array of all values
fn map_values(args: &[RayValue]) -> Option<RayValue> {
    let entries = match args.first() {
        Some(RayValue::Map(e)) => e,
        _ => return Some(RayValue::Array(vec![])),
    };

    let values: Vec<RayValue> = entries.iter().map(|(_, v)| v.clone()).collect();
    Some(RayValue::Array(values))
}

/// map.entries(m) - Returns array of [key, value] tuples
fn map_entries(args: &[RayValue]) -> Option<RayValue> {
    let entries = match args.first() {
        Some(RayValue::Map(e)) => e,
        _ => return Some(RayValue::Array(vec![])),
    };

    let pairs: Vec<RayValue> = entries
        .iter()
        .map(|(k, v)| RayValue::Tuple(vec![k.clone(), v.clone()]))
        .collect();
    Some(RayValue::Array(pairs))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Utility Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// map.copy(m) - Returns shallow copy of map
fn map_copy(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Map(entries)) => Some(RayValue::Map(entries.clone())),
        _ => Some(RayValue::Na),
    }
}

/// map.put_all(m1, m2) - Returns new map with all entries from both maps (m2 overwrites m1)
fn map_put_all(args: &[RayValue]) -> Option<RayValue> {
    let entries1 = match args.first() {
        Some(RayValue::Map(e)) => e.clone(),
        _ => return Some(RayValue::Na),
    };
    let entries2 = match args.get(1) {
        Some(RayValue::Map(e)) => e,
        _ => return Some(RayValue::Map(entries1)),
    };

    let mut result = entries1;

    // Add or update with entries from second map
    for (k2, v2) in entries2 {
        let mut found = false;
        for (k1, v1) in &mut result {
            if values_equal(k1, k2) {
                *v1 = v2.clone();
                found = true;
                break;
            }
        }
        if !found {
            result.push((k2.clone(), v2.clone()));
        }
    }

    Some(RayValue::Map(result))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Compare two RayValues for equality (used for map key comparison)
fn values_equal(a: &RayValue, b: &RayValue) -> bool {
    match (a, b) {
        (RayValue::Na, RayValue::Na) => true,
        (RayValue::Number(x), RayValue::Number(y)) => (x - y).abs() < f64::EPSILON,
        (RayValue::Bool(x), RayValue::Bool(y)) => x == y,
        (RayValue::String(x), RayValue::String(y)) => x == y,
        (RayValue::Color(c1), RayValue::Color(c2)) => c1 == c2,
        _ => false,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_new_creates_empty_map() {
        let result = call("new", &[]);
        assert!(matches!(result, Some(RayValue::Map(v)) if v.is_empty()));
    }

    #[test]
    fn map_size_returns_count() {
        let map = RayValue::Map(vec![
            (RayValue::String("a".to_string()), RayValue::Number(1.0)),
            (RayValue::String("b".to_string()), RayValue::Number(2.0)),
        ]);
        let result = call("size", &[map]);
        assert_eq!(result, Some(RayValue::Number(2.0)));
    }

    #[test]
    fn map_put_adds_entry() {
        let map = RayValue::Map(vec![]);
        let result = call(
            "put",
            &[
                map,
                RayValue::String("key".to_string()),
                RayValue::Number(42.0),
            ],
        );
        if let Some(RayValue::Map(entries)) = result {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].0, RayValue::String("key".to_string()));
            assert_eq!(entries[0].1, RayValue::Number(42.0));
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn map_put_updates_existing() {
        let map = RayValue::Map(vec![(
            RayValue::String("key".to_string()),
            RayValue::Number(1.0),
        )]);
        let result = call(
            "put",
            &[
                map,
                RayValue::String("key".to_string()),
                RayValue::Number(99.0),
            ],
        );
        if let Some(RayValue::Map(entries)) = result {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].1, RayValue::Number(99.0));
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn map_get_retrieves_value() {
        let map = RayValue::Map(vec![
            (RayValue::String("a".to_string()), RayValue::Number(10.0)),
            (RayValue::String("b".to_string()), RayValue::Number(20.0)),
        ]);
        let result = call("get", &[map, RayValue::String("b".to_string())]);
        assert_eq!(result, Some(RayValue::Number(20.0)));
    }

    #[test]
    fn map_get_returns_default_if_not_found() {
        let map = RayValue::Map(vec![]);
        let result = call(
            "get",
            &[
                map,
                RayValue::String("missing".to_string()),
                RayValue::Number(-1.0), // default value
            ],
        );
        assert_eq!(result, Some(RayValue::Number(-1.0)));
    }

    #[test]
    fn map_get_returns_na_if_not_found() {
        let map = RayValue::Map(vec![]);
        let result = call("get", &[map, RayValue::String("missing".to_string())]);
        assert_eq!(result, Some(RayValue::Na));
    }

    #[test]
    fn map_contains_finds_key() {
        let map = RayValue::Map(vec![(
            RayValue::String("key".to_string()),
            RayValue::Number(1.0),
        )]);
        let result = call(
            "contains",
            &[map.clone(), RayValue::String("key".to_string())],
        );
        assert_eq!(result, Some(RayValue::Bool(true)));

        let result = call("contains", &[map, RayValue::String("other".to_string())]);
        assert_eq!(result, Some(RayValue::Bool(false)));
    }

    #[test]
    fn map_remove_removes_entry() {
        let map = RayValue::Map(vec![
            (RayValue::String("a".to_string()), RayValue::Number(1.0)),
            (RayValue::String("b".to_string()), RayValue::Number(2.0)),
        ]);
        let result = call("remove", &[map, RayValue::String("a".to_string())]);
        if let Some(RayValue::Tuple(parts)) = result {
            if let RayValue::Map(entries) = &parts[0] {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].0, RayValue::String("b".to_string()));
            } else {
                panic!("expected map");
            }
            assert_eq!(parts[1], RayValue::Number(1.0));
        } else {
            panic!("expected tuple");
        }
    }

    #[test]
    fn map_clear_returns_empty() {
        let map = RayValue::Map(vec![(
            RayValue::String("key".to_string()),
            RayValue::Number(1.0),
        )]);
        let result = call("clear", &[map]);
        assert!(matches!(result, Some(RayValue::Map(v)) if v.is_empty()));
    }

    #[test]
    fn map_keys_returns_all_keys() {
        let map = RayValue::Map(vec![
            (RayValue::String("a".to_string()), RayValue::Number(1.0)),
            (RayValue::String("b".to_string()), RayValue::Number(2.0)),
        ]);
        let result = call("keys", &[map]);
        if let Some(RayValue::Array(keys)) = result {
            assert_eq!(keys.len(), 2);
            assert!(keys.contains(&RayValue::String("a".to_string())));
            assert!(keys.contains(&RayValue::String("b".to_string())));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn map_values_returns_all_values() {
        let map = RayValue::Map(vec![
            (RayValue::String("a".to_string()), RayValue::Number(1.0)),
            (RayValue::String("b".to_string()), RayValue::Number(2.0)),
        ]);
        let result = call("values", &[map]);
        if let Some(RayValue::Array(values)) = result {
            assert_eq!(values.len(), 2);
            assert!(values.contains(&RayValue::Number(1.0)));
            assert!(values.contains(&RayValue::Number(2.0)));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn map_entries_returns_pairs() {
        let map = RayValue::Map(vec![(
            RayValue::String("key".to_string()),
            RayValue::Number(42.0),
        )]);
        let result = call("entries", &[map]);
        if let Some(RayValue::Array(pairs)) = result {
            assert_eq!(pairs.len(), 1);
            if let RayValue::Tuple(pair) = &pairs[0] {
                assert_eq!(pair[0], RayValue::String("key".to_string()));
                assert_eq!(pair[1], RayValue::Number(42.0));
            } else {
                panic!("expected tuple");
            }
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn map_copy_creates_copy() {
        let map = RayValue::Map(vec![(
            RayValue::String("key".to_string()),
            RayValue::Number(1.0),
        )]);
        let result = call("copy", &[map.clone()]);
        assert_eq!(result, Some(map));
    }

    #[test]
    fn map_put_all_merges_maps() {
        let map1 = RayValue::Map(vec![(
            RayValue::String("a".to_string()),
            RayValue::Number(1.0),
        )]);
        let map2 = RayValue::Map(vec![(
            RayValue::String("b".to_string()),
            RayValue::Number(2.0),
        )]);
        let result = call("put_all", &[map1, map2]);
        if let Some(RayValue::Map(entries)) = result {
            assert_eq!(entries.len(), 2);
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn map_put_all_overwrites() {
        let map1 = RayValue::Map(vec![(
            RayValue::String("key".to_string()),
            RayValue::Number(1.0),
        )]);
        let map2 = RayValue::Map(vec![(
            RayValue::String("key".to_string()),
            RayValue::Number(99.0),
        )]);
        let result = call("put_all", &[map1, map2]);
        if let Some(RayValue::Map(entries)) = result {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].1, RayValue::Number(99.0));
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn map_with_number_keys() {
        let map = RayValue::Map(vec![]);
        let result = call(
            "put",
            &[
                map,
                RayValue::Number(1.0),
                RayValue::String("one".to_string()),
            ],
        );
        let map = result.unwrap();
        let result = call("get", &[map, RayValue::Number(1.0)]);
        assert_eq!(result, Some(RayValue::String("one".to_string())));
    }
}
