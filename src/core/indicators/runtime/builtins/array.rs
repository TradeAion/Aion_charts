//! Array namespace builtin functions for AionDSL.
//!
//! Provides Pine Script-compatible array operations including:
//! - Creation: `array.new<type>`, `array.from`
//! - Access: `array.get`, `array.set`, `array.size`
//! - Mutation: `array.push`, `array.pop`, `array.shift`, `array.unshift`
//! - Transformation: `array.slice`, `array.concat`, `array.copy`, `array.reverse`, `array.sort`
//! - Aggregation: `array.avg`, `array.sum`, `array.min`, `array.max`, `array.stdev`, `array.variance`
//! - Search: `array.indexof`, `array.lastindexof`, `array.includes`, `array.binary_search`
//! - Utilities: `array.clear`, `array.fill`, `array.join`

use crate::core::indicators::runtime::value::RayValue;

/// Array namespace builtin function dispatch.
pub fn call(name: &str, args: &[RayValue]) -> Option<RayValue> {
    match name {
        // Creation functions
        "new" | "new_float" => array_new(args),
        "new_int" => array_new(args),
        "new_bool" => array_new_bool(args),
        "new_string" => array_new_string(args),
        "new_color" => array_new_color(args),
        "from" => array_from(args),

        // Access functions
        "size" => array_size(args),
        "get" => array_get(args),

        // Mutation functions (return modified array for immutable semantics)
        "set" => array_set(args),
        "push" => array_push(args),
        "pop" => array_pop(args),
        "shift" => array_shift(args),
        "unshift" => array_unshift(args),
        "insert" => array_insert(args),
        "remove" => array_remove(args),
        "clear" => array_clear(args),
        "fill" => array_fill(args),

        // Transformation functions
        "slice" => array_slice(args),
        "concat" => array_concat(args),
        "copy" => array_copy(args),
        "reverse" => array_reverse(args),
        "sort" => array_sort(args),
        "sort_indices" => array_sort_indices(args),

        // Aggregation functions
        "avg" => array_avg(args),
        "sum" => array_sum(args),
        "min" => array_min(args),
        "max" => array_max(args),
        "stdev" => array_stdev(args),
        "variance" => array_variance(args),
        "median" => array_median(args),
        "mode" => array_mode(args),
        "range" => array_range(args),
        "covariance" => array_covariance(args),
        "percentile_linear_interpolation" | "percentile" => array_percentile(args),
        "percentrank" => array_percentrank(args),

        // Search functions
        "indexof" => array_indexof(args),
        "lastindexof" => array_lastindexof(args),
        "includes" => array_includes(args),
        "binary_search" => array_binary_search(args),
        "binary_search_leftmost" => array_binary_search_leftmost(args),
        "binary_search_rightmost" => array_binary_search_rightmost(args),

        // Utility functions
        "join" => array_join(args),
        "first" => array_first(args),
        "last" => array_last(args),
        "every" => array_every(args),
        "some" => array_some(args),

        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Creation Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// array.new<float>(size, initial_value) - Create array with size elements
fn array_new(args: &[RayValue]) -> Option<RayValue> {
    let size = args
        .first()
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);
    let initial = args.get(1).cloned().unwrap_or(RayValue::Na);
    let elements = vec![initial; size];
    Some(RayValue::Array(elements))
}

/// array.new_bool(size, initial_value)
fn array_new_bool(args: &[RayValue]) -> Option<RayValue> {
    let size = args
        .first()
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);
    let initial = args.get(1).and_then(RayValue::as_bool).unwrap_or(false);
    let elements = vec![RayValue::Bool(initial); size];
    Some(RayValue::Array(elements))
}

/// array.new_string(size, initial_value)
fn array_new_string(args: &[RayValue]) -> Option<RayValue> {
    let size = args
        .first()
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);
    let initial = args
        .get(1)
        .and_then(RayValue::as_string)
        .unwrap_or("")
        .to_string();
    let elements = vec![RayValue::String(initial); size];
    Some(RayValue::Array(elements))
}

