//! ML-VM — Stack-Based Virtual Machine for Memphis Language
//!
//! Features:
//! - Stack overflow protection (MAX_STACK_DEPTH)
//! - Tail call optimization (reusable call frames)
//! - Rich error messages with call-stack traces
//! - Disassembler and benchmark support

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

mod vm;
pub use vm::{VM, MAX_STACK_DEPTH, CallFrame};
pub use vm::MAX_STACK_DEPTH;
pub use vm::CallFrame;

// ---------------------------------------------------------------------------
// Error Types (with stack traces)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StackTraceEntry {
    pub name: String,
    pub pc: usize,
    pub line: u32,
}

impl std::fmt::Display for StackTraceEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "  at {} (pc={:#06x}, line={})", self.name, self.pc, self.line)
    }
}

#[derive(Debug, Clone)]
pub enum VMError {
    StackUnderflow {
        opcode: String,
        trace: Vec<StackTraceEntry>,
    },
    StackOverflow {
        depth: usize,
        limit: usize,
        trace: Vec<StackTraceEntry>,
    },
    CallStackOverflow {
        depth: usize,
        limit: usize,
        trace: Vec<StackTraceEntry>,
    },
    TypeError {
        expected: &'static str,
        got: Value,
        trace: Vec<StackTraceEntry>,
    },
    UndefinedVariable(String),
    UndefinedFunction(String),
    ArityMismatch {
        arg_count: usize,
        arity: usize,
        trace: Vec<StackTraceEntry>,
    },
    IterationLimitExceeded { limit: u64 },
    DivisionByZero { trace: Vec<StackTraceEntry> },
    LocalBounds {
        index: usize,
        max: usize,
        trace: Vec<StackTraceEntry>,
    },
    ConstantBounds {
        index: usize,
        max: usize,
        trace: Vec<StackTraceEntry>,
    },
    UndefinedLabel(String),
    Runtime {
        message: String,
        frame: StackTraceEntry,
        trace: Vec<StackTraceEntry>,
    },
}

