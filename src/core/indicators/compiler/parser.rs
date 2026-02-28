use crate::core::indicators::compiler::ast::{
    AstAssign, AstCall, AstFnDecl, AstForLoop, AstIf, AstInputDecl, AstProgram, AstReturn,
    AstStatement, AstVarDecl,
};
use crate::core::indicators::compiler::diagnostics::{
    CompileDiagnostic, DiagnosticSeverity, SourceSpan,
};
use crate::core::indicators::compiler::types::Token;

pub fn parse_program(
    _tokens: &[Token],
    source: &str,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstProgram> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut cursor = 0usize;
    let mut name = None;
    let mut inputs = Vec::new();
    let mut statements = Vec::new();

    while cursor < lines.len() {
        let ln = cursor + 1;
        let trimmed = lines[cursor].trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            cursor = cursor.saturating_add(1);
            continue;
        }

        if trimmed.starts_with("indicator(") {
            name = extract_indicator_name(trimmed);
            cursor = cursor.saturating_add(1);
            continue;
        }

        if trimmed.starts_with("input.") {
            if let Some(input_decl) = parse_input_decl(trimmed) {
                inputs.push(input_decl);
            } else {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1100".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "invalid input declaration".to_string(),
                    hint: Some("expected input.<type>(\"name\", default=<value>)".to_string()),
                    span: Some(SourceSpan {
                        line: ln,
                        column: 1,
                        len: trimmed.len(),
                    }),
                });
            }
            cursor = cursor.saturating_add(1);
            continue;
        }

        if let Some(statement) = parse_statement(&lines, &mut cursor, diagnostics) {
            statements.push(statement);
        }
    }

    if statements.is_empty() {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1101".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "program contains no executable statements".to_string(),
            hint: Some("add at least one statement".to_string()),
            span: Some(SourceSpan {
                line: 1,
                column: 1,
                len: 0,
            }),
        });
        return None;
    }

    Some(AstProgram {
        name,
        inputs,
        statements,
    })
}

fn parse_statement(
    lines: &[&str],
    cursor: &mut usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstStatement> {
    if *cursor >= lines.len() {
        return None;
    }

    let line_no = (*cursor).saturating_add(1);
    let trimmed = lines[*cursor].trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        *cursor = (*cursor).saturating_add(1);
        return None;
    }

    if let Some(statement) = parse_fn_decl(lines, cursor, diagnostics) {
        return Some(statement);
    }
    if let Some(statement) = parse_if_statement(lines, cursor, diagnostics) {
        return Some(statement);
    }
    if let Some(statement) = parse_for_loop_statement(lines, cursor, diagnostics) {
        return Some(statement);
    }
    if let Some(statement) = parse_var_decl_statement(trimmed, line_no) {
        *cursor = (*cursor).saturating_add(1);
        return Some(statement);
    }
    if let Some(statement) = parse_return_statement(trimmed, line_no) {
        *cursor = (*cursor).saturating_add(1);
        return Some(statement);
    }
    if let Some(statement) = parse_assign_statement(trimmed, line_no) {
        *cursor = (*cursor).saturating_add(1);
        return Some(statement);
    }
    if let Some(call) = parse_call_statement(trimmed, line_no) {
        *cursor = (*cursor).saturating_add(1);
        return Some(AstStatement::Call(call));
    }

    diagnostics.push(CompileDiagnostic {
        code: "INDL-1102".to_string(),
        severity: DiagnosticSeverity::Error,
        message: "unsupported statement in indicator script".to_string(),
        hint: Some("use supported v1 forms such as input.*, var/let, if/for/fn blocks, plot/viz.*, req.series, and obj.* calls".to_string()),
        span: Some(SourceSpan {
            line: line_no,
            column: 1,
            len: trimmed.len(),
        }),
    });
    *cursor = (*cursor).saturating_add(1);
    None
}