/// array.new_color(size, initial_value)
fn array_new_color(args: &[RayValue]) -> Option<RayValue> {
    let size = args
        .first()
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);
    let initial = args.get(1).cloned().unwrap_or(RayValue::Na);
    let elements = vec![initial; size];
    Some(RayValue::Array(elements))
}

/// array.from(val1, val2, ...) - Create array from values
fn array_from(args: &[RayValue]) -> Option<RayValue> {
    Some(RayValue::Array(args.to_vec()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Access Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// array.size(arr) - Returns size of array
fn array_size(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Array(elements)) => Some(RayValue::Number(elements.len() as f64)),
        _ => Some(RayValue::Na),
    }
}

/// array.get(arr, index) - Get element at index
fn array_get(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let index = args.get(1).and_then(RayValue::as_number)? as usize;
    arr.get(index).cloned().or(Some(RayValue::Na))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Mutation Functions (returning new arrays for immutable semantics)
// ═══════════════════════════════════════════════════════════════════════════════

/// array.set(arr, index, value) - Returns new array with element set
fn array_set(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };
    let index = args.get(1).and_then(RayValue::as_number)? as usize;
    let value = args.get(2).cloned().unwrap_or(RayValue::Na);

    if index >= arr.len() {
        return Some(RayValue::Na);
    }

    let mut new_arr = arr;
    new_arr[index] = value;
    Some(RayValue::Array(new_arr))
}

/// array.push(arr, value) - Returns new array with value appended
fn array_push(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1).cloned().unwrap_or(RayValue::Na);

    let mut new_arr = arr;
    new_arr.push(value);
    Some(RayValue::Array(new_arr))
}

/// array.pop(arr) - Returns tuple [new_array, popped_value]
fn array_pop(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };

    if arr.is_empty() {
        return Some(RayValue::Tuple(vec![RayValue::Array(vec![]), RayValue::Na]));
    }

    let mut new_arr = arr;
    let popped = new_arr.pop().unwrap_or(RayValue::Na);
    Some(RayValue::Tuple(vec![RayValue::Array(new_arr), popped]))
}

/// array.shift(arr) - Returns tuple [new_array, shifted_value] (removes first element)
fn array_shift(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };

    if arr.is_empty() {
        return Some(RayValue::Tuple(vec![RayValue::Array(vec![]), RayValue::Na]));
    }

    let mut new_arr = arr;
    let shifted = new_arr.remove(0);
    Some(RayValue::Tuple(vec![RayValue::Array(new_arr), shifted]))
}

/// array.unshift(arr, value) - Returns new array with value prepended
fn array_unshift(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1).cloned().unwrap_or(RayValue::Na);

    let mut new_arr = arr;
    new_arr.insert(0, value);
    Some(RayValue::Array(new_arr))
}

/// array.insert(arr, index, value) - Returns new array with value inserted at index
fn array_insert(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };
    let index = args.get(1).and_then(RayValue::as_number)? as usize;
    let value = args.get(2).cloned().unwrap_or(RayValue::Na);

    let mut new_arr = arr;
    let insert_at = index.min(new_arr.len());
    new_arr.insert(insert_at, value);
    Some(RayValue::Array(new_arr))
}

/// array.remove(arr, index) - Returns tuple [new_array, removed_value]
fn array_remove(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };
    let index = args.get(1).and_then(RayValue::as_number)? as usize;

    if index >= arr.len() {
        return Some(RayValue::Tuple(vec![RayValue::Array(arr), RayValue::Na]));
    }

    let mut new_arr = arr;
    let removed = new_arr.remove(index);
    Some(RayValue::Tuple(vec![RayValue::Array(new_arr), removed]))
}

/// array.clear(arr) - Returns empty array
fn array_clear(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Array(_)) => Some(RayValue::Array(vec![])),
        _ => Some(RayValue::Na),
    }
}

/// array.fill(arr, value, index_from, index_to) - Returns array with range filled
fn array_fill(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1).cloned().unwrap_or(RayValue::Na);
    let from = args
        .get(2)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);
    let to = args
        .get(3)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(arr.len());

    let mut new_arr = arr;
    let end = to.min(new_arr.len());
    for i in from..end {
        new_arr[i] = value.clone();
    }
    Some(RayValue::Array(new_arr))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Transformation Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// array.slice(arr, index_from, index_to) - Returns slice of array
