#[derive(Debug, Clone, Default)]
pub struct IndicatorDecl {
    /// Indicator title (displayed in chart header)
    pub title: Option<String>,
    /// Short title (abbreviated name)
    pub shorttitle: Option<String>,
    /// Whether the indicator overlays on the main chart (true) or uses a separate pane (false)
    pub overlay: Option<bool>,
    /// Price format: "price" (default), "volume", "percent", "inherit"
    pub format: Option<String>,
    /// Decimal precision for display (0-16)
    pub precision: Option<i32>,
    /// Price scale mode: "right", "left", "none", "inherit"
    pub scale: Option<String>,
    /// Maximum bars to look back (affects performance)
    pub max_bars_back: Option<i32>,
    /// Default timeframe for the indicator
    pub timeframe: Option<String>,
    /// Gaps handling: "barmerge.gaps_off" (default), "barmerge.gaps_on"
    pub timeframe_gaps: Option<String>,
    /// Whether to enable dynamic requests
    pub dynamic_requests: Option<bool>,
    /// Whether to track realtime updates
    pub calc_on_every_tick: Option<bool>,
    /// Maximum number of labels
    pub max_labels_count: Option<i32>,
    /// Maximum number of lines
    pub max_lines_count: Option<i32>,
    /// Maximum number of boxes
    pub max_boxes_count: Option<i32>,
    /// Maximum number of tables
    pub max_tables_count: Option<i32>,
    /// Maximum number of polylines
    pub max_polylines_count: Option<i32>,
}

/// Strategy declaration parameters from strategy() call.
#[derive(Debug, Clone, Default)]
pub struct StrategyDecl {
    /// Strategy title (displayed in chart header)
    pub title: Option<String>,
    /// Short title (abbreviated name)
    pub shorttitle: Option<String>,
    /// Whether the strategy overlays on the main chart
    pub overlay: Option<bool>,
    /// Price format
    pub format: Option<String>,
    /// Decimal precision for display
    pub precision: Option<i32>,
    /// Price scale mode
    pub scale: Option<String>,
    /// Maximum bars to look back
    pub max_bars_back: Option<i32>,
    /// Whether to track realtime updates
    pub calc_on_every_tick: Option<bool>,
    /// Whether to recalculate on order fill
    pub calc_on_order_fills: Option<bool>,
    /// Initial capital for backtesting
    pub initial_capital: Option<f64>,
    /// Default quantity for orders
    pub default_qty_value: Option<f64>,
    /// Default quantity type: "fixed", "percent_of_equity", "cash"
    pub default_qty_type: Option<String>,
    /// Currency for the strategy
    pub currency: Option<String>,
    /// Commission type: "percent", "cash_per_contract", "cash_per_order"
    pub commission_type: Option<String>,
    /// Commission value
    pub commission_value: Option<f64>,
    /// Slippage in ticks
    pub slippage: Option<i32>,
    /// Process orders on bar close
    pub process_orders_on_close: Option<bool>,
    /// Close entries rule: "FIFO", "ANY"
    pub close_entries_rule: Option<String>,
    /// Maximum open entries in same direction
    pub max_bars_back_entries: Option<i32>,
    /// Pyramiding: maximum number of entries in same direction
    pub pyramiding: Option<i32>,
    /// Fill orders on bar magnifier
    pub fill_orders_on_standard_ohlc: Option<bool>,
    /// Use bar magnifier for order fills
    pub use_bar_magnifier: Option<bool>,
    /// Risk free rate for Sharpe ratio calculation
    pub risk_free_rate: Option<f64>,
    /// Margin requirement (for futures)
    pub margin_long: Option<f64>,
    pub margin_short: Option<f64>,
    /// Maximum number of labels
    pub max_labels_count: Option<i32>,
    /// Maximum number of lines
    pub max_lines_count: Option<i32>,
    /// Maximum number of boxes
    pub max_boxes_count: Option<i32>,
    /// Maximum number of tables
    pub max_tables_count: Option<i32>,
}

/// Script type: indicator or strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScriptType {
    #[default]
    Indicator,
    Strategy,
}

#[derive(Debug, Clone)]
pub struct AstProgram {
    pub name: Option<String>,
    pub script_type: ScriptType,
    pub indicator_decl: IndicatorDecl,
    pub strategy_decl: StrategyDecl,
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
