use crate::core::indicators::compiler::ast::{
    AstBinaryOp, AstCall, AstExpr, AstFnDecl, AstProgram, AstSeriesField, AstStatement, AstSwitch,
    AstUnaryOp, AstWhile,
};
use crate::core::indicators::compiler::diagnostics::{
    CompileDiagnostic, DiagnosticSeverity, SourceSpan,
};
use crate::core::indicators::language::CompileMode;
use crate::core::indicators::{
    IrBinaryOp, IrCall, IrCallArg, IrCallKind, IrExpr, IrSeriesField, OpCode,
};
use std::collections::HashMap;

const MAX_WHILE_UNROLL: usize = 128;

#[derive(Debug, Clone)]
pub struct LoweredIr {
    pub opcodes: Vec<OpCode>,
    pub calls: Vec<IrCall>,
    pub diagnostics: Vec<CompileDiagnostic>,
}

#[derive(Debug, Clone)]
struct FnInlineFrame {
    result_var: String,
    returned_var: String,
}

pub fn lower_to_ir(program: &AstProgram, compile_mode: CompileMode) -> LoweredIr {
    let mut opcodes = Vec::new();
    let mut calls = Vec::new();
    let mut diagnostics = Vec::new();
    let mut functions = HashMap::<String, AstFnDecl>::new();
    let mut inline_call_counter = 0u64;

    for statement in &program.statements {
        if let AstStatement::FnDecl(function) = statement {
            functions.insert(function.name.to_ascii_lowercase(), function.clone());
        }
    }

    let mut call_stack = Vec::<String>::new();
    for statement in &program.statements {
        lower_statement(
            statement,
            None,
            &functions,
            &mut call_stack,
            &mut opcodes,
            &mut calls,
            &mut diagnostics,
            compile_mode,
            None,
            &mut inline_call_counter,
        );
    }

    if opcodes.is_empty() {
        opcodes.push(OpCode::Nop);
    }
    opcodes.push(OpCode::Halt);

    LoweredIr {
        opcodes,
        calls,
        diagnostics,
    }
}

