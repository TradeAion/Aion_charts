//! String namespace builtin functions for AionDSL.
//!
//! Provides Pine Script-compatible string operations including:
//! - Basic: `str.length`, `str.substring`, `str.replace`, `str.replace_all`
//! - Case: `str.upper`, `str.lower`
//! - Search: `str.contains`, `str.startswith`, `str.endswith`, `str.pos`
//! - Conversion: `str.tostring`, `str.tonumber`
//! - Manipulation: `str.split`, `str.trim`, `str.trim_left`, `str.trim_right`
//! - Formatting: `str.format`, `str.format_time`

use crate::core::indicators::runtime::value::RayValue;

/// String namespace builtin function dispatch.
pub fn call(name: &str, args: &[RayValue]) -> Option<RayValue> {
    match name {
        // Basic functions
        "length" => str_length(args),
        "substring" => str_substring(args),
        "replace" => str_replace(args),
        "replace_all" => str_replace_all(args),

        // Case functions
        "upper" | "toupper" => str_upper(args),
        "lower" | "tolower" => str_lower(args),

        // Search functions
        "contains" => str_contains(args),
        "startswith" => str_startswith(args),
        "endswith" => str_endswith(args),
        "pos" | "indexof" => str_pos(args),
        "match" => str_match(args),

        // Conversion functions
        "tostring" => str_tostring(args),
        "tonumber" => str_tonumber(args),

        // Manipulation functions
        "split" => str_split(args),
        "trim" => str_trim(args),
        "trim_left" | "ltrim" => str_trim_left(args),
        "trim_right" | "rtrim" => str_trim_right(args),
        "repeat" => str_repeat(args),
        "reverse" => str_reverse(args),
        "concat" => str_concat(args),

        // Character functions
        "char" | "char_at" => str_char_at(args),

        // Formatting functions
        "format" => str_format(args),
        "format_time" => str_format_time(args),

        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Basic Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// str.length(s) - Returns length of string
fn str_length(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::String(s)) => Some(RayValue::Number(s.chars().count() as f64)),
        _ => Some(RayValue::Na),
    }
}

/// str.substring(s, begin_pos, end_pos) - Returns substring
fn str_substring(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Na),
    };

    let begin = args
        .get(1)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);

    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();

    let end = args
        .get(2)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(len);

    let start = begin.min(len);
    let stop = end.min(len);

    if start >= stop {
        return Some(RayValue::String(String::new()));
    }

    let result: String = chars[start..stop].iter().collect();
    Some(RayValue::String(result))
}

/// str.replace(s, target, replacement, occurrence) - Replace first occurrence
fn str_replace(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Na),
    };

    let target = match args.get(1) {
        Some(RayValue::String(t)) => t,
        _ => return Some(RayValue::String(s.clone())),
    };

    let replacement = match args.get(2) {
        Some(RayValue::String(r)) => r,
        _ => return Some(RayValue::String(s.clone())),
    };

    // occurrence: which occurrence to replace (0-based), default 0
    let occurrence = args
        .get(3)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);

    // Find nth occurrence
    let mut result = s.clone();
    let mut count = 0;
    let mut search_from = 0;

    while let Some(pos) = result[search_from..].find(target.as_str()) {
        let abs_pos = search_from + pos;
        if count == occurrence {
            result = format!(
                "{}{}{}",
                &result[..abs_pos],
                replacement,
                &result[abs_pos + target.len()..]
            );
            break;
        }
        count += 1;
        search_from = abs_pos + target.len();
    }

    Some(RayValue::String(result))
}

/// str.replace_all(s, target, replacement) - Replace all occurrences
fn str_replace_all(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Na),
    };

    let target = match args.get(1) {
        Some(RayValue::String(t)) => t,
        _ => return Some(RayValue::String(s.clone())),
    };

    let replacement = match args.get(2) {
        Some(RayValue::String(r)) => r,
        _ => return Some(RayValue::String(s.clone())),
    };

    let result = s.replace(target.as_str(), replacement.as_str());
    Some(RayValue::String(result))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Case Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// str.upper(s) - Convert to uppercase
