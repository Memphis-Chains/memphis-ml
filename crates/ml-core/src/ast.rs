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
    UnaryOp { op: String, operand: Box<MLExpr> },
    /// Early return from a function: (return expr)
    Return(Box<MLExpr>),
    /// Nil/null value
    Nil,
}

impl MLExpr {
    pub fn parse(source: &str) -> Result<Self, crate::error::ParseError> {
        crate::parser::Parser::new(source).parse()
    }
}

/// An upvalue — a captured variable from an outer scope.
/// `Local` means the variable is in the current frame (index into locals).
/// `Upvalue` means the variable is captured from an outer closure.
#[derive(Debug, Clone, PartialEq)]
pub enum UpvarKind {
    /// Captured from the immediate parent scope
    Local(usize),
    /// Captured from a nested closure's parent scope
    Up(usize),
}

/// A closure: captures its arguments, body, and a map of captured upvalues.
#[derive(Debug, Clone, PartialEq)]
pub struct Closure {
    pub args: Vec<String>,
    pub body: Box<MLExpr>,
    /// Captured variables: var_name -> upvalue_kind
    pub upvars: Vec<(String, UpvarKind)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MLValue {
    Unit,
    Bool(bool),
    Number(f64),
    String(String),
    /// First-class function: (args, body)
    Fn(Vec<String>, Box<MLExpr>),
    /// Nil/null value
    Nil,
    /// A closure with captured upvalues
    Closure(Closure),
    /// Internal sentinel for early return — NOT exposed to ML programs
    Return(Box<MLValue>),
}

impl MLValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            MLValue::Bool(b) => Some(*b),
            MLValue::Number(n) => Some(*n != 0.0),
            MLValue::String(s) => Some(!s.is_empty()),
            MLValue::Unit => Some(false),
            MLValue::Fn(..) | MLValue::Closure(..) => Some(true),
            MLValue::Nil => Some(false),
            MLValue::Return(_) => Some(false),
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            MLValue::Number(n) => Some(*n),
            MLValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            MLValue::Fn(..) | MLValue::Closure(..) => Some(1.0),
            _ => None,
        }
    }
}
