use super::types::{AnchorPoint, DrawingPoint, DrawingStyle, DrawingTool, FibonacciLevel};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

pub const DRAWINGS_SNAPSHOT_VERSION: u32 = 3;

fn snapshot_version() -> u32 {
    DRAWINGS_SNAPSHOT_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DrawingSnapshot {
    #[serde(default = "snapshot_version")]
    pub version: u32,
    #[serde(default)]
    pub drawings: Vec<SerializedDrawing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedDrawing {
    #[serde(default)]
    pub id: u64,
    pub tool: String,
    #[serde(default)]
    pub style: SerializedDrawingStyle,
    #[serde(default)]
    pub anchors: Vec<SerializedAnchorPoint>,
    /// Brush intermediate points (excluding first/last anchors).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<SerializedDrawingPoint>,
    /// Drawing label text for text-capable tools. Omitted when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Horizontal alignment for drawing labels / fibonacci level labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal_align: Option<String>,
    /// Vertical alignment for drawing labels / fibonacci level labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>,
    /// Optional text font size override in CSS px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_font_size: Option<f64>,
    /// Optional italic override for drawing/fibonacci labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_italic: Option<bool>,
    /// Optional text color override [r, g, b, a].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<[f32; 4]>,
    /// Custom Fibonacci levels. Omitted for non-fibonacci drawings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fibonacci_levels: Vec<SerializedFibonacciLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedAnchorPoint {
    pub point: SerializedDrawingPoint,
    #[serde(default = "default_hit_radius")]
    pub hit_radius: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SerializedDrawingPoint {
    pub bar_index: f64,
    pub price: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedDrawingStyle {
    pub color: [f32; 4],
    pub line_width: f64,
    pub fill_color: Option<[f32; 4]>,
    pub dash: Option<[f64; 2]>,
    pub font_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedFibonacciLevel {
    pub ratio: f64,
    pub label: String,
}

impl Default for SerializedDrawingStyle {
    fn default() -> Self {
        (&DrawingStyle::default()).into()
    }
}

fn default_hit_radius() -> f64 {
    5.0
}

impl From<&DrawingStyle> for SerializedDrawingStyle {
    fn from(value: &DrawingStyle) -> Self {
        Self {
            color: value.color,
            line_width: value.line_width,
            fill_color: value.fill_color,
            dash: value.dash,
            font_size: value.font_size,
        }
    }
}

impl From<SerializedDrawingStyle> for DrawingStyle {
    fn from(value: SerializedDrawingStyle) -> Self {
        Self {
            color: value.color,
            line_width: value.line_width,
            fill_color: value.fill_color,
            dash: value.dash,
            font_size: value.font_size,
        }
    }
}

impl From<DrawingPoint> for SerializedDrawingPoint {
    fn from(value: DrawingPoint) -> Self {
        Self {
            bar_index: value.bar_index,
            price: value.price,
            timestamp: value.timestamp,
        }
    }
}

impl From<&DrawingPoint> for SerializedDrawingPoint {
    fn from(value: &DrawingPoint) -> Self {
        Self {
            bar_index: value.bar_index,
            price: value.price,
            timestamp: value.timestamp,
        }
    }
}

impl From<SerializedDrawingPoint> for DrawingPoint {
    fn from(value: SerializedDrawingPoint) -> Self {
        Self {
            bar_index: value.bar_index,
            price: value.price,
            timestamp: value.timestamp,
        }
    }
}

impl From<&AnchorPoint> for SerializedAnchorPoint {
    fn from(value: &AnchorPoint) -> Self {
        Self {
            point: SerializedDrawingPoint::from(value.point),
            hit_radius: value.hit_radius,
        }
    }
}

impl From<SerializedAnchorPoint> for AnchorPoint {
    fn from(value: SerializedAnchorPoint) -> Self {
        AnchorPoint {
            point: value.point.into(),
            hit_radius: value.hit_radius,
        }
    }
}

impl From<&FibonacciLevel> for SerializedFibonacciLevel {
    fn from(value: &FibonacciLevel) -> Self {
        Self {
            ratio: value.ratio,
            label: value.label.clone(),
        }
    }
}

impl From<SerializedFibonacciLevel> for FibonacciLevel {
    fn from(value: SerializedFibonacciLevel) -> Self {
        FibonacciLevel {
            ratio: value.ratio,
            label: value.label,
        }
    }
}

#[derive(Debug)]
pub enum DrawingsMigrationError {
    UnknownVersion(u32),
    Incompatible { from: u32, to: u32 },
    SerdeError(serde_json::Error),
}

impl fmt::Display for DrawingsMigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownVersion(version) => {
                write!(f, "unknown drawing snapshot version {version}")
            }
            Self::Incompatible { from, to } => write!(
                f,
                "drawing snapshot version {from} is newer than supported version {to}"
            ),
            Self::SerdeError(err) => write!(f, "drawing snapshot serde error: {err}"),
        }
    }
}

impl Error for DrawingsMigrationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SerdeError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for DrawingsMigrationError {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeError(value)
    }
}

fn read_snapshot_version(payload: &serde_json::Value) -> Result<u32, DrawingsMigrationError> {
    let raw_version = payload
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(DrawingsMigrationError::UnknownVersion(0))?;

    if raw_version == 0 || raw_version > u32::MAX as u64 {
        return Err(DrawingsMigrationError::UnknownVersion(
            raw_version.min(u32::MAX as u64) as u32,
        ));
    }

    Ok(raw_version as u32)
}

fn apply_snapshot_migration_step(
    from_version: u32,
    mut payload: serde_json::Value,
) -> Result<serde_json::Value, DrawingsMigrationError> {
    match from_version {
        1 => {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert(
                    "version".to_string(),
                    serde_json::Value::from((from_version + 1) as u64),
                );
            }
            Ok(payload)
        }
        2 => {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert(
                    "version".to_string(),
                    serde_json::Value::from((from_version + 1) as u64),
                );
            }
            Ok(payload)
        }
        // Future migrations should be added here as `vN -> vN+1` transforms.
        _ => Err(DrawingsMigrationError::UnknownVersion(from_version)),
    }
}