fn str_upper(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::String(s)) => Some(RayValue::String(s.to_uppercase())),
        _ => Some(RayValue::Na),
    }
}

/// str.lower(s) - Convert to lowercase
fn str_lower(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::String(s)) => Some(RayValue::String(s.to_lowercase())),
        _ => Some(RayValue::Na),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Search Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// str.contains(s, substr) - Check if string contains substring
fn str_contains(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Bool(false)),
    };

    let substr = match args.get(1) {
        Some(RayValue::String(t)) => t,
        _ => return Some(RayValue::Bool(false)),
    };

    Some(RayValue::Bool(s.contains(substr.as_str())))
}

/// str.startswith(s, substr) - Check if string starts with substring
fn str_startswith(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Bool(false)),
    };

    let substr = match args.get(1) {
        Some(RayValue::String(t)) => t,
        _ => return Some(RayValue::Bool(false)),
    };

    Some(RayValue::Bool(s.starts_with(substr.as_str())))
}

/// str.endswith(s, substr) - Check if string ends with substring
fn str_endswith(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Bool(false)),
    };

    let substr = match args.get(1) {
        Some(RayValue::String(t)) => t,
        _ => return Some(RayValue::Bool(false)),
    };

    Some(RayValue::Bool(s.ends_with(substr.as_str())))
}

/// str.pos(s, substr) - Find position of substring, -1 if not found
fn str_pos(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Number(-1.0)),
    };

    let substr = match args.get(1) {
        Some(RayValue::String(t)) => t,
        _ => return Some(RayValue::Number(-1.0)),
    };

    // Find position in character indices (not bytes)
    let chars: Vec<char> = s.chars().collect();
    let pattern: Vec<char> = substr.chars().collect();

    if pattern.is_empty() {
        return Some(RayValue::Number(0.0));
    }

    for i in 0..=chars.len().saturating_sub(pattern.len()) {
        if chars[i..i + pattern.len()] == pattern[..] {
            return Some(RayValue::Number(i as f64));
        }
    }

    Some(RayValue::Number(-1.0))
}

/// str.match(s, pattern) - Simple pattern matching (not full regex)
/// Returns true if pattern is found in string
fn str_match(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Bool(false)),
    };

    let pattern = match args.get(1) {
        Some(RayValue::String(p)) => p,
        _ => return Some(RayValue::Bool(false)),
    };

    // Simple contains check - full regex would require additional dependency
    Some(RayValue::Bool(s.contains(pattern.as_str())))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Conversion Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// str.tostring(value, format) - Convert value to string
fn str_tostring(args: &[RayValue]) -> Option<RayValue> {
    let value = args.first()?;
    let format = args.get(1).and_then(RayValue::as_string);

    match value {
        RayValue::Na => Some(RayValue::String("na".to_string())),
        RayValue::Number(n) => {
            if let Some(fmt) = format {
                // Parse format string like "#.##" or "0.00"
                let decimals = fmt
                    .chars()
                    .rev()
                    .take_while(|c| *c == '#' || *c == '0')
                    .count();
                Some(RayValue::String(format!("{:.prec$}", n, prec = decimals)))
            } else {
                Some(RayValue::String(n.to_string()))
            }
        }
        RayValue::Bool(b) => Some(RayValue::String(b.to_string())),
        RayValue::String(s) => Some(RayValue::String(s.clone())),
        RayValue::Color(c) => Some(RayValue::String(format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            c.r, c.g, c.b, c.a
        ))),
        RayValue::Tuple(elements) | RayValue::Array(elements) => {
            let parts: Vec<String> = elements
                .iter()
                .map(|e| {
                    str_tostring(&[e.clone()])
                        .and_then(|v| match v {
                            RayValue::String(s) => Some(s),
                            _ => None,
                        })
                        .unwrap_or_else(|| "na".to_string())
                })
                .collect();
            Some(RayValue::String(format!("[{}]", parts.join(", "))))
        }
        RayValue::Map(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    let key_str = str_tostring(&[k.clone()])
                        .and_then(|v| match v {
                            RayValue::String(s) => Some(s),
                            _ => None,
                        })
                        .unwrap_or_else(|| "na".to_string());
                    let val_str = str_tostring(&[v.clone()])
                        .and_then(|v| match v {
                            RayValue::String(s) => Some(s),
                            _ => None,
                        })
                        .unwrap_or_else(|| "na".to_string());
                    format!("{}: {}", key_str, val_str)
                })
                .collect();
            Some(RayValue::String(format!("{{{}}}", parts.join(", "))))
        }
    }
}

