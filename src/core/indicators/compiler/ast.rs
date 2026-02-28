#[derive(Debug, Clone)]
pub struct AstProgram {
    pub name: Option<String>,
    pub inputs: Vec<AstInputDecl>,
    pub statements: Vec<AstStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AstUnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AstSeriesField {
    Open,
    High,
    Low,
    Close,
    Volume,
    Time,
    BarIndex,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AstExpr {
    Bool(bool),
    Number(f64),
    Na,
    String(String),
    Var(String),
    VarIndexed {
        name: String,
        index: Box<AstExpr>,
    },
    Unary {
        op: AstUnaryOp,
        expr: Box<AstExpr>,
    },
    Binary {
        lhs: Box<AstExpr>,
        op: AstBinaryOp,
        rhs: Box<AstExpr>,
    },
    Conditional {
        condition: Box<AstExpr>,
        then_expr: Box<AstExpr>,
        else_expr: Box<AstExpr>,
    },
    ReqSeries {
        symbol: String,
        timeframe: String,
        field: String,
        mode: String,
        /// barmerge gaps setting: "barmerge.gaps_on" (default) or "barmerge.gaps_off"
        gaps: Option<String>,
        /// barmerge lookahead setting: "barmerge.lookahead_off" (default) or "barmerge.lookahead_on"
        lookahead: Option<String>,
        index: Option<Box<AstExpr>>,
    },
    Series {
        field: AstSeriesField,
        index: Option<Box<AstExpr>>,
    },
    FnCall {
        name: String,
        args: Vec<AstExpr>,
    },
    /// Hex color literal (#RRGGBB or #RRGGBBAA)
    Color {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    },
}

#[derive(Debug, Clone)]
pub struct AstInputDecl {
    pub name: String,
    pub type_name: String,
    pub default_value: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum AstStatement {
    Call(AstCall),
    VarDecl(AstVarDecl),
    Assign(AstAssign),
    TupleAssign(AstTupleAssign),
    If(AstIf),
    Switch(AstSwitch),
    While(AstWhile),
    For(AstForLoop),
    FnDecl(AstFnDecl),
    Return(AstReturn),
}

#[derive(Debug, Clone)]
pub struct AstCall {
    pub function: String,
    pub args: Vec<String>,
    pub arg_exprs: Vec<Option<AstExpr>>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstForLoop {
    pub iterator: String,
    pub start: usize,
    pub end: usize,
    pub start_expr: Option<AstExpr>,
    pub end_expr: Option<AstExpr>,
    pub body: Vec<AstStatement>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstVarDecl {
    pub is_persistent: bool,
    pub name: String,
    pub value: Option<String>,
    pub value_expr: Option<AstExpr>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstAssign {
    pub name: String,
    pub value: String,
    pub value_expr: Option<AstExpr>,
    pub line: usize,
    pub column: usize,
}

/// Tuple destructuring assignment: [a, b, c] = expr
#[derive(Debug, Clone)]
pub struct AstTupleAssign {
    pub names: Vec<String>,
    pub value: String,
    pub value_expr: Option<AstExpr>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstIf {
    pub condition: String,
    pub condition_expr: Option<AstExpr>,
    pub then_branch: Vec<AstStatement>,
    pub else_branch: Vec<AstStatement>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstWhile {
    pub condition: String,
    pub condition_expr: Option<AstExpr>,
    pub body: Vec<AstStatement>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstSwitchCase {
    pub value: String,
    pub value_expr: Option<AstExpr>,
    pub body: Vec<AstStatement>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstSwitch {
    pub subject: String,
    pub subject_expr: Option<AstExpr>,
    pub cases: Vec<AstSwitchCase>,
    pub default_branch: Vec<AstStatement>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstFnDecl {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<AstStatement>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstReturn {
    pub value: Option<String>,
    pub value_expr: Option<AstExpr>,
    pub line: usize,
    pub column: usize,
}
