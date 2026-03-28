//! ML-VM — Stack-Based Virtual Machine for Memphis Language
//! 
//! Executes ML bytecode compiled from AST.

use ml_core::MLExpr;

mod vm;
use crate::vm::VM;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum VMError {
    #[error("stack underflow at opcode {opcode:?}")]
    StackUnderflow { opcode: OpCode },
    #[error("type error: expected {expected}, got {got:?}")]
    TypeError { expected: &'static str, got: Value },
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
}

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

// ---------------------------------------------------------------------------
// Bytecode Opcodes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OpCode {
    // Stack ops
    Push,
    Pop,
    Dup,
    
    // Constants
    Const(u16),
    
    // Locals
    Load(u8),
    Store(u8),
    LoadArg(u8),
    
    // Control
    Jmp(u16),
    JmpIf(u16),
    JmpIfNot(u16),
    Call(u8, u8),
    Return,
    
    // Binary ops
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Not,
    
    // Gate/Sensor ops
    GateOn,
    GateOff,
    GateToggle,
    ReadTemp,
    ReadHumidity,
    ReadBool,
    Actuate,
    
    // Special
    MakeClosure(u8),
    Halt,
}

impl OpCode {
    fn encode(self) -> Vec<u8> {
        match self {
            OpCode::Push => vec![0x00],
            OpCode::Pop => vec![0x01],
            OpCode::Dup => vec![0x02],
            OpCode::Const(idx) => {
                vec![0x03, (idx >> 8) as u8, idx as u8]
            }
            OpCode::Load(idx) => vec![0x04, idx],
            OpCode::Store(idx) => vec![0x05, idx],
            OpCode::LoadArg(idx) => vec![0x06, idx],
            OpCode::Jmp(offset) => {
                vec![0x07, (offset >> 8) as u8, offset as u8]
            }
            OpCode::JmpIf(offset) => {
                vec![0x08, (offset >> 8) as u8, offset as u8]
            }
            OpCode::JmpIfNot(offset) => {
                vec![0x09, (offset >> 8) as u8, offset as u8]
            }
            OpCode::Call(arg_count, local_count) => vec![0x0A, arg_count, local_count],
            OpCode::Return => vec![0x09],
            OpCode::Add => vec![0x10],
            OpCode::Sub => vec![0x11],
            OpCode::Mul => vec![0x12],
            OpCode::Div => vec![0x13],
            OpCode::Mod => vec![0x14],
            OpCode::Eq => vec![0x15],
            OpCode::Ne => vec![0x16],
            OpCode::Lt => vec![0x17],
            OpCode::Gt => vec![0x18],
            OpCode::Le => vec![0x19],
            OpCode::Ge => vec![0x1A],
            OpCode::And => vec![0x1B],
            OpCode::Or => vec![0x1C],
            OpCode::Not => vec![0x1D],
            OpCode::GateOn => vec![0x20],
            OpCode::GateOff => vec![0x21],
            OpCode::GateToggle => vec![0x22],
            OpCode::ReadTemp => vec![0x23],
            OpCode::ReadHumidity => vec![0x24],
            OpCode::ReadBool => vec![0x25],
            OpCode::Actuate => vec![0x26],
            OpCode::MakeClosure(upvalue_count) => vec![0x30, upvalue_count],
            OpCode::Halt => vec![0xFF],
        }
    }
}

impl TryFrom<u8> for OpCode {
    type Error = VMError;
    
    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0x00 => Ok(OpCode::Push),
            0x01 => Ok(OpCode::Pop),
            0x02 => Ok(OpCode::Dup),
            0x03 => Ok(OpCode::Const(0)), // placeholder, read operands
            0x04 => Ok(OpCode::Load(0)),
            0x05 => Ok(OpCode::Store(0)),
            0x06 => Ok(OpCode::LoadArg(0)),
            0x07 => Ok(OpCode::Jmp(0)),
            0x08 => Ok(OpCode::JmpIf(0)),
            0x09 => Ok(OpCode::JmpIfNot(0)),
            0x0A => Ok(OpCode::Call(0, 0)),
            0x0B => Ok(OpCode::Return),
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
            0x1E => Ok(OpCode::GateOn),
            0x1F => Ok(OpCode::GateOff),
            0x20 => Ok(OpCode::GateToggle),
            0x21 => Ok(OpCode::ReadTemp),
            0x22 => Ok(OpCode::ReadHumidity),
            0x23 => Ok(OpCode::ReadBool),
            0x24 => Ok(OpCode::Actuate),
            0x30 => Ok(OpCode::MakeClosure(0)),
            0xFF => Ok(OpCode::Halt),
            _ => Err(VMError::InvalidBytecode(format!("unknown opcode: {byte:#x}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Value & Function Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Unit,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<Value>),
    Function(Function),
}

impl Value {
    fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }
    
    fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Unit => false,
            Value::List(_) => true,
            _ => true,
        }
    }
    