/// str.tonumber(s) - Convert string to number
fn str_tonumber(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        Some(RayValue::Number(n)) => return Some(RayValue::Number(*n)),
        _ => return Some(RayValue::Na),
    };

    match s.trim().parse::<f64>() {
        Ok(n) => Some(RayValue::Number(n)),
        Err(_) => Some(RayValue::Na),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Manipulation Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// str.split(s, separator) - Split string into array
fn str_split(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Array(vec![])),
    };

    let separator = match args.get(1) {
        Some(RayValue::String(sep)) => sep.as_str(),
        _ => ",",
    };

    let parts: Vec<RayValue> = s
        .split(separator)
        .map(|part| RayValue::String(part.to_string()))
        .collect();

    Some(RayValue::Array(parts))
}

/// str.trim(s) - Remove leading and trailing whitespace
fn str_trim(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::String(s)) => Some(RayValue::String(s.trim().to_string())),
        _ => Some(RayValue::Na),
    }
}

/// str.trim_left(s) - Remove leading whitespace
fn str_trim_left(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::String(s)) => Some(RayValue::String(s.trim_start().to_string())),
        _ => Some(RayValue::Na),
    }
}

/// str.trim_right(s) - Remove trailing whitespace
fn str_trim_right(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::String(s)) => Some(RayValue::String(s.trim_end().to_string())),
        _ => Some(RayValue::Na),
    }
}

/// str.repeat(s, count) - Repeat string count times
fn str_repeat(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Na),
    };

    let count = args
        .get(1)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(1);

    // Limit to reasonable size to prevent memory issues
    let count = count.min(10000);
    Some(RayValue::String(s.repeat(count)))
}

/// str.reverse(s) - Reverse string
fn str_reverse(args: &[RayValue]) -> Option<RayValue> {
    match args.first() {
        Some(RayValue::String(s)) => Some(RayValue::String(s.chars().rev().collect())),
        _ => Some(RayValue::Na),
    }
}