fn parse_fn_decl(
    lines: &[&str],
    cursor: &mut usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstStatement> {
    let line_no = (*cursor).saturating_add(1);
    let trimmed = lines[*cursor].trim();
    if !trimmed.starts_with("fn ") {
        return None;
    }

    let header = trimmed.trim_start_matches("fn ").trim();
    let open = header.find('(');
    let close = header.rfind(')');
    let (Some(open), Some(close)) = (open, close) else {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1102".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "unsupported statement in indicator script".to_string(),
            hint: Some("function declarations must use `fn name(arg, ...) { ... }`".to_string()),
            span: Some(SourceSpan {
                line: line_no,
                column: 1,
                len: trimmed.len(),
            }),
        });
        *cursor = (*cursor).saturating_add(1);
        return None;
    };
    if close <= open {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1102".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "unsupported statement in indicator script".to_string(),
            hint: Some("function declarations must use `fn name(arg, ...) { ... }`".to_string()),
            span: Some(SourceSpan {
                line: line_no,
                column: 1,
                len: trimmed.len(),
            }),
        });
        *cursor = (*cursor).saturating_add(1);
        return None;
    }

    let fn_name = header[..open].trim().to_string();
    let params_raw = &header[(open + 1)..close];
    let params = if params_raw.trim().is_empty() {
        Vec::new()
    } else {
        params_raw
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    };
    let tail = header[(close + 1)..].trim();
    let body = parse_block_after_header(lines, cursor, tail, line_no, "function", diagnostics);

    Some(AstStatement::FnDecl(AstFnDecl {
        name: fn_name,
        params,
        body,
        line: line_no,
        column: 1,
    }))
}

fn parse_if_statement(
    lines: &[&str],
    cursor: &mut usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstStatement> {
    let line_no = (*cursor).saturating_add(1);
    let trimmed = lines[*cursor].trim();
    if !trimmed.starts_with("if ") {
        return None;
    }

    let header = trimmed.trim_start_matches("if ").trim();
    let (condition, tail) = split_header_condition_and_tail(header);
    let then_branch = parse_block_after_header(lines, cursor, tail, line_no, "if", diagnostics);
    let else_branch = parse_optional_else_branch(lines, cursor, diagnostics);

    Some(AstStatement::If(AstIf {
        condition,
        then_branch,
        else_branch,
        line: line_no,
        column: 1,
    }))
}

fn parse_optional_else_branch(
    lines: &[&str],
    cursor: &mut usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Vec<AstStatement> {
    let mut probe = *cursor;
    while probe < lines.len() {
        let trimmed = lines[probe].trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            probe = probe.saturating_add(1);
            continue;
        }
        let normalized = trim_leading_block_close(trimmed);
        if !normalized.starts_with("else") {
            return Vec::new();
        }

        let line_no = probe.saturating_add(1);
        let tail = normalized.trim_start_matches("else").trim();
        if tail.starts_with("if ") {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1105".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "else-if requires explicit nested if block in v1".to_string(),
                hint: Some("rewrite as `else { if ... { ... } }`".to_string()),
                span: Some(SourceSpan {
                    line: line_no,
                    column: 1,
                    len: trimmed.len(),
                }),
            });
            *cursor = probe.saturating_add(1);
            return Vec::new();
        }

        *cursor = probe;
        return parse_block_after_header(lines, cursor, tail, line_no, "else", diagnostics);
    }
    Vec::new()
}