fn lower_statement(
    statement: &AstStatement,
    guard: Option<&IrExpr>,
    functions: &HashMap<String, AstFnDecl>,
    call_stack: &mut Vec<String>,
    opcodes: &mut Vec<OpCode>,
    calls: &mut Vec<IrCall>,
    diagnostics: &mut Vec<CompileDiagnostic>,
    compile_mode: CompileMode,
    active_frame: Option<&FnInlineFrame>,
    inline_call_counter: &mut u64,
) {
    match statement {
        AstStatement::Call(call) => {
            lower_call(
                call,
                guard,
                functions,
                call_stack,
                opcodes,
                calls,
                diagnostics,
                compile_mode,
                inline_call_counter,
            );
        }
        // BUG-3 fix: assign iterator variable to correct value each iteration
        AstStatement::For(for_loop) => {
            opcodes.push(OpCode::BranchIfTrue);
            let end = for_loop.end.min(for_loop.start.saturating_add(1024));
            for iter_val in for_loop.start..=end {
                // Emit a let-binding for the iterator variable with the current value
                calls.push(IrCall {
                    kind: IrCallKind::StateLetDecl,
                    args: vec![
                        IrCallArg::Text(for_loop.iterator.clone()),
                        IrCallArg::Expr(IrExpr::Number(iter_val as f64)),
                    ],
                    guard: guard.cloned(),
                    declaration_order: for_loop.line.saturating_sub(1) as u32,
                });
                opcodes.push(OpCode::StoreSeries);
                for body_stmt in &for_loop.body {
                    lower_statement(
                        body_stmt,
                        guard,
                        functions,
                        call_stack,
                        opcodes,
                        calls,
                        diagnostics,
                        compile_mode,
                        active_frame,
                        inline_call_counter,
                    );
                }
            }
        }
        // BUG-7 fix: emit diagnostic when if-condition fails to parse
        AstStatement::If(branch) => {
            opcodes.push(OpCode::BranchIfTrue);
            let condition = match lower_expr_with_fallback(
                branch.condition_expr.as_ref(),
                &branch.condition,
                branch.line,
                branch.column,
                diagnostics,
                compile_mode,
            ) {
                Some(expr) => expr,
                None => return,
            };

            let then_guard = combine_guards(guard, Some(condition.clone()));
            let else_guard =
                combine_guards(guard, Some(IrExpr::UnaryNot(Box::new(condition.clone()))));

            for body_stmt in &branch.then_branch {
                lower_statement(
                    body_stmt,
                    then_guard.as_ref(),
                    functions,
                    call_stack,
                    opcodes,
                    calls,
                    diagnostics,
                    compile_mode,
                    active_frame,
                    inline_call_counter,
                );
            }

            if !branch.else_branch.is_empty() {
                opcodes.push(OpCode::BranchIfFalse);
                for body_stmt in &branch.else_branch {
                    lower_statement(
                        body_stmt,
                        else_guard.as_ref(),
                        functions,
                        call_stack,
                        opcodes,
                        calls,
                        diagnostics,
                        compile_mode,
                        active_frame,
                        inline_call_counter,
                    );
                }
            }
        }
        AstStatement::Switch(AstSwitch {
            subject,
            subject_expr,
            cases,
            default_branch,
            line,
            column,
        }) => {
            opcodes.push(OpCode::BranchIfTrue);
            let subject_expr = match lower_expr_with_fallback(
                subject_expr.as_ref(),
                subject,
                *line,
                *column,
                diagnostics,
                compile_mode,
            ) {
                Some(expr) => expr,
                None => return,
            };

            let mut matched_expr = None::<IrExpr>;
            for case in cases {
                let case_expr = match lower_expr_with_fallback(
                    case.value_expr.as_ref(),
                    &case.value,
                    case.line,
                    case.column,
                    diagnostics,
                    compile_mode,
                ) {
                    Some(expr) => expr,
                    None => continue,
                };

                let eq_expr = IrExpr::Binary {
                    lhs: Box::new(subject_expr.clone()),
                    op: IrBinaryOp::Eq,
                    rhs: Box::new(case_expr),
                };
                let case_guard_expr = if let Some(prev_match) = matched_expr.as_ref() {
                    IrExpr::Binary {
                        lhs: Box::new(IrExpr::UnaryNot(Box::new(prev_match.clone()))),
                        op: IrBinaryOp::And,
                        rhs: Box::new(eq_expr.clone()),
                    }
                } else {
                    eq_expr.clone()
                };
                let case_guard = combine_guards(guard, Some(case_guard_expr));
                for body_stmt in &case.body {
                    lower_statement(
                        body_stmt,
                        case_guard.as_ref(),
                        functions,
                        call_stack,
                        opcodes,
                        calls,
                        diagnostics,
                        compile_mode,
                        active_frame,
                        inline_call_counter,
                    );
                }

                matched_expr = Some(match matched_expr {
                    Some(prev) => IrExpr::Binary {
                        lhs: Box::new(prev),
                        op: IrBinaryOp::Or,
                        rhs: Box::new(eq_expr),
                    },
                    None => eq_expr,
                });
            }

            if !default_branch.is_empty() {
                let default_expr = matched_expr
                    .map(|matched| IrExpr::UnaryNot(Box::new(matched)))
                    .unwrap_or(IrExpr::Bool(true));
                let default_guard = combine_guards(guard, Some(default_expr));
                for body_stmt in default_branch {
                    lower_statement(
                        body_stmt,
                        default_guard.as_ref(),
                        functions,
                        call_stack,
                        opcodes,
                        calls,
                        diagnostics,
                        compile_mode,
                        active_frame,
                        inline_call_counter,
                    );
                }
            }
        }
        AstStatement::While(AstWhile {
            condition,
            condition_expr,
            body,
            line,
            column,
        }) => {
            opcodes.push(OpCode::BranchIfTrue);
            let condition = match lower_expr_with_fallback(
                condition_expr.as_ref(),
                condition,
                *line,
                *column,
                diagnostics,
                compile_mode,
            ) {
                Some(expr) => expr,
                None => return,
            };

            for _ in 0..MAX_WHILE_UNROLL {
                let loop_guard = combine_guards(guard, Some(condition.clone()));
                for body_stmt in body {
                    lower_statement(
                        body_stmt,
                        loop_guard.as_ref(),
                        functions,
                        call_stack,
                        opcodes,
                        calls,
                        diagnostics,
                        compile_mode,
                        active_frame,
                        inline_call_counter,
                    );
                }
            }
        }
        // BUG-7 fix: emit diagnostic when var/let value expression fails to parse
        AstStatement::VarDecl(decl) => {
            let kind = if decl.is_persistent {
                IrCallKind::StateVarDecl
            } else {
                IrCallKind::StateLetDecl
            };
            let value_expr = match &decl.value {
                Some(raw) => lower_expr_with_fallback(
                    decl.value_expr.as_ref(),
                    raw,
                    decl.line,
                    decl.column,
                    diagnostics,
                    compile_mode,
                )
                .unwrap_or(IrExpr::Na),
                None => IrExpr::Na,
            };
            calls.push(IrCall {
                kind,
                args: vec![
                    IrCallArg::Text(decl.name.clone()),
                    IrCallArg::Expr(value_expr),
                ],
                guard: guard.cloned(),
                declaration_order: decl.line.saturating_sub(1) as u32,
            });
            if decl.value.is_some() {
                opcodes.push(OpCode::StoreSeries);
            } else {
                opcodes.push(OpCode::Nop);
            }
        }
        // BUG-7 fix: emit diagnostic when assignment value expression fails to parse
        AstStatement::Assign(assign) => {
            let value_expr = lower_expr_with_fallback(
                assign.value_expr.as_ref(),
                &assign.value,
                assign.line,
                assign.column,
                diagnostics,
                compile_mode,
            )
            .unwrap_or(IrExpr::Na);
            calls.push(IrCall {
                kind: IrCallKind::StateAssign,
                args: vec![
                    IrCallArg::Text(assign.name.clone()),
                    IrCallArg::Expr(value_expr),
                ],
                guard: guard.cloned(),
                declaration_order: assign.line.saturating_sub(1) as u32,
            });
            // Use the already-parsed result rather than parsing twice
            if !matches!(value_expr_is_na_fallback(&assign.value), true) {
                opcodes.push(OpCode::StoreSeries);
            } else {
                opcodes.push(OpCode::Nop);
            }
        }
        // Tuple destructuring: [a, b, c] = expr
        AstStatement::TupleAssign(tuple_assign) => {
            let value_expr = lower_expr_with_fallback(
                tuple_assign.value_expr.as_ref(),
                &tuple_assign.value,
                tuple_assign.line,
                tuple_assign.column,
                diagnostics,
                compile_mode,
            )
            .unwrap_or(IrExpr::Na);

            // Create args: first the expression, then the variable names
            let mut args = vec![IrCallArg::Expr(value_expr)];
            for name in &tuple_assign.names {
                args.push(IrCallArg::Text(name.clone()));
            }

            calls.push(IrCall {
                kind: IrCallKind::StateTupleAssign,
                args,
                guard: guard.cloned(),
                declaration_order: tuple_assign.line.saturating_sub(1) as u32,
            });
            opcodes.push(OpCode::StoreSeries);
        }
        AstStatement::FnDecl(_) => {}
        // BUG-4 fix: capture return value and stop subsequent inlined statements.
        AstStatement::Return(ret) => {
            let Some(frame) = active_frame else {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1302".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "return encountered outside active function frame".to_string(),
                    hint: Some("move return into an `fn` body".to_string()),
                    span: Some(SourceSpan {
                        line: ret.line,
                        column: ret.column,
                        len: 6,
                    }),
                });
                return;
            };

            let return_guard = combine_guards(
                guard,
                Some(IrExpr::UnaryNot(Box::new(IrExpr::Var(
                    frame.returned_var.clone(),
                )))),
            );

            if let Some(ref value_raw) = ret.value {
                let value_expr = lower_expr_with_fallback(
                    ret.value_expr.as_ref(),
                    value_raw,
                    ret.line,
                    ret.column,
                    diagnostics,
                    compile_mode,
                )
                .unwrap_or(IrExpr::Na);
                calls.push(IrCall {
                    kind: IrCallKind::StateAssign,
                    args: vec![
                        IrCallArg::Text(frame.result_var.clone()),
                        IrCallArg::Expr(value_expr),
                    ],
                    guard: return_guard.clone(),
                    declaration_order: ret.line.saturating_sub(1) as u32,
                });
                opcodes.push(OpCode::StoreSeries);
            }

            calls.push(IrCall {
                kind: IrCallKind::StateAssign,
                args: vec![
                    IrCallArg::Text(frame.returned_var.clone()),
                    IrCallArg::Expr(IrExpr::Bool(true)),
                ],
                guard: return_guard,
                declaration_order: ret.line.saturating_sub(1) as u32,
            });
            opcodes.push(OpCode::StoreSeries);
        }
    }
}

fn lower_expr_with_fallback(
    ast_expr: Option<&AstExpr>,
    raw: &str,
    line: usize,
    column: usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
    compile_mode: CompileMode,
) -> Option<IrExpr> {
    if let Some(ast_expr) = ast_expr {
        if let Some(expr) = lower_ast_expr(ast_expr) {
            return Some(expr);
        }
        if matches!(compile_mode, CompileMode::RayDslV2) {
            let diag_count_before = diagnostics.len();
            let _ = parse_expression_with_diagnostic(raw, line, column, diagnostics, compile_mode);
            if diagnostics.len() == diag_count_before {
                emit_v2_structured_expr_required(
                    raw,
                    line,
                    column,
                    diagnostics,
                    "expression cannot be lowered from AST in v2 mode",
                );
            }
            return None;
        }
        return parse_expression_with_diagnostic(raw, line, column, diagnostics, compile_mode);
    }

    if matches!(compile_mode, CompileMode::RayDslV2) {
        let diag_count_before = diagnostics.len();
        let _ = parse_expression_with_diagnostic(raw, line, column, diagnostics, compile_mode);
        if diagnostics.len() == diag_count_before {
            emit_v2_structured_expr_required(
                raw,
                line,
                column,
                diagnostics,
                "expression requires structured AST parsing in v2 mode",
            );
        }
        return None;
    }

    parse_expression_with_diagnostic(raw, line, column, diagnostics, compile_mode)
}

