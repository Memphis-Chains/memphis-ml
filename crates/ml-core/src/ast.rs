// ML-Core AST — Memphis Language Core

#[derive(Debug, Clone, PartialEq)]
pub enum MLExpr {
    Gate { id: String, state: String },
    Read { sensor: String },
    Sequence(Vec<MLExpr>),
    If { condition: Box<MLExpr>, then_branch: Box<MLExpr>, else_: Option<Box<MLExpr>> },
    Wait { ms: u64 },
    Log { message: Box<MLExpr> },
    Let { name: String, value: Box<MLExpr>, body: Box<MLExpr> },
    Var(String),
    Bool(bool),
    Number(f64),
    String(String),
    Fn { args: Vec<String>, body: Box<MLExpr> },
    /// Named function definition: (fn name (args) body) — stored in Runtime::functions
    Defn { name: String, args: Vec<String>, body: Box<MLExpr> },
    Call { name: String, args: Vec<MLExpr> },
    Set { name: String, value: Box<MLExpr> },
    While { condition: Box<MLExpr>, body: Box<MLExpr> },
    Begin(Vec<MLExpr>),
    BinaryOp { op: String, left: Box<MLExpr>, right: Box<MLExpr> },
}

impl MLExpr {
    pub fn parse(source: &str) -> Result<Self, crate::error::ParseError> {
        crate::parser::Parser::new(source).parse()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MLValue {
    Unit,
    Bool(bool),
    Number(f64),
    String(String),
    /// First-class function: (args, body)
    Fn(Vec<String>, Box<MLExpr>),
}

impl MLValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            MLValue::Bool(b) => Some(*b),
            MLValue::Number(n) => Some(*n != 0.0),
            MLValue::String(s) => Some(!s.is_empty()),
            MLValue::Unit => Some(false),
            MLValue::Fn(..) => Some(true),
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            MLValue::Number(n) => Some(*n),
            MLValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            MLValue::Fn(..) => Some(1.0),
            _ => None,
        }
    }
}
