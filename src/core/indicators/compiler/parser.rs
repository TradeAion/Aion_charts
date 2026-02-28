use crate::core::indicators::compiler::ast::{
    AstAssign, AstBinaryOp, AstCall, AstExpr, AstFnDecl, AstForLoop, AstIf, AstInputDecl,
    AstProgram, AstReturn, AstSeriesField, AstStatement, AstSwitch, AstSwitchCase, AstTupleAssign,
    AstUnaryOp, AstVarDecl, AstWhile, IndicatorDecl,
};
use crate::core::indicators::compiler::diagnostics::{
    CompileDiagnostic, DiagnosticSeverity, SourceSpan,
};
use crate::core::indicators::compiler::lexer;
use crate::core::indicators::compiler::types::{
    DelimiterKind, KeywordKind, OperatorKind, Token, TokenKind,
};

pub fn parse_program(
    tokens: &[Token],
    _source: &str,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstProgram> {
    validate_token_stream(tokens, diagnostics);

    let error_count_before = diagnostics
        .iter()
        .filter(|diag| matches!(diag.severity, DiagnosticSeverity::Error))
        .count();
    if let Some(program) = parse_program_token_cursor(tokens, diagnostics) {
        return Some(program);
    }
    let error_count_after = diagnostics
        .iter()
        .filter(|diag| matches!(diag.severity, DiagnosticSeverity::Error))
        .count();
    if error_count_after > error_count_before {
        return None;
    }

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
    None
}

fn validate_token_stream(tokens: &[Token], diagnostics: &mut Vec<CompileDiagnostic>) {
    let mut stack = Vec::<(DelimiterKind, usize, usize)>::new();
    for token in tokens {
        let TokenKind::Delimiter(kind) = token.kind else {
            continue;
        };
        match kind {
            DelimiterKind::LParen | DelimiterKind::LBracket | DelimiterKind::LBrace => {
                stack.push((kind, token.line, token.column));
            }
            DelimiterKind::RParen | DelimiterKind::RBracket | DelimiterKind::RBrace => {
                let Some((open, _open_line, _open_column)) = stack.pop() else {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1106".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!("unmatched closing delimiter '{}'", token.lexeme),
                        hint: Some("remove the delimiter or add the matching opener".to_string()),
                        span: Some(SourceSpan {
                            line: token.line,
                            column: token.column,
                            len: token.lexeme.len().max(1),
                        }),
                    });
                    continue;
                };
                if !is_matching_delimiter(open, kind) {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1106".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "mismatched delimiters: opener '{}' does not match closer '{}'",
                            delimiter_lexeme(open),
                            token.lexeme
                        ),
                        hint: Some(
                            "fix delimiter pairing to balance blocks and expressions".to_string(),
                        ),
                        span: Some(SourceSpan {
                            line: token.line,
                            column: token.column,
                            len: token.lexeme.len().max(1),
                        }),
                    });
                }
            }
            DelimiterKind::Comma | DelimiterKind::Dot | DelimiterKind::Semicolon => {}
        }
    }

    for (open, line, column) in stack.into_iter().rev() {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1106".to_string(),
            severity: DiagnosticSeverity::Error,
            message: format!("unclosed delimiter '{}'", delimiter_lexeme(open)),
            hint: Some("add the matching closing delimiter".to_string()),
            span: Some(SourceSpan {
                line,
                column,
                len: 1,
            }),
        });
    }
}

fn is_matching_delimiter(open: DelimiterKind, close: DelimiterKind) -> bool {
    matches!(
        (open, close),
        (DelimiterKind::LParen, DelimiterKind::RParen)
            | (DelimiterKind::LBracket, DelimiterKind::RBracket)
            | (DelimiterKind::LBrace, DelimiterKind::RBrace)
    )
}

fn delimiter_lexeme(kind: DelimiterKind) -> &'static str {
    match kind {
        DelimiterKind::LParen => "(",
        DelimiterKind::RParen => ")",
        DelimiterKind::LBracket => "[",
        DelimiterKind::RBracket => "]",
        DelimiterKind::LBrace => "{",
        DelimiterKind::RBrace => "}",
        DelimiterKind::Comma => ",",
        DelimiterKind::Dot => ".",
        DelimiterKind::Semicolon => ";",
    }
}