/// str.concat(s1, s2, ...) - Concatenate strings
fn str_concat(args: &[RayValue]) -> Option<RayValue> {
    let mut result = String::new();
    for arg in args {
        match arg {
            RayValue::String(s) => result.push_str(s),
            RayValue::Number(n) => result.push_str(&n.to_string()),
            RayValue::Bool(b) => result.push_str(&b.to_string()),
            _ => {}
        }
    }
    Some(RayValue::String(result))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Character Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// str.char(s, index) - Get character at index
fn str_char_at(args: &[RayValue]) -> Option<RayValue> {
    let s = match args.first() {
        Some(RayValue::String(s)) => s,
        _ => return Some(RayValue::Na),
    };

    let index = args
        .get(1)
        .and_then(RayValue::as_number)
        .map(|n| n as usize)
        .unwrap_or(0);

    match s.chars().nth(index) {
        Some(c) => Some(RayValue::String(c.to_string())),
        None => Some(RayValue::Na),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Formatting Functions
// ═══════════════════════════════════════════════════════════════════════════════

/// str.format(format_string, arg0, arg1, ...) - Format string with placeholders
/// Supports {0}, {1}, etc. placeholders
fn str_format(args: &[RayValue]) -> Option<RayValue> {
    let format_str = match args.first() {
        Some(RayValue::String(s)) => s.clone(),
        _ => return Some(RayValue::Na),
    };

    let mut result = format_str;

    // Replace {0}, {1}, etc. with corresponding arguments
    for (i, arg) in args.iter().skip(1).enumerate() {
        let placeholder = format!("{{{}}}", i);
        let replacement = match arg {
            RayValue::String(s) => s.clone(),
            RayValue::Number(n) => n.to_string(),
            RayValue::Bool(b) => b.to_string(),
            RayValue::Na => "na".to_string(),
            _ => arg.to_display_text().unwrap_or_else(|| "na".to_string()),
        };
        result = result.replace(&placeholder, &replacement);
    }

    Some(RayValue::String(result))
}

/// str.format_time(time, format, timezone) - Format timestamp
/// Simplified version - full implementation would need chrono
fn str_format_time(args: &[RayValue]) -> Option<RayValue> {
    let timestamp = args.first().and_then(RayValue::as_number)?;
    let _format = args
        .get(1)
        .and_then(RayValue::as_string)
        .unwrap_or("yyyy-MM-dd");

    // Convert milliseconds timestamp to basic ISO format
    // Full implementation would use chrono for proper formatting
    let seconds = (timestamp / 1000.0) as i64;
    let days_since_epoch = seconds / 86400;
    let time_of_day = seconds % 86400;

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let secs = time_of_day % 60;

    // Simplified date calculation (approximate, doesn't handle leap years properly)
    let year = 1970 + (days_since_epoch / 365);
    let day_of_year = days_since_epoch % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    let formatted = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hours, minutes, secs
    );

    Some(RayValue::String(formatted))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_length_returns_char_count() {
        let result = call("length", &[RayValue::String("hello".to_string())]);
        assert_eq!(result, Some(RayValue::Number(5.0)));
    }

    #[test]
    fn str_length_handles_unicode() {
        let result = call("length", &[RayValue::String("héllo".to_string())]);
        assert_eq!(result, Some(RayValue::Number(5.0)));
    }

    #[test]
    fn str_substring_extracts_range() {
        let result = call(
            "substring",
            &[
                RayValue::String("hello world".to_string()),
                RayValue::Number(0.0),
                RayValue::Number(5.0),
            ],
        );
        assert_eq!(result, Some(RayValue::String("hello".to_string())));
    }

    #[test]
    fn str_substring_to_end() {
        let result = call(
            "substring",
            &[RayValue::String("hello".to_string()), RayValue::Number(2.0)],
        );
        assert_eq!(result, Some(RayValue::String("llo".to_string())));
    }

    #[test]
    fn str_replace_replaces_first() {
        let result = call(
            "replace",
            &[
                RayValue::String("hello hello".to_string()),
                RayValue::String("hello".to_string()),
                RayValue::String("hi".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::String("hi hello".to_string())));
    }

    #[test]
    fn str_replace_all_replaces_all() {
        let result = call(
            "replace_all",
            &[
                RayValue::String("hello hello".to_string()),
                RayValue::String("hello".to_string()),
                RayValue::String("hi".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::String("hi hi".to_string())));
    }

    #[test]
    fn str_upper_converts() {
        let result = call("upper", &[RayValue::String("hello".to_string())]);
        assert_eq!(result, Some(RayValue::String("HELLO".to_string())));
    }

    #[test]
    fn str_lower_converts() {
        let result = call("lower", &[RayValue::String("HELLO".to_string())]);
        assert_eq!(result, Some(RayValue::String("hello".to_string())));
    }

    #[test]
    fn str_contains_finds_substring() {
        let result = call(
            "contains",
            &[
                RayValue::String("hello world".to_string()),
                RayValue::String("world".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::Bool(true)));

        let result = call(
            "contains",
            &[
                RayValue::String("hello".to_string()),
                RayValue::String("world".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::Bool(false)));
    }

    #[test]
    fn str_startswith_checks_prefix() {
        let result = call(
            "startswith",
            &[
                RayValue::String("hello world".to_string()),
                RayValue::String("hello".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::Bool(true)));
    }

    #[test]
    fn str_endswith_checks_suffix() {
        let result = call(
            "endswith",
            &[
                RayValue::String("hello world".to_string()),
                RayValue::String("world".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::Bool(true)));
    }

    #[test]
    fn str_pos_finds_position() {
        let result = call(
            "pos",
            &[
                RayValue::String("hello world".to_string()),
                RayValue::String("world".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::Number(6.0)));
    }

    #[test]
    fn str_pos_returns_neg1_if_not_found() {
        let result = call(
            "pos",
            &[
                RayValue::String("hello".to_string()),
                RayValue::String("world".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::Number(-1.0)));
    }

    #[test]
    fn str_tostring_converts_number() {
        let result = call("tostring", &[RayValue::Number(42.5)]);
        assert_eq!(result, Some(RayValue::String("42.5".to_string())));
    }

    #[test]
    fn str_tostring_formats_number() {
        let result = call(
            "tostring",
            &[
                RayValue::Number(42.567),
                RayValue::String("#.##".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::String("42.57".to_string())));
    }

    #[test]
    fn str_tonumber_parses() {
        let result = call("tonumber", &[RayValue::String("42.5".to_string())]);
        assert_eq!(result, Some(RayValue::Number(42.5)));
    }

    #[test]
    fn str_tonumber_returns_na_for_invalid() {
        let result = call("tonumber", &[RayValue::String("not a number".to_string())]);
        assert_eq!(result, Some(RayValue::Na));
    }

    #[test]
    fn str_split_creates_array() {
        let result = call(
            "split",
            &[
                RayValue::String("a,b,c".to_string()),
                RayValue::String(",".to_string()),
            ],
        );
        if let Some(RayValue::Array(parts)) = result {
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0], RayValue::String("a".to_string()));
            assert_eq!(parts[1], RayValue::String("b".to_string()));
            assert_eq!(parts[2], RayValue::String("c".to_string()));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn str_trim_removes_whitespace() {
        let result = call("trim", &[RayValue::String("  hello  ".to_string())]);
        assert_eq!(result, Some(RayValue::String("hello".to_string())));
    }

    #[test]
    fn str_trim_left_removes_leading() {
        let result = call("trim_left", &[RayValue::String("  hello  ".to_string())]);
        assert_eq!(result, Some(RayValue::String("hello  ".to_string())));
    }

    #[test]
    fn str_trim_right_removes_trailing() {
        let result = call("trim_right", &[RayValue::String("  hello  ".to_string())]);
        assert_eq!(result, Some(RayValue::String("  hello".to_string())));
    }

    #[test]
    fn str_repeat_repeats() {
        let result = call(
            "repeat",
            &[RayValue::String("ab".to_string()), RayValue::Number(3.0)],
        );
        assert_eq!(result, Some(RayValue::String("ababab".to_string())));
    }

    #[test]
    fn str_reverse_reverses() {
        let result = call("reverse", &[RayValue::String("hello".to_string())]);
        assert_eq!(result, Some(RayValue::String("olleh".to_string())));
    }

    #[test]
    fn str_concat_joins() {
        let result = call(
            "concat",
            &[
                RayValue::String("hello".to_string()),
                RayValue::String(" ".to_string()),
                RayValue::String("world".to_string()),
            ],
        );
        assert_eq!(result, Some(RayValue::String("hello world".to_string())));
    }

    #[test]
    fn str_char_at_gets_char() {
        let result = call(
            "char",
            &[RayValue::String("hello".to_string()), RayValue::Number(1.0)],
        );
        assert_eq!(result, Some(RayValue::String("e".to_string())));
    }

    #[test]
    fn str_format_replaces_placeholders() {
        let result = call(
            "format",
            &[
                RayValue::String("Hello {0}, you have {1} messages".to_string()),
                RayValue::String("User".to_string()),
                RayValue::Number(5.0),
            ],
        );
        assert_eq!(
            result,
            Some(RayValue::String(
                "Hello User, you have 5 messages".to_string()
            ))
        );
    }
}