fn emit_v2_structured_expr_required(
    raw: &str,
    line: usize,
    column: usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
    message: &str,
) {
    diagnostics.push(CompileDiagnostic {
        code: "INDL-1401".to_string(),
        severity: DiagnosticSeverity::Error,
        message: format!("{}: '{}'", message, truncate_for_display(raw, 40)),
        hint: Some(
            "fix expression syntax or use currently supported v2 expression forms".to_string(),
        ),
        span: Some(SourceSpan {
            line,
            column,
            len: raw.len().max(1),
        }),
    });
}
fn lower_ast_expr(expr: &AstExpr) -> Option<IrExpr> {
    match expr {
        AstExpr::Bool(value) => Some(IrExpr::Bool(*value)),
        AstExpr::Number(value) => Some(IrExpr::Number(*value)),
        AstExpr::Na => Some(IrExpr::Na),
        AstExpr::String(_) => None,
        AstExpr::Var(name) => Some(IrExpr::Var(name.clone())),
        AstExpr::VarIndexed { name, index } => Some(IrExpr::VarIndexed {
            name: name.clone(),
            index: Box::new(lower_ast_expr(index)?),
        }),
        AstExpr::Unary { op, expr } => {
            let inner = lower_ast_expr(expr)?;
            match op {
                AstUnaryOp::Not => Some(IrExpr::UnaryNot(Box::new(inner))),
                AstUnaryOp::Neg => Some(IrExpr::UnaryNeg(Box::new(inner))),
            }
        }
        AstExpr::Binary { lhs, op, rhs } => {
            let lhs = lower_ast_expr(lhs)?;
            let rhs = lower_ast_expr(rhs)?;
            Some(IrExpr::Binary {
                lhs: Box::new(lhs),
                op: lower_ast_binary_op(*op),
                rhs: Box::new(rhs),
            })
        }
        AstExpr::Conditional {
            condition,
            then_expr,
            else_expr,
        } => {
            let condition = lower_ast_expr(condition)?;
            let then_expr = lower_ast_expr(then_expr)?;
            let else_expr = lower_ast_expr(else_expr)?;
            Some(IrExpr::Conditional {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            })
        }
        AstExpr::ReqSeries {
            symbol,
            timeframe,
            field,
            mode,
            gaps,
            lookahead,
            index,
        } => Some(IrExpr::ReqSeries {
            symbol: symbol.clone(),
            timeframe: timeframe.clone(),
            field: field.clone(),
            mode: mode.clone(),
            gaps: gaps.clone(),
            lookahead: lookahead.clone(),
            index: match index {
                Some(inner) => Some(Box::new(lower_ast_expr(inner)?)),
                None => None,
            },
        }),
        AstExpr::Series { field, index } => Some(IrExpr::Series {
            field: lower_ast_series_field(*field),
            index: match index {
                Some(inner) => Some(Box::new(lower_ast_expr(inner)?)),
                None => None,
            },
        }),
        // Expression-level function calls (e.g., nz(x), math.abs(y))
        AstExpr::FnCall { name, args } => {
            let mut ir_args = Vec::with_capacity(args.len());
            for arg in args {
                ir_args.push(lower_ast_expr(arg)?);
            }
            Some(IrExpr::FnCall {
                name: name.clone(),
                args: ir_args,
            })
        }
        AstExpr::Color { r, g, b, a } => Some(IrExpr::Color {
            r: *r,
            g: *g,
            b: *b,
            a: *a,
        }),
    }
}

fn lower_ast_binary_op(op: AstBinaryOp) -> IrBinaryOp {
    match op {
        AstBinaryOp::Add => IrBinaryOp::Add,
        AstBinaryOp::Sub => IrBinaryOp::Sub,
        AstBinaryOp::Mul => IrBinaryOp::Mul,
        AstBinaryOp::Div => IrBinaryOp::Div,
        AstBinaryOp::Mod => IrBinaryOp::Mod,
        AstBinaryOp::Pow => IrBinaryOp::Pow,
        AstBinaryOp::Gt => IrBinaryOp::Gt,
        AstBinaryOp::Gte => IrBinaryOp::Gte,
        AstBinaryOp::Lt => IrBinaryOp::Lt,
        AstBinaryOp::Lte => IrBinaryOp::Lte,
        AstBinaryOp::Eq => IrBinaryOp::Eq,
        AstBinaryOp::Neq => IrBinaryOp::Neq,
        AstBinaryOp::And => IrBinaryOp::And,
        AstBinaryOp::Or => IrBinaryOp::Or,
    }
}

fn lower_ast_series_field(field: AstSeriesField) -> IrSeriesField {
    match field {
        AstSeriesField::Open => IrSeriesField::Open,
        AstSeriesField::High => IrSeriesField::High,
        AstSeriesField::Low => IrSeriesField::Low,
        AstSeriesField::Close => IrSeriesField::Close,
        AstSeriesField::Volume => IrSeriesField::Volume,
        AstSeriesField::Time => IrSeriesField::Time,
        AstSeriesField::BarIndex => IrSeriesField::BarIndex,
    }
}

/// Returns true if `parse_expression` would fail for this value (i.e., the
/// value was treated as Na due to a parse error).
fn value_expr_is_na_fallback(raw: &str) -> bool {
    parse_expression(raw).is_none()
}

/// Parse an expression and emit a diagnostic on failure (BUG-7 fix).
fn parse_expression_with_diagnostic(
    raw: &str,
    line: usize,
    column: usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
    compile_mode: CompileMode,
) -> Option<IrExpr> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parser = ExprParser::new(trimmed);
    let strict = matches!(compile_mode, CompileMode::RayDslV2);
    let severity = if strict {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Warning
    };
    match parser.parse_expression() {
        Ok(expr) => {
            parser.skip_ws();
            if parser.is_done() {
                Some(expr)
            } else {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1400".to_string(),
                    severity,
                    message: format!(
                        "failed to fully parse expression '{}' -- trailing content ignored",
                        truncate_for_display(raw, 60)
                    ),
                    hint: Some("check expression syntax".to_string()),
                    span: Some(SourceSpan {
                        line,
                        column,
                        len: raw.len(),
                    }),
                });
                Some(expr)
            }
        }
        Err(err_msg) => {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1400".to_string(),
                severity,
                message: format!(
                    "failed to parse expression '{}': {}",
                    truncate_for_display(raw, 40),
                    err_msg
                ),
                hint: Some("treating as na -- check expression syntax".to_string()),
                span: Some(SourceSpan {
                    line,
                    column,
                    len: raw.len(),
                }),
            });
            None
        }
    }
}

