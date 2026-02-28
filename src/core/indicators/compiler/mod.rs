pub mod ast;
pub mod diagnostics;
pub mod lexer;
pub mod lower_ir;
pub mod parser;
pub mod typecheck;
pub mod types;

use crate::core::indicators::compiler::ast::AstProgram;
use crate::core::indicators::compiler::diagnostics::{
    CompileDiagnostic, DiagnosticSeverity, SourceSpan,
};
use crate::core::indicators::compiler::lower_ir::lower_to_ir;
use crate::core::indicators::compiler::parser::parse_program;
use crate::core::indicators::compiler::typecheck::typecheck_program;
use crate::core::indicators::language::normalize_source;
use crate::core::indicators::{
    IndicatorProgram, InputSchemaField, IrCall, OpCode, OutputSchemaField, ResourceDecl,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct CompileOutput {
    pub normalized_source: String,
    pub source_hash: String,
    pub program: Option<IndicatorProgram>,
    pub diagnostics: Vec<CompileDiagnostic>,
}

pub fn compile_source(
    source: &str,
    ir_version: u32,
    stdlib_version: u32,
    feature_flags: &[String],
) -> CompileOutput {
    let normalized_source = normalize_source(source);
    let source_hash = source_sha256(&normalized_source);
    let mut diagnostics = Vec::new();

    if normalized_source.is_empty() {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1000".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "indicator source is empty".to_string(),
            hint: Some("provide at least an indicator declaration".to_string()),
            span: Some(SourceSpan {
                line: 1,
                column: 1,
                len: 0,
            }),
        });
        return CompileOutput {
            normalized_source,
            source_hash,
            program: None,
            diagnostics,
        };
    }

    let tokens = lexer::lex(&normalized_source, &mut diagnostics);
    let ast: Option<AstProgram> = parse_program(&tokens, &normalized_source, &mut diagnostics);
    if let Some(ref ast_program) = ast {
        typecheck_program(ast_program, &normalized_source, &mut diagnostics);
    }
    if diagnostics
        .iter()
        .any(|d| matches!(d.severity, DiagnosticSeverity::Error))
    {
        return CompileOutput {
            normalized_source,
            source_hash,
            program: None,
            diagnostics,
        };
    }

    let (opcodes, ir_calls): (Vec<OpCode>, Vec<IrCall>) = if let Some(ref ast_program) = ast {
        let lowered = lower_to_ir(ast_program);
        (lowered.opcodes, lowered.calls)
    } else {
        (vec![OpCode::Nop, OpCode::Halt], Vec::new())
    };

    let program_name = ast
        .as_ref()
        .and_then(|p| p.name.clone())
        .unwrap_or_else(|| "untitled".to_string());
    let input_schema = ast
        .as_ref()
        .map(|p| p.inputs.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|i| InputSchemaField {
            name: i.name,
            type_name: i.type_name,
            default_value: i.default_value,
        })
        .collect::<Vec<_>>();
    let output_schema = vec![OutputSchemaField {
        name: "main".to_string(),
        output_type: "series".to_string(),
    }];
    let resource_decl = ResourceDecl {
        max_objects: 1000,
        max_vertices_per_frame: 2_000_000,
    };

    let program = IndicatorProgram {
        program_id: 0,
        name: program_name,
        ir_version,
        stdlib_version,
        source_hash: source_hash.clone(),
        feature_flags: feature_flags.to_vec(),
        constants: Vec::new(),
        opcodes,
        ir_calls,
        input_schema,
        output_schema,
        resource_decl,
    };

    CompileOutput {
        normalized_source,
        source_hash,
        program: Some(program),
        diagnostics,
    }
}

fn source_sha256(normalized_source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalized_source.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::compile_source;
    use crate::core::indicators::{INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION};

    #[test]
    fn allows_comment_lines() {
        let output = compile_source(
            "indicator(\"t\")\n// comment\nplot(close)",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_some(), "expected program to compile");
        assert!(
            !output
                .diagnostics
                .iter()
                .any(|d| matches!(d.severity, super::DiagnosticSeverity::Error)),
            "expected no compile errors"
        );
    }

    #[test]
    fn rejects_unsupported_raw_statement() {
        let output = compile_source(
            "indicator(\"t\")\nwhile true",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_none(), "expected compile failure");
        assert!(
            output.diagnostics.iter().any(|d| d.code == "INDL-1102"),
            "expected INDL-1102 diagnostic"
        );
    }

    #[test]
    fn accepts_typed_statements_and_functions() {
        let output = compile_source(
            "indicator(\"t\")\nvar x = close\nlet y = open\nfn myPlot() {\n  plot(close)\n}\nif true {\n  myPlot()\n}\nfor i = 0 to 2 {\n  plot(volume)\n}",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_some(), "expected compile success");
        assert!(
            !output
                .diagnostics
                .iter()
                .any(|d| matches!(d.severity, super::DiagnosticSeverity::Error)),
            "expected no compile errors"
        );
    }

    #[test]
    fn accepts_inline_else_block_closure_style() {
        let output = compile_source(
            "indicator(\"t\")\nvar x = na\nif close > open {\n  x = high\n} else {\n  x = low\n}\nplot(x)",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_some(), "expected compile success");
        assert!(
            !output
                .diagnostics
                .iter()
                .any(|d| matches!(d.severity, super::DiagnosticSeverity::Error)),
            "expected no compile errors"
        );
    }
}
