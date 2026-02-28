use crate::core::indicators::compiler::diagnostics::{
    CompileDiagnostic, DiagnosticSeverity, SourceSpan,
};
use crate::core::indicators::compiler::types::{Token, TokenKind};

pub fn lex(source: &str, diagnostics: &mut Vec<CompileDiagnostic>) -> Vec<Token> {
    let mut tokens = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let line_no = line_index + 1;
        let bytes = line.as_bytes();
        let mut cursor = 0usize;
        while cursor < bytes.len() {
            let ch = bytes[cursor] as char;
            if ch.is_ascii_whitespace() {
                cursor += 1;
                continue;
            }

            let start = cursor;
            if is_ident_start(ch) {
                cursor += 1;
                while cursor < bytes.len() && is_ident_part(bytes[cursor] as char) {
                    cursor += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Identifier,
                    lexeme: line[start..cursor].to_string(),
                    line: line_no,
                    column: start + 1,
                });
                continue;
            }

            if ch.is_ascii_digit()
                || (ch == '.'
                    && cursor + 1 < bytes.len()
                    && (bytes[cursor + 1] as char).is_ascii_digit())
            {
                cursor += 1;
                let mut seen_dot = ch == '.';
                while cursor < bytes.len() {
                    let next = bytes[cursor] as char;
                    if next.is_ascii_digit() {
                        cursor += 1;
                        continue;
                    }
                    if next == '.' && !seen_dot {
                        seen_dot = true;
                        cursor += 1;
                        continue;
                    }
                    break;
                }
                tokens.push(Token {
                    kind: TokenKind::Number,
                    lexeme: line[start..cursor].to_string(),
                    line: line_no,
                    column: start + 1,
                });
                continue;
            }

            if ch == '"' {
                cursor += 1;
                let mut escaped = false;
                while cursor < bytes.len() {
                    let next = bytes[cursor] as char;
                    if escaped {
                        escaped = false;
                        cursor += 1;
                        continue;
                    }
                    if next == '\\' {
                        escaped = true;
                        cursor += 1;
                        continue;
                    }
                    if next == '"' {
                        cursor += 1;
                        break;
                    }
                    cursor += 1;
                }
                if cursor > bytes.len() || !line[start..cursor].ends_with('"') {
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1002".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: "unterminated string literal".to_string(),
                        hint: Some("close the string with a matching quote".to_string()),
                        span: Some(SourceSpan {
                            line: line_no,
                            column: start + 1,
                            len: line.len().saturating_sub(start),
                        }),
                    });
                }
                tokens.push(Token {
                    kind: TokenKind::String,
                    lexeme: line[start..cursor.min(bytes.len())].to_string(),
                    line: line_no,
                    column: start + 1,
                });
                continue;
            }

            if is_punctuation(ch) {
                cursor += 1;
                tokens.push(Token {
                    kind: TokenKind::Punctuation,
                    lexeme: line[start..cursor].to_string(),
                    line: line_no,
                    column: start + 1,
                });
                continue;
            }

            diagnostics.push(CompileDiagnostic {
                code: "INDL-1003".to_string(),
                severity: DiagnosticSeverity::Warning,
                message: format!("unknown token '{}'", ch),
                hint: Some("token ignored by lexer".to_string()),
                span: Some(SourceSpan {
                    line: line_no,
                    column: start + 1,
                    len: 1,
                }),
            });
            cursor += 1;
        }

        tokens.push(Token {
            kind: TokenKind::Newline,
            lexeme: "\n".to_string(),
            line: line_no,
            column: line.len() + 1,
        });
    }

    if tokens.is_empty() {
        diagnostics.push(CompileDiagnostic {
            code: "INDL-1001".to_string(),
            severity: DiagnosticSeverity::Error,
            message: "no tokens generated from source".to_string(),
            hint: Some("verify indicator source encoding".to_string()),
            span: Some(SourceSpan {
                line: 1,
                column: 1,
                len: 0,
            }),
        });
    }
    tokens
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'
}

fn is_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | ','
            | ':'
            | ';'
            | '+'
            | '-'
            | '*'
            | '/'
            | '%'
            | '='
            | '<'
            | '>'
            | '!'
    )
}