fn truncate_for_display(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

fn combine_guards(parent: Option<&IrExpr>, child: Option<IrExpr>) -> Option<IrExpr> {
    match (parent, child) {
        (None, None) => None,
        (Some(parent_expr), None) => Some(parent_expr.clone()),
        (None, Some(child_expr)) => Some(child_expr),
        (Some(parent_expr), Some(child_expr)) => Some(IrExpr::Binary {
            lhs: Box::new(parent_expr.clone()),
            op: IrBinaryOp::And,
            rhs: Box::new(child_expr),
        }),
    }
}

fn lower_call(
    call: &AstCall,
    guard: Option<&IrExpr>,
    functions: &HashMap<String, AstFnDecl>,
    call_stack: &mut Vec<String>,
    opcodes: &mut Vec<OpCode>,
    calls: &mut Vec<IrCall>,
    diagnostics: &mut Vec<CompileDiagnostic>,
    compile_mode: CompileMode,
    inline_call_counter: &mut u64,
) {
    if let Some((kind, opcode)) = map_call_kind(&call.function) {
        calls.push(IrCall {
            kind: kind.clone(),
            args: lower_call_args(&kind, &call.args),
            guard: guard.cloned(),
            declaration_order: call.line.saturating_sub(1) as u32,
        });
        opcodes.push(opcode);
        return;
    }

    let function_name = call.function.trim().to_ascii_lowercase();
    let Some(function) = functions.get(&function_name) else {
        // BUG-1 fix: emit diagnostic for unknown function instead of silently dropping
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1300".to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!("unknown function '{}'", call.function.trim()),
            hint: suggest_function_name(call.function.trim()),
            span: Some(SourceSpan {
                line: call.line,
                column: call.column,
                len: call.function.trim().len(),
            }),
        });
        return;
    };
    if call_stack.contains(&function_name) {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1302".to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!("recursive call to '{}' detected", call.function.trim()),
            hint: Some("recursion is not supported; use loops instead".to_string()),
            span: Some(SourceSpan {
                line: call.line,
                column: call.column,
                len: call.function.trim().len(),
            }),
        });
        return;
    }

    const MAX_CALL_DEPTH: usize = 64;
    if call_stack.len() >= MAX_CALL_DEPTH {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1303".to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!(
                "call stack depth limit ({}) exceeded when calling '{}'",
                MAX_CALL_DEPTH,
                call.function.trim()
            ),
            hint: Some("reduce nesting of function calls".to_string()),
            span: Some(SourceSpan {
                line: call.line,
                column: call.column,
                len: call.function.trim().len(),
            }),
        });
        return;
    }

    // BUG-2 fix: bind function parameters to caller arguments before inlining body
    if call.args.len() != function.params.len() {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1301".to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!(
                "function '{}' expects {} argument(s) but got {}",
                call.function.trim(),
                function.params.len(),
                call.args.len()
            ),
            hint: Some("update the call site to match the function signature".to_string()),
            span: Some(SourceSpan {
                line: call.line,
                column: call.column,
                len: call.function.trim().len(),
            }),
        });
        return;
    }

    let frame_id = *inline_call_counter;
    *inline_call_counter = inline_call_counter.saturating_add(1);
    let frame = FnInlineFrame {
        result_var: format!("__fn_result_{}__", frame_id),
        returned_var: format!("__fn_returned_{}__", frame_id),
    };

    calls.push(IrCall {
        kind: IrCallKind::StateLetDecl,
        args: vec![
            IrCallArg::Text(frame.returned_var.clone()),
            IrCallArg::Expr(IrExpr::Bool(false)),
        ],
        guard: guard.cloned(),
        declaration_order: call.line.saturating_sub(1) as u32,
    });
    opcodes.push(OpCode::StoreSeries);

    calls.push(IrCall {
        kind: IrCallKind::StateLetDecl,
        args: vec![
            IrCallArg::Text(frame.result_var.clone()),
            IrCallArg::Expr(IrExpr::Na),
        ],
        guard: guard.cloned(),
        declaration_order: call.line.saturating_sub(1) as u32,
    });
    opcodes.push(OpCode::StoreSeries);

    call_stack.push(function_name.clone());
    for (param_idx, param_name) in function.params.iter().enumerate() {
        let raw_arg = call
            .args
            .get(param_idx)
            .map(|raw| raw.as_str())
            .unwrap_or("");
        let arg_expr = match call
            .arg_exprs
            .get(param_idx)
            .and_then(|expr| expr.as_ref())
            .and_then(lower_ast_expr)
        {
            Some(expr) => expr,
            None if matches!(compile_mode, CompileMode::RayDslV2) => {
                emit_v2_structured_expr_required(
                    raw_arg,
                    call.line,
                    call.column,
                    diagnostics,
                    &format!(
                        "function argument {} for '{}' requires structured AST parsing in v2 mode",
                        param_idx.saturating_add(1),
                        call.function.trim()
                    ),
                );
                IrExpr::Na
            }
            None => call
                .args
                .get(param_idx)
                .and_then(|raw| parse_expression(raw))
                .unwrap_or(IrExpr::Na),
        };
        calls.push(IrCall {
            kind: IrCallKind::StateLetDecl,
            args: vec![
                IrCallArg::Text(param_name.clone()),
                IrCallArg::Expr(arg_expr),
            ],
            guard: guard.cloned(),
            declaration_order: call.line.saturating_sub(1) as u32,
        });
        opcodes.push(OpCode::StoreSeries);
    }

    let body_guard = combine_guards(
        guard,
        Some(IrExpr::UnaryNot(Box::new(IrExpr::Var(
            frame.returned_var.clone(),
        )))),
    );
    for statement in &function.body {
        lower_statement(
            statement,
            body_guard.as_ref(),
            functions,
            call_stack,
            opcodes,
            calls,
            diagnostics,
            compile_mode,
            Some(&frame),
            inline_call_counter,
        );
    }
    call_stack.pop();
}

/// Suggest a similar known function name for typo correction.
fn suggest_function_name(unknown: &str) -> Option<String> {
    let known = [
        "plot",
        "plotcandle",
        "plotbar",
        "plothistogram",
        "plotarea",
        "plotshape",
        "fillbetween",
        "fill_between",
        "box.new",
        "box.set",
        "box.delete",
        "label.new",
        "label.set",
        "label.delete",
        "polyline.new",
        "polyline.set",
        "polyline.delete",
        "obj.delete",
        "req.series",
    ];
    let lower = unknown.to_ascii_lowercase();
    // Simple prefix/substring match for suggestions
    for name in &known {
        if name.starts_with(&lower) || lower.starts_with(name) {
            return Some(format!("did you mean '{}'?", name));
        }
    }
    // Levenshtein-like: check if only 1-2 chars differ
    for name in &known {
        if levenshtein_distance(&lower, name) <= 2 {
            return Some(format!("did you mean '{}'?", name));
        }
    }
    Some("check function name spelling".to_string())
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let a_len = a_bytes.len();
    let b_len = b_bytes.len();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0usize; b_len + 1];
    for i in 1..=a_len {
        curr_row[0] = i;
        for j in 1..=b_len {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
                0
            } else {
                1
            };
            curr_row[j] = (prev_row[j] + 1)
                .min(curr_row[j - 1] + 1)
                .min(prev_row[j - 1] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }
    prev_row[b_len]
}