pub fn migrate_snapshot(
    payload: &serde_json::Value,
) -> Result<DrawingSnapshot, DrawingsMigrationError> {
    let version = read_snapshot_version(payload)?;
    if version > DRAWINGS_SNAPSHOT_VERSION {
        return Err(DrawingsMigrationError::Incompatible {
            from: version,
            to: DRAWINGS_SNAPSHOT_VERSION,
        });
    }

    let mut migrated = payload.clone();
    let mut current_version = version;
    while current_version < DRAWINGS_SNAPSHOT_VERSION {
        migrated = apply_snapshot_migration_step(current_version, migrated)?;
        current_version += 1;
    }

    Ok(serde_json::from_value(migrated)?)
}

pub fn drawing_tool_to_key(tool: DrawingTool) -> &'static str {
    tool.as_api_key()
}

pub fn drawing_tool_from_key(key: &str) -> Option<DrawingTool> {
    DrawingTool::from_api_key(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_snapshot() -> DrawingSnapshot {
        DrawingSnapshot {
            version: DRAWINGS_SNAPSHOT_VERSION,
            drawings: vec![SerializedDrawing {
                id: 7,
                tool: "trend_line".to_string(),
                style: SerializedDrawingStyle::default(),
                anchors: vec![
                    SerializedAnchorPoint {
                        point: SerializedDrawingPoint {
                            bar_index: 10.0,
                            price: 100.25,
                            timestamp: Some(1_700_000_000_000),
                        },
                        hit_radius: 5.0,
                    },
                    SerializedAnchorPoint {
                        point: SerializedDrawingPoint {
                            bar_index: 20.0,
                            price: 110.75,
                            timestamp: Some(1_700_000_060_000),
                        },
                        hit_radius: 5.0,
                    },
                ],
                points: Vec::new(),
                text: None,
                horizontal_align: None,
                vertical_align: None,
                text_font_size: None,
                text_italic: None,
                text_color: None,
                fibonacci_levels: Vec::new(),
            }],
        }
    }

    #[test]
    fn current_snapshot_round_trips_through_migration_api() {
        let snapshot = sample_snapshot();
        let payload = serde_json::to_value(&snapshot).unwrap();

        let migrated = migrate_snapshot(&payload).unwrap();

        assert_eq!(migrated, snapshot);
    }

    #[test]
    fn unknown_snapshot_version_returns_unknown_version() {
        let payload = json!({
            "version": 0,
            "drawings": [],
        });

        let err = migrate_snapshot(&payload).unwrap_err();

        assert!(matches!(err, DrawingsMigrationError::UnknownVersion(0)));
    }

    #[test]
    fn future_snapshot_version_returns_incompatible() {
        let payload = json!({
            "version": DRAWINGS_SNAPSHOT_VERSION + 1,
            "drawings": [],
        });

        let err = migrate_snapshot(&payload).unwrap_err();

        assert!(matches!(
            err,
            DrawingsMigrationError::Incompatible {
                from,
                to: DRAWINGS_SNAPSHOT_VERSION,
            } if from == DRAWINGS_SNAPSHOT_VERSION + 1
        ));
    }

    #[test]
    fn v1_snapshot_migrates_with_legacy_label_align_and_defaults_new_fields() {
        let payload = json!({
            "version": 1,
            "drawings": [{
                "id": 9,
                "tool": "fibonacci",
                "style": SerializedDrawingStyle::default(),
                "anchors": [{
                    "point": { "bar_index": 10.0, "price": 100.0, "timestamp": null },
                    "hit_radius": 5.0
                }, {
                    "point": { "bar_index": 20.0, "price": 120.0, "timestamp": null },
                    "hit_radius": 5.0
                }],
                "points": [],
                "label_align": "right"
            }]
        });

        let migrated = migrate_snapshot(&payload).expect("migrate v1 snapshot");

        assert_eq!(migrated.version, DRAWINGS_SNAPSHOT_VERSION);
        assert_eq!(migrated.drawings.len(), 1);
        assert_eq!(migrated.drawings[0].horizontal_align, None);
        assert_eq!(migrated.drawings[0].vertical_align, None);
        assert_eq!(migrated.drawings[0].text_font_size, None);
        assert_eq!(migrated.drawings[0].text_italic, None);
        assert_eq!(migrated.drawings[0].text_color, None);
        assert!(migrated.drawings[0].fibonacci_levels.is_empty());
    }

    #[test]
    fn v2_snapshot_migrates_and_defaults_text_style_fields() {
        let payload = json!({
            "version": 2,
            "drawings": [{
                "id": 11,
                "tool": "trend_line",
                "style": SerializedDrawingStyle::default(),
                "anchors": [{
                    "point": { "bar_index": 10.0, "price": 100.0, "timestamp": null },
                    "hit_radius": 5.0
                }, {
                    "point": { "bar_index": 20.0, "price": 120.0, "timestamp": null },
                    "hit_radius": 5.0
                }],
                "points": [],
                "text": "Dev",
                "horizontal_align": "left",
                "vertical_align": "middle"
            }]
        });

        let migrated = migrate_snapshot(&payload).expect("migrate v2 snapshot");

        assert_eq!(migrated.version, DRAWINGS_SNAPSHOT_VERSION);
        assert_eq!(migrated.drawings[0].text_font_size, None);
        assert_eq!(migrated.drawings[0].text_italic, None);
        assert_eq!(migrated.drawings[0].text_color, None);
    }
}
