//! ML-Core — Memphis Language for hardware control

pub mod ast;
pub mod error;
pub mod lexer;
pub mod machine;
pub mod parser;

pub use ast::{MLExpr, MLValue};
pub use error::{ParseError, RuntimeError};
pub use lexer::{tokenize, Token, TokenKind};
pub use machine::{Machine, MockMachine, MlHalMachine, Runtime};
pub use machine::Runtime as MlRuntime;

/// Szybki parsing + execution
pub fn run(source: &str) -> Result<MLValue, Box<dyn std::error::Error>> {
    let expr = parser::Parser::new(source).parse()?;
    let machine = MockMachine::new();
    let mut runtime = Runtime::new(machine);
    runtime.execute(expr).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