fn array_slice(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let from = args
        .get(1)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);
    let to = args
        .get(2)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(arr.len());

    let start = from.min(arr.len());
    let end = to.min(arr.len());
    if start >= end {
        return Some(RayValue::Array(vec![]));
    }
    Some(RayValue::Array(arr[start..end].to_vec()))
}

/// array.concat(arr1, arr2) - Returns concatenation of arrays
fn array_concat(args: &[RayValue]) -> Option<RayValue> {
    let arr1 = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };
    let arr2 = match args.get(1) {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Array(arr1)),
    };

    let mut result = arr1;
    result.extend(arr2);
    Some(RayValue::Array(result))
}

/// array.copy(arr) - Returns shallow copy of array
fn array_copy(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Array(elements)) => Some(RayValue::Array(elements.clone())),
        _ => Some(RayValue::Na),
    }
}

/// array.reverse(arr) - Returns reversed array
fn array_reverse(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };

    let mut new_arr = arr;
    new_arr.reverse();
    Some(RayValue::Array(new_arr))
}

/// array.sort(arr, order) - Returns sorted array (order: 1=asc, -1=desc)
fn array_sort(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements.clone(),
        _ => return Some(RayValue::Na),
    };
    let order = args.get(1).and_then(RayValue::as_number).unwrap_or(1.0);

    // Extract numbers for sorting
    let mut indexed: Vec<(usize, f64)> = arr
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.as_number().map(|n| (i, n)))
        .collect();

    if order >= 0.0 {
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    } else {
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Rebuild array: sorted numbers, then non-numbers at end
    let mut result: Vec<RayValue> = indexed.iter().map(|(i, _)| arr[*i].clone()).collect();

    // Append non-number values
    for (i, v) in arr.iter().enumerate() {
        if v.as_number().is_none() {
            result.push(arr[i].clone());
        }
    }

    Some(RayValue::Array(result))
}

/// array.sort_indices(arr, order) - Returns indices that would sort the array
fn array_sort_indices(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let order = args.get(1).and_then(RayValue::as_number).unwrap_or(1.0);

    let mut indexed: Vec<(usize, f64)> = arr
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.as_number().map(|n| (i, n)))
        .collect();

    if order >= 0.0 {
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    } else {
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    let indices: Vec<RayValue> = indexed
        .iter()
        .map(|(i, _)| RayValue::Number(*i as f64))
        .collect();

    Some(RayValue::Array(indices))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Aggregation Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// array.avg(arr) - Returns average of numeric elements
fn array_avg(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();
    if numbers.is_empty() {
        return Some(RayValue::Na);
    }

    Some(RayValue::Number(
        numbers.iter().sum::<f64>() / numbers.len() as f64,
    ))
}

/// array.sum(arr) - Returns sum of numeric elements
fn array_sum(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };

    let sum: f64 = arr.iter().filter_map(RayValue::as_number).sum();
    Some(RayValue::Number(sum))
}

/// array.min(arr) - Returns minimum numeric element
fn array_min(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();
    if numbers.is_empty() {
        return Some(RayValue::Na);
    }

    Some(RayValue::Number(
        numbers.iter().cloned().fold(f64::INFINITY, f64::min),
    ))
}

/// array.max(arr) - Returns maximum numeric element
fn array_max(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();
    if numbers.is_empty() {
        return Some(RayValue::Na);
    }

    Some(RayValue::Number(
        numbers.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    ))
}

/// array.stdev(arr, biased) - Returns standard deviation
fn array_stdev(args: &[RayValue]) -> Option<RayValue> {
    let variance = array_variance(args)?;
    match variance {
        RayValue::Number(v) => Some(RayValue::Number(v.sqrt())),
        _ => Some(RayValue::Na),
    }
}

