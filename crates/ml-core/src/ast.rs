// ML-Core AST — Memphis Language Core

#[derive(Debug, Clone)]
pub enum MLExpr {
    // Sterowanie bramkami
    Gate { id: String, state: GateState },
    // Odczyt czujników
    Read { sensor: String },
    // Sekwencja poleceń
    Sequence(Vec<MLExpr>),
    // Warunek if
    If { condition: Condition, then_branch: Box<MLExpr>, else_: Option<Box<MLExpr>> },
    // Czekanie
    Wait { ms: u64 },
    // Log
    Log { message: String },
    // Zmienna lokalna
    Let { name: String, value: Box<MLExpr>, body: Box<MLExpr> },
    // Referencja do zmiennej
    Var(String),
    // Wartości
    Bool(bool),
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GateState {
    On,
    Off,
    Toggle,
}

#[derive(Debug, Clone)]
pub enum Condition {
    Bool(bool),
    Eq(Box<MLValue>, Box<MLValue>),
    Gt(Box<MLValue>, Box<MLValue>),
    Lt(Box<MLValue>, Box<MLValue>),
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
    Not(Box<Condition>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MLValue {
    Unit,
    Bool(bool),
    Number(f64),
    String(String),
    Var(String),
    Sensor(String),
    Gate(String),
}

impl MLExpr {
    pub fn parse(source: &str) -> Result<Self, crate::error::ParseError> {
        crate::parser::Parser::new(source).parse()
    }
}