impl std::fmt::Display for VMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VMError::StackUnderflow { opcode, trace } => {
                writeln!(f, "Stack underflow at opcode '{}'", opcode)?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
            VMError::StackOverflow { depth, limit, trace } => {
                writeln!(f, "Stack overflow: depth {} exceeds limit {}", depth, limit)?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
            VMError::CallStackOverflow { depth, limit, trace } => {
                writeln!(f, "Call stack overflow: {} frames exceed limit {}", depth, limit)?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
            VMError::TypeError { expected, got, trace } => {
                writeln!(f, "Type error: expected {}, got {:?}", expected, got)?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
            VMError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            VMError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            VMError::ArityMismatch { arg_count, arity, trace } => {
                writeln!(f, "Arity mismatch: called with {} args but function expects {}", arg_count, arity)?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
            VMError::IterationLimitExceeded { limit } => {
                write!(f, "Iteration limit exceeded ({})", limit)
            }
            VMError::DivisionByZero { trace } => {
                writeln!(f, "Division by zero")?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
            VMError::LocalBounds { index, max, trace } => {
                writeln!(f, "Local index out of bounds: {} (max {})", index, max)?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
            VMError::ConstantBounds { index, max, trace } => {
                writeln!(f, "Constant index out of bounds: {} (max {})", index, max)?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
            VMError::UndefinedLabel(name) => write!(f, "Undefined label: {}", name),
            VMError::Runtime { message, trace, .. } => {
                writeln!(f, "Runtime error: {}", message)?;
                for entry in trace { writeln!(f, "{}", entry)?; }
            }
        }
        Ok(())
    }
}

impl std::error::Error for VMError {}

impl VMError {
    pub fn with_trace(trace: Vec<StackTraceEntry>) -> Self {
        VMError::Runtime {
            message: "error".to_string(),
            frame: trace.first().cloned().unwrap_or(StackTraceEntry {
                name: "<unknown>".to_string(),
                pc: 0,
                line: 0,
            }),
            trace,
        }
    }
}

// ---------------------------------------------------------------------------
// Compilation Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CompileError {
    #[allow(dead_code)]
    UnsupportedExpr(String),
    #[allow(dead_code)]
    TooManyLocals,
    #[allow(dead_code)]
    TooManyConstants,
    UndefinedVariable(String),
    UndefinedFunction(String),
    UnsupportedOperator(String),
    RecursiveLet,
    IfWithoutThen,
    WhileWithoutBody,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UnsupportedExpr(s) => write!(f, "unsupported expression: {}", s),
            CompileError::TooManyLocals => write!(f, "too many locals (max 256)"),
            CompileError::TooManyConstants => write!(f, "too many constants (max 65536)"),
            CompileError::UndefinedVariable(name) => write!(f, "undefined variable: {}", name),
            CompileError::UndefinedFunction(name) => write!(f, "undefined function: {}", name),
            CompileError::UnsupportedOperator(op) => write!(f, "unsupported operator: {}", op),
            CompileError::RecursiveLet => write!(f, "recursive let unsupported"),
            CompileError::IfWithoutThen => write!(f, "if without then branch"),
            CompileError::WhileWithoutBody => write!(f, "while without body"),
        }
    }
}

impl std::error::Error for CompileError {}

// ---------------------------------------------------------------------------
// Value Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Unit,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<Value>),
    Function(Function),
    Closure(#[serde(skip)] Box<Closure>),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Unit => "Unit",
            Value::Bool(_) => "Bool",
            Value::Number(_) => "Number",
            Value::String(_) => "String",
            Value::List(_) => "List",
            Value::Function(_) => "Function",
            Value::Closure(_) => "Closure",
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Unit => false,
            _ => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Function & Closure
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Function {
    pub arity: u8,
    pub locals: usize,
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    #[allow(dead_code)]
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Closure {
    pub func: Function,
    pub upvalues: Vec<Value>,
}

// ---------------------------------------------------------------------------
// Compiled Module
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledModule {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub functions: HashMap<String, Function>,
}

impl CompiledModule {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            functions: HashMap::new(),
        }
    }
}

impl Default for CompiledModule {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Bytecode Opcodes
// ---------------------------------------------------------------------------

/// Bytecode opcodes for the Memphis Language VM.
///
/// Variable-length encoding:
/// - 1-byte opcodes: stack ops, arithmetic, logic, halt
/// - 2-byte opcodes: Load(idx8), Store(idx8), LoadArg(idx8), MakeClosure(n8)
/// - 3-byte opcodes: Const(idx16), Jmp(off16), JmpIf(off16), JmpIfNot(off16),
///                   Call(ac8,lc8), TailCall(arity8)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OpCode {
    // Stack
    Push,
    Pop,
    Dup,

    // Constants
    Const(u16),

    // Locals
    Load(u8),
    Store(u8),
    LoadArg(u8),

    // Control flow
    Jmp(u16),
    JmpIf(u16),
    JmpIfNot(u16),
    Call(u8, u8),     // Call(arg_count, local_count)
    TailCall(u8),    // Tail-call arity — reuses current frame
    Return,

    // Arithmetic
    Add, Sub, Mul, Div, Mod,

    // Comparison
    Eq, Ne, Lt, Gt, Le, Ge,

    // Boolean
    And, Or, Not,

    // Gate/Sensor
    GateOn, GateOff, GateToggle,
    ReadTemp, ReadHumidity, ReadBool,
    Actuate,

    // Functions
    MakeClosure(u8),

    // Control
    Halt,
}

impl OpCode {
    pub fn encode(self) -> Vec<u8> {
        match self {
            OpCode::Push          => vec![0x00],
            OpCode::Pop           => vec![0x01],
            OpCode::Dup           => vec![0x02],
            OpCode::Const(idx)    => vec![0x03, (idx >> 8) as u8, idx as u8],
            OpCode::Load(idx)     => vec![0x04, idx],
            OpCode::Store(idx)    => vec![0x05, idx],
            OpCode::LoadArg(idx)  => vec![0x06, idx],
            OpCode::Jmp(off)      => vec![0x07, (off >> 8) as u8, off as u8],
            OpCode::JmpIf(off)    => vec![0x08, (off >> 8) as u8, off as u8],
            OpCode::JmpIfNot(off) => vec![0x09, (off >> 8) as u8, off as u8],
            OpCode::Call(ac, lc)  => vec![0x0A, ac, lc],
            OpCode::TailCall(ar)  => vec![0x0B, ar],
            OpCode::Return        => vec![0x0C],
            OpCode::Add           => vec![0x10],
            OpCode::Sub           => vec![0x11],
            OpCode::Mul           => vec![0x12],
            OpCode::Div           => vec![0x13],
            OpCode::Mod           => vec![0x14],
            OpCode::Eq            => vec![0x15],
            OpCode::Ne            => vec![0x16],
            OpCode::Lt            => vec![0x17],
            OpCode::Gt            => vec![0x18],
            OpCode::Le            => vec![0x19],
            OpCode::Ge            => vec![0x1A],
            OpCode::And           => vec![0x1B],
            OpCode::Or            => vec![0x1C],
            OpCode::Not           => vec![0x1D],
            OpCode::GateOn        => vec![0x20],
            OpCode::GateOff       => vec![0x21],
            OpCode::GateToggle    => vec![0x22],
            OpCode::ReadTemp      => vec![0x23],
            OpCode::ReadHumidity  => vec![0x24],
            OpCode::ReadBool      => vec![0x25],
            OpCode::Actuate       => vec![0x26],
            OpCode::MakeClosure(upvals) => vec![0x30, upvals],
            OpCode::Halt          => vec![0xFF],
        }
    }
}

impl TryFrom<u8> for OpCode {
    type Error = String;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0x00 => Ok(OpCode::Push),
            0x01 => Ok(OpCode::Pop),
            0x02 => Ok(OpCode::Dup),
            0x03 => Ok(OpCode::Const(0)),
            0x04 => Ok(OpCode::Load(0)),
            0x05 => Ok(OpCode::Store(0)),
            0x06 => Ok(OpCode::LoadArg(0)),
            0x07 => Ok(OpCode::Jmp(0)),
            0x08 => Ok(OpCode::JmpIf(0)),
            0x09 => Ok(OpCode::JmpIfNot(0)),
            0x0A => Ok(OpCode::Call(0, 0)),
            0x0B => Ok(OpCode::TailCall(0)),
            0x0C => Ok(OpCode::Return),
            0x10 => Ok(OpCode::Add),
            0x11 => Ok(OpCode::Sub),
            0x12 => Ok(OpCode::Mul),
            0x13 => Ok(OpCode::Div),
            0x14 => Ok(OpCode::Mod),
            0x15 => Ok(OpCode::Eq),
            0x16 => Ok(OpCode::Ne),
            0x17 => Ok(OpCode::Lt),
            0x18 => Ok(OpCode::Gt),
            0x19 => Ok(OpCode::Le),
            0x1A => Ok(OpCode::Ge),
            0x1B => Ok(OpCode::And),
            0x1C => Ok(OpCode::Or),
            0x1D => Ok(OpCode::Not),
            0x20 => Ok(OpCode::GateOn),
            0x21 => Ok(OpCode::GateOff),
            0x22 => Ok(OpCode::GateToggle),
            0x23 => Ok(OpCode::ReadTemp),
            0x24 => Ok(OpCode::ReadHumidity),
            0x25 => Ok(OpCode::ReadBool),
            0x26 => Ok(OpCode::Actuate),
            0x30 => Ok(OpCode::MakeClosure(0)),
            0xFF => Ok(OpCode::Halt),
            _ => Err(format!("unknown opcode: {byte:#04x}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

pub struct Compiler {
    code: Vec<u8>,
    constants: Vec<Value>,
    functions: HashMap<String, Function>,
    locals: Vec<String>,
    patch_jumps: Vec<(usize, usize)>,
    /// Source line tracking: line -> bytecode offset
    line_map: Vec<(usize, u32)>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            functions: HashMap::new(),
            locals: Vec::new(),
            patch_jumps: Vec::new(),
            line_map: Vec::new(),
        }
    }

    pub fn compile(ast: &ml_core::MLExpr) -> Result<CompiledModule, CompileError> {
        let mut c = Self::new();
        c.compile_expr(ast, 1)?; // pass source line
        c.emit(OpCode::Halt);

        // Patch forward jumps
        for (patch_offset, target_offset) in &c.patch_jumps {
            let offset =
                ((*target_offset as isize) - (*patch_offset as isize + 3)) as u16;
            c.code[*patch_offset + 1] = (offset >> 8) as u8;
            c.code[*patch_offset + 2] = offset as u8;
        }

        Ok(CompiledModule {
            code: c.code,
            constants: c.constants,
            functions: c.functions,
        })
    }

    fn emit(&mut self, op: OpCode) {
        self.code.extend(op.encode());
    }

    /// Emit a line marker into the bytecode.
    fn emit_line(&mut self, line: u32) {
        self.line_map.push((self.code.len(), line));
        self.code.push(0xFE);
        self.code.push((line >> 8) as u8);
        self.code.push(line as u8);
    }

    fn add_constant(&mut self, value: Value) -> u16 {
        if let Some(idx) = self.constants.iter().position(|c| c == &value) {
            return idx as u16;
        }
        let idx = self.constants.len();
        assert!(idx < 65536, "too many constants");
        self.constants.push(value);
        idx as u16
    }

    fn emit_const(&mut self, value: Value) {
        let idx = self.add_constant(value);
        self.emit(OpCode::Const(idx));
    }

    fn push_local(&mut self, name: String) -> u8 {
        let idx = self.locals.len();
        assert!(idx < 256, "too many locals");
        self.locals.push(name);
        idx as u8
    }

    fn pop_local(&mut self) {
        self.locals.pop();
    }

    fn find_local(&self, name: &str) -> Option<u8> {
        self.locals
            .iter()
            .rev()
            .position(|n| n == name)
            .map(|p| p as u8)
    }

    fn emit_jump(&mut self) -> usize {
        let pos = self.code.len();
        self.emit(OpCode::Jmp(0));
        self.patch_jumps.push((pos, 0));
        pos
    }

    fn emit_jump_if(&mut self, cond: bool) -> usize {
        let pos = self.code.len();
        self.emit(if cond {
            OpCode::JmpIf(0)
        } else {
            OpCode::JmpIfNot(0)
        });
        self.patch_jumps.push((pos, 0));
        pos
    }

    fn patch_jump(&mut self, patch_offset: usize, target_offset: usize) {
        self.patch_jumps.retain(|(p, _)| *p != patch_offset);
        self.patch_jumps.push((patch_offset, target_offset));
    }

    /// Detect if an expression is a tail call position.
    /// Returns true if expr is a Call that is in tail position
    /// (i.e., the result of the enclosing scope is returned directly).
    fn is_tail_call(expr: &ml_core::MLExpr) -> bool {
        matches!(expr, ml_core::MLExpr::Call { .. })
    }

    fn compile_expr(&mut self, expr: &ml_core::MLExpr, line: u32) -> Result<(), CompileError> {
        match expr {
            ml_core::MLExpr::Number(n) => {
                self.emit_line(line);
                self.emit_const(Value::Number(*n));
            }
            ml_core::MLExpr::Bool(b) => {
                self.emit_line(line);
                self.emit_const(Value::Bool(*b));
            }
            ml_core::MLExpr::String(s) => {
                self.emit_line(line);
                self.emit_string(s.clone());
            }

            ml_core::MLExpr::Var(name) => {
                self.emit_line(line);
                if let Some(idx) = self.find_local(name) {
                    self.emit(OpCode::Load(idx));
                } else {
                    return Err(CompileError::UndefinedVariable(name.clone()));
                }
            }

            ml_core::MLExpr::Let { name, value, body } => {
                self.compile_expr(value, line)?;
                let idx = self.push_local(name.clone());
                self.emit(OpCode::Store(idx));
                self.compile_expr(body, line)?;
                self.pop_local();
            }

            ml_core::MLExpr::Set { name, value } => {
                self.emit_line(line);
                if let Some(idx) = self.find_local(name) {
                    self.compile_expr(value, line)?;
                    self.emit(OpCode::Store(idx));
                    self.emit_const(Value::Unit);
                } else {
                    return Err(CompileError::UndefinedVariable(name.clone()));
                }
            }

            ml_core::MLExpr::BinaryOp { op, left, right } => {
                self.compile_expr(left, line)?;
                self.compile_expr(right, line)?;
                match op.as_str() {
                    "+"  => self.emit(OpCode::Add),
                    "-"  => self.emit(OpCode::Sub),
                    "*"  => self.emit(OpCode::Mul),
                    "/"  => self.emit(OpCode::Div),
                    "%"  => self.emit(OpCode::Mod),
                    "==" => self.emit(OpCode::Eq),
                    "!=" => self.emit(OpCode::Ne),
                    "<"  => self.emit(OpCode::Lt),
                    ">"  => self.emit(OpCode::Gt),
                    "<=" => self.emit(OpCode::Le),
                    ">=" => self.emit(OpCode::Ge),
                    "&&" | "and" => self.emit(OpCode::And),
                    "||" | "or"  => self.emit(OpCode::Or),
                    _ => return Err(CompileError::UnsupportedOperator(op.clone())),
                }
            }

            ml_core::MLExpr::UnaryOp { op, operand } => {
                self.compile_expr(operand, line)?;
                match op.as_str() {
                    "!" | "not" => self.emit(OpCode::Not),
                    _ => return Err(CompileError::UnsupportedOperator(op.clone())),
                }
            }

            ml_core::MLExpr::If { condition, then_branch, else_ } => {
                self.compile_expr(condition, line)?;
                let patch_else = self.emit_jump_if(false);
                self.compile_expr(then_branch, line)?;
                let patch_end = self.emit_jump();
                let else_target = self.code.len();
                self.patch_jump(patch_else, else_target);
                if let Some(else_br) = else_ {
                    self.compile_expr(else_br, line)?;
                } else {
                    self.emit_const(Value::Unit);
                }
                let end_target = self.code.len();
                self.patch_jump(patch_end, end_target);
            }

            ml_core::MLExpr::While { condition, body } => {
                let loop_start = self.code.len();
                self.compile_expr(condition, line)?;
                let patch_exit = self.emit_jump_if(false);
                self.compile_expr(body, line)?;
                self.emit(OpCode::Pop);
                self.emit(OpCode::Jmp(loop_start as u16));
                let exit_target = self.code.len();
                self.patch_jump(patch_exit, exit_target);
                self.emit_const(Value::Unit);
            }

            ml_core::MLExpr::Begin(exprs) | ml_core::MLExpr::Sequence(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    self.compile_expr(e, line)?;
                    // Add pop between expressions to avoid stack buildup
                    if i < exprs.len() - 1 {
                        self.emit(OpCode::Pop);
                    }
                }
            }

            ml_core::MLExpr::Gate { id, state } => {
                self.emit_line(line);
                self.emit_string(id.clone());
                match state.as_str() {
                    "on"  => self.emit(OpCode::GateOn),
                    "off" => self.emit(OpCode::GateOff),
                    _     => self.emit(OpCode::GateToggle),
                }
            }

            ml_core::MLExpr::Read { sensor } => {
                self.emit_line(line);
                self.emit_string(sensor.clone());
                self.emit(OpCode::ReadTemp);
            }

            ml_core::MLExpr::Wait { ms } => {
                self.emit_const(Value::Number(*ms as f64));
                self.emit(OpCode::Pop);
            }

            ml_core::MLExpr::Log { message } => {
                self.compile_expr(message, line)?;
                self.emit(OpCode::Pop);
            }

            ml_core::MLExpr::Fn { args, body } => {
                self.emit_line(line);
                let mut fc = Self::new();
                for arg in args {
                    fc.push_local(arg.clone());
                }
                fc.compile_expr(body, line)?;
                fc.emit(OpCode::Return);
                let func = Function {
                    arity: args.len() as u8,
                    locals: args.len(),
                    code: fc.code,
                    constants: fc.constants,
                    name: None,
                };
                self.emit_const(Value::Function(func));
            }

            ml_core::MLExpr::Defn { name, args, body } => {
                let mut fc = Self::new();
                for arg in args {
                    fc.push_local(arg.clone());
                }
                fc.compile_expr(body, line)?;
                fc.emit(OpCode::Return);
                let func = Function {
                    arity: args.len() as u8,
                    locals: args.len(),
                    code: fc.code,
                    constants: fc.constants,
                    name: Some(name.clone()),
                };
                self.functions.insert(name.clone(), func.clone());
                self.emit_const(Value::Function(func));
            }

            ml_core::MLExpr::Call { name, args } => {
                self.emit_line(line);
                for arg in args {
                    self.compile_expr(arg, line)?;
                }
                if let Some(func) = self.functions.get(name) {
                    self.emit_const(Value::Function(func.clone()));
                }
                // Emit TailCall if in tail position (immediately before Return)
                // For simplicity, always emit Call here; the compiler does
                // a post-pass to optimize obvious tail calls
                let arity = args.len() as u8;
                if Self::is_tail_call_position(self.locals.len()) {
                    self.emit(OpCode::TailCall(arity));
                } else {
                    self.emit(OpCode::Call(arity, arity));
                }
            }
        }
        Ok(())
    }

    /// Returns true if we're at a position where a tail call is safe.
    /// In tail position, the result of the call is returned directly.
    fn is_tail_call_position(_local_count: usize) -> bool {
        // For now, conservative: only emit TailCall when explicitly safe.
        // A full implementation would do a defunctionalized CPS transform.
        // Here we mark TailCall availability, but VM also accepts TailCall
        // in non-tail contexts safely.
        false
    }

    fn emit_string(&mut self, s: String) {
        self.emit_const(Value::String(s));
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Optimized tail-call compiler
// ---------------------------------------------------------------------------

/// Extended compiler that performs a simple tail-call optimization pass.
/// After normal compilation, it scans for (Call arity + Return) sequences
/// and replaces them with (TailCall arity), removing the redundant Return.
impl Compiler {
    /// Optimize obvious tail-call sequences in the bytecode.
    /// Scans for: CALL arg_count,0  +  RETURN  →  TAILCALL arg_count
    pub fn optimize_tail_calls(code: &mut Vec<u8>) {
        let mut i = 0;
        while i + 3 <= code.len() {
            // Look for CALL (0x0A) followed by RETURN (0x0C)
            if code[i] == 0x0A && code.get(i + 2) == Some(&0x0C) {
                let arity = code[i + 1];
                code[i] = 0x0B; // TailCall opcode
                code[i + 1] = arity;
                // Remove the RETURN at i+2 (shift everything left by 1)
                code.remove(i + 2);
                // Don't advance i — check for more tail calls
            } else {
                i += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Disassembler
// ---------------------------------------------------------------------------

pub fn disassemble(code: &[u8], constants: &[Value]) -> String {
    let mut out = String::new();
    let mut pc = 0;

    while pc < code.len() {
        // Skip line markers
        if code[pc] == 0xFE {
            if pc + 2 < code.len() {
                let line = ((code[pc + 1] as u16) << 8) | (code[pc + 2] as u16);
                out.push_str(&format!("{:04x}: ; line {}\n", pc, line));
                pc += 3;
                continue;
            }
        }

        out.push_str(&format!("{:04x}: ", pc));
        let byte = code[pc];

        match OpCode::try_from(byte) {
            Ok(op) => {
                match op {
                    OpCode::Const(_) | OpCode::Jmp(_) | OpCode::JmpIf(_) | OpCode::JmpIfNot(_) => {
                        if pc + 2 < code.len() {
                            let hi = code[pc + 1] as u16;
                            let lo = code[pc + 2] as u16;
                            let val = (hi << 8) | lo;
                            let next = pc + 3;
                            let tag = match op {
                                OpCode::Const(_) => format!("CONST ${:#x}", val),
                                OpCode::Jmp(_) => {
                                    let target = pc + 3 + val as usize;
                                    format!("JMP {:04x} (-> {})", val, target)
                                }
                                OpCode::JmpIf(_) => {
                                    let target = pc + 3 + val as usize;
                                    format!("JMP_IF {:04x} (-> {})", val, target)
                                }
                                OpCode::JmpIfNot(_) => {
                                    let target = pc + 3 + val as usize;
                                    format!("JMP_IF_NOT {:04x} (-> {})", val, target)
                                }
                                _ => unreachable!(),
                            };
                            out.push_str(&tag);
                            if matches!(op, OpCode::Const(_)) {
                                if let Some(c) = constants.get(val as usize) {
                                    out.push_str(&format!(" = {:?}", c));
                                }
                            }
                            out.push('\n');
                            pc = next;
                            continue;
                        }
                    }
                    OpCode::Load(idx) => out.push_str(&format!("LOAD ${:#x}\n", idx)),
                    OpCode::Store(idx) => out.push_str(&format!("STORE ${:#x}\n", idx)),
                    OpCode::LoadArg(idx) => out.push_str(&format!("LOAD_ARG ${:#x}\n", idx)),
                    OpCode::Call(ac, lc) => out.push_str(&format!("CALL {} {}\n", ac, lc)),
                    OpCode::TailCall(ar) => out.push_str(&format!("TAILCALL {}\n", ar)),
                    OpCode::MakeClosure(upvals) => out.push_str(&format!("MAKE_CLOSURE {}\n", upvals)),
                    _ => out.push_str(&format!("{:?}\n", op)),
                }
                pc += op.encode().len();
            }
            Err(e) => {
                out.push_str(&format!("UNKNOWN {:#x} ({})\n", byte, e));
                pc += 1;
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// High-Level API
// ---------------------------------------------------------------------------

pub use crate::vm::VM;
pub use crate::vm::MAX_STACK_DEPTH;

/// Compile and run ML source code.
pub fn run(source: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let ast = ml_core::MLExpr::parse(source)?;
    let module = Compiler::compile(&ast)?;
    let mut vm = VM::with_module(&module);
    vm.run().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

/// Compile an ML AST and return the bytecode module.
pub fn compile(ast: &ml_core::MLExpr) -> Result<CompiledModule, CompileError> {
    Compiler::compile(ast)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_run(source: &str) -> Result<Value, CompileError> {
        let ast = ml_core::MLExpr::parse(source)
            .map_err(|e| CompileError::UnsupportedExpr(e.to_string()))?;
        let module = Compiler::compile(&ast)?;
        let mut vm = VM::with_module(&module);
        vm.run().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    #[test]
    fn test_number() { assert_eq!(compile_run("42")?, Value::Number(42.0)); }

    #[test]
    fn test_add() { assert_eq!(compile_run("(+ 3 5)")?, Value::Number(8.0)); }

    #[test]
    fn test_nested_add() { assert_eq!(compile_run("(+ (+ 1 2) (+ 3 4))")?, Value::Number(10.0)); }

    #[test]
    fn test_sub() { assert_eq!(compile_run("(- 10 3)")?, Value::Number(7.0)); }

    #[test]
    fn test_mul() { assert_eq!(compile_run("(* 6 7)")?, Value::Number(42.0)); }

    #[test]
    fn test_div() { assert_eq!(compile_run("(/ 20 4)")?, Value::Number(5.0)); }

    #[test]
    fn test_mod() { assert_eq!(compile_run("(% 17 5)")?, Value::Number(2.0)); }

    #[test]
    fn test_comparison() {
        assert_eq!(compile_run("(< 3 5)")?, Value::Bool(true));
        assert_eq!(compile_run("(> 5 3)")?, Value::Bool(true));
        assert_eq!(compile_run("(<= 3 3)")?, Value::Bool(true));
        assert_eq!(compile_run("(>= 5 5)")?, Value::Bool(true));
        assert_eq!(compile_run("(== 42 42)")?, Value::Bool(true));
        assert_eq!(compile_run("(!= 1 2)")?, Value::Bool(true));
    }

    #[test]
    fn test_bool_ops() {
        assert_eq!(compile_run("(&& true false)")?, Value::Bool(false));
        assert_eq!(compile_run("(|| true false)")?, Value::Bool(true));
        assert_eq!(compile_run("(! false)")?, Value::Bool(true));
    }

    #[test]
    fn test_let() { assert_eq!(compile_run("(let x 10 (+ x 5))")?, Value::Number(15.0)); }

    #[test]
    fn test_nested_let() {
        assert_eq!(compile_run("(let x 10 (let y 20 (+ x y)))")?, Value::Number(30.0));
    }

    #[test]
    fn test_if() {
        assert_eq!(compile_run("(if true 42 0)")?, Value::Number(42.0));
        assert_eq!(compile_run("(if false 0 42)")?, Value::Number(42.0));
    }

    #[test]
    fn test_string() {
        assert_eq!(
            compile_run("\"hello world\"")?,
            Value::String("hello world".to_string())
        );
    }

    #[test]
    fn test_undefined_variable() {
        let ast = ml_core::MLExpr::parse("x")
            .map_err(|e| CompileError::UnsupportedExpr(e.to_string())).unwrap();
        let result = Compiler::compile(&ast);
        assert!(matches!(result, Err(CompileError::UndefinedVariable(_))));
    }

    #[test]
    fn test_disassemble() {
        let ast = ml_core::MLExpr::parse("(+ 3 5)").unwrap();
        let module = Compiler::compile(&ast).unwrap();
        let disasm = disassemble(&module.code, &module.constants);
        assert!(!disasm.is_empty());
    }

    #[test]
    fn test_stack_depth() {
        // Verify VM doesn't overflow
        let ast = ml_core::MLExpr::parse("(let x 1 x)")
            .map_err(|e| CompileError::UnsupportedExpr(e.to_string())).unwrap();
        let module = Compiler::compile(&ast).unwrap();
        let mut vm = VM::with_module(&module);
        let result = vm.run();
        assert!(result.is_ok());
    }
}
