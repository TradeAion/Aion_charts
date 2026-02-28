use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub line: usize,
    pub column: usize,
    pub len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub hint: Option<String>,
    pub span: Option<SourceSpan>,
}
