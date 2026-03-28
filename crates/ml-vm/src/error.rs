//! Error types for the ML Virtual Machine.

use thiserror::Error;

// ---------------------------------------------------------------------------
// VM Runtime Errors
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum VmError {
    #[error("stack underflow at opcode {opcode:?}")]
    StackUnderflow { opcode: &'static str },

    #[error("type error: expected {expected}, got {got:?}")]
    TypeError { expected: &'static str, got: String },

    #[error("undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("undefined function: {0}")]
    UndefinedFunction(String),

    #[error("call with {arg_count} args but function expects {arity}")]
    ArityMismatch { arg_count: usize, arity: usize },

    #[error("iteration limit exceeded ({limit})")]
    IterationLimitExceeded { limit: u64 },

    #[error("invalid bytecode: {0}")]
    InvalidBytecode(String),

    #[error("machine error: {0}")]
    MachineError(String),

    #[error("division by zero")]
    DivisionByZero,

    #[error("local index out of bounds: {index} (max {max})")]
    LocalBounds { index: usize, max: usize },

    #[error("constant index out of bounds: {index} (max {max})")]
    ConstantBounds { index: usize, max: usize },

    #[error("undefined label: {0}")]
    UndefinedLabel(String),

    #[error("halt")]
    Halt,
}

// ---------------------------------------------------------------------------
// Compilation Errors
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum CompileError {
    #[error("unsupported expression: {0}")]
    UnsupportedExpr(String),

    #[error("too many locals (max 256)")]
    TooManyLocals,

    #[error("too many constants (max 65536)")]
    TooManyConstants,

    #[error("undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("undefined function: {0}")]
    UndefinedFunction(String),

    #[error("unsupported binary operator: {0}")]
    UnsupportedOperator(String),

    #[error("recursive let unsupported")]
    RecursiveLet,

    #[error("if without then branch")]
    IfWithoutThen,

    #[error("while without body")]
    WhileWithoutBody,
}
