use crate::core::indicators::compiler::diagnostics::{
    CompileDiagnostic, DiagnosticSeverity, SourceSpan,
};
use crate::core::indicators::compiler::types::{
    keyword_kind_for_ident, DelimiterKind, OperatorKind, Token, TokenKind,
};

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
            if ch == '/' && cursor + 1 < bytes.len() && bytes[cursor + 1] as char == '/' {
                tokens.push(Token {
                    kind: TokenKind::Comment,
                    lexeme: line[cursor..].to_string(),
                    line: line_no,
                    column: start + 1,
                });
                break;
            }

            if is_ident_start(ch) {
                cursor += 1;
                while cursor < bytes.len() && is_ident_part(bytes[cursor] as char) {
                    cursor += 1;
                }
                let lexeme = line[start..cursor].to_string();
                let kind = keyword_kind_for_ident(&lexeme)
                    .map(TokenKind::Keyword)
                    .unwrap_or(TokenKind::Identifier);
                tokens.push(Token {
                    kind,
                    lexeme,
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

            // Hex color literal: #RRGGBB or #RRGGBBAA
            if ch == '#' {
                cursor += 1;
                let hex_start = cursor;
                while cursor < bytes.len() && (bytes[cursor] as char).is_ascii_hexdigit() {
                    cursor += 1;
                }
                let hex_len = cursor - hex_start;
                if hex_len == 6 || hex_len == 8 {
                    tokens.push(Token {
                        kind: TokenKind::ColorLiteral,
                        lexeme: line[start..cursor].to_string(),
                        line: line_no,
                        column: start + 1,
                    });
                    continue;
                } else {
                    // Invalid hex color format - emit warning and treat as unknown
                    diagnostics.push(CompileDiagnostic {
                        code: "INDL-1004".to_string(),
                        severity: DiagnosticSeverity::Warning,
                        message: format!(
                            "invalid hex color literal '{}' (expected 6 or 8 hex digits)",
                            &line[start..cursor]
                        ),
                        hint: Some("use #RRGGBB or #RRGGBBAA format".to_string()),
                        span: Some(SourceSpan {
                            line: line_no,
                            column: start + 1,
                            len: cursor - start,
                        }),
                    });
                    // Don't emit a token for invalid hex color
                    continue;
                }
            }

            if let Some((kind, width)) = match_operator(&line[cursor..]) {
                cursor += width;
                tokens.push(Token {
                    kind: TokenKind::Operator(kind),
                    lexeme: line[start..cursor].to_string(),
                    line: line_no,
                    column: start + 1,
                });
                continue;
            }

            if let Some(kind) = match_delimiter(ch) {
                cursor += 1;
                tokens.push(Token {
                    kind: TokenKind::Delimiter(kind),
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

    tokens.push(Token {
        kind: TokenKind::Eof,
        lexeme: String::new(),
        line: source.lines().count().max(1),
        column: 1,
    });

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

fn match_operator(raw: &str) -> Option<(OperatorKind, usize)> {
    let candidates: [(&str, OperatorKind); 13] = [
        ("==", OperatorKind::EqEq),
        ("!=", OperatorKind::NotEq),
        (">=", OperatorKind::Gte),
        ("<=", OperatorKind::Lte),
        ("&&", OperatorKind::AndAnd),
        ("||", OperatorKind::OrOr),
        ("+=", OperatorKind::PlusEq),
        ("-=", OperatorKind::MinusEq),
        ("**", OperatorKind::StarStar),
        ("*=", OperatorKind::StarEq),
        ("/=", OperatorKind::SlashEq),
        ("=>", OperatorKind::Arrow),
        ("=", OperatorKind::Assign),
    ];
    for (token, kind) in candidates {
        if raw.starts_with(token) {
            return Some((kind, token.len()));
        }
    }
    let first = raw.chars().next()?;
    let kind = match first {
        '+' => OperatorKind::Plus,
        '-' => OperatorKind::Minus,
        '*' => OperatorKind::Star,
        '/' => OperatorKind::Slash,
        '%' => OperatorKind::Percent,
        '^' => OperatorKind::Caret,
        '!' => OperatorKind::Bang,
        '>' => OperatorKind::Gt,
        '<' => OperatorKind::Lt,
        '?' => OperatorKind::Question,
        ':' => OperatorKind::Colon,
        _ => return None,
    };
    Some((kind, 1))
}

fn match_delimiter(ch: char) -> Option<DelimiterKind> {
    match ch {
        '(' => Some(DelimiterKind::LParen),
        ')' => Some(DelimiterKind::RParen),
        '[' => Some(DelimiterKind::LBracket),
        ']' => Some(DelimiterKind::RBracket),
        '{' => Some(DelimiterKind::LBrace),
        '}' => Some(DelimiterKind::RBrace),
        ',' => Some(DelimiterKind::Comma),
        '.' => Some(DelimiterKind::Dot),
        ';' => Some(DelimiterKind::Semicolon),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::lex;
    use crate::core::indicators::compiler::types::{
        DelimiterKind, KeywordKind, OperatorKind, TokenKind,
    };

    #[test]
    fn tokenizes_keywords_operators_and_delimiters() {
        let mut diagnostics = Vec::new();
        let tokens = lex(
            "indicator(\"t\")\nif close >= open { x += 1 }\ny = 2 ** 3\nz = close > open ? high : low\nswitch close { case open { z = high } default { z = low } }\n//@version=2",
            &mut diagnostics,
        );
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            diagnostics
        );
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword(KeywordKind::Indicator)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword(KeywordKind::If)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword(KeywordKind::Switch)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword(KeywordKind::Case)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword(KeywordKind::Default)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Operator(OperatorKind::Gte)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Operator(OperatorKind::PlusEq)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Operator(OperatorKind::StarStar)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Operator(OperatorKind::Question)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Operator(OperatorKind::Colon)));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Delimiter(DelimiterKind::LParen)));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Comment));
        assert!(
            tokens.last().map(|t| t.kind) == Some(TokenKind::Eof),
            "expected EOF token"
        );
    }

    #[test]
    fn tokenizes_hex_color_literals() {
        let mut diagnostics = Vec::new();
        let tokens = lex("var c1 = #FF0000\nvar c2 = #00FF00FF", &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            diagnostics
        );

        // Find color literals
        let color_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::ColorLiteral)
            .collect();

        assert_eq!(color_tokens.len(), 2, "expected 2 color literals");
        assert_eq!(color_tokens[0].lexeme, "#FF0000");
        assert_eq!(color_tokens[1].lexeme, "#00FF00FF");
    }

    #[test]
    fn warns_on_invalid_hex_color() {
        let mut diagnostics = Vec::new();
        let tokens = lex("var c = #ABC", &mut diagnostics); // 3 hex digits - invalid

        assert_eq!(diagnostics.len(), 1, "expected 1 diagnostic");
        assert_eq!(diagnostics[0].code, "INDL-1004");

        // No color token should be emitted for invalid hex
        assert!(
            !tokens.iter().any(|t| t.kind == TokenKind::ColorLiteral),
            "should not emit color token for invalid hex"
        );
    }
}