/// array.variance(arr, biased) - Returns variance
fn array_variance(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let biased = args.get(1).and_then(RayValue::as_bool).unwrap_or(true);

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();
    if numbers.is_empty() {
        return Some(RayValue::Na);
    }

    let n = numbers.len();
    let mean = numbers.iter().sum::<f64>() / n as f64;
    let sum_sq: f64 = numbers.iter().map(|x| (x - mean).powi(2)).sum();

    let divisor = if biased { n } else { n.saturating_sub(1) };
    if divisor == 0 {
        return Some(RayValue::Na);
    }

    Some(RayValue::Number(sum_sq / divisor as f64))
}

/// array.median(arr) - Returns median value
fn array_median(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };

    let mut numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();
    if numbers.is_empty() {
        return Some(RayValue::Na);
    }

    numbers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = numbers.len();
    let median = if n % 2 == 0 {
        (numbers[n / 2 - 1] + numbers[n / 2]) / 2.0
    } else {
        numbers[n / 2]
    };

    Some(RayValue::Number(median))
}

/// array.mode(arr) - Returns most frequent value
fn array_mode(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();
    if numbers.is_empty() {
        return Some(RayValue::Na);
    }

    // Count occurrences (using tolerance for floating point)
    let mut counts: Vec<(f64, usize)> = Vec::new();
    for n in &numbers {
        let mut found = false;
        for (v, count) in &mut counts {
            if (*v - *n).abs() < f64::EPSILON {
                *count += 1;
                found = true;
                break;
            }
        }
        if !found {
            counts.push((*n, 1));
        }
    }

    let mode = counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(v, _)| *v)?;

    Some(RayValue::Number(mode))
}

/// array.range(arr) - Returns max - min
fn array_range(args: &[RayValue]) -> Option<RayValue> {
    let min = array_min(args)?;
    let max = array_max(args)?;

    match (min, max) {
        (RayValue::Number(min_v), RayValue::Number(max_v)) => Some(RayValue::Number(max_v - min_v)),
        _ => Some(RayValue::Na),
    }
}

/// array.covariance(arr1, arr2) - Returns covariance between two arrays
fn array_covariance(args: &[RayValue]) -> Option<RayValue> {
    let arr1 = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let arr2 = match args.get(1) {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };

    let nums1: Vec<f64> = arr1.iter().filter_map(RayValue::as_number).collect();
    let nums2: Vec<f64> = arr2.iter().filter_map(RayValue::as_number).collect();

    let n = nums1.len().min(nums2.len());
    if n == 0 {
        return Some(RayValue::Na);
    }

    let mean1 = nums1.iter().take(n).sum::<f64>() / n as f64;
    let mean2 = nums2.iter().take(n).sum::<f64>() / n as f64;

    let cov: f64 = (0..n)
        .map(|i| (nums1[i] - mean1) * (nums2[i] - mean2))
        .sum::<f64>()
        / n as f64;

    Some(RayValue::Number(cov))
}

/// array.percentile_linear_interpolation(arr, percentage) - Returns percentile value
fn array_percentile(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let percentage = args.get(1).and_then(RayValue::as_number)?;

    let mut numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();
    if numbers.is_empty() {
        return Some(RayValue::Na);
    }

    numbers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = numbers.len();

    // Linear interpolation
    let p = (percentage / 100.0).clamp(0.0, 1.0);
    let index = p * (n - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    let frac = index - lower as f64;

    let result = if lower == upper || upper >= n {
        numbers[lower.min(n - 1)]
    } else {
        numbers[lower] + frac * (numbers[upper] - numbers[lower])
    };

    Some(RayValue::Number(result))
}

/// array.percentrank(arr, value) - Returns percent rank of value in array
fn array_percentrank(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1).and_then(RayValue::as_number)?;

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();
    if numbers.is_empty() {
        return Some(RayValue::Na);
    }

    let count_below = numbers.iter().filter(|&&n| n < value).count();
    let rank = (count_below as f64 / numbers.len() as f64) * 100.0;

    Some(RayValue::Number(rank))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Search Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// array.indexof(arr, value) - Returns first index of value, or -1
fn array_indexof(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1)?;

    for (i, elem) in arr.iter().enumerate() {
        if values_equal(elem, value) {
            return Some(RayValue::Number(i as f64));
        }
    }

    Some(RayValue::Number(-1.0))
}

