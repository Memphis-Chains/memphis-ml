// ML-Core Error types

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken(String),
    UnexpectedEof,
    InvalidNumber(String),
    UnclosedParen,
    EmptyExpr,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken(s) => write!(f, "unexpected token: {}", s),
            ParseError::UnexpectedEof => write!(f, "unexpected end of input"),
            ParseError::InvalidNumber(s) => write!(f, "invalid number: {}", s),
            ParseError::UnclosedParen => write!(f, "unclosed parenthesis"),
            ParseError::EmptyExpr => write!(f, "empty expression"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug)]
pub enum RuntimeError {
    GateNotFound(String),
    SensorNotFound(String),
    UndefinedVariable(String),
    TypeMismatch(String),
    Machine(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::GateNotFound(s) => write!(f, "gate not found: {}", s),
            RuntimeError::SensorNotFound(s) => write!(f, "sensor not found: {}", s),
            RuntimeError::UndefinedVariable(s) => write!(f, "undefined variable: {}", s),
            RuntimeError::TypeMismatch(s) => write!(f, "type mismatch: {}", s),
            RuntimeError::Machine(s) => write!(f, "machine error: {}", s),
        }
    }
}

impl std::error::Error for RuntimeError {}
