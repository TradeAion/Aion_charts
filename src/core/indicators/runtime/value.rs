use serde::{Deserialize, Serialize};

/// RGBA color representation for indicator styling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RayColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RayColor {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn with_transparency(&self, transparency: u8) -> Self {
        // transparency: 0 = opaque, 100 = fully transparent
        let clamped = transparency.min(100) as u16;
        let alpha = 255u16.saturating_sub(clamped * 255 / 100) as u8;
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: alpha,
        }
    }

    pub fn to_rgba_array(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    /// Parse hex color like "#RRGGBB" or "#RRGGBBAA"
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::new(r, g, b, a))
            }
            _ => None,
        }
    }
}

// Named color constants
impl RayColor {
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const RED: Self = Self::rgb(255, 0, 0);
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    pub const BLUE: Self = Self::rgb(0, 0, 255);
    pub const YELLOW: Self = Self::rgb(255, 255, 0);
    pub const ORANGE: Self = Self::rgb(255, 165, 0);
    pub const PURPLE: Self = Self::rgb(128, 0, 128);
    pub const AQUA: Self = Self::rgb(0, 255, 255);
    pub const LIME: Self = Self::rgb(0, 255, 0);
    pub const MAROON: Self = Self::rgb(128, 0, 0);
    pub const NAVY: Self = Self::rgb(0, 0, 128);
    pub const OLIVE: Self = Self::rgb(128, 128, 0);
    pub const SILVER: Self = Self::rgb(192, 192, 192);
    pub const GRAY: Self = Self::rgb(128, 128, 128);
    pub const TEAL: Self = Self::rgb(0, 128, 128);
    pub const FUCHSIA: Self = Self::rgb(255, 0, 255);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RayValue {
    Na,
    Number(f64),
    Bool(bool),
    String(String),
    Color(RayColor),
    /// Tuple for multi-value returns (e.g., ta.macd returns [macd, signal, histogram])
    Tuple(Vec<RayValue>),
    /// Array collection type for array.* operations
    Array(Vec<RayValue>),
    /// Map collection type for map.* operations (stored as Vec of key-value pairs)
    Map(Vec<(RayValue, RayValue)>),
}

impl RayValue {
    pub fn from_optional_number(value: Option<f64>) -> Self {
        match value {
            Some(number) => Self::Number(number),
            None => Self::Na,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Na => None,
            Self::Number(value) => Some(*value),
            Self::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
            Self::String(_) | Self::Color(_) | Self::Tuple(_) | Self::Array(_) | Self::Map(_) => {
                None
            }
        }
    }

    pub fn as_color(&self) -> Option<&RayColor> {
        match self {
            Self::Color(color) => Some(color),
            _ => None,
        }
    }

    /// Get tuple element at index (0-based)
    pub fn get_tuple_element(&self, index: usize) -> Option<&RayValue> {
        match self {
            Self::Tuple(elements) => elements.get(index),
            _ => None,
        }
    }

    /// Get tuple length
    pub fn tuple_len(&self) -> Option<usize> {
        match self {
            Self::Tuple(elements) => Some(elements.len()),
            _ => None,
        }
    }

    pub fn is_na(&self) -> bool {
        matches!(self, Self::Na)
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Na => false,
            Self::Bool(value) => *value,
            Self::Number(value) => value.abs() > f64::EPSILON,
            Self::String(value) => !value.is_empty(),
            Self::Color(_) => true,
            Self::Tuple(elements) => !elements.is_empty(),
            Self::Array(elements) => !elements.is_empty(),
            Self::Map(entries) => !entries.is_empty(),
        }
    }

