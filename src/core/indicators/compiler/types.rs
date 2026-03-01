#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Number,
    String,
    ColorLiteral, // #RRGGBB or #RRGGBBAA hex color
    Keyword(KeywordKind),
    Operator(OperatorKind),
    Delimiter(DelimiterKind),
    Comment,
    Newline,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordKind {
    Indicator,
    Strategy,
    Input,
    Var,
    Let,
    Fn,
    Return,
    If,
    Else,
    Case,
    Default,
    For,
    To,
    While,
    Switch,
    True,
    False,
    Na,
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorKind {
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    Percent,
    Caret,
    Assign,
    EqEq,
    NotEq,
    Bang,
    Gt,
    Gte,
    Lt,
    Lte,
    AndAnd,
    OrOr,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    Question,
    Colon,
    Arrow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterKind {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Semicolon,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

pub fn keyword_kind_for_ident(ident: &str) -> Option<KeywordKind> {
    match ident.to_ascii_lowercase().as_str() {
        "indicator" => Some(KeywordKind::Indicator),
        "strategy" => Some(KeywordKind::Strategy),
        "input" => Some(KeywordKind::Input),
        "var" => Some(KeywordKind::Var),
        "let" => Some(KeywordKind::Let),
        "fn" => Some(KeywordKind::Fn),
        "return" => Some(KeywordKind::Return),
        "if" => Some(KeywordKind::If),
        "else" => Some(KeywordKind::Else),
        "case" => Some(KeywordKind::Case),
        "default" => Some(KeywordKind::Default),
        "for" => Some(KeywordKind::For),
        "to" => Some(KeywordKind::To),
        "while" => Some(KeywordKind::While),
        "switch" => Some(KeywordKind::Switch),
        "true" => Some(KeywordKind::True),
        "false" => Some(KeywordKind::False),
        "na" => Some(KeywordKind::Na),
        "and" => Some(KeywordKind::And),
        "or" => Some(KeywordKind::Or),
        "not" => Some(KeywordKind::Not),
        _ => None,
    }
}
