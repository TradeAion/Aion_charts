use crate::core::indicators::compiler::ast::{AstProgram, AstStatement};
use crate::core::indicators::compiler::diagnostics::{
    CompileDiagnostic, DiagnosticSeverity, SourceSpan,
};
use std::collections::HashSet;

pub fn typecheck_program(
    program: &AstProgram,
    _source: &str,
    diagnostics: &mut Vec<CompileDiagnostic>,
) {
    let mut fn_names = HashSet::new();
    for statement in &program.statements {
        if let AstStatement::FnDecl(function) = statement {
            if !fn_names.insert(function.name.to_ascii_lowercase()) {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1207".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("duplicate function declaration '{}'", function.name),
                    hint: Some("rename one of the function declarations".to_string()),
                    span: Some(SourceSpan {
                        line: function.line,
                        column: function.column,
                        len: function.name.len(),
                    }),
                });
            }
        }
    }

    typecheck_statements(&program.statements, None, diagnostics);
}

fn typecheck_statements(
    statements: &[AstStatement],
    current_function: Option<&str>,
    diagnostics: &mut Vec<CompileDiagnostic>,
) {
    for statement in statements {
        match statement {
            AstStatement::For(for_loop) => {
                if for_loop.end > 1024 {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1200".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!("loop bound {} exceeds static max 1024", for_loop.end),
                        hint: Some("reduce loop upper bound to <= 1024".to_string()),
                        span: Some(SourceSpan {
                            line: for_loop.line,
                            column: for_loop.column,
                            len: 1,
                        }),
                    });
                }
                if for_loop.start > for_loop.end {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1203".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "loop start cannot be greater than loop end".to_string(),
                        hint: Some("use an increasing static loop range".to_string()),
                        span: Some(SourceSpan {
                            line: for_loop.line,
                            column: for_loop.column,
                            len: 1,
                        }),
                    });
                }
                typecheck_statements(&for_loop.body, current_function, diagnostics);
            }
            AstStatement::Call(call) => {
                let fn_name = call.function.to_ascii_lowercase();
                if fn_name == "eval" || fn_name == "date.now" {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1202".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "non-deterministic or dynamic runtime features are not allowed"
                            .to_string(),
                        hint: Some("remove eval and wall-clock access from script".to_string()),
                        span: Some(SourceSpan {
                            line: call.line,
                            column: call.column,
                            len: call.function.len(),
                        }),
                    });
                }
                if let Some(in_fn) = current_function {
                    if fn_name == in_fn.to_ascii_lowercase() {
                        diagnostics.push(CompileDiagnostic {
                            code: "INDL-1204".to_string(),
                            severity: DiagnosticSeverity::Error,
                            message: "recursive function calls are not allowed".to_string(),
                            hint: Some(
                                "remove direct recursion for deterministic execution".to_string(),
                            ),
                            span: Some(SourceSpan {
                                line: call.line,
                                column: call.column,
                                len: call.function.len(),
                            }),
                        });
                    }
                }
            }
            AstStatement::VarDecl(decl) => {
                if decl.name.is_empty() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1208".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "variable name cannot be empty".to_string(),
                        hint: Some(
                            "declare variable as `var name = ...` or `let name = ...`".to_string(),
                        ),
                        span: Some(SourceSpan {
                            line: decl.line,
                            column: decl.column,
                            len: 1,
                        }),
                    });
                }
            }
            AstStatement::Assign(assign) => {
                if assign.name.is_empty() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1209".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "assignment target cannot be empty".to_string(),
                        hint: Some("use `name = expression`".to_string()),
                        span: Some(SourceSpan {
                            line: assign.line,
                            column: assign.column,
                            len: 1,
                        }),
                    });
                }
                if assign.value.is_empty() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1210".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "assignment value cannot be empty".to_string(),
                        hint: Some("provide a valid expression on the right side".to_string()),
                        span: Some(SourceSpan {
                            line: assign.line,
                            column: assign.column,
                            len: 1,
                        }),
                    });
                }
            }
            AstStatement::If(branch) => {
                if branch.condition.trim().is_empty() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1211".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "if condition cannot be empty".to_string(),
                        hint: Some("provide a boolean-compatible condition".to_string()),
                        span: Some(SourceSpan {
                            line: branch.line,
                            column: branch.column,
                            len: 2,
                        }),
                    });
                }
                typecheck_statements(&branch.then_branch, current_function, diagnostics);
                typecheck_statements(&branch.else_branch, current_function, diagnostics);
            }
            AstStatement::While(loop_stmt) => {
                if loop_stmt.condition.trim().is_empty() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1213".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "while condition cannot be empty".to_string(),
                        hint: Some("provide a boolean-compatible condition".to_string()),
                        span: Some(SourceSpan {
                            line: loop_stmt.line,
                            column: loop_stmt.column,
                            len: 5,
                        }),
                    });
                }
                typecheck_statements(&loop_stmt.body, current_function, diagnostics);
            }
            AstStatement::Switch(switch_stmt) => {
                if switch_stmt.subject.trim().is_empty() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1214".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "switch subject cannot be empty".to_string(),
                        hint: Some("provide an expression after `switch`".to_string()),
                        span: Some(SourceSpan {
                            line: switch_stmt.line,
                            column: switch_stmt.column,
                            len: 6,
                        }),
                    });
                }
                for case in &switch_stmt.cases {
                    if case.value.trim().is_empty() {
                        diagnostics.push(CompileDiagnostic {
                            code: "INDL-1215".to_string(),
                            severity: DiagnosticSeverity::Error,
                            message: "switch case value cannot be empty".to_string(),
                            hint: Some("provide an expression after `case`".to_string()),
                            span: Some(SourceSpan {
                                line: case.line,
                                column: case.column,
                                len: 4,
                            }),
                        });
                    }
                    typecheck_statements(&case.body, current_function, diagnostics);
                }
                typecheck_statements(&switch_stmt.default_branch, current_function, diagnostics);
            }
            AstStatement::FnDecl(function) => {
                if function.name.trim().is_empty() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1212".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "function name cannot be empty".to_string(),
                        hint: Some("declare as `fn name(...) { ... }`".to_string()),
                        span: Some(SourceSpan {
                            line: function.line,
                            column: function.column,
                            len: 2,
                        }),
                    });
                }
                typecheck_statements(&function.body, Some(&function.name), diagnostics);
            }
            AstStatement::Return(ret) => {
                if current_function.is_none() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1206".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "return is only allowed inside function bodies".to_string(),
                        hint: Some("move return into an `fn` block".to_string()),
                        span: Some(SourceSpan {
                            line: ret.line,
                            column: ret.column,
                            len: 6,
                        }),
                    });
                }
            }
            AstStatement::TupleAssign(tuple_assign) => {
                // Tuple assignments like [a, b, c] = expr are valid anywhere
                // We could add checks for empty names here if needed
                if tuple_assign.names.is_empty() {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1213".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "tuple destructuring requires at least one variable".to_string(),
                        hint: Some("use [a, b, c] = expr syntax".to_string()),
                        span: Some(SourceSpan {
                            line: tuple_assign.line,
                            column: tuple_assign.column,
                            len: 1,
                        }),
                    });
                }
            }
        }
    }
}
