pub const RAYDSL_NAME: &str = "raydsl";
pub const RAYDSL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileMode {
    RayDslV1,
    RayDslV2,
}

impl CompileMode {
    pub fn as_version(self) -> u32 {
        match self {
            Self::RayDslV1 => 1,
            Self::RayDslV2 => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompileModeParseWarning {
    pub line: usize,
    pub column: usize,
    pub len: usize,
    pub message: String,
    pub hint: String,
}

#[derive(Debug, Clone)]
pub struct CompileModeParseResult {
    pub mode: CompileMode,
    pub warning: Option<CompileModeParseWarning>,
}

pub fn parse_compile_mode(source: &str) -> CompileModeParseResult {
    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(version_raw) = trimmed.strip_prefix("//@version=") {
            let version_token = version_raw.trim();
            if version_token == "1" {
                return CompileModeParseResult {
                    mode: CompileMode::RayDslV1,
                    warning: None,
                };
            }
            if version_token == "2" {
                return CompileModeParseResult {
                    mode: CompileMode::RayDslV2,
                    warning: None,
                };
            }
            return CompileModeParseResult {
                mode: CompileMode::RayDslV1,
                warning: Some(CompileModeParseWarning {
                    line: line_no,
                    column: 1,
                    len: trimmed.len(),
                    message: format!(
                        "unsupported RayDSL version '{}'; falling back to version 1",
                        version_token
                    ),
                    hint: "use //@version=1 or //@version=2".to_string(),
                }),
            };
        }
        if trimmed.starts_with("//") {
            continue;
        }
        break;
    }
    CompileModeParseResult {
        mode: CompileMode::RayDslV1,
        warning: None,
    }
}

pub fn normalize_source(source: &str) -> String {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{parse_compile_mode, CompileMode};

    #[test]
    fn parse_compile_mode_defaults_to_v1() {
        let parsed = parse_compile_mode("indicator(\"t\")\nplot(close)");
        assert_eq!(parsed.mode, CompileMode::RayDslV1);
        assert!(parsed.warning.is_none());
    }

    #[test]
    fn parse_compile_mode_accepts_v2_header() {
        let parsed = parse_compile_mode("//@version=2\nindicator(\"t\")\nplot(close)");
        assert_eq!(parsed.mode, CompileMode::RayDslV2);
        assert!(parsed.warning.is_none());
    }

    #[test]
    fn parse_compile_mode_warns_on_invalid_header() {
        let parsed = parse_compile_mode("//@version=99\nindicator(\"t\")\nplot(close)");
        assert_eq!(parsed.mode, CompileMode::RayDslV1);
        assert!(parsed.warning.is_some());
    }
}