fn map_call_kind(function: &str) -> Option<(IrCallKind, OpCode)> {
    let normalized = function.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "plotcandle" | "viz.plotcandle" => Some((IrCallKind::PlotCandle, OpCode::EmitPlotCandle)),
        "plotbar" | "viz.plotbar" => Some((IrCallKind::PlotBar, OpCode::EmitPlotBar)),
        "plothistogram" | "plot_histogram" | "viz.plothistogram" | "viz.plot_histogram" => {
            Some((IrCallKind::PlotHistogram, OpCode::CallBuiltin))
        }
        "plotarea" | "viz.plotarea" => Some((IrCallKind::PlotArea, OpCode::EmitPlotArea)),
        "fillbetween" | "fill_between" | "viz.fillbetween" | "viz.fill_between" => {
            Some((IrCallKind::FillBetween, OpCode::EmitFillBetween))
        }
        "plotshape" | "viz.plotshape" => Some((IrCallKind::PlotShape, OpCode::EmitPlotShape)),
        "plot" | "viz.plot" => Some((IrCallKind::PlotLine, OpCode::EmitPlotLine)),
        "box.new" | "obj.new_box" => Some((IrCallKind::ObjBoxNew, OpCode::EmitDrawBox)),
        "box.set" | "obj.set_box" => Some((IrCallKind::ObjBoxSet, OpCode::EmitDrawBox)),
        "box.delete" | "obj.delete_box" => Some((IrCallKind::ObjBoxDelete, OpCode::EmitDrawBox)),
        "label.new" | "obj.new_label" => Some((IrCallKind::ObjLabelNew, OpCode::EmitDrawLabel)),
        "label.set" | "obj.set_label" => Some((IrCallKind::ObjLabelSet, OpCode::EmitDrawLabel)),
        "label.delete" | "obj.delete_label" => {
            Some((IrCallKind::ObjLabelDelete, OpCode::EmitDrawLabel))
        }
        "line.new" | "obj.new_line" => Some((IrCallKind::ObjLineNew, OpCode::EmitDrawLine)),
        "line.set" | "obj.set_line" => Some((IrCallKind::ObjLineSet, OpCode::EmitDrawLine)),
        "line.delete" | "obj.delete_line" => {
            Some((IrCallKind::ObjLineDelete, OpCode::EmitDrawLine))
        }
        "polyline.new" | "obj.new_polyline" => {
            Some((IrCallKind::ObjPolylineNew, OpCode::EmitDrawPolyline))
        }
        "polyline.set" | "obj.set_polyline" => {
            Some((IrCallKind::ObjPolylineSet, OpCode::EmitDrawPolyline))
        }
        "polyline.delete" | "obj.delete_polyline" => {
            Some((IrCallKind::ObjPolylineDelete, OpCode::EmitDrawPolyline))
        }
        "obj.delete" | "obj.remove" => Some((IrCallKind::ObjDelete, OpCode::CallBuiltin)),
        "req.series" => Some((IrCallKind::RequestSeries, OpCode::RequestSeries)),
        _ => None,
    }
}

fn lower_call_args(kind: &IrCallKind, args: &[String]) -> Vec<IrCallArg> {
    args.iter()
        .map(|arg| lower_call_arg(kind, arg))
        .collect::<Vec<_>>()
}

fn lower_call_arg(kind: &IrCallKind, arg: &str) -> IrCallArg {
    let Some((key, value)) = split_named_arg(arg) else {
        return lower_positional_arg(kind, arg);
    };
    let name = key.trim().to_string();
    let trimmed_value = value.trim();
    if let Some(expr) = parse_expression(trimmed_value) {
        return IrCallArg::NamedExpr { name, value: expr };
    }
    IrCallArg::NamedText {
        name,
        value: parse_text_argument(trimmed_value),
    }
}

fn split_named_arg(raw: &str) -> Option<(&str, &str)> {
    let bytes = raw.as_bytes();
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut depth_brace = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if *ch == b'\\' {
                escaped = true;
                continue;
            }
            if *ch == b'"' {
                in_string = false;
            }
            continue;
        }

        match *ch {
            b'"' => in_string = true,
            b'(' => depth_paren = depth_paren.saturating_add(1),
            b')' => depth_paren = depth_paren.saturating_sub(1),
            b'[' => depth_bracket = depth_bracket.saturating_add(1),
            b']' => depth_bracket = depth_bracket.saturating_sub(1),
            b'{' => depth_brace = depth_brace.saturating_add(1),
            b'}' => depth_brace = depth_brace.saturating_sub(1),
            b'=' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                let prev = idx.checked_sub(1).and_then(|i| bytes.get(i)).copied();
                let next = bytes.get(idx + 1).copied();
                if matches!(prev, Some(b'!' | b'<' | b'>' | b'=')) || next == Some(b'=') {
                    continue;
                }
                return Some((&raw[..idx], &raw[idx + 1..]));
            }
            _ => {}
        }
    }
    None
}

fn lower_positional_arg(kind: &IrCallKind, arg: &str) -> IrCallArg {
    if matches!(kind, IrCallKind::FillBetween) {
        return IrCallArg::Text(parse_text_argument(arg));
    }
    if let Some(expr) = parse_expression(arg) {
        return IrCallArg::Expr(expr);
    }
    IrCallArg::Text(parse_text_argument(arg))
}