fn parse_program_token_cursor(
    tokens: &[Token],
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstProgram> {
    let mut parser = ProgramTokenCursor::new(tokens);
    parser.parse_program(diagnostics)
}

struct ProgramTokenCursor<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> ProgramTokenCursor<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_program(&mut self, diagnostics: &mut Vec<CompileDiagnostic>) -> Option<AstProgram> {
        let mut name = None;
        let mut indicator_decl = IndicatorDecl::default();
        let mut inputs = Vec::new();
        let mut statements = Vec::new();

        loop {
            self.skip_trivia();
            let Some(token) = self.peek().cloned() else {
                break;
            };
            if matches!(token.kind, TokenKind::Eof) {
                break;
            }

            if self.is_indicator_start() {
                let statement_tokens = self.collect_statement_tokens();
                let header = tokens_to_source(&statement_tokens);
                (name, indicator_decl) = parse_indicator_decl(&header);
                continue;
            }

            if self.is_input_start() {
                let statement_tokens = self.collect_statement_tokens();
                let header = tokens_to_source(&statement_tokens);
                if let Some(input_decl) = parse_input_decl(&header) {
                    inputs.push(input_decl);
                } else {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1100".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "invalid input declaration".to_string(),
                        hint: Some("expected input.<type>(\"name\", default=<value>)".to_string()),
                        span: Some(SourceSpan {
                            line: token.line,
                            column: token.column,
                            len: header.len().max(1),
                        }),
                    });
                }
                continue;
            }

            if let Some(statement) = self.parse_statement(diagnostics) {
                statements.push(statement);
            } else if self.consume_delimiter(DelimiterKind::RBrace) {
                continue;
            } else {
                self.advance_to_statement_boundary();
            }
        }

        if statements.is_empty() {
            return None;
        }

        Some(AstProgram {
            name,
            indicator_decl,
            inputs,
            statements,
        })
    }

    fn parse_statement(
        &mut self,
        diagnostics: &mut Vec<CompileDiagnostic>,
    ) -> Option<AstStatement> {
        self.skip_trivia();
        let token = self.peek()?.clone();
        match token.kind {
            TokenKind::Keyword(KeywordKind::Fn) => self.parse_fn_decl(diagnostics),
            TokenKind::Keyword(KeywordKind::If) => self.parse_if_statement(diagnostics),
            TokenKind::Keyword(KeywordKind::Switch) => self.parse_switch_statement(diagnostics),
            TokenKind::Keyword(KeywordKind::While) => self.parse_while_statement(diagnostics),
            TokenKind::Keyword(KeywordKind::For) => self.parse_for_loop_statement(diagnostics),
            TokenKind::Keyword(KeywordKind::Var)
            | TokenKind::Keyword(KeywordKind::Let)
            | TokenKind::Keyword(KeywordKind::Return)
            | TokenKind::Identifier
            | TokenKind::Delimiter(DelimiterKind::LBracket) => {
                self.parse_simple_statement(diagnostics)
            }
            TokenKind::Delimiter(DelimiterKind::RBrace) | TokenKind::Eof => None,
            _ => {
                let unsupported = self.collect_statement_tokens();
                let raw = tokens_to_source(&unsupported);
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1102".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "unsupported statement in indicator script".to_string(),
                    hint: Some("use supported v1 forms such as input.*, var/let, if/switch/while/for/fn blocks, plot/viz.*, req.series, and obj.* calls".to_string()),
                    span: Some(SourceSpan {
                        line: token.line,
                        column: token.column,
                        len: raw.len().max(1),
                    }),
                });
                None
            }
        }
    }

    fn parse_fn_decl(&mut self, diagnostics: &mut Vec<CompileDiagnostic>) -> Option<AstStatement> {
        let Some(fn_token) = self.bump().cloned() else {
            return None;
        };
        let (header, has_block) = self.collect_header_until_block_start();
        if !has_block {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1103".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "function block must end header with '{'".to_string(),
                hint: Some("use braces for multi-statement blocks in v1".to_string()),
                span: Some(SourceSpan {
                    line: fn_token.line,
                    column: fn_token.column,
                    len: 1,
                }),
            });
            return None;
        }

        let Some((name, params)) = parse_fn_signature_tokens(&header) else {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1102".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "unsupported statement in indicator script".to_string(),
                hint: Some(
                    "function declarations must use `fn name(arg, ...) { ... }`".to_string(),
                ),
                span: Some(SourceSpan {
                    line: fn_token.line,
                    column: fn_token.column,
                    len: tokens_to_source(&header).len().max(1),
                }),
            });
            self.consume_block_recovery();
            return None;
        };

        let body = self.parse_block("function", diagnostics, fn_token.line, fn_token.column);
        Some(AstStatement::FnDecl(AstFnDecl {
            name,
            params,
            body,
            line: fn_token.line,
            column: fn_token.column,
        }))
    }

    fn parse_if_statement(
        &mut self,
        diagnostics: &mut Vec<CompileDiagnostic>,
    ) -> Option<AstStatement> {
        let Some(if_token) = self.bump().cloned() else {
            return None;
        };
        let (condition_tokens, has_block) = self.collect_header_until_block_start();
        if !has_block {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1103".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "if block must end header with '{'".to_string(),
                hint: Some("use braces for multi-statement blocks in v1".to_string()),
                span: Some(SourceSpan {
                    line: if_token.line,
                    column: if_token.column,
                    len: 1,
                }),
            });
            return None;
        }

        let condition = tokens_to_source(&condition_tokens);
        let condition_expr = parse_expression_ast_tokens(&condition_tokens);
        let then_branch = self.parse_block("if", diagnostics, if_token.line, if_token.column);

        self.skip_trivia();
        let mut else_branch = Vec::new();
        if self.consume_keyword(KeywordKind::Else) {
            self.skip_trivia();
            if self.consume_keyword(KeywordKind::If) {
                self.pos = self.pos.saturating_sub(1);
                if let Some(nested_if) = self.parse_if_statement(diagnostics) {
                    else_branch.push(nested_if);
                }
            } else if self.consume_delimiter(DelimiterKind::LBrace) {
                else_branch = self.parse_block("else", diagnostics, if_token.line, if_token.column);
            } else {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1103".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "else block must end header with '{'".to_string(),
                    hint: Some("use braces for multi-statement blocks in v1".to_string()),
                    span: Some(SourceSpan {
                        line: if_token.line,
                        column: if_token.column,
                        len: 1,
                    }),
                });
            }
        }

        Some(AstStatement::If(AstIf {
            condition,
            condition_expr,
            then_branch,
            else_branch,
            line: if_token.line,
            column: if_token.column,
        }))
    }

    fn parse_switch_statement(
        &mut self,
        diagnostics: &mut Vec<CompileDiagnostic>,
    ) -> Option<AstStatement> {
        let Some(switch_token) = self.bump().cloned() else {
            return None;
        };
        let (subject_tokens, has_block) = self.collect_header_until_block_start();
        if !has_block {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1103".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "switch block must end header with '{'".to_string(),
                hint: Some("use braces for multi-statement blocks in v1".to_string()),
                span: Some(SourceSpan {
                    line: switch_token.line,
                    column: switch_token.column,
                    len: 1,
                }),
            });
            return None;
        }

        let subject = tokens_to_source(&subject_tokens);
        let subject_expr = parse_expression_ast_tokens(&subject_tokens);
        let mut cases = Vec::<AstSwitchCase>::new();
        let mut default_branch = Vec::<AstStatement>::new();

        loop {
            self.skip_trivia();
            let Some(token) = self.peek().cloned() else {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1104".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "unterminated switch block".to_string(),
                    hint: Some("add a closing `}`".to_string()),
                    span: Some(SourceSpan {
                        line: switch_token.line,
                        column: switch_token.column,
                        len: 1,
                    }),
                });
                break;
            };
            if matches!(token.kind, TokenKind::Eof) {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1104".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "unterminated switch block".to_string(),
                    hint: Some("add a closing `}`".to_string()),
                    span: Some(SourceSpan {
                        line: switch_token.line,
                        column: switch_token.column,
                        len: 1,
                    }),
                });
                break;
            }
            if self.consume_delimiter(DelimiterKind::RBrace) {
                break;
            }

            if self.consume_keyword(KeywordKind::Case) {
                let (value_tokens, has_case_block) = self.collect_header_until_block_start();
                if !has_case_block {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1103".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "case block must end header with '{'".to_string(),
                        hint: Some("use braces for multi-statement blocks in v1".to_string()),
                        span: Some(SourceSpan {
                            line: token.line,
                            column: token.column,
                            len: 1,
                        }),
                    });
                    self.advance_to_statement_boundary();
                    continue;
                }
                let value = tokens_to_source(&value_tokens);
                let value_expr = parse_expression_ast_tokens(&value_tokens);
                let body = self.parse_block("case", diagnostics, token.line, token.column);
                cases.push(AstSwitchCase {
                    value,
                    value_expr,
                    body,
                    line: token.line,
                    column: token.column,
                });
                continue;
            }

            if self.consume_keyword(KeywordKind::Default) {
                let has_default_block = self.consume_delimiter(DelimiterKind::LBrace);
                if !has_default_block {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1103".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "default block must end header with '{'".to_string(),
                        hint: Some("use braces for multi-statement blocks in v1".to_string()),
                        span: Some(SourceSpan {
                            line: token.line,
                            column: token.column,
                            len: 1,
                        }),
                    });
                    self.advance_to_statement_boundary();
                    continue;
                }
                default_branch = self.parse_block("default", diagnostics, token.line, token.column);
                continue;
            }

            diagnostics.push(CompileDiagnostic {
                code: "INDL-1102".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "unsupported statement in switch block".to_string(),
                hint: Some("use `case <expr> { ... }` or `default { ... }`".to_string()),
                span: Some(SourceSpan {
                    line: token.line,
                    column: token.column,
                    len: token.lexeme.len().max(1),
                }),
            });
            self.advance_to_statement_boundary();
        }

        Some(AstStatement::Switch(AstSwitch {
            subject,
            subject_expr,
            cases,
            default_branch,
            line: switch_token.line,
            column: switch_token.column,
        }))
    }

    fn parse_while_statement(
        &mut self,
        diagnostics: &mut Vec<CompileDiagnostic>,
    ) -> Option<AstStatement> {
        let Some(while_token) = self.bump().cloned() else {
            return None;
        };
        let (condition_tokens, has_block) = self.collect_header_until_block_start();
        if !has_block {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1103".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "while block must end header with '{'".to_string(),
                hint: Some("use braces for multi-statement blocks in v1".to_string()),
                span: Some(SourceSpan {
                    line: while_token.line,
                    column: while_token.column,
                    len: 1,
                }),
            });
            return None;
        }

        let condition = tokens_to_source(&condition_tokens);
        let condition_expr = parse_expression_ast_tokens(&condition_tokens);
        let body = self.parse_block("while", diagnostics, while_token.line, while_token.column);
        Some(AstStatement::While(AstWhile {
            condition,
            condition_expr,
            body,
            line: while_token.line,
            column: while_token.column,
        }))
    }

    fn parse_for_loop_statement(
        &mut self,
        diagnostics: &mut Vec<CompileDiagnostic>,
    ) -> Option<AstStatement> {
        let Some(for_token) = self.bump().cloned() else {
            return None;
        };
        let (header_tokens, has_block) = self.collect_header_until_block_start();
        if !has_block {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1103".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "for block must end header with '{'".to_string(),
                hint: Some("use braces for multi-statement blocks in v1".to_string()),
                span: Some(SourceSpan {
                    line: for_token.line,
                    column: for_token.column,
                    len: 1,
                }),
            });
            return None;
        }

        let header_raw = tokens_to_source(&header_tokens);
        let parts = header_raw.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 5 || parts[1] != "=" || !parts[3].eq_ignore_ascii_case("to") {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1102".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "unsupported statement in indicator script".to_string(),
                hint: Some("for loops must use `for i = <start> to <end> { ... }`".to_string()),
                span: Some(SourceSpan {
                    line: for_token.line,
                    column: for_token.column,
                    len: header_raw.len().max(1),
                }),
            });
            self.consume_block_recovery();
            return None;
        }

        let iterator = parts[0].to_string();
        let start_raw = parts[2];
        let end_raw = parts[4];

        // Try to parse as static integers (required for now; dynamic bounds = Phase 2)
        let start = start_raw.parse::<usize>().unwrap_or(0);
        let end = end_raw.parse::<usize>().unwrap_or(start);

        // Parse expressions for future dynamic support
        let start_expr = parse_expression_ast(start_raw, for_token.line, 1, diagnostics);
        let end_expr = parse_expression_ast(end_raw, for_token.line, 1, diagnostics);

        // Validate static bounds are provided
        if start_raw.parse::<usize>().is_err() {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1102".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "unsupported statement in indicator script".to_string(),
                hint: Some("for loop start bound must be a static integer literal (dynamic bounds coming in Phase 2)".to_string()),
                span: Some(SourceSpan {
                    line: for_token.line,
                    column: for_token.column,
                    len: header_raw.len().max(1),
                }),
            });
            self.consume_block_recovery();
            return None;
        }
        if end_raw.parse::<usize>().is_err() {
            diagnostics.push(CompileDiagnostic {
                code: "INDL-1102".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "unsupported statement in indicator script".to_string(),
                hint: Some("for loop end bound must be a static integer literal (dynamic bounds coming in Phase 2)".to_string()),
                span: Some(SourceSpan {
                    line: for_token.line,
                    column: for_token.column,
                    len: header_raw.len().max(1),
                }),
            });
            self.consume_block_recovery();
            return None;
        }

        let body = self.parse_block("for", diagnostics, for_token.line, for_token.column);
        Some(AstStatement::For(AstForLoop {
            iterator,
            start,
            end,
            start_expr,
            end_expr,
            body,
            line: for_token.line,
            column: for_token.column,
        }))
    }

    fn parse_simple_statement(
        &mut self,
        diagnostics: &mut Vec<CompileDiagnostic>,
    ) -> Option<AstStatement> {
        let statement_tokens = self.collect_statement_tokens();
        if statement_tokens.is_empty() {
            return None;
        }
        let line_no = statement_tokens
            .first()
            .map(|token| token.line)
            .unwrap_or(1);
        let raw = tokens_to_source(&statement_tokens);
        if let Some(statement) = parse_var_decl_statement(&raw, line_no, diagnostics) {
            return Some(statement);
        }
        if let Some(statement) = parse_return_statement(&raw, line_no, diagnostics) {
            return Some(statement);
        }
        if let Some(statement) = parse_assign_statement(&raw, line_no, diagnostics) {
            return Some(statement);
        }
        if let Some(call) = parse_call_statement(&raw, line_no, diagnostics) {
            return Some(AstStatement::Call(call));
        }

        diagnostics.push(CompileDiagnostic {
            code: "INDL-1102".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "unsupported statement in indicator script".to_string(),
            hint: Some("use supported v1 forms such as input.*, var/let, if/switch/while/for/fn blocks, plot/viz.*, req.series, and obj.* calls".to_string()),
            span: Some(SourceSpan {
                line: line_no,
                column: 1,
                len: raw.len().max(1),
            }),
        });
        None
    }

    fn parse_block(
        &mut self,
        context: &str,
        diagnostics: &mut Vec<CompileDiagnostic>,
        block_line: usize,
        block_column: usize,
    ) -> Vec<AstStatement> {
        let mut body = Vec::new();
        loop {
            self.skip_trivia();
            let Some(token) = self.peek().cloned() else {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1104".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("unterminated {} block", context),
                    hint: Some("add a closing `}`".to_string()),
                    span: Some(SourceSpan {
                        line: block_line,
                        column: block_column,
                        len: 1,
                    }),
                });
                break;
            };

            if matches!(token.kind, TokenKind::Eof) {
                diagnostics.push(CompileDiagnostic {
                    code: "INDL-1104".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("unterminated {} block", context),
                    hint: Some("add a closing `}`".to_string()),
                    span: Some(SourceSpan {
                        line: block_line,
                        column: block_column,
                        len: 1,
                    }),
                });
                break;
            }

            if self.consume_delimiter(DelimiterKind::RBrace) {
                break;
            }

            if let Some(statement) = self.parse_statement(diagnostics) {
                body.push(statement);
            } else {
                self.advance_to_statement_boundary();
            }
        }
        body
    }

    fn consume_block_recovery(&mut self) {
        self.skip_trivia();
        if !self.consume_delimiter(DelimiterKind::LBrace) {
            return;
        }
        let mut depth = 1usize;
        while let Some(token) = self.bump() {
            match token.kind {
                TokenKind::Delimiter(DelimiterKind::LBrace) => {
                    depth = depth.saturating_add(1);
                }
                TokenKind::Delimiter(DelimiterKind::RBrace) => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_header_until_block_start(&mut self) -> (Vec<Token>, bool) {
        let mut header = Vec::new();
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;

        while let Some(token) = self.peek().cloned() {
            match token.kind {
                TokenKind::Comment => {
                    self.pos = self.pos.saturating_add(1);
                }
                TokenKind::Newline | TokenKind::Eof => return (header, false),
                TokenKind::Delimiter(DelimiterKind::LParen) => {
                    paren_depth = paren_depth.saturating_add(1);
                    header.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
                TokenKind::Delimiter(DelimiterKind::RParen) => {
                    paren_depth = paren_depth.saturating_sub(1);
                    header.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
                TokenKind::Delimiter(DelimiterKind::LBracket) => {
                    bracket_depth = bracket_depth.saturating_add(1);
                    header.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
                TokenKind::Delimiter(DelimiterKind::RBracket) => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                    header.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
                TokenKind::Delimiter(DelimiterKind::LBrace)
                    if paren_depth == 0 && bracket_depth == 0 =>
                {
                    self.pos = self.pos.saturating_add(1);
                    return (header, true);
                }
                _ => {
                    header.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
            }
        }

        (header, false)
    }

    fn collect_statement_tokens(&mut self) -> Vec<Token> {
        let mut collected = Vec::new();
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;

        while let Some(token) = self.peek().cloned() {
            match token.kind {
                TokenKind::Comment => {
                    self.pos = self.pos.saturating_add(1);
                    break;
                }
                TokenKind::Newline | TokenKind::Eof => {
                    self.pos = self.pos.saturating_add(1);
                    break;
                }
                TokenKind::Delimiter(DelimiterKind::Semicolon)
                    if paren_depth == 0 && bracket_depth == 0 =>
                {
                    self.pos = self.pos.saturating_add(1);
                    break;
                }
                TokenKind::Delimiter(DelimiterKind::RBrace)
                    if paren_depth == 0 && bracket_depth == 0 =>
                {
                    break;
                }
                TokenKind::Delimiter(DelimiterKind::LParen) => {
                    paren_depth = paren_depth.saturating_add(1);
                    collected.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
                TokenKind::Delimiter(DelimiterKind::RParen) => {
                    paren_depth = paren_depth.saturating_sub(1);
                    collected.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
                TokenKind::Delimiter(DelimiterKind::LBracket) => {
                    bracket_depth = bracket_depth.saturating_add(1);
                    collected.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
                TokenKind::Delimiter(DelimiterKind::RBracket) => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                    collected.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
                _ => {
                    collected.push(token);
                    self.pos = self.pos.saturating_add(1);
                }
            }
        }

        collected
    }

    fn advance_to_statement_boundary(&mut self) {
        while let Some(token) = self.peek() {
            match token.kind {
                TokenKind::Newline | TokenKind::Eof => {
                    self.pos = self.pos.saturating_add(1);
                    break;
                }
                TokenKind::Delimiter(DelimiterKind::Semicolon) => {
                    self.pos = self.pos.saturating_add(1);
                    break;
                }
                TokenKind::Delimiter(DelimiterKind::RBrace) => break,
                _ => self.pos = self.pos.saturating_add(1),
            }
        }
    }

    fn skip_trivia(&mut self) {
        while let Some(token) = self.peek() {
            if matches!(token.kind, TokenKind::Comment | TokenKind::Newline) {
                self.pos = self.pos.saturating_add(1);
            } else {
                break;
            }
        }
    }

    fn is_indicator_start(&self) -> bool {
        matches!(
            self.peek().map(|token| token.kind),
            Some(TokenKind::Keyword(KeywordKind::Indicator))
        )
    }

    fn is_input_start(&self) -> bool {
        matches!(
            self.peek().map(|token| &token.kind),
            Some(TokenKind::Identifier)
        ) && self
            .peek()
            .map(|token| token.lexeme.starts_with("input."))
            .unwrap_or(false)
    }

    fn consume_keyword(&mut self, keyword: KeywordKind) -> bool {
        let Some(token) = self.peek() else {
            return false;
        };
        if token.kind == TokenKind::Keyword(keyword) {
            self.pos = self.pos.saturating_add(1);
            true
        } else {
            false
        }
    }

    fn consume_delimiter(&mut self, delimiter: DelimiterKind) -> bool {
        let Some(token) = self.peek() else {
            return false;
        };
        if token.kind == TokenKind::Delimiter(delimiter) {
            self.pos = self.pos.saturating_add(1);
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos)?;
        self.pos = self.pos.saturating_add(1);
        Some(token)
    }
}

fn parse_fn_signature_tokens(tokens: &[Token]) -> Option<(String, Vec<String>)> {
    let mut idx = 0usize;
    let name = tokens.get(idx)?.lexeme.clone();
    if !is_assignment_target(&name) {
        return None;
    }
    idx = idx.saturating_add(1);

    if !matches!(
        tokens.get(idx).map(|token| token.kind),
        Some(TokenKind::Delimiter(DelimiterKind::LParen))
    ) {
        return None;
    }
    idx = idx.saturating_add(1);

    let mut params = Vec::new();
    while idx < tokens.len() {
        if matches!(
            tokens.get(idx).map(|token| token.kind),
            Some(TokenKind::Delimiter(DelimiterKind::RParen))
        ) {
            idx = idx.saturating_add(1);
            break;
        }
        let param = tokens.get(idx)?.lexeme.clone();
        if !is_assignment_target(&param) {
            return None;
        }
        params.push(param);
        idx = idx.saturating_add(1);
        if matches!(
            tokens.get(idx).map(|token| token.kind),
            Some(TokenKind::Delimiter(DelimiterKind::Comma))
        ) {
            idx = idx.saturating_add(1);
            continue;
        }
        if matches!(
            tokens.get(idx).map(|token| token.kind),
            Some(TokenKind::Delimiter(DelimiterKind::RParen))
        ) {
            idx = idx.saturating_add(1);
            break;
        }
        return None;
    }

    if idx != tokens.len() {
        return None;
    }
    Some((name, params))
}

fn tokens_to_source(tokens: &[Token]) -> String {
    let mut output = String::new();
    for token in tokens {
        if output.is_empty() {
            output.push_str(&token.lexeme);
            continue;
        }
        let need_space = output
            .chars()
            .last()
            .map(|last| need_space_between(last, &token.kind))
            .unwrap_or(false);
        if need_space {
            output.push(' ');
        }
        output.push_str(&token.lexeme);
    }
    output
}

fn need_space_between(previous: char, next: &TokenKind) -> bool {
    if previous == '(' || previous == '[' || previous == '{' || previous == '.' {
        return false;
    }
    if matches!(
        next,
        TokenKind::Delimiter(
            DelimiterKind::RParen
                | DelimiterKind::RBracket
                | DelimiterKind::RBrace
                | DelimiterKind::Comma
                | DelimiterKind::Dot
                | DelimiterKind::Semicolon
        )
    ) {
        return false;
    }
    true
}

fn parse_var_decl_statement(
    line: &str,
    line_no: usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstStatement> {
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
    let value_expr = value
        .as_ref()
        .and_then(|raw| parse_expression_ast(raw, line_no, 1, diagnostics));
    Some(AstStatement::VarDecl(AstVarDecl {
        is_persistent,
        name,
        value,
        value_expr,
        line: line_no,
        column: 1,
    }))
}

fn parse_assign_statement(
    line: &str,
    line_no: usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstStatement> {
    // Check compound assignments first: +=, -=, *=, /=
    let compound_ops: &[(&str, AstBinaryOp)] = &[
        ("+=", AstBinaryOp::Add),
        ("-=", AstBinaryOp::Sub),
        ("*=", AstBinaryOp::Mul),
        ("/=", AstBinaryOp::Div),
    ];
    for (op_str, op) in compound_ops {
        if let Some(op_index) = line.find(op_str) {
            let left = line[..op_index].trim();
            let right = line[(op_index + op_str.len())..]
                .trim()
                .trim_end_matches(';')
                .trim();
            if left.is_empty() || right.is_empty() || !is_assignment_target(left) {
                continue;
            }
            let desugared = format!("{} {} ({})", left, &op_str[..1], right);
            let value_expr = parse_expression_ast(
                right,
                line_no,
                op_index.saturating_add(op_str.len() + 1),
                diagnostics,
            )
            .map(|rhs_expr| AstExpr::Binary {
                lhs: Box::new(AstExpr::Var(left.to_string())),
                op: *op,
                rhs: Box::new(rhs_expr),
            });
            return Some(AstStatement::Assign(AstAssign {
                name: left.to_string(),
                value: desugared,
                value_expr,
                line: line_no,
                column: 1,
            }));
        }
    }

    let equals_index = line.find('=')?;
    if line.get(equals_index.saturating_sub(1)..=equals_index) == Some("==")
        || line.get(equals_index..=equals_index.saturating_add(1)) == Some("==")
    {
        return None;
    }
    let left = line[..equals_index].trim();
    let right = line[(equals_index + 1)..]
        .trim()
        .trim_end_matches(';')
        .trim();
    if left.is_empty() || right.is_empty() {
        return None;
    }

    // Check for tuple destructuring: [a, b, c] = expr
    if left.starts_with('[') && left.ends_with(']') {
        let inner = left[1..left.len() - 1].trim();
        let names: Vec<String> = inner
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if names.is_empty() || names.iter().any(|n| !is_assignment_target(n)) {
            return None;
        }
        let value_expr =
            parse_expression_ast(right, line_no, equals_index.saturating_add(2), diagnostics);
        return Some(AstStatement::TupleAssign(AstTupleAssign {
            names,
            value: right.to_string(),
            value_expr,
            line: line_no,
            column: 1,
        }));
    }

    // Regular assignment
    if !is_assignment_target(left) {
        return None;
    }
    let value_expr =
        parse_expression_ast(right, line_no, equals_index.saturating_add(2), diagnostics);
    Some(AstStatement::Assign(AstAssign {
        name: left.to_string(),
        value: right.to_string(),
        value_expr,
        line: line_no,
        column: 1,
    }))
}

fn parse_return_statement(
    line: &str,
    line_no: usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstStatement> {
    if !line.starts_with("return") {
        return None;
    }
    let rest = line
        .trim_start_matches("return")
        .trim()
        .trim_end_matches(';');
    let value = if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    };
    let value_expr = value
        .as_ref()
        .and_then(|raw| parse_expression_ast(raw, line_no, 1, diagnostics));
    Some(AstStatement::Return(AstReturn {
        value,
        value_expr,
        line: line_no,
        column: 1,
    }))
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

fn parse_indicator_decl(line: &str) -> (Option<String>, IndicatorDecl) {
    let mut decl = IndicatorDecl::default();

    // Extract all arguments from indicator(...)
    let Some(open_paren) = line.find('(') else {
        return (None, decl);
    };
    let Some(close_paren) = line.rfind(')') else {
        return (None, decl);
    };

    let args_str = &line[open_paren + 1..close_paren];
    let args = split_indicator_args(args_str);

    let mut name = None;

    for (idx, arg) in args.iter().enumerate() {
        let trimmed = arg.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if it's a named argument (key=value)
        if let Some((key, value)) = parse_named_indicator_arg(trimmed) {
            match key.to_lowercase().as_str() {
                "title" => {
                    if let Some(s) = extract_string_value(value) {
                        name = Some(s.clone());
                        decl.title = Some(s);
                    }
                }
                "shorttitle" => {
                    if let Some(s) = extract_string_value(value) {
                        decl.shorttitle = Some(s);
                    }
                }
                "overlay" => {
                    decl.overlay = Some(parse_bool_value(value));
                }
                "format" => {
                    if let Some(s) = extract_string_or_const(value) {
                        decl.format = Some(s);
                    }
                }
                "precision" => {
                    if let Ok(v) = value.trim().parse::<i32>() {
                        decl.precision = Some(v.clamp(0, 16));
                    }
                }
                "scale" => {
                    if let Some(s) = extract_string_or_const(value) {
                        decl.scale = Some(s);
                    }
                }
                "max_bars_back" => {
                    if let Ok(v) = value.trim().parse::<i32>() {
                        decl.max_bars_back = Some(v);
                    }
                }
                "timeframe" => {
                    if let Some(s) = extract_string_value(value) {
                        decl.timeframe = Some(s);
                    }
                }
                "timeframe_gaps" => {
                    if let Some(s) = extract_string_or_const(value) {
                        decl.timeframe_gaps = Some(s);
                    }
                }
                "dynamic_requests" => {
                    decl.dynamic_requests = Some(parse_bool_value(value));
                }
                "calc_on_every_tick" => {
                    decl.calc_on_every_tick = Some(parse_bool_value(value));
                }
                "max_labels_count" => {
                    if let Ok(v) = value.trim().parse::<i32>() {
                        decl.max_labels_count = Some(v);
                    }
                }
                "max_lines_count" => {
                    if let Ok(v) = value.trim().parse::<i32>() {
                        decl.max_lines_count = Some(v);
                    }
                }
                "max_boxes_count" => {
                    if let Ok(v) = value.trim().parse::<i32>() {
                        decl.max_boxes_count = Some(v);
                    }
                }
                "max_tables_count" => {
                    if let Ok(v) = value.trim().parse::<i32>() {
                        decl.max_tables_count = Some(v);
                    }
                }
                "max_polylines_count" => {
                    if let Ok(v) = value.trim().parse::<i32>() {
                        decl.max_polylines_count = Some(v);
                    }
                }
                _ => {}
            }
        } else if idx == 0 {
            // First positional argument is the title
            if let Some(s) = extract_string_value(trimmed) {
                name = Some(s.clone());
                decl.title = Some(s);
            }
        } else if idx == 1 {
            // Second positional argument is shorttitle
            if let Some(s) = extract_string_value(trimmed) {
                decl.shorttitle = Some(s);
            }
        } else if idx == 2 {
            // Third positional argument is overlay
            decl.overlay = Some(parse_bool_value(trimmed));
        }
    }

    (name, decl)
}

fn split_indicator_args(args_str: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;
    let mut paren_depth = 0;

    for ch in args_str.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        if ch == '\\' && in_string {
            current.push(ch);
            escape = true;
            continue;
        }

        if ch == '"' {
            in_string = !in_string;
            current.push(ch);
            continue;
        }

        if !in_string {
            if ch == '(' {
                paren_depth += 1;
                current.push(ch);
                continue;
            }
            if ch == ')' {
                paren_depth -= 1;
                current.push(ch);
                continue;
            }
            if ch == ',' && paren_depth == 0 {
                result.push(current.trim().to_string());
                current = String::new();
                continue;
            }
        }

        current.push(ch);
    }

    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }

    result
}

fn parse_named_indicator_arg(arg: &str) -> Option<(&str, &str)> {
    // Look for `key=value` pattern, handling quotes properly
    let mut in_string = false;
    let mut escape = false;

    for (idx, ch) in arg.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == '=' && !in_string {
            let key = arg[..idx].trim();
            let value = arg[idx + 1..].trim();
            return Some((key, value));
        }
    }
    None
}

fn extract_string_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}

fn extract_string_or_const(value: &str) -> Option<String> {
    let trimmed = value.trim();
    // If it's a quoted string, extract it
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        return Some(trimmed[1..trimmed.len() - 1].to_string());
    }
    // Otherwise, it's a constant like format.price, scale.right, etc.
    if !trimmed.is_empty() {
        return Some(trimmed.to_string());
    }
    None
}

fn parse_bool_value(value: &str) -> bool {
    let trimmed = value.trim().to_lowercase();
    matches!(trimmed.as_str(), "true" | "1" | "yes")
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

fn parse_call_statement(
    line: &str,
    line_no: usize,
    diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstCall> {
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
    let arg_exprs = args
        .iter()
        .map(|raw| parse_expression_ast(raw, line_no, open.saturating_add(2), diagnostics))
        .collect::<Vec<_>>();
    Some(AstCall {
        function: function.to_string(),
        args,
        arg_exprs,
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

fn parse_expression_ast(
    raw: &str,
    _line: usize,
    _column: usize,
    _diagnostics: &mut Vec<CompileDiagnostic>,
) -> Option<AstExpr> {
    let mut expr_lexer_diagnostics = Vec::new();
    let tokens = lexer::lex(raw, &mut expr_lexer_diagnostics)
        .into_iter()
        .filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Newline | TokenKind::Comment | TokenKind::Eof
            )
        })
        .collect::<Vec<_>>();
    parse_expression_ast_tokens(&tokens)
}

fn parse_expression_ast_tokens(tokens: &[Token]) -> Option<AstExpr> {
    if tokens.is_empty() {
        return None;
    }

    let mut parser = ExprAstParser::new(tokens.to_vec());
    let expr = parser.parse_expression().ok()?;
    if parser.is_done() {
        Some(expr)
    } else {
        None
    }
}

struct ExprAstParser {
    tokens: Vec<Token>,
    pos: usize,
}

impl ExprAstParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn is_done(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned()?;
        self.pos = self.pos.saturating_add(1);
        Some(token)
    }

    fn consume_operator(&mut self, kind: OperatorKind) -> bool {
        let Some(token) = self.peek() else {
            return false;
        };
        if token.kind == TokenKind::Operator(kind) {
            self.pos = self.pos.saturating_add(1);
            true
        } else {
            false
        }
    }

    fn consume_keyword(&mut self, kind: KeywordKind) -> bool {
        let Some(token) = self.peek() else {
            return false;
        };
        if token.kind == TokenKind::Keyword(kind) {
            self.pos = self.pos.saturating_add(1);
            true
        } else {
            false
        }
    }

    fn consume_delimiter(&mut self, kind: DelimiterKind) -> bool {
        let Some(token) = self.peek() else {
            return false;
        };
        if token.kind == TokenKind::Delimiter(kind) {
            self.pos = self.pos.saturating_add(1);
            true
        } else {
            false
        }
    }

    fn parse_expression(&mut self) -> Result<AstExpr, ()> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<AstExpr, ()> {
        let condition = self.parse_or()?;
        if !self.consume_operator(OperatorKind::Question) {
            return Ok(condition);
        }

        let then_expr = self.parse_expression()?;
        if !self.consume_operator(OperatorKind::Colon) {
            return Err(());
        }
        let else_expr = self.parse_ternary()?;
        Ok(AstExpr::Conditional {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        })
    }

    fn parse_or(&mut self) -> Result<AstExpr, ()> {
        let mut expr = self.parse_and()?;
        loop {
            if self.consume_operator(OperatorKind::OrOr) || self.consume_keyword(KeywordKind::Or) {
                let rhs = self.parse_and()?;
                expr = AstExpr::Binary {
                    lhs: Box::new(expr),
                    op: AstBinaryOp::Or,
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<AstExpr, ()> {
        let mut expr = self.parse_equality()?;
        loop {
            if self.consume_operator(OperatorKind::AndAnd) || self.consume_keyword(KeywordKind::And)
            {
                let rhs = self.parse_equality()?;
                expr = AstExpr::Binary {
                    lhs: Box::new(expr),
                    op: AstBinaryOp::And,
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<AstExpr, ()> {
        let mut expr = self.parse_compare()?;
        loop {
            let op = if self.consume_operator(OperatorKind::EqEq) {
                Some(AstBinaryOp::Eq)
            } else if self.consume_operator(OperatorKind::NotEq) {
                Some(AstBinaryOp::Neq)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_compare()?;
            expr = AstExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_compare(&mut self) -> Result<AstExpr, ()> {
        let mut expr = self.parse_add_sub()?;
        loop {
            let op = if self.consume_operator(OperatorKind::Gte) {
                Some(AstBinaryOp::Gte)
            } else if self.consume_operator(OperatorKind::Lte) {
                Some(AstBinaryOp::Lte)
            } else if self.consume_operator(OperatorKind::Gt) {
                Some(AstBinaryOp::Gt)
            } else if self.consume_operator(OperatorKind::Lt) {
                Some(AstBinaryOp::Lt)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_add_sub()?;
            expr = AstExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_add_sub(&mut self) -> Result<AstExpr, ()> {
        let mut expr = self.parse_mul_div()?;
        loop {
            let op = if self.consume_operator(OperatorKind::Plus) {
                Some(AstBinaryOp::Add)
            } else if self.consume_operator(OperatorKind::Minus) {
                Some(AstBinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_mul_div()?;
            expr = AstExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_mul_div(&mut self) -> Result<AstExpr, ()> {
        let mut expr = self.parse_pow()?;
        loop {
            let op = if self.consume_operator(OperatorKind::Star) {
                Some(AstBinaryOp::Mul)
            } else if self.consume_operator(OperatorKind::Slash) {
                Some(AstBinaryOp::Div)
            } else if self.consume_operator(OperatorKind::Percent) {
                Some(AstBinaryOp::Mod)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_pow()?;
            expr = AstExpr::Binary {
                lhs: Box::new(expr),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_pow(&mut self) -> Result<AstExpr, ()> {
        let expr = self.parse_unary()?;
        if self.consume_operator(OperatorKind::StarStar)
            || self.consume_operator(OperatorKind::Caret)
        {
            let rhs = self.parse_pow()?;
            return Ok(AstExpr::Binary {
                lhs: Box::new(expr),
                op: AstBinaryOp::Pow,
                rhs: Box::new(rhs),
            });
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<AstExpr, ()> {
        if self.consume_operator(OperatorKind::Minus) {
            let inner = self.parse_unary()?;
            return Ok(AstExpr::Unary {
                op: AstUnaryOp::Neg,
                expr: Box::new(inner),
            });
        }
        if self.consume_operator(OperatorKind::Bang) || self.consume_keyword(KeywordKind::Not) {
            let inner = self.parse_unary()?;
            return Ok(AstExpr::Unary {
                op: AstUnaryOp::Not,
                expr: Box::new(inner),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<AstExpr, ()> {
        if self.consume_delimiter(DelimiterKind::LParen) {
            let expr = self.parse_expression()?;
            if !self.consume_delimiter(DelimiterKind::RParen) {
                return Err(());
            }
            return Ok(expr);
        }

        let Some(token) = self.peek().cloned() else {
            return Err(());
        };

        match token.kind {
            TokenKind::Number => {
                let _ = self.bump();
                let value = token.lexeme.parse::<f64>().map_err(|_| ())?;
                Ok(AstExpr::Number(value))
            }
            TokenKind::String => {
                let _ = self.bump();
                Ok(AstExpr::String(parse_string_literal(&token.lexeme)))
            }
            TokenKind::ColorLiteral => {
                let _ = self.bump();
                // Parse #RRGGBB or #RRGGBBAA
                let hex = token.lexeme.trim_start_matches('#');
                let (r, g, b, a) = parse_hex_color(hex).ok_or(())?;
                Ok(AstExpr::Color { r, g, b, a })
            }
            TokenKind::Keyword(KeywordKind::True) => {
                let _ = self.bump();
                Ok(AstExpr::Bool(true))
            }
            TokenKind::Keyword(KeywordKind::False) => {
                let _ = self.bump();
                Ok(AstExpr::Bool(false))
            }
            TokenKind::Keyword(KeywordKind::Na) => {
                let _ = self.bump();
                Ok(AstExpr::Na)
            }
            TokenKind::Identifier => self.parse_identifier_expr(),
            _ => Err(()),
        }
    }

    fn parse_identifier_expr(&mut self) -> Result<AstExpr, ()> {
        let Some(token) = self.bump() else {
            return Err(());
        };
        let ident = token.lexeme;

        if self.consume_delimiter(DelimiterKind::LParen) {
            let args = self.parse_call_args()?;
            return self.parse_call_expr(&ident, args);
        }

        let index = if self.consume_delimiter(DelimiterKind::LBracket) {
            let idx = self.parse_expression()?;
            if !self.consume_delimiter(DelimiterKind::RBracket) {
                return Err(());
            }
            Some(Box::new(idx))
        } else {
            None
        };

        if let Some(field) = map_series_field(&ident) {
            return Ok(AstExpr::Series { field, index });
        }

        if let Some(index) = index {
            return Ok(AstExpr::VarIndexed { name: ident, index });
        }

        Ok(AstExpr::Var(ident))
    }

    fn parse_call_args(&mut self) -> Result<Vec<AstExpr>, ()> {
        let mut args = Vec::new();
        if self.consume_delimiter(DelimiterKind::RParen) {
            return Ok(args);
        }

        loop {
            let expr = self.parse_expression()?;
            args.push(expr);
            if self.consume_delimiter(DelimiterKind::Comma) {
                continue;
            }
            if self.consume_delimiter(DelimiterKind::RParen) {
                break;
            }
            return Err(());
        }

        Ok(args)
    }

    fn parse_call_expr(&mut self, ident: &str, args: Vec<AstExpr>) -> Result<AstExpr, ()> {
        // Handle req.series and request.security specially
        if ident.eq_ignore_ascii_case("req.series")
            || ident.eq_ignore_ascii_case("request.security")
        {
            if args.len() < 3 {
                return Err(());
            }

            let symbol = expr_to_text_arg(args.first().ok_or(())?).ok_or(())?;
            let timeframe = expr_to_text_arg(args.get(1).ok_or(())?).ok_or(())?;
            let field = expr_to_text_arg(args.get(2).ok_or(())?).ok_or(())?;

            // Parse optional args: either (mode, gaps, lookahead) or (gaps, lookahead)
            // Pine Script signature: request.security(symbol, timeframe, expression [, gaps [, lookahead]])
            // Internal signature: req.series(symbol, timeframe, field, mode [, gaps [, lookahead]])
            // Detect by checking if arg 3 starts with "barmerge." or is a mode value
            let arg3 = args.get(3).and_then(expr_to_text_arg);
            let (mode, gaps, lookahead) = match arg3.as_deref() {
                // Pine Script style: gaps at arg 3
                Some(s) if s.starts_with("barmerge.") => {
                    let gaps = arg3;
                    let lookahead = args.get(4).and_then(expr_to_text_arg);
                    ("confirmed".to_string(), gaps, lookahead)
                }
                // Internal style: mode at arg 3, gaps/lookahead at 4/5
                Some(mode_val) => {
                    let gaps = args.get(4).and_then(expr_to_text_arg);
                    let lookahead = args.get(5).and_then(expr_to_text_arg);
                    (mode_val.to_string(), gaps, lookahead)
                }
                // No arg 3 - use defaults
                None => ("confirmed".to_string(), None, None),
            };

            let index = if self.consume_delimiter(DelimiterKind::LBracket) {
                let idx = self.parse_expression()?;
                if !self.consume_delimiter(DelimiterKind::RBracket) {
                    return Err(());
                }
                Some(Box::new(idx))
            } else {
                None
            };

            return Ok(AstExpr::ReqSeries {
                symbol,
                timeframe,
                field,
                mode,
                gaps,
                lookahead,
                index,
            });
        }

        // User-defined function call
        Ok(AstExpr::FnCall {
            name: ident.to_string(),
            args,
        })
    }
}

fn expr_to_text_arg(expr: &AstExpr) -> Option<String> {
    match expr {
        AstExpr::String(text) | AstExpr::Var(text) => Some(text.clone()),
        AstExpr::Number(number) => Some(number.to_string()),
        AstExpr::Bool(value) => Some(if *value { "true" } else { "false" }.to_string()),
        AstExpr::Na => Some("na".to_string()),
        // Handle series field references like `close`, `open`, etc.
        AstExpr::Series { field, index: None } => {
            let field_name = match field {
                AstSeriesField::Open => "open",
                AstSeriesField::High => "high",
                AstSeriesField::Low => "low",
                AstSeriesField::Close => "close",
                AstSeriesField::Volume => "volume",
                AstSeriesField::Time => "time",
                AstSeriesField::BarIndex => "bar_index",
            };
            Some(field_name.to_string())
        }
        _ => None,
    }
}

fn parse_string_literal(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Parse hex color string (without #) to RGBA components.
/// Supports 6-digit (#RRGGBB) and 8-digit (#RRGGBBAA) formats.
fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8, u8)> {
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b, 255))
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
        Some((r, g, b, a))
    } else {
        None
    }
}

fn map_series_field(ident: &str) -> Option<AstSeriesField> {
    match ident {
        "open" | "ctx.open" => Some(AstSeriesField::Open),
        "high" | "ctx.high" => Some(AstSeriesField::High),
        "low" | "ctx.low" => Some(AstSeriesField::Low),
        "close" | "ctx.close" => Some(AstSeriesField::Close),
        "volume" | "ctx.volume" => Some(AstSeriesField::Volume),
        "time" | "ctx.time" => Some(AstSeriesField::Time),
        "bar_index" | "ctx.bar_index" => Some(AstSeriesField::BarIndex),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_program;
    use crate::core::indicators::compiler::ast::{AstBinaryOp, AstExpr, AstStatement};
    use crate::core::indicators::compiler::diagnostics::DiagnosticSeverity;
    use crate::core::indicators::compiler::lexer;

    #[test]
    fn populates_structured_expression_fields() {
        let source = "indicator(\"t\")\nlet x = (close + open) / 2\nx = req.series(\"BTCUSD\", \"1h\", \"close\")[1] + close\nplot(x)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::VarDecl(var_decl) = &program.statements[0] else {
            panic!("expected var decl");
        };
        assert!(
            var_decl.value_expr.is_some(),
            "expected structured var expression"
        );

        let AstStatement::Assign(assign) = &program.statements[1] else {
            panic!("expected assignment");
        };
        let Some(AstExpr::Binary { lhs, .. }) = assign.value_expr.as_ref() else {
            panic!("expected binary structured assignment expression");
        };
        assert!(
            matches!(lhs.as_ref(), AstExpr::ReqSeries { .. }),
            "expected req.series on left side"
        );
    }

    #[test]
    fn keeps_named_call_args_as_raw_when_not_expression() {
        let source = "indicator(\"t\")\nplothistogram(volume, base=100)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");

        let AstStatement::Call(call) = &program.statements[0] else {
            panic!("expected call statement");
        };
        assert_eq!(call.arg_exprs.len(), 2);
        assert!(
            call.arg_exprs[0].is_some(),
            "expected expression for first arg"
        );
        assert!(call.arg_exprs[1].is_none(), "expected raw-only named arg");
    }

    #[test]
    fn parses_variable_history_index_expression() {
        let source = "indicator(\"t\")\nlet x = close\nplot(x[1])";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::Call(call) = &program.statements[1] else {
            panic!("expected plot call");
        };
        let Some(AstExpr::VarIndexed { name, .. }) = call.arg_exprs[0].as_ref() else {
            panic!("expected variable indexed expression");
        };
        assert_eq!(name, "x");
    }

    #[test]
    fn parses_modulo_and_power_with_expected_precedence() {
        let source = "indicator(\"t\")\nlet x = 2 + 3 % 2 ** 3\nplot(x)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::VarDecl(var_decl) = &program.statements[0] else {
            panic!("expected var decl");
        };
        let Some(AstExpr::Binary {
            op: AstBinaryOp::Add,
            rhs,
            ..
        }) = var_decl.value_expr.as_ref()
        else {
            panic!("expected top-level add expression");
        };
        let AstExpr::Binary {
            op: AstBinaryOp::Mod,
            rhs: mod_rhs,
            ..
        } = rhs.as_ref()
        else {
            panic!("expected modulo on add rhs");
        };
        let AstExpr::Binary {
            op: AstBinaryOp::Pow,
            ..
        } = mod_rhs.as_ref()
        else {
            panic!("expected power on modulo rhs");
        };
    }

    #[test]
    fn parses_ternary_expression_with_expected_shape() {
        let source = "indicator(\"t\")\nlet x = close > open ? high : low\nplot(x)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::VarDecl(var_decl) = &program.statements[0] else {
            panic!("expected var decl");
        };
        let Some(AstExpr::Conditional {
            condition,
            then_expr,
            else_expr,
        }) = var_decl.value_expr.as_ref()
        else {
            panic!("expected ternary expression");
        };
        assert!(
            matches!(
                condition.as_ref(),
                AstExpr::Binary {
                    op: AstBinaryOp::Gt,
                    ..
                }
            ),
            "expected comparison condition"
        );
        assert!(
            matches!(then_expr.as_ref(), AstExpr::Series { .. }),
            "expected series expression for then branch"
        );
        assert!(
            matches!(else_expr.as_ref(), AstExpr::Series { .. }),
            "expected series expression for else branch"
        );
    }

    #[test]
    fn parses_while_loop_statement() {
        let source = "indicator(\"t\")\nlet x = 0\nwhile x < 2 {\n  x = x + 1\n}\nplot(x)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::While(loop_stmt) = &program.statements[1] else {
            panic!("expected while loop statement");
        };
        assert!(
            matches!(
                loop_stmt.condition_expr.as_ref(),
                Some(AstExpr::Binary {
                    op: AstBinaryOp::Lt,
                    ..
                })
            ),
            "expected structured while condition expression"
        );
        assert_eq!(
            loop_stmt.body.len(),
            1,
            "expected one statement in while body"
        );
    }

    #[test]
    fn parses_switch_statement_with_cases_and_default() {
        let source = "indicator(\"t\")\nswitch close {\n  case open {\n    plot(high)\n  }\n  case high {\n    plot(low)\n  }\n  default {\n    plot(close)\n  }\n}";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::Switch(switch_stmt) = &program.statements[0] else {
            panic!("expected switch statement");
        };
        assert_eq!(switch_stmt.cases.len(), 2, "expected two switch cases");
        assert_eq!(
            switch_stmt.default_branch.len(),
            1,
            "expected default branch to have one statement"
        );
    }

    #[test]
    fn parses_else_if_chain_as_nested_if() {
        let source = "indicator(\"t\")\nif close > open {\n  plot(close)\n} else if close < open {\n  plot(open)\n} else {\n  plot(high)\n}";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::If(root_if) = &program.statements[0] else {
            panic!("expected root if statement");
        };
        assert_eq!(
            root_if.else_branch.len(),
            1,
            "expected nested else-if branch"
        );
        let AstStatement::If(nested_if) = &root_if.else_branch[0] else {
            panic!("expected nested if in else branch");
        };
        assert_eq!(
            nested_if.else_branch.len(),
            1,
            "expected final else branch to remain attached to nested if"
        );
    }

    #[test]
    fn parses_compound_add_assignment() {
        let source = "indicator(\"t\")\nlet x = 1\nx += 2\nplot(x)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::Assign(assign) = &program.statements[1] else {
            panic!("expected assignment statement");
        };
        assert_eq!(assign.name, "x");
        let Some(AstExpr::Binary {
            lhs,
            op: AstBinaryOp::Add,
            rhs,
        }) = assign.value_expr.as_ref()
        else {
            panic!("expected binary add expression for compound assignment");
        };
        assert!(matches!(lhs.as_ref(), AstExpr::Var(name) if name == "x"));
        assert!(matches!(rhs.as_ref(), AstExpr::Number(v) if (*v - 2.0).abs() < f64::EPSILON));
    }

    #[test]
    fn parses_compound_sub_assignment() {
        let source = "indicator(\"t\")\nlet x = 10\nx -= 3\nplot(x)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::Assign(assign) = &program.statements[1] else {
            panic!("expected assignment statement");
        };
        let Some(AstExpr::Binary {
            op: AstBinaryOp::Sub,
            ..
        }) = assign.value_expr.as_ref()
        else {
            panic!("expected binary sub expression for compound assignment");
        };
    }

    #[test]
    fn parses_compound_mul_assignment() {
        let source = "indicator(\"t\")\nlet x = 5\nx *= 2\nplot(x)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::Assign(assign) = &program.statements[1] else {
            panic!("expected assignment statement");
        };
        let Some(AstExpr::Binary {
            op: AstBinaryOp::Mul,
            ..
        }) = assign.value_expr.as_ref()
        else {
            panic!("expected binary mul expression for compound assignment");
        };
    }

    #[test]
    fn parses_compound_div_assignment() {
        let source = "indicator(\"t\")\nlet x = 8\nx /= 4\nplot(x)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::Assign(assign) = &program.statements[1] else {
            panic!("expected assignment statement");
        };
        let Some(AstExpr::Binary {
            op: AstBinaryOp::Div,
            ..
        }) = assign.value_expr.as_ref()
        else {
            panic!("expected binary div expression for compound assignment");
        };
    }

    #[test]
    fn parses_function_call_in_expression() {
        let source = "indicator(\"t\")\nfn double(x) { return x * 2 }\nlet y = double(5)\nplot(y)";
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::VarDecl(var_decl) = &program.statements[1] else {
            panic!("expected var decl");
        };
        let Some(AstExpr::FnCall { name, args }) = var_decl.value_expr.as_ref() else {
            panic!("expected function call expression");
        };
        assert_eq!(name, "double");
        assert_eq!(args.len(), 1);
        assert!(matches!(args[0], AstExpr::Number(v) if (v - 5.0).abs() < f64::EPSILON));
    }

    #[test]
    fn parses_request_security_with_barmerge_options() {
        // Phase 4 acceptance gate test
        let source = r#"indicator("t")
let htfClose = request.security(syminfo.tickerid, "1D", close, barmerge.gaps_off, barmerge.lookahead_on)
plot(htfClose)"#;
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::VarDecl(var_decl) = &program.statements[0] else {
            panic!("expected var decl");
        };
        assert_eq!(var_decl.name, "htfClose");
        let Some(AstExpr::ReqSeries {
            symbol,
            timeframe,
            field,
            gaps,
            lookahead,
            ..
        }) = var_decl.value_expr.as_ref()
        else {
            panic!("expected ReqSeries expression");
        };
        assert_eq!(symbol, "syminfo.tickerid");
        assert_eq!(timeframe, "1D");
        assert_eq!(field, "close");
        assert_eq!(gaps.as_deref(), Some("barmerge.gaps_off"));
        assert_eq!(lookahead.as_deref(), Some("barmerge.lookahead_on"));
    }

    #[test]
    fn parses_req_series_with_barmerge_gaps_only() {
        let source = r#"indicator("t")
let x = req.series("BTCUSD", "1h", "close", "confirmed", barmerge.gaps_off)
plot(x)"#;
        let mut diagnostics = Vec::new();
        let tokens = lexer::lex(source, &mut diagnostics);
        let program = parse_program(&tokens, source, &mut diagnostics).expect("expected AST");
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.severity, DiagnosticSeverity::Error)),
            "unexpected parser errors: {:?}",
            diagnostics
        );

        let AstStatement::VarDecl(var_decl) = &program.statements[0] else {
            panic!("expected var decl");
        };
        let Some(AstExpr::ReqSeries {
            gaps, lookahead, ..
        }) = var_decl.value_expr.as_ref()
        else {
            panic!("expected ReqSeries expression");
        };
        assert_eq!(gaps.as_deref(), Some("barmerge.gaps_off"));
        assert!(lookahead.is_none());
    }
}