    /// Returns the type name of the value (for error messages and introspection)
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Na => "na",
            Self::Number(_) => "float",
            Self::Bool(_) => "bool",
            Self::String(_) => "string",
            Self::Color(_) => "color",
            Self::Tuple(_) => "tuple",
            Self::Array(_) => "array",
            Self::Map(_) => "map",
        }
    }

    pub fn to_display_text(&self) -> Option<String> {
        match self {
            Self::Na => None,
            Self::Number(value) => Some(value.to_string()),
            Self::Bool(value) => Some(value.to_string()),
            Self::String(value) => Some(value.clone()),
            Self::Color(color) => Some(format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)),
            Self::Tuple(elements) | Self::Array(elements) => {
                let parts: Vec<String> = elements
                    .iter()
                    .map(|e| e.to_display_text().unwrap_or_else(|| "na".to_string()))
                    .collect();
                Some(format!("[{}]", parts.join(", ")))
            }
            Self::Map(entries) => {
                let parts: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| {
                        let key_str = k.to_display_text().unwrap_or_else(|| "na".to_string());
                        let val_str = v.to_display_text().unwrap_or_else(|| "na".to_string());
                        format!("{}: {}", key_str, val_str)
                    })
                    .collect();
                Some(format!("{{{}}}", parts.join(", ")))
            }
        }
    }

    /// Get array element at index (0-based)
    pub fn get_array_element(&self, index: usize) -> Option<&RayValue> {
        match self {
            Self::Array(elements) => elements.get(index),
            _ => None,
        }
    }

    /// Get array length
    pub fn array_len(&self) -> Option<usize> {
        match self {
            Self::Array(elements) => Some(elements.len()),
            _ => None,
        }
    }

    /// Get map value by key
    pub fn get_map_value(&self, key: &RayValue) -> Option<&RayValue> {
        match self {
            Self::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Get map size
    pub fn map_size(&self) -> Option<usize> {
        match self {
            Self::Map(entries) => Some(entries.len()),
            _ => None,
        }
    }

    /// Convert value to string representation
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Convert value to bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::Number(n) => Some(n.abs() > f64::EPSILON),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RayColor, RayValue};

    #[test]
    fn converts_bool_to_numeric_compatibly() {
        assert_eq!(RayValue::Bool(true).as_number(), Some(1.0));
        assert_eq!(RayValue::Bool(false).as_number(), Some(0.0));
    }

    #[test]
    fn color_from_hex_parses_rgb() {
        let color = RayColor::from_hex("#FF8000").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 0);
        assert_eq!(color.a, 255);
    }

    #[test]
    fn color_from_hex_parses_rgba() {
        let color = RayColor::from_hex("#FF800080").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 0);
        assert_eq!(color.a, 128);
    }

    #[test]
    fn color_with_transparency_applies_percent() {
        let color = RayColor::WHITE.with_transparency(50);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 255);
        assert_eq!(color.b, 255);
        assert!(color.a < 255 && color.a > 0);
    }

    #[test]
    fn color_value_is_truthy() {
        assert!(RayValue::Color(RayColor::RED).is_truthy());
    }

    #[test]
    fn color_display_text_formats_hex() {
        let color = RayValue::Color(RayColor::rgb(255, 128, 0));
        assert_eq!(color.to_display_text(), Some("#FF8000".to_string()));
    }

    // Array tests
    #[test]
    fn array_is_truthy_when_non_empty() {
        let arr = RayValue::Array(vec![RayValue::Number(1.0)]);
        assert!(arr.is_truthy());
    }

    #[test]
    fn array_is_falsy_when_empty() {
        let arr = RayValue::Array(vec![]);
        assert!(!arr.is_truthy());
    }

    #[test]
    fn array_type_name() {
        let arr = RayValue::Array(vec![]);
        assert_eq!(arr.type_name(), "array");
    }

    #[test]
    fn array_display_text() {
        let arr = RayValue::Array(vec![
            RayValue::Number(1.0),
            RayValue::Number(2.0),
            RayValue::Number(3.0),
        ]);
        assert_eq!(arr.to_display_text(), Some("[1, 2, 3]".to_string()));
    }

    #[test]
    fn array_get_element() {
        let arr = RayValue::Array(vec![RayValue::Number(10.0), RayValue::Number(20.0)]);
        assert_eq!(arr.get_array_element(0), Some(&RayValue::Number(10.0)));
        assert_eq!(arr.get_array_element(1), Some(&RayValue::Number(20.0)));
        assert_eq!(arr.get_array_element(2), None);
    }

    #[test]
    fn array_len() {
        let arr = RayValue::Array(vec![RayValue::Number(1.0), RayValue::Number(2.0)]);
        assert_eq!(arr.array_len(), Some(2));
        assert_eq!(RayValue::Number(1.0).array_len(), None);
    }

    // Map tests
    #[test]
    fn map_is_truthy_when_non_empty() {
        let map = RayValue::Map(vec![(
            RayValue::String("key".to_string()),
            RayValue::Number(1.0),
        )]);
        assert!(map.is_truthy());
    }

    #[test]
    fn map_is_falsy_when_empty() {
        let map = RayValue::Map(vec![]);
        assert!(!map.is_truthy());
    }

    #[test]
    fn map_type_name() {
        let map = RayValue::Map(vec![]);
        assert_eq!(map.type_name(), "map");
    }

    #[test]
    fn map_display_text() {
        let map = RayValue::Map(vec![
            (RayValue::String("a".to_string()), RayValue::Number(1.0)),
            (RayValue::String("b".to_string()), RayValue::Number(2.0)),
        ]);
        assert_eq!(map.to_display_text(), Some("{a: 1, b: 2}".to_string()));
    }

    #[test]
    fn map_get_value() {
        let map = RayValue::Map(vec![
            (
                RayValue::String("key1".to_string()),
                RayValue::Number(100.0),
            ),
            (
                RayValue::String("key2".to_string()),
                RayValue::Number(200.0),
            ),
        ]);
        assert_eq!(
            map.get_map_value(&RayValue::String("key1".to_string())),
            Some(&RayValue::Number(100.0))
        );
        assert_eq!(
            map.get_map_value(&RayValue::String("key3".to_string())),
            None
        );
    }

    #[test]
    fn map_size() {
        let map = RayValue::Map(vec![(
            RayValue::String("a".to_string()),
            RayValue::Number(1.0),
        )]);
        assert_eq!(map.map_size(), Some(1));
        assert_eq!(RayValue::Number(1.0).map_size(), None);
    }

    #[test]
    fn type_names_are_correct() {
        assert_eq!(RayValue::Na.type_name(), "na");
        assert_eq!(RayValue::Number(1.0).type_name(), "float");
        assert_eq!(RayValue::Bool(true).type_name(), "bool");
        assert_eq!(RayValue::String("test".to_string()).type_name(), "string");
        assert_eq!(RayValue::Color(RayColor::RED).type_name(), "color");
        assert_eq!(RayValue::Tuple(vec![]).type_name(), "tuple");
    }

    #[test]
    fn as_string_works() {
        assert_eq!(
            RayValue::String("hello".to_string()).as_string(),
            Some("hello")
        );
        assert_eq!(RayValue::Number(1.0).as_string(), None);
    }

    #[test]
    fn as_bool_works() {
        assert_eq!(RayValue::Bool(true).as_bool(), Some(true));
        assert_eq!(RayValue::Bool(false).as_bool(), Some(false));
        assert_eq!(RayValue::Number(1.0).as_bool(), Some(true));
        assert_eq!(RayValue::Number(0.0).as_bool(), Some(false));
        assert_eq!(RayValue::String("test".to_string()).as_bool(), None);
    }
}