    fn type_name(&self) -> &'static str {
        match self {
            Value::Unit => "Unit",
            Value::Bool(_) => "Bool",
            Value::Number(_) => "Number",
            Value::String(_) => "String",
            Value::List(_) => "List",
            Value::Function(_) => "Function",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Function {
    pub arity: u8,
    pub locals: usize,
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
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
// Compiler
// ---------------------------------------------------------------------------

pub struct Compiler {
    code: Vec<u8>,
    constants: Vec<Value>,
    functions: HashMap<String, Function>,
    locals: Vec<String>,
    labels: HashMap<String, usize>,
    patch_jumps: Vec<(usize, usize)>, // (patch_offset, target_offset)
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            functions: HashMap::new(),
            locals: Vec::new(),
            labels: HashMap::new(),
            patch_jumps: Vec::new(),
        }
    }
    
    pub fn compile(ast: &MLExpr) -> Result<CompiledModule, CompileError> {
        let mut compiler = Self::new();
        compiler.compile_expr(ast)?;
        compiler.emit(OpCode::Halt);
        
        // Patch all jump offsets
        for (patch_offset, target_offset) in &compiler.patch_jumps {
            let offset = *target_offset as u16;
            compiler.code[*patch_offset] = (offset >> 8) as u8;
            compiler.code[*patch_offset + 1] = offset as u8;
        }
        
        Ok(CompiledModule {
            code: compiler.code,
            constants: compiler.constants,
            functions: compiler.functions,
        })
    }
    
    fn emit(&mut self, op: OpCode) {
        self.code.extend(op.encode());
    }
    
    fn add_constant(&mut self, value: Value) -> u16 {
        if let Some(idx) = self.constants.iter().position(|c| c == &value) {
            idx as u16
        } else {
            let idx = self.constants.len();
            if idx >= 65536 {
                panic!("too many constants");
            }
            self.constants.push(value);
            idx as u16
        }
    }
    
    fn push_local(&mut self, name: String) -> u8 {
        let idx = self.locals.len();
        if idx >= 256 {
            panic!("too many locals");
        }
        self.locals.push(name);
        idx as u8
    }
    
    fn pop_local(&mut self) {
        self.locals.pop();
    }
    
    fn get_local(&self, name: &str) -> Option<u8> {
        self.locals.iter().rev().position(|n| n == name).map(|p| p as u8)
    }
    
    fn emit_const(&mut self, value: Value) {
        let idx = self.add_constant(value);
        self.emit(OpCode::Const(idx));
    }
    
    fn emit_jump(&mut self, target_offset: usize) -> usize {
        let patch_offset = self.code.len();
        self.emit(OpCode::Jmp(0)); // placeholder
        self.patch_jumps.push((patch_offset, target_offset));
        patch_offset
    }
    
    fn emit_jump_if(&mut self, cond: bool) -> usize {
        let patch_offset = self.code.len();
        if cond {
            self.emit(OpCode::JmpIf(0));
        } else {
            self.emit(OpCode::JmpIfNot(0));
        }
        self.patch_jumps.push((patch_offset, 0)); // target filled later
        patch_offset
    }
    
    fn patch_jump(&mut self, patch_offset: usize, target_offset: usize) {
        if let Some((existing_patch, _)) = self.patch_jumps.iter().find(|(p, _)| *p == patch_offset) {
            self.patch_jumps.retain(|(p, _)| *p != patch_offset);
        }
        self.patch_jumps.push((patch_offset, target_offset));
    }
    
    fn compile_expr(&mut self, expr: &MLExpr) -> Result<(), CompileError> {
        match expr {
            MLExpr::Number(n) => {
                self.emit_const(Value::Number(*n));
            }
            MLExpr::Bool(b) => {
                self.emit_const(Value::Bool(*b));
            }
            MLExpr::String(s) => {
                self.emit_const(Value::String(s.clone()));
            }
            MLExpr::Var(name) => {
                if let Some(idx) = self.get_local(name) {
                    self.emit(OpCode::Load(idx));
                } else {
                    return Err(CompileError::UndefinedVariable(name.clone()));
                }
            }
            MLExpr::BinaryOp { op, left, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                match op.as_str() {
                    "+" => self.emit(OpCode::Add),
                    "-" => self.emit(OpCode::Sub),
                    "*" => self.emit(OpCode::Mul),
                    "/" => self.emit(OpCode::Div),
                    "%" => self.emit(OpCode::Mod),
                    "==" => self.emit(OpCode::Eq),
                    "!=" => self.emit(OpCode::Ne),
                    "<" => self.emit(OpCode::Lt),
                    ">" => self.emit(OpCode::Gt),
                    "<=" => self.emit(OpCode::Le),
                    ">=" => self.emit(OpCode::Ge),
                    "&&" | "and" => self.emit(OpCode::And),
                    "||" | "or" => self.emit(OpCode::Or),
                    "!" => self.emit(OpCode::Not),
                    _ => return Err(CompileError::UnsupportedOperator(op.clone())),
                }
            }
            MLExpr::UnaryOp { op, operand } => {
                self.compile_expr(operand)?;
                match op.as_str() {
                    "not" | "!" => self.emit(OpCode::Not),
                    _ => return Err(CompileError::UnsupportedOperator(op.clone())),
                }
            }
            MLExpr::Let { name, value, body } => {
                // Compile value
                self.compile_expr(value)?;
                // Store as local
                let idx = self.push_local(name.clone());
                self.emit(OpCode::Store(idx));
                // Compile body
                self.compile_expr(body)?;
                // Pop local (leave value on stack as result)
                self.pop_local();
            }
            MLExpr::Set { name, value } => {
                if let Some(idx) = self.get_local(name) {
                    self.compile_expr(value)?;
                    self.emit(OpCode::Store(idx));
                } else {
                    return Err(CompileError::UndefinedVariable(name.clone()));
                }
            }
            MLExpr::If { condition, then_branch, else_ } => {
                let else_offset = self.code.len() + 6; // approximate, will be patched
                
                // Compile condition (for now just compile then_branch and use JmpIfNot)
                self.compile_expr(then_branch)?;
                let patch_else = self.emit_jump_if(false);
                
                // Then branch is on stack
                // Jump past else
                let end_patch = self.emit_jump(0); // placeholder
                
                // Patch the else jump to here
                let else_target = self.code.len();
                self.patch_jump(patch_else, else_target);
                
                if let Some(else_branch) = else_ {
                    self.compile_expr(else_branch)?;
                }
                
                // Patch end jump
                let end_target = self.code.len();
                self.patch_jump(end_patch, end_target);
            }
            MLExpr::While { condition, body } => {
                let loop_start = self.code.len();
                self.compile_expr(condition)?;
                let patch_exit = self.emit_jump_if(false);
                self.compile_expr(body)?;
                self.emit(OpCode::Jmp(loop_start as u16));
                let exit_target = self.code.len();
                self.patch_jump(patch_exit, exit_target);
            }
            MLExpr::Begin(exprs) => {
                for expr in exprs {
                    self.compile_expr(expr)?;
                }
            }
            MLExpr::Sequence(exprs) => {
                for expr in exprs {
                    self.compile_expr(expr)?;
                }
            }
            MLExpr::Gate { id, state } => {
                self.emit_const(Value::String(id.clone()));
                match state.as_str() {
                    "on" => self.emit(OpCode::GateOn),
                    "off" => self.emit(OpCode::GateOff),
                    _ => self.emit(OpCode::GateToggle),
                }
            }
            MLExpr::Read { sensor } => {
                self.emit_const(Value::String(sensor.clone()));
                self.emit(OpCode::ReadTemp);
            }
            MLExpr::Wait { ms } => {
                // Wait is not directly supported in VM; emit a no-op placeholder
                self.emit_const(Value::Number(*ms as f64));
                self.emit(OpCode::Pop); // consume the ms value
            }
            MLExpr::Log { .. } => {
                // Log: compile the message expression, then pop the result (VM doesn't support log natively)
                // For now, just emit a placeholder
                self.emit(OpCode::Pop);
            }
            MLExpr::Fn { args, body } => {
                // Compile function into a nested CompiledModule
                let mut func_compiler = Self::new();
                for arg in args {
                    func_compiler.push_local(arg.clone());
                }
                func_compiler.compile_expr(body)?;
                func_compiler.emit(OpCode::Return);
                
                let func = Function {
                    arity: args.len() as u8,
                    locals: args.len(),
                    code: func_compiler.code,
                    constants: func_compiler.constants,
                };
                
                self.emit_const(Value::Function(func));
            }
            MLExpr::Defn { name, args, body } => {
                // Named function — compile and store
                let mut func_compiler = Self::new();
                for arg in args.iter().cloned() {
                    func_compiler.push_local(arg);
                }
                func_compiler.compile_expr(body)?;
                func_compiler.emit(OpCode::Return);
                
                let func = Function {
                    arity: args.len() as u8,
                    locals: args.len(),
                    code: func_compiler.code,
                    constants: func_compiler.constants,
                };
                self.functions.insert(name.clone(), func);
                self.emit_const(Value::Unit); // defn returns unit
            }
            MLExpr::Call { name, args } => {
                // Push arguments
                for arg in args {
                    self.compile_expr(arg)?;
                }
                // Push a placeholder for function (we'd look it up)
                // For now, just emit Call with arg count
                self.emit(OpCode::Call(args.len() as u8, args.len() as u8));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Disassembler
// ---------------------------------------------------------------------------

pub fn disassemble(code: &[u8], constants: &[Value]) -> String {
    let mut output = String::new();
    let mut pc = 0;
    
    while pc < code.len() {
        output.push_str(&format!("{:04x}: ", pc));
        
        let byte = code[pc];
        match OpCode::try_from(byte) {
            Ok(op) => {
                match op {
                    OpCode::Const(_) => {
                        if pc + 2 < code.len() {
                            let idx = ((code[pc + 1] as u16) << 8) | (code[pc + 2] as u16);
                            let const_val = constants.get(idx as usize)
                                .map(|v| format!("{:?}", v))
                                .unwrap_or_else(|| "??".to_string());
                            output.push_str(&format!("CONST ${:#x} = {}\n", idx, const_val));
                            pc += 3;
                        } else {
                            output.push_str("CONST (truncated)\n");
                            pc += 1;
                        }
                    }
                    OpCode::Load(idx) => {
                        output.push_str(&format!("LOAD ${:#x}\n", idx));
                        pc += 2;
                    }
                    OpCode::Store(idx) => {
                        output.push_str(&format!("STORE ${:#x}\n", idx));
                        pc += 2;
                    }
                    OpCode::LoadArg(idx) => {
                        output.push_str(&format!("LOAD_ARG ${:#x}\n", idx));
                        pc += 2;
                    }
                    OpCode::Jmp(_) => {
                        if pc + 2 < code.len() {
                            let offset = ((code[pc + 1] as u16) << 8) | (code[pc + 2] as u16);
                            output.push_str(&format!("JMP {:04x} (-> {})\n", offset, pc + 3 + offset as usize));
                            pc += 3;
                        } else {
                            output.push_str("JMP (truncated)\n");
                            pc += 1;
                        }
                    }
                    OpCode::JmpIf(_) => {
                        if pc + 2 < code.len() {
                            let offset = ((code[pc + 1] as u16) << 8) | (code[pc + 2] as u16);
                            output.push_str(&format!("JMP_IF {:04x} (-> {})\n", offset, pc + 3 + offset as usize));
                            pc += 3;
                        } else {
                            output.push_str("JMP_IF (truncated)\n");
                            pc += 1;
                        }
                    }
                    OpCode::JmpIfNot(_) => {
                        if pc + 2 < code.len() {
                            let offset = ((code[pc + 1] as u16) << 8) | (code[pc + 2] as u16);
                            output.push_str(&format!("JMP_IF_NOT {:04x} (-> {})\n", offset, pc + 3 + offset as usize));
                            pc += 3;
                        } else {
                            output.push_str("JMP_IF_NOT (truncated)\n");
                            pc += 1;
                        }
                    }
                    OpCode::Call(arg_count, local_count) => {
                        output.push_str(&format!("CALL {} {}\n", arg_count, local_count));
                        pc += 3;
                    }
                    OpCode::MakeClosure(upvalue_count) => {
                        output.push_str(&format!("MAKE_CLOSURE {}\n", upvalue_count));
                        pc += 2;
                    }
                    _ => {
                        output.push_str(&format!("{:?}\n", op));
                        pc += 1;
                    }
                }
            }
            Err(_) => {
                output.push_str(&format!("UNKNOWN {:#x}\n", byte));
                pc += 1;
            }
        }
    }
    
    output
}

// ---------------------------------------------------------------------------
// High-Level API
// ---------------------------------------------------------------------------

/// Compile and run ML source code
pub fn run(source: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let ast = MLExpr::parse(source)?;
    let module = Compiler::compile(&ast)?;
    let mut vm = VM::new(module.code, module.constants);
    vm.run().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

/// Compile ML AST and return bytecode module
pub fn compile(ast: &MLExpr) -> Result<CompiledModule, CompileError> {
    Compiler::compile(ast)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    
    fn test_vm(source: &str, expected: Value) {
        let ast = MLExpr::parse(source).expect("parse failed");
        let module = Compiler::compile(&ast).expect("compile failed");
        eprintln!("vm source: {}", source);
        eprintln!("vm bytecode: {:02x?}", module.code);
        eprintln!("vm constants: {:?}", module.constants);
        
        #[cfg(feature = "disasm")]
        {
            println!("=== Disassembly ===");
            println!("{}", disassemble(&module.code, &module.constants));
        }
        
        let mut vm = VM::new(module.code, module.constants);
        let result = vm.run();
        
        match result {
            Ok(v) => assert_eq!(v, expected, "result mismatch"),
            Err(e) => panic!("VM error: {}", e),
        }
    }
    
    #[test]
    fn test_number() {
        let ast = ml_core::MLExpr::parse("42").unwrap();
        let module = Compiler::compile(&ast).unwrap();
        eprintln!("compile code: {:02x?}", module.code);
        eprintln!("compile constants: {:?}", module.constants);
        test_vm("42", Value::Number(42.0));
    }
    
    #[test]
    fn test_add() {
        test_vm("(+ 3 5)", Value::Number(8.0));
    }
    
    #[test]
    fn test_nested_add() {
        test_vm("(+ (+ 1 2) (+ 3 4))", Value::Number(10.0));
    }
    
    #[test]
    fn test_sub() {
        test_vm("(- 10 3)", Value::Number(7.0));
    }
    
    #[test]
    fn test_mul() {
        test_vm("(* 6 7)", Value::Number(42.0));
    }
    
    #[test]
    fn test_div() {
        test_vm("(/ 20 4)", Value::Number(5.0));
    }
    
    #[test]
    fn test_mod() {
        test_vm("(% 17 5)", Value::Number(2.0));
    }
    
    #[test]
    fn test_comparison() {
        test_vm("(< 3 5)", Value::Bool(true));
        test_vm("(> 5 3)", Value::Bool(true));
        test_vm("(<= 3 3)", Value::Bool(true));
        test_vm("(>= 5 5)", Value::Bool(true));
        test_vm("(== 42 42)", Value::Bool(true));
        test_vm("(!= 1 2)", Value::Bool(true));
    }
    
    #[test]
    fn test_bool_ops() {
        test_vm("(&& true false)", Value::Bool(false));
        test_vm("(|| true false)", Value::Bool(true));
        test_vm("(! false)", Value::Bool(true));
    }
    
    #[test]
    fn test_let() {
        let source = "(let x 10 (+ x 5))";
        let ast = ml_core::MLExpr::parse(source).unwrap();
        let module = Compiler::compile(&ast).unwrap();
        eprintln!("test_let bytecode: {:02x?}", module.code);
        let mut vm = VM::new(module.code, module.constants);
        let result = vm.run();
        eprintln!("test_let result: {:?}", result);
        test_vm(source, Value::Number(15.0));
    }
    
    #[test]
    fn test_nested_let() {
        test_vm("(let x 10 (let y 20 (+ x y)))", Value::Number(30.0));
    }
    
    #[test]
    fn test_if() {
        test_vm("(if true 42 0)", Value::Number(42.0));
        test_vm("(if false 0 42)", Value::Number(42.0));
    }
    
    #[test]
    fn test_string() {
        test_vm("\"hello world\"", Value::String("hello world".to_string()));
    }
    
    #[test]
    fn test_compile_simple() {
        let ast = MLExpr::parse("(+ 3 5)").unwrap();
        let module = Compiler::compile(&ast).unwrap();
        assert!(!module.code.is_empty());
    }
    
    #[test]
    fn test_disassemble() {
        let ast = MLExpr::parse("(+ 3 5)").unwrap();
        let module = Compiler::compile(&ast).unwrap();
        let disasm = disassemble(&module.code, &module.constants);
        assert!(!disasm.is_empty());
        println!("{}", disasm);
    }
}