fn parse_for_loop_statement(
    lines: &[&str],
    cursor: &mut usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstStatement> {
    let line_no = (*cursor).saturating_add(1);
    let trimmed = lines[*cursor].trim();
    if !trimmed.starts_with("for ") {
        return None;
    }

    let header = trimmed.trim_start_matches("for ").trim();
    let (range_part, tail) = split_header_condition_and_tail(header);
    let tokens = range_part.split_whitespace().collect::<Vec<_>>();
    if tokens.len() < 5 || tokens[1] != "=" || tokens[3] != "to" {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1102".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "unsupported statement in indicator script".to_string(),
            hint: Some("for loops must use `for i = <start> to <end> { ... }`".to_string()),
            span: Some(SourceSpan {
                line: line_no,
                column: 1,
                len: trimmed.len(),
            }),
        });
        *cursor = (*cursor).saturating_add(1);
        return None;
    }
    let iterator = tokens[0].to_string();
    let Some(start) = tokens[2].parse::<usize>().ok() else {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1102".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "unsupported statement in indicator script".to_string(),
            hint: Some("for loop start bound must be a static integer literal".to_string()),
            span: Some(SourceSpan {
                line: line_no,
                column: 1,
                len: trimmed.len(),
            }),
        });
        *cursor = (*cursor).saturating_add(1);
        return None;
    };
    let Some(end) = tokens[4].parse::<usize>().ok() else {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1102".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "unsupported statement in indicator script".to_string(),
            hint: Some("for loop end bound must be a static integer literal".to_string()),
            span: Some(SourceSpan {
                line: line_no,
                column: 1,
                len: trimmed.len(),
            }),
        });
        *cursor = (*cursor).saturating_add(1);
        return None;
    };
    let body = if tail.is_empty() {
        *cursor = (*cursor).saturating_add(1);
        Vec::new()
    } else {
        parse_block_after_header(lines, cursor, tail, line_no, "for", diagnostics)
    };

    Some(AstStatement::For(AstForLoop {
        iterator,
        start,
        end,
        body,
        line: line_no,
        column: 1,
    }))
}

fn parse_var_decl_statement(line: &str, line_no: usize) -> Option<AstStatement> {
    let (is_persistent, prefix) = if line.starts_with("var ") {
        (true, "var ")
    } else if line.starts_with("let ") {
        (false, "let ")
    } else {
        return None;
    };
    let rest = line.trim_start_matches(prefix).trim();
    if rest.is_empty() {
        return None;
    }
    let (name, value) = if let Some((name, value)) = rest.split_once('=') {
        (
            name.trim().to_string(),
            Some(value.trim().trim_end_matches(';').trim().to_string()),
        )
    } else {
        (rest.trim_end_matches(';').trim().to_string(), None)
    };
    Some(AstStatement::VarDecl(AstVarDecl {
        is_persistent,
        name,
        value,
        line: line_no,
        column: 1,
    }))
}

fn parse_assign_statement(line: &str, line_no: usize) -> Option<AstStatement> {
    let equals_index = line.find('=')?;
    if line.get(equals_index.saturating_sub(1)..=equals_index) == Some("==")
        || line.get(equals_index..=equals_index.saturating_add(1)) == Some("==")
    {
        return None;
    }
    let left = line[..equals_index].trim();
    let right = line[(equals_index + 1)..].trim().trim_end_matches(';').trim();
    if left.is_empty() || right.is_empty() || !is_assignment_target(left) {
        return None;
    }
    Some(AstStatement::Assign(AstAssign {
        name: left.to_string(),
        value: right.to_string(),
        line: line_no,
        column: 1,
    }))
}

fn parse_return_statement(line: &str, line_no: usize) -> Option<AstStatement> {
    if !line.starts_with("return") {
        return None;
    }
    let rest = line.trim_start_matches("return").trim().trim_end_matches(';');
    let value = if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    };
    Some(AstStatement::Return(AstReturn {
        value,
        line: line_no,
        column: 1,
    }))
}

fn parse_block_after_header(
    lines: &[&str],
    cursor: &mut usize,
    tail: &str,
    line_no: usize,
    context: &str,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Vec<AstStatement> {
    if tail != "{" {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1103".to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!("{} block must end header with '{{'", context),
            hint: Some("use braces for multi-statement blocks in v1".to_string()),
            span: Some(SourceSpan {
                line: line_no,
                column: 1,
                len: lines[*cursor].trim().len(),
            }),
        });
        *cursor = (*cursor).saturating_add(1);
        return Vec::new();
    }

    *cursor = (*cursor).saturating_add(1);
    parse_block(lines, cursor, diagnostics, context)
}

