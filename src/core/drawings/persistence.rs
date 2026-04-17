use super::types::{AnchorPoint, DrawingPoint, DrawingStyle, DrawingTool};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

pub const DRAWINGS_SNAPSHOT_VERSION: u32 = 1;

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
    /// Fibonacci label alignment ("left", "center", "right"). Omitted for other tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_align: Option<String>,
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
    _payload: serde_json::Value,
) -> Result<serde_json::Value, DrawingsMigrationError> {
    match from_version {
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
                label_align: None,
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
}