fn parse_text_argument(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_expression(input: &str) -> Option<IrExpr> {
    let mut parser = ExprParser::new(input);
    let expr = parser.parse_expression().ok()?;
    parser.skip_ws();
    if parser.is_done() {
        Some(expr)
    } else {
        None
    }
}

struct ExprParser<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> ExprParser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    fn is_done(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn take_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source[self.pos..].starts_with(value)
    }

    fn consume(&mut self, value: &str) -> bool {
        if self.starts_with(value) {
            self.pos += value.len();
            true
        } else {
            false
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        if !self.starts_with(keyword) {
            return false;
        }
        let end = self.pos + keyword.len();
        let boundary = self
            .source
            .get(end..)
            .and_then(|tail| tail.chars().next())
            .map(|ch| !is_ident_part(ch))
            .unwrap_or(true);
        if !boundary {
            return false;
        }
        self.pos = end;
        true
    }

    fn parse_expression(&mut self) -> Result<IrExpr, String> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<IrExpr, String> {
        let condition = self.parse_or()?;
        self.skip_ws();
        if !self.consume("?") {
            return Ok(condition);
        }
        let then_expr = self.parse_expression()?;
        self.skip_ws();
        if !self.consume(":") {
            return Err("expected ':' in ternary expression".to_string());
        }
        let else_expr = self.parse_ternary()?;
        Ok(IrExpr::Conditional {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        })
    }

    fn parse_or(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_and()?;
        loop {
            self.skip_ws();
            if self.consume("||") || self.consume_keyword("or") {
                let rhs = self.parse_and()?;
                expr = IrExpr::Binary {
                    lhs: Box::new(expr),
                    op: IrBinaryOp::Or,
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_equality()?;
        loop {
            self.skip_ws();
            if self.consume("&&") || self.consume_keyword("and") {
                let rhs = self.parse_equality()?;
                expr = IrExpr::Binary {
                    lhs: Box::new(expr),
                    op: IrBinaryOp::And,
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_compare()?;
        loop {
            self.skip_ws();
            let op = if self.consume("==") {
                Some(IrBinaryOp::Eq)
            } else if self.consume("!=") {
                Some(IrBinaryOp::Neq)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_compare()?;
            expr = IrExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_compare(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_add_sub()?;
        loop {
            self.skip_ws();
            let op = if self.consume(">=") {
                Some(IrBinaryOp::Gte)
            } else if self.consume("<=") {
                Some(IrBinaryOp::Lte)
            } else if self.consume(">") {
                Some(IrBinaryOp::Gt)
            } else if self.consume("<") {
                Some(IrBinaryOp::Lt)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_add_sub()?;
            expr = IrExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_add_sub(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_mul_div()?;
        loop {
            self.skip_ws();
            let op = if self.consume("+") {
                Some(IrBinaryOp::Add)
            } else if self.consume("-") {
                Some(IrBinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_mul_div()?;
            expr = IrExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_mul_div(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_pow()?;
        loop {
            self.skip_ws();
            let op = if self.consume("*") {
                Some(IrBinaryOp::Mul)
            } else if self.consume("/") {
                Some(IrBinaryOp::Div)
            } else if self.consume("%") {
                Some(IrBinaryOp::Mod)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_pow()?;
            expr = IrExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_pow(&mut self) -> Result<IrExpr, String> {
        let expr = self.parse_unary()?;
        self.skip_ws();
        if self.consume("**") || self.consume("^") {
            let rhs = self.parse_pow()?;
            return Ok(IrExpr::Binary {
                lhs: Box::new(expr),
                op: IrBinaryOp::Pow,
                rhs: Box::new(rhs),
            });
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<IrExpr, String> {
        self.skip_ws();
        if self.consume("!") || self.consume_keyword("not") {
            let inner = self.parse_unary()?;
            return Ok(IrExpr::UnaryNot(Box::new(inner)));
        }
        if self.consume("-") {
            let inner = self.parse_unary()?;
            return Ok(IrExpr::UnaryNeg(Box::new(inner)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<IrExpr, String> {
        self.skip_ws();
        match self.peek_char() {
            Some('(') => {
                self.take_char();
                let expr = self.parse_expression()?;
                self.skip_ws();
                if !matches!(self.take_char(), Some(')')) {
                    return Err("expected ')'".to_string());
                }
                Ok(expr)
            }
            Some(ch) if ch.is_ascii_digit() || ch == '.' => self.parse_number(),
            Some(ch) if is_ident_start(ch) => self.parse_identifier_expr(),
            _ => Err("unexpected token in expression".to_string()),
        }
    }

    fn parse_number(&mut self) -> Result<IrExpr, String> {
        let start = self.pos;

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.take_char();
            } else {
                break;
            }
        }
        if matches!(self.peek_char(), Some('.')) {
            self.take_char();
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    self.take_char();
                } else {
                    break;
                }
            }
        }
        if matches!(self.peek_char(), Some('e' | 'E')) {
            self.take_char();
            if matches!(self.peek_char(), Some('+' | '-')) {
                self.take_char();
            }
            let mut has_exp_digits = false;
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    has_exp_digits = true;
                    self.take_char();
                } else {
                    break;
                }
            }
            if !has_exp_digits {
                return Err("invalid exponent".to_string());
            }
        }

        let raw = &self.source[start..self.pos];
        let value = raw
            .parse::<f64>()
            .map_err(|_| "invalid number".to_string())?;
        Ok(IrExpr::Number(value))
    }

    fn parse_identifier_expr(&mut self) -> Result<IrExpr, String> {
        let ident = self.parse_identifier();
        if ident.eq_ignore_ascii_case("na") {
            return Ok(IrExpr::Na);
        }
        if ident.eq_ignore_ascii_case("true") {
            return Ok(IrExpr::Bool(true));
        }
        if ident.eq_ignore_ascii_case("false") {
            return Ok(IrExpr::Bool(false));
        }

        self.skip_ws();
        if matches!(self.peek_char(), Some('(')) {
            let args = self.parse_call_args()?;
            return self.parse_call_expr(&ident, &args);
        }

        self.skip_ws();
        let index = if matches!(self.peek_char(), Some('[')) {
            self.take_char();
            let expr = self.parse_expression()?;
            self.skip_ws();
            if !matches!(self.take_char(), Some(']')) {
                return Err("expected ']'".to_string());
            }
            Some(Box::new(expr))
        } else {
            None
        };

        if let Some(field) = map_series_field(&ident) {
            return Ok(IrExpr::Series { field, index });
        }

        if let Some(index) = index {
            return Ok(IrExpr::VarIndexed { name: ident, index });
        }

        Ok(IrExpr::Var(ident))
    }

    fn parse_call_args(&mut self) -> Result<Vec<String>, String> {
        if !matches!(self.take_char(), Some('(')) {
            return Err("expected '('".to_string());
        }
        let args_start = self.pos;
        let mut depth = 1usize;
        let mut in_string = false;
        let mut escaped = false;
        while let Some(ch) = self.take_char() {
            if ch == '\\' && in_string {
                escaped = !escaped;
                continue;
            }
            if ch == '"' && !escaped {
                in_string = !in_string;
                continue;
            }
            escaped = false;
            if in_string {
                continue;
            }
            if ch == '(' {
                depth = depth.saturating_add(1);
            } else if ch == ')' {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let args_end = self.pos.saturating_sub(1);
                    return Ok(split_top_level_args(&self.source[args_start..args_end]));
                }
            }
        }
        Err("unterminated call arguments".to_string())
    }

    fn parse_call_expr(&mut self, ident: &str, args: &[String]) -> Result<IrExpr, String> {
        // Special handling for req.series and request.security
        if ident == "req.series" || ident == "request.security" {
            if args.len() < 3 {
                return Err(
                    "req.series requires (symbol, timeframe, field[, mode[, gaps[, lookahead]]])"
                        .to_string(),
                );
            }
            let mode = args
                .get(3)
                .map(|it| parse_text_argument(it))
                .unwrap_or_else(|| "confirmed".to_string());
            let gaps = args.get(4).map(|it| parse_text_argument(it));
            let lookahead = args.get(5).map(|it| parse_text_argument(it));

            let mut expr = IrExpr::ReqSeries {
                symbol: parse_text_argument(&args[0]),
                timeframe: parse_text_argument(&args[1]),
                field: parse_text_argument(&args[2]),
                mode,
                gaps,
                lookahead,
                index: None,
            };

            self.skip_ws();
            if matches!(self.peek_char(), Some('[')) {
                self.take_char();
                let index_expr = self.parse_expression()?;
                self.skip_ws();
                if !matches!(self.take_char(), Some(']')) {
                    return Err("expected ']'".to_string());
                }
                if let IrExpr::ReqSeries { index, .. } = &mut expr {
                    *index = Some(Box::new(index_expr));
                }
            }

            return Ok(expr);
        }

        // Generic function call - parse arguments as expressions and create FnCall
        let mut ir_args = Vec::with_capacity(args.len());
        for arg in args {
            let trimmed = arg.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Recursively parse each argument as an expression
            let mut arg_parser = ExprParser::new(trimmed);
            match arg_parser.parse_expression() {
                Ok(expr) => {
                    arg_parser.skip_ws();
                    if arg_parser.is_done() {
                        ir_args.push(expr);
                    } else {
                        // Couldn't fully parse - this is an error
                        return Err(format!("failed to parse argument '{}'", trimmed));
                    }
                }
                Err(e) => return Err(format!("failed to parse argument '{}': {}", trimmed, e)),
            }
        }

        Ok(IrExpr::FnCall {
            name: ident.to_string(),
            args: ir_args,
        })
    }

    fn parse_identifier(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if is_ident_part(ch) {
                self.take_char();
            } else {
                break;
            }
        }
        self.source[start..self.pos].to_string()
    }
}

fn split_top_level_args(raw_args: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut depth_brace = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut arg_start = 0usize;

    let bytes = raw_args.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'\\' && in_string {
            escaped = !escaped;
            i += 1;
            continue;
        }
        if ch == b'"' && !escaped {
            in_string = !in_string;
            i += 1;
            continue;
        }
        escaped = false;
        if in_string {
            i += 1;
            continue;
        }

        match ch {
            b'(' => depth_paren = depth_paren.saturating_add(1),
            b')' => depth_paren = depth_paren.saturating_sub(1),
            b'[' => depth_bracket = depth_bracket.saturating_add(1),
            b']' => depth_bracket = depth_bracket.saturating_sub(1),
            b'{' => depth_brace = depth_brace.saturating_add(1),
            b'}' => depth_brace = depth_brace.saturating_sub(1),
            b',' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                let arg = raw_args[arg_start..i].trim();
                if !arg.is_empty() {
                    args.push(arg.to_string());
                }
                arg_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    if arg_start <= raw_args.len() {
        let tail = raw_args[arg_start..].trim();
        if !tail.is_empty() {
            args.push(tail.to_string());
        }
    }

    args
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'
}

fn map_series_field(ident: &str) -> Option<IrSeriesField> {
    match ident {
        "open" | "ctx.open" => Some(IrSeriesField::Open),
        "high" | "ctx.high" => Some(IrSeriesField::High),
        "low" | "ctx.low" => Some(IrSeriesField::Low),
        "close" | "ctx.close" => Some(IrSeriesField::Close),
        "volume" | "ctx.volume" => Some(IrSeriesField::Volume),
        "time" | "ctx.time" => Some(IrSeriesField::Time),
        "bar_index" | "ctx.bar_index" => Some(IrSeriesField::BarIndex),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::lower_to_ir;
    use crate::core::indicators::compiler::ast::{
        AstAssign, AstBinaryOp, AstCall, AstExpr, AstFnDecl, AstProgram, AstStatement, AstSwitch,
        AstSwitchCase, AstVarDecl, AstWhile,
    };
    use crate::core::indicators::compiler::diagnostics::DiagnosticSeverity;
    use crate::core::indicators::language::CompileMode;
    use crate::core::indicators::{IrCallArg, IrCallKind, IrExpr};

    #[test]
    fn v2_rejects_missing_structured_expr_fallback() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![AstStatement::VarDecl(AstVarDecl {
                is_persistent: false,
                name: "x".to_string(),
                value: Some("1e3".to_string()),
                value_expr: None,
                line: 2,
                column: 1,
            })],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV2);
        assert!(
            lowered.diagnostics.iter().any(|diag| {
                diag.code == "INDL-1401" && matches!(diag.severity, DiagnosticSeverity::Error)
            }),
            "expected INDL-1401 v2 structured-expression error"
        );
    }

    #[test]
    fn v1_keeps_string_fallback_when_structured_expr_missing() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![AstStatement::VarDecl(AstVarDecl {
                is_persistent: false,
                name: "x".to_string(),
                value: Some("1e3".to_string()),
                value_expr: None,
                line: 2,
                column: 1,
            })],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV1);
        assert!(
            !lowered
                .diagnostics
                .iter()
                .any(|diag| matches!(diag.severity, DiagnosticSeverity::Error)),
            "did not expect v1 lowering errors"
        );
        let value_expr = lowered
            .calls
            .iter()
            .find(|call| call.kind == IrCallKind::StateLetDecl)
            .and_then(|call| call.args.get(1))
            .expect("expected let declaration value");
        match value_expr {
            IrCallArg::Expr(IrExpr::Number(value)) => {
                assert!(
                    (*value - 1000.0).abs() < f64::EPSILON,
                    "expected 1e3 fallback parse"
                );
            }
            _ => panic!("expected numeric expression for v1 fallback"),
        }
    }

    #[test]
    fn v2_rejects_unstructured_function_argument_fallback() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![
                AstStatement::FnDecl(AstFnDecl {
                    name: "f".to_string(),
                    params: vec!["v".to_string()],
                    body: vec![AstStatement::Call(AstCall {
                        function: "plot".to_string(),
                        args: vec!["v".to_string()],
                        arg_exprs: vec![None],
                        line: 3,
                        column: 3,
                    })],
                    line: 2,
                    column: 1,
                }),
                AstStatement::Call(AstCall {
                    function: "f".to_string(),
                    args: vec!["1e3".to_string()],
                    arg_exprs: vec![None],
                    line: 5,
                    column: 1,
                }),
            ],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV2);
        assert!(
            lowered.diagnostics.iter().any(|diag| {
                diag.code == "INDL-1401"
                    && diag.message.contains("function argument")
                    && matches!(diag.severity, DiagnosticSeverity::Error)
            }),
            "expected INDL-1401 for function argument fallback in v2"
        );
    }

    #[test]
    fn lowers_structured_modulo_and_power_ops() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![AstStatement::VarDecl(AstVarDecl {
                is_persistent: false,
                name: "x".to_string(),
                value: Some("2 % 3 ** 2".to_string()),
                value_expr: Some(AstExpr::Binary {
                    lhs: Box::new(AstExpr::Number(2.0)),
                    op: AstBinaryOp::Mod,
                    rhs: Box::new(AstExpr::Binary {
                        lhs: Box::new(AstExpr::Number(3.0)),
                        op: AstBinaryOp::Pow,
                        rhs: Box::new(AstExpr::Number(2.0)),
                    }),
                }),
                line: 2,
                column: 1,
            })],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV2);
        assert!(
            !lowered
                .diagnostics
                .iter()
                .any(|diag| matches!(diag.severity, DiagnosticSeverity::Error)),
            "did not expect lowering errors: {:?}",
            lowered.diagnostics
        );
        let value_expr = lowered
            .calls
            .iter()
            .find(|call| call.kind == IrCallKind::StateLetDecl)
            .and_then(|call| call.args.get(1))
            .expect("expected let declaration value");
        let IrCallArg::Expr(IrExpr::Binary { op, rhs, .. }) = value_expr else {
            panic!("expected binary expression");
        };
        assert_eq!(*op, crate::core::indicators::IrBinaryOp::Mod);
        let IrExpr::Binary { op: rhs_op, .. } = rhs.as_ref() else {
            panic!("expected rhs power expression");
        };
        assert_eq!(*rhs_op, crate::core::indicators::IrBinaryOp::Pow);
    }

    #[test]
    fn lowers_structured_ternary_expression() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![AstStatement::VarDecl(AstVarDecl {
                is_persistent: false,
                name: "x".to_string(),
                value: Some("true ? 1 : 2".to_string()),
                value_expr: Some(AstExpr::Conditional {
                    condition: Box::new(AstExpr::Bool(true)),
                    then_expr: Box::new(AstExpr::Number(1.0)),
                    else_expr: Box::new(AstExpr::Number(2.0)),
                }),
                line: 2,
                column: 1,
            })],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV2);
        assert!(
            !lowered
                .diagnostics
                .iter()
                .any(|diag| matches!(diag.severity, DiagnosticSeverity::Error)),
            "did not expect lowering errors: {:?}",
            lowered.diagnostics
        );
        let value_expr = lowered
            .calls
            .iter()
            .find(|call| call.kind == IrCallKind::StateLetDecl)
            .and_then(|call| call.args.get(1))
            .expect("expected let declaration value");
        let IrCallArg::Expr(IrExpr::Conditional {
            condition,
            then_expr,
            else_expr,
        }) = value_expr
        else {
            panic!("expected ternary expression");
        };
        assert!(matches!(condition.as_ref(), IrExpr::Bool(true)));
        assert!(matches!(then_expr.as_ref(), IrExpr::Number(1.0)));
        assert!(matches!(else_expr.as_ref(), IrExpr::Number(2.0)));
    }

    #[test]
    fn lowers_structured_variable_history_index_expr() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![AstStatement::VarDecl(AstVarDecl {
                is_persistent: false,
                name: "x_prev".to_string(),
                value: Some("x[1]".to_string()),
                value_expr: Some(AstExpr::VarIndexed {
                    name: "x".to_string(),
                    index: Box::new(AstExpr::Number(1.0)),
                }),
                line: 2,
                column: 1,
            })],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV2);
        assert!(
            !lowered
                .diagnostics
                .iter()
                .any(|diag| matches!(diag.severity, DiagnosticSeverity::Error)),
            "did not expect lowering errors: {:?}",
            lowered.diagnostics
        );
        let value_expr = lowered
            .calls
            .iter()
            .find(|call| call.kind == IrCallKind::StateLetDecl)
            .and_then(|call| call.args.get(1))
            .expect("expected let declaration value");
        let IrCallArg::Expr(IrExpr::VarIndexed { name, .. }) = value_expr else {
            panic!("expected indexed var expression");
        };
        assert_eq!(name, "x");
    }

    #[test]
    fn lowers_while_loop_with_guarded_body_calls() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![AstStatement::While(AstWhile {
                condition: "x < 2".to_string(),
                condition_expr: Some(AstExpr::Binary {
                    lhs: Box::new(AstExpr::Var("x".to_string())),
                    op: AstBinaryOp::Lt,
                    rhs: Box::new(AstExpr::Number(2.0)),
                }),
                body: vec![AstStatement::Assign(AstAssign {
                    name: "x".to_string(),
                    value: "x + 1".to_string(),
                    value_expr: Some(AstExpr::Binary {
                        lhs: Box::new(AstExpr::Var("x".to_string())),
                        op: AstBinaryOp::Add,
                        rhs: Box::new(AstExpr::Number(1.0)),
                    }),
                    line: 3,
                    column: 3,
                })],
                line: 2,
                column: 1,
            })],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV2);
        assert!(
            !lowered
                .diagnostics
                .iter()
                .any(|diag| matches!(diag.severity, DiagnosticSeverity::Error)),
            "did not expect lowering errors: {:?}",
            lowered.diagnostics
        );
        assert!(
            lowered
                .calls
                .iter()
                .any(|call| call.kind == IrCallKind::StateAssign && call.guard.is_some()),
            "expected guarded state assignment from while body"
        );
    }

    #[test]
    fn lowers_switch_to_guarded_case_calls() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![AstStatement::Switch(AstSwitch {
                subject: "x".to_string(),
                subject_expr: Some(AstExpr::Var("x".to_string())),
                cases: vec![AstSwitchCase {
                    value: "1".to_string(),
                    value_expr: Some(AstExpr::Number(1.0)),
                    body: vec![AstStatement::Assign(AstAssign {
                        name: "x".to_string(),
                        value: "2".to_string(),
                        value_expr: Some(AstExpr::Number(2.0)),
                        line: 4,
                        column: 3,
                    })],
                    line: 3,
                    column: 3,
                }],
                default_branch: vec![],
                line: 2,
                column: 1,
            })],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV2);
        assert!(
            !lowered
                .diagnostics
                .iter()
                .any(|diag| matches!(diag.severity, DiagnosticSeverity::Error)),
            "did not expect lowering errors: {:?}",
            lowered.diagnostics
        );
        assert!(
            lowered
                .calls
                .iter()
                .any(|call| call.kind == IrCallKind::StateAssign && call.guard.is_some()),
            "expected guarded state assignment from switch case"
        );
    }

    #[test]
    fn rejects_recursive_function_call() {
        let program = AstProgram {
            name: Some("t".to_string()),
            inputs: Vec::new(),
            statements: vec![
                AstStatement::FnDecl(AstFnDecl {
                    name: "recurse".to_string(),
                    params: vec!["n".to_string()],
                    body: vec![AstStatement::Call(AstCall {
                        function: "recurse".to_string(),
                        args: vec!["n".to_string()],
                        arg_exprs: vec![Some(AstExpr::Var("n".to_string()))],
                        line: 3,
                        column: 3,
                    })],
                    line: 2,
                    column: 1,
                }),
                AstStatement::Call(AstCall {
                    function: "recurse".to_string(),
                    args: vec!["5".to_string()],
                    arg_exprs: vec![Some(AstExpr::Number(5.0))],
                    line: 5,
                    column: 1,
                }),
            ],
        };

        let lowered = lower_to_ir(&program, CompileMode::RayDslV2);
        assert!(
            lowered.diagnostics.iter().any(|diag| {
                diag.code == "INDL-1302"
                    && diag.message.contains("recursive")
                    && matches!(diag.severity, DiagnosticSeverity::Error)
            }),
            "expected INDL-1302 error for recursive call: {:?}",
            lowered.diagnostics
        );
    }
}