/// array.lastindexof(arr, value) - Returns last index of value, or -1
fn array_lastindexof(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1)?;

    for (i, elem) in arr.iter().enumerate().rev() {
        if values_equal(elem, value) {
            return Some(RayValue::Number(i as f64));
        }
    }

    Some(RayValue::Number(-1.0))
}

/// array.includes(arr, value) - Returns true if array contains value
fn array_includes(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Bool(false)),
    };
    let value = args.get(1)?;

    for elem in arr {
        if values_equal(elem, value) {
            return Some(RayValue::Bool(true));
        }
    }

    Some(RayValue::Bool(false))
}

/// array.binary_search(arr, value) - Returns index if found, or -1 (array must be sorted)
fn array_binary_search(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1).and_then(RayValue::as_number)?;

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();

    match numbers.binary_search_by(|n| n.partial_cmp(&value).unwrap_or(std::cmp::Ordering::Equal)) {
        Ok(i) => Some(RayValue::Number(i as f64)),
        Err(_) => Some(RayValue::Number(-1.0)),
    }
}

/// array.binary_search_leftmost(arr, value) - Returns leftmost index where value could be inserted
fn array_binary_search_leftmost(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1).and_then(RayValue::as_number)?;

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();

    let idx = numbers.partition_point(|&n| n < value);
    Some(RayValue::Number(idx as f64))
}

/// array.binary_search_rightmost(arr, value) - Returns rightmost index where value could be inserted
fn array_binary_search_rightmost(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let value = args.get(1).and_then(RayValue::as_number)?;

    let numbers: Vec<f64> = arr.iter().filter_map(RayValue::as_number).collect();

    let idx = numbers.partition_point(|&n| n <= value);
    Some(RayValue::Number(idx as f64))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Utility Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// array.join(arr, separator) - Joins array elements into string
fn array_join(args: &[RayValue]) -> Option<RayValue> {
    let arr = match args.first() {
        Some(RayValue::Array(elements)) => elements,
        _ => return Some(RayValue::Na),
    };
    let separator = args.get(1).and_then(RayValue::as_string).unwrap_or(",");

    let parts: Vec<String> = arr
        .iter()
        .map(|v| v.to_display_text().unwrap_or_else(|| "na".to_string()))
        .collect();

    Some(RayValue::String(parts.join(separator)))
}

/// array.first(arr) - Returns first element
fn array_first(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Array(elements)) => elements.first().cloned().or(Some(RayValue::Na)),
        _ => Some(RayValue::Na),
    }
}

/// array.last(arr) - Returns last element
fn array_last(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Array(elements)) => elements.last().cloned().or(Some(RayValue::Na)),
        _ => Some(RayValue::Na),
    }
}

/// array.every(arr) - Returns true if all elements are truthy
fn array_every(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Array(elements)) => {
            Some(RayValue::Bool(elements.iter().all(RayValue::is_truthy)))
        }
        _ => Some(RayValue::Bool(false)),
    }
}

