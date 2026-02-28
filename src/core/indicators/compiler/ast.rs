#[derive(Debug, Clone)]
pub struct AstProgram {
    pub name: Option<String>,
    pub inputs: Vec<AstInputDecl>,
    pub statements: Vec<AstStatement>,
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
    If(AstIf),
    For(AstForLoop),
    FnDecl(AstFnDecl),
    Return(AstReturn),
}

#[derive(Debug, Clone)]
pub struct AstCall {
    pub function: String,
    pub args: Vec<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstForLoop {
    pub iterator: String,
    pub start: usize,
    pub end: usize,
    pub body: Vec<AstStatement>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstVarDecl {
    pub is_persistent: bool,
    pub name: String,
    pub value: Option<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstAssign {
    pub name: String,
    pub value: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct AstIf {
    pub condition: String,
    pub then_branch: Vec<AstStatement>,
    pub else_branch: Vec<AstStatement>,
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
    pub line: usize,
    pub column: usize,
}