fn parse_block(
    lines: &[&str],
    cursor: &mut usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
    context: &str,
) -> Vec<AstStatement> {
    let block_start_line = (*cursor).saturating_add(1);
    let mut body = Vec::new();
    while *cursor < lines.len() {
        let trimmed = lines[*cursor].trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            *cursor = (*cursor).saturating_add(1);
            continue;
        }
        if trimmed == "}" {
            *cursor = (*cursor).saturating_add(1);
            return body;
        }
        // Support inline style: `} else {` by ending the current block here.
        if trimmed.starts_with('}') && trim_leading_block_close(trimmed).starts_with("else") {
            return body;
        }
        if let Some(statement) = parse_statement(lines, cursor, diagnostics) {
            body.push(statement);
        }
    }

    diagnostics.push(CompileDiagnostic {
        code: "INDL-1104".to_string(),
        severity: DiagnosticSeverity::Error,
        message: format!("unterminated {} block", context),
        hint: Some("add a closing `}`".to_string()),
        span: Some(SourceSpan {
            line: block_start_line,
            column: 1,
            len: 1,
        }),
    });
    body
}

fn trim_leading_block_close(input: &str) -> &str {
    if let Some(rest) = input.strip_prefix('}') {
        rest.trim_start()
    } else {
        input
    }
}

fn split_header_condition_and_tail(header: &str) -> (String, &str) {
    let trimmed = header.trim();
    if let Some(prefix) = trimmed.strip_suffix('{') {
        (prefix.trim().to_string(), "{")
    } else {
        (trimmed.to_string(), "")
    }
}

fn is_assignment_target(target: &str) -> bool {
    let mut chars = target.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn extract_indicator_name(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_input_decl(line: &str) -> Option<AstInputDecl> {
    let type_start = "input.".len();
    let open_paren = line[type_start..].find('(')? + type_start;
    let type_name = line[type_start..open_paren].trim().to_string();

    let first_quote = line.find('"')?;
    let after_first_quote = &line[first_quote + 1..];
    let second_quote = after_first_quote.find('"')?;
    let name = after_first_quote[..second_quote].to_string();

    let default_value = if let Some(default_idx) = line.find("default=") {
        let default_raw = line[(default_idx + "default=".len())..]
            .trim()
            .trim_end_matches(')')
            .trim();
        parse_default_value(default_raw)
    } else {
        serde_json::Value::Null
    };

    Some(AstInputDecl {
        name,
        type_name,
        default_value,
    })
}

fn parse_default_value(raw: &str) -> serde_json::Value {
    if raw.eq_ignore_ascii_case("true") {
        return serde_json::Value::Bool(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return serde_json::Value::Bool(false);
    }
    if let Ok(v) = raw.parse::<i64>() {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = raw.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(v) {
            return serde_json::Value::Number(n);
        }
    }
    let trimmed = raw.trim_matches('"');
    serde_json::Value::String(trimmed.to_string())
}

fn parse_call_statement(line: &str, line_no: usize) -> Option<AstCall> {
    let open = line.find('(')?;
    if !line.ends_with(')') || open == 0 {
        return None;
    }
    let function = line[..open].trim();
    if function.is_empty() {
        return None;
    }

    let args_str = &line[(open + 1)..(line.len().saturating_sub(1))];
    let args = split_top_level_args(args_str);
    Some(AstCall {
        function: function.to_string(),
        args,
        line: line_no,
        column: open + 1,
    })
}

fn split_top_level_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for ch in input.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                current.push(ch);
            }
            '(' => {
                depth_paren = depth_paren.saturating_add(1);
                current.push(ch);
            }
            ')' => {
                depth_paren = depth_paren.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                depth_bracket = depth_bracket.saturating_add(1);
                current.push(ch);
            }
            ']' => {
                depth_bracket = depth_bracket.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth_paren == 0 && depth_bracket == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    args.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        args.push(trimmed.to_string());
    }
    args
}