/// array.some(arr) - Returns true if any element is truthy
fn array_some(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::Array(elements)) => {
            Some(RayValue::Bool(elements.iter().any(RayValue::is_truthy)))
        }
        _ => Some(RayValue::Bool(false)),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Compare two RayValues for equality
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
    fn array_new_creates_array_with_size() {
        let result = call("new", &[RayValue::Number(3.0), RayValue::Number(0.0)]);
        assert!(matches!(result, Some(RayValue::Array(v)) if v.len() == 3));
    }

    #[test]
    fn array_from_creates_from_values() {
        let result = call(
            "from",
            &[
                RayValue::Number(1.0),
                RayValue::Number(2.0),
                RayValue::Number(3.0),
            ],
        );
        assert!(matches!(result, Some(RayValue::Array(v)) if v.len() == 3));
    }

    #[test]
    fn array_size_returns_length() {
        let arr = RayValue::Array(vec![RayValue::Number(1.0), RayValue::Number(2.0)]);
        let result = call("size", &[arr]);
        assert_eq!(result, Some(RayValue::Number(2.0)));
    }

    #[test]
    fn array_get_retrieves_element() {
        let arr = RayValue::Array(vec![
            RayValue::Number(10.0),
            RayValue::Number(20.0),
            RayValue::Number(30.0),
        ]);
        let result = call("get", &[arr, RayValue::Number(1.0)]);
        assert_eq!(result, Some(RayValue::Number(20.0)));
    }

    #[test]
    fn array_set_updates_element() {
        let arr = RayValue::Array(vec![RayValue::Number(1.0), RayValue::Number(2.0)]);
        let result = call("set", &[arr, RayValue::Number(0.0), RayValue::Number(99.0)]);
        if let Some(RayValue::Array(v)) = result {
            assert_eq!(v[0], RayValue::Number(99.0));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn array_push_appends_element() {
        let arr = RayValue::Array(vec![RayValue::Number(1.0)]);
        let result = call("push", &[arr, RayValue::Number(2.0)]);
        if let Some(RayValue::Array(v)) = result {
            assert_eq!(v.len(), 2);
            assert_eq!(v[1], RayValue::Number(2.0));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn array_pop_removes_last() {
        let arr = RayValue::Array(vec![RayValue::Number(1.0), RayValue::Number(2.0)]);
        let result = call("pop", &[arr]);
        if let Some(RayValue::Tuple(v)) = result {
            assert_eq!(v.len(), 2);
            if let RayValue::Array(new_arr) = &v[0] {
                assert_eq!(new_arr.len(), 1);
            }
            assert_eq!(v[1], RayValue::Number(2.0));
        } else {
            panic!("expected tuple");
        }
    }

    #[test]
    fn array_slice_returns_subarray() {
        let arr = RayValue::Array(vec![
            RayValue::Number(1.0),
            RayValue::Number(2.0),
            RayValue::Number(3.0),
            RayValue::Number(4.0),
        ]);
        let result = call(
            "slice",
            &[arr, RayValue::Number(1.0), RayValue::Number(3.0)],
        );
        if let Some(RayValue::Array(v)) = result {
            assert_eq!(v.len(), 2);
            assert_eq!(v[0], RayValue::Number(2.0));
            assert_eq!(v[1], RayValue::Number(3.0));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn array_concat_joins_arrays() {
        let arr1 = RayValue::Array(vec![RayValue::Number(1.0)]);
        let arr2 = RayValue::Array(vec![RayValue::Number(2.0)]);
        let result = call("concat", &[arr1, arr2]);
        if let Some(RayValue::Array(v)) = result {
            assert_eq!(v.len(), 2);
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn array_reverse_reverses() {
        let arr = RayValue::Array(vec![
            RayValue::Number(1.0),
            RayValue::Number(2.0),
            RayValue::Number(3.0),
        ]);
        let result = call("reverse", &[arr]);
        if let Some(RayValue::Array(v)) = result {
            assert_eq!(v[0], RayValue::Number(3.0));
            assert_eq!(v[2], RayValue::Number(1.0));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn array_sort_ascending() {
        let arr = RayValue::Array(vec![
            RayValue::Number(3.0),
            RayValue::Number(1.0),
            RayValue::Number(2.0),
        ]);
        let result = call("sort", &[arr, RayValue::Number(1.0)]);
        if let Some(RayValue::Array(v)) = result {
            assert_eq!(v[0], RayValue::Number(1.0));
            assert_eq!(v[1], RayValue::Number(2.0));
            assert_eq!(v[2], RayValue::Number(3.0));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn array_avg_computes_average() {
        let arr = RayValue::Array(vec![
            RayValue::Number(2.0),
            RayValue::Number(4.0),
            RayValue::Number(6.0),
        ]);
        let result = call("avg", &[arr]);
        assert_eq!(result, Some(RayValue::Number(4.0)));
    }

    #[test]
    fn array_sum_computes_sum() {
        let arr = RayValue::Array(vec![
            RayValue::Number(1.0),
            RayValue::Number(2.0),
            RayValue::Number(3.0),
        ]);
        let result = call("sum", &[arr]);
        assert_eq!(result, Some(RayValue::Number(6.0)));
    }

    #[test]
    fn array_min_finds_minimum() {
        let arr = RayValue::Array(vec![
            RayValue::Number(5.0),
            RayValue::Number(2.0),
            RayValue::Number(8.0),
        ]);
        let result = call("min", &[arr]);
        assert_eq!(result, Some(RayValue::Number(2.0)));
    }

    #[test]
    fn array_max_finds_maximum() {
        let arr = RayValue::Array(vec![
            RayValue::Number(5.0),
            RayValue::Number(2.0),
            RayValue::Number(8.0),
        ]);
        let result = call("max", &[arr]);
        assert_eq!(result, Some(RayValue::Number(8.0)));
    }

    #[test]
    fn array_indexof_finds_element() {
        let arr = RayValue::Array(vec![
            RayValue::Number(10.0),
            RayValue::Number(20.0),
            RayValue::Number(30.0),
        ]);
        let result = call("indexof", &[arr, RayValue::Number(20.0)]);
        assert_eq!(result, Some(RayValue::Number(1.0)));
    }

    #[test]
    fn array_indexof_returns_neg1_if_not_found() {
        let arr = RayValue::Array(vec![RayValue::Number(1.0)]);
        let result = call("indexof", &[arr, RayValue::Number(99.0)]);
        assert_eq!(result, Some(RayValue::Number(-1.0)));
    }

    #[test]
    fn array_includes_finds_element() {
        let arr = RayValue::Array(vec![RayValue::Number(1.0), RayValue::Number(2.0)]);
        let result = call("includes", &[arr.clone(), RayValue::Number(2.0)]);
        assert_eq!(result, Some(RayValue::Bool(true)));

        let result = call("includes", &[arr, RayValue::Number(99.0)]);
        assert_eq!(result, Some(RayValue::Bool(false)));
    }

    #[test]
    fn array_join_creates_string() {
        let arr = RayValue::Array(vec![
            RayValue::Number(1.0),
            RayValue::Number(2.0),
            RayValue::Number(3.0),
        ]);
        let result = call("join", &[arr, RayValue::String("-".to_string())]);
        assert_eq!(result, Some(RayValue::String("1-2-3".to_string())));
    }

    #[test]
    fn array_first_returns_first() {
        let arr = RayValue::Array(vec![RayValue::Number(10.0), RayValue::Number(20.0)]);
        let result = call("first", &[arr]);
        assert_eq!(result, Some(RayValue::Number(10.0)));
    }

    #[test]
    fn array_last_returns_last() {
        let arr = RayValue::Array(vec![RayValue::Number(10.0), RayValue::Number(20.0)]);
        let result = call("last", &[arr]);
        assert_eq!(result, Some(RayValue::Number(20.0)));
    }

    #[test]
    fn array_median_computes_median() {
        let arr = RayValue::Array(vec![
            RayValue::Number(1.0),
            RayValue::Number(2.0),
            RayValue::Number(3.0),
            RayValue::Number(4.0),
            RayValue::Number(5.0),
        ]);
        let result = call("median", &[arr]);
        assert_eq!(result, Some(RayValue::Number(3.0)));
    }

    #[test]
    fn array_every_checks_all_truthy() {
        let arr = RayValue::Array(vec![RayValue::Bool(true), RayValue::Number(1.0)]);
        let result = call("every", &[arr]);
        assert_eq!(result, Some(RayValue::Bool(true)));

        let arr = RayValue::Array(vec![RayValue::Bool(true), RayValue::Bool(false)]);
        let result = call("every", &[arr]);
        assert_eq!(result, Some(RayValue::Bool(false)));
    }

    #[test]
    fn array_some_checks_any_truthy() {
        let arr = RayValue::Array(vec![RayValue::Bool(false), RayValue::Number(1.0)]);
        let result = call("some", &[arr]);
        assert_eq!(result, Some(RayValue::Bool(true)));

        let arr = RayValue::Array(vec![RayValue::Bool(false), RayValue::Number(0.0)]);
        let result = call("some", &[arr]);
        assert_eq!(result, Some(RayValue::Bool(false)));
    }
}
