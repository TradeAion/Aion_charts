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
use crate::core::indicators::language::{normalize_source, parse_compile_mode};
use crate::core::indicators::{
    IndicatorDeclMeta, IndicatorProgram, InputSchemaField, IrCall, OpCode, OutputSchemaField,
    ResourceDecl,
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
    let compile_mode = parse_compile_mode(source);
    let mut diagnostics = Vec::new();
    if let Some(warning) = compile_mode.warning {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1010".to_string(),
            severity: DiagnosticSeverity::Warning,
            message: warning.message,
            hint: Some(warning.hint),
            span: Some(SourceSpan {
                line: warning.line,
                column: warning.column,
                len: warning.len,
            }),
        });
    }
    let normalized_source = normalize_source(source);
    let source_hash = source_sha256(&normalized_source);
    let compile_mode_key = format!("raydsl_v{}", compile_mode.mode.as_version());

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

    let (opcodes, ir_calls, indicator_decl): (Vec<OpCode>, Vec<IrCall>, _) =
        if let Some(ref ast_program) = ast {
            let lowered = lower_to_ir(ast_program, compile_mode.mode);
            // Collect diagnostics from the lowering pass (BUG-1, BUG-7 fixes)
            diagnostics.extend(lowered.diagnostics);
            (lowered.opcodes, lowered.calls, Some(lowered.indicator_decl))
        } else {
            (vec![OpCode::Nop, OpCode::Halt], Vec::new(), None)
        };

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

    // Convert AST IndicatorDecl to serializable IndicatorDeclMeta
    let indicator_meta = indicator_decl
        .map(|d| IndicatorDeclMeta {
            title: d.title,
            shorttitle: d.shorttitle,
            overlay: d.overlay,
            format: d.format,
            precision: d.precision,
            scale: d.scale,
            max_bars_back: d.max_bars_back,
            timeframe: d.timeframe,
            timeframe_gaps: d.timeframe_gaps,
            dynamic_requests: d.dynamic_requests,
            calc_on_every_tick: d.calc_on_every_tick,
            max_labels_count: d.max_labels_count,
            max_lines_count: d.max_lines_count,
            max_boxes_count: d.max_boxes_count,
            max_tables_count: d.max_tables_count,
            max_polylines_count: d.max_polylines_count,
        })
        .unwrap_or_default();

    let program = IndicatorProgram {
        program_id: 0,
        name: program_name,
        compile_mode: compile_mode_key,
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
        indicator_meta,
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
    use crate::core::indicators::{
        IrCallArg, IrCallKind, IrExpr, INDICATOR_IR_VERSION, INDICATOR_STDLIB_VERSION,
    };
    use std::collections::HashSet;

    fn contains_returned_flag(expr: &IrExpr) -> bool {
        match expr {
            IrExpr::Var(name) => name.starts_with("__fn_returned_"),
            IrExpr::VarIndexed { name, index } => {
                name.starts_with("__fn_returned_") || contains_returned_flag(index)
            }
            IrExpr::UnaryNot(inner) | IrExpr::UnaryNeg(inner) => contains_returned_flag(inner),
            IrExpr::Binary { lhs, rhs, .. } => {
                contains_returned_flag(lhs) || contains_returned_flag(rhs)
            }
            IrExpr::Conditional {
                condition,
                then_expr,
                else_expr,
            } => {
                contains_returned_flag(condition)
                    || contains_returned_flag(then_expr)
                    || contains_returned_flag(else_expr)
            }
            IrExpr::Series { index, .. } => index
                .as_ref()
                .map(|inner| contains_returned_flag(inner))
                .unwrap_or(false),
            IrExpr::ReqSeries { index, .. } => index
                .as_ref()
                .map(|inner| contains_returned_flag(inner))
                .unwrap_or(false),
            IrExpr::FnCall { args, .. } => args.iter().any(contains_returned_flag),
            IrExpr::Bool(_) | IrExpr::Number(_) | IrExpr::Na | IrExpr::Color { .. } => false,
        }
    }

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
            "indicator(\"t\")\nrepeat close",
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
    fn accepts_while_loops() {
        let output = compile_source(
            "indicator(\"t\")\nlet x = 0\nwhile x < 2 {\n  x = x + 1\n}\nplot(x)",
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
    fn accepts_switch_statements() {
        let output = compile_source(
            "indicator(\"t\")\nlet x = close\nswitch close {\n  case open {\n    x = high\n  }\n  case high {\n    x = low\n  }\n  default {\n    x = close\n  }\n}\nplot(x)",
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

    #[test]
    fn accepts_else_if_chains() {
        let output = compile_source(
            "indicator(\"t\")\nvar x = na\nif close > open {\n  x = high\n} else if close < open {\n  x = low\n} else {\n  x = close\n}\nplot(x)",
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
    fn accepts_ternary_expressions() {
        let output = compile_source(
            "indicator(\"t\")\nlet x = close > open ? high : low\nplot(x)",
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
    fn rejects_unknown_function_call() {
        let output = compile_source(
            "indicator(\"t\")\nplot(close)\nunknown_fn(close)",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_none(), "expected compile failure");
        assert!(
            output.diagnostics.iter().any(|d| d.code == "INDL-1300"),
            "expected INDL-1300 diagnostic"
        );
    }

    #[test]
    fn rejects_function_arity_mismatch() {
        let output = compile_source(
            "indicator(\"t\")\nfn myFn(a, b) {\n  plot(a)\n}\nmyFn(close)",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_none(), "expected compile failure");
        assert!(
            output.diagnostics.iter().any(|d| d.code == "INDL-1301"),
            "expected INDL-1301 diagnostic"
        );
    }

    #[test]
    fn return_guards_following_inlined_statements() {
        let output = compile_source(
            "indicator(\"t\")\nfn stopThenPlot() {\n  return\n  plot(close)\n}\nstopThenPlot()",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        let program = output.program.expect("expected compile success");
        let plot_call = program
            .ir_calls
            .iter()
            .find(|call| call.kind == IrCallKind::PlotLine)
            .expect("expected inlined plot call");
        let guard = plot_call
            .guard
            .as_ref()
            .expect("expected guard on inlined statement");
        assert!(
            contains_returned_flag(guard),
            "expected guard to reference returned flag"
        );
    }

    #[test]
    fn each_function_call_gets_unique_returned_flag() {
        let output = compile_source(
            "indicator(\"t\")\nfn f() {\n  return\n}\nf()\nf()",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        let program = output.program.expect("expected compile success");
        let mut returned_vars = HashSet::<String>::new();
        for call in &program.ir_calls {
            if call.kind != IrCallKind::StateLetDecl {
                continue;
            }
            let Some(IrCallArg::Text(name)) = call.args.first() else {
                continue;
            };
            if name.starts_with("__fn_returned_") {
                returned_vars.insert(name.clone());
            }
        }
        assert_eq!(
            returned_vars.len(),
            2,
            "expected unique returned flag per call frame"
        );
    }

    #[test]
    fn v1_keeps_expression_parse_failures_as_warnings() {
        let output = compile_source(
            "indicator(\"t\")\nlet x = close +\nplot(x)",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(
            output.program.is_some(),
            "expected compile success in v1 mode"
        );
        assert!(
            output.diagnostics.iter().any(|d| {
                d.code == "INDL-1400" && matches!(d.severity, super::DiagnosticSeverity::Warning)
            }),
            "expected INDL-1400 warning"
        );
    }

    #[test]
    fn v2_promotes_expression_parse_failures_to_errors() {
        let output = compile_source(
            "//@version=2\nindicator(\"t\")\nlet x = close +\nplot(x)",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(
            output.program.is_none(),
            "expected compile failure in v2 mode"
        );
        assert!(
            output.diagnostics.iter().any(|d| {
                d.code == "INDL-1400" && matches!(d.severity, super::DiagnosticSeverity::Error)
            }),
            "expected INDL-1400 error"
        );
    }

    #[test]
    fn invalid_version_header_emits_warning_and_falls_back_to_v1() {
        let output = compile_source(
            "//@version=99\nindicator(\"t\")\nplot(close)",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(
            output.program.is_some(),
            "expected fallback compile success"
        );
        assert!(
            output.diagnostics.iter().any(|d| d.code == "INDL-1010"
                && matches!(d.severity, super::DiagnosticSeverity::Warning)),
            "expected version warning diagnostic"
        );
    }

    #[test]
    fn rejects_unbalanced_delimiters_from_token_validation() {
        let output = compile_source(
            "indicator(\"t\")\nif (close > open {\n  plot(close)\n}",
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_none(), "expected compile failure");
        assert!(
            output.diagnostics.iter().any(|d| d.code == "INDL-1106"),
            "expected INDL-1106 diagnostic"
        );
    }

    #[test]
    fn parses_indicator_declaration_parameters() {
        let output = compile_source(
            r#"indicator("My Indicator", shorttitle="MI", overlay=true, precision=2, max_bars_back=500)
plot(close)"#,
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_some(), "expected compile success");
        let program = output.program.unwrap();
        assert_eq!(program.name, "My Indicator");
        assert_eq!(
            program.indicator_meta.title,
            Some("My Indicator".to_string())
        );
        assert_eq!(program.indicator_meta.shorttitle, Some("MI".to_string()));
        assert_eq!(program.indicator_meta.overlay, Some(true));
        assert_eq!(program.indicator_meta.precision, Some(2));
        assert_eq!(program.indicator_meta.max_bars_back, Some(500));
    }

    #[test]
    fn parses_indicator_format_and_scale() {
        let output = compile_source(
            r#"indicator("Volume Indicator", format=format.volume, scale=scale.none)
plot(volume)"#,
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_some(), "expected compile success");
        let program = output.program.unwrap();
        assert_eq!(
            program.indicator_meta.format,
            Some("format.volume".to_string())
        );
        assert_eq!(program.indicator_meta.scale, Some("scale.none".to_string()));
    }

    #[test]
    fn parses_indicator_max_objects_counts() {
        let output = compile_source(
            r#"indicator("Object Test", max_labels_count=100, max_lines_count=50, max_boxes_count=25, max_tables_count=10)
plot(close)"#,
            INDICATOR_IR_VERSION,
            INDICATOR_STDLIB_VERSION,
            &[],
        );
        assert!(output.program.is_some(), "expected compile success");
        let program = output.program.unwrap();
        assert_eq!(program.indicator_meta.max_labels_count, Some(100));
        assert_eq!(program.indicator_meta.max_lines_count, Some(50));
        assert_eq!(program.indicator_meta.max_boxes_count, Some(25));
        assert_eq!(program.indicator_meta.max_tables_count, Some(10));
    }
}
