//! Execution engine for the ML bytecode VM.

use crate::error::VmError;
use crate::opcode::{self, OpCode};
use ml_core::{Machine, MockMachine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Value
// ---------------------------------------------------------------------------

/// Runtime values on the VM stack and in locals.
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
            Value::Function(_) => true,
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

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::List(vs) => write!(f, "[{:?}]", vs),
            Value::Function(_) => write!(f, "<fn>"),
        }
    }
}

// ---------------------------------------------------------------------------
// Function
// ---------------------------------------------------------------------------

/// A compiled function stored in the constant pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Function {
    pub arity: u8,
    pub locals: usize,
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
}

// ---------------------------------------------------------------------------
// Frame (call frame for functions)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Frame {
    /// Local variables for this frame
    locals: Vec<Value>,
    /// Program counter to return to
    return_pc: usize,
}

impl Frame {
    fn new(locals: usize, return_pc: usize) -> Self {
        Self {
            locals: vec![Value::Unit; locals],
            return_pc,
        }
    }
}

// ---------------------------------------------------------------------------
// Compiled Module
// ---------------------------------------------------------------------------

/// A bytecode module ready for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledModule {
    /// Raw bytecode instructions
    pub code: Vec<u8>,
    /// Constant pool (CONST i loads constants[i])
    pub constants: Vec<Value>,
    /// Named functions defined in this module
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
// Virtual Machine
// ---------------------------------------------------------------------------

/// Stack-based bytecode VM for ML programs.
pub struct VM {
    /// Instruction memory (current function's bytecode)
    code: Vec<u8>,
    /// Constant pool (current function's constants)
    constants: Vec<Value>,
    /// Current function's local slots
    locals: Vec<Value>,
    /// Operand stack
    stack: Vec<Value>,
    /// Call frames stack
    frames: Vec<Frame>,
    /// Program counter (index into `code`)
    pc: usize,
    /// Hardware machine (for gate/sensor ops)
    machine: Box<dyn Machine>,
    /// Max iterations before aborting
    max_iterations: u64,
    /// Iteration counter
    iteration_count: u64,
    /// Global functions
    functions: HashMap<String, Function>,
}

impl std::fmt::Debug for VM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VM")
            .field("pc", &self.pc)
            .field("stack_len", &self.stack.len())
            .field("frames_len", &self.frames.len())
            .field("iteration_count", &self.iteration_count)
            .finish()
    }
}

impl VM {
    /// Create a new VM from a compiled module, using a MockMachine.
    pub fn new(module: CompiledModule) -> Self {
        Self::with_machine(module, Box::new(MockMachine::new()))
    }

    /// Create a new VM with a custom machine backend.
    pub fn with_machine(module: CompiledModule, machine: Box<dyn Machine>) -> Self {
        Self {
            code: module.code,
            constants: module.constants,
            locals: Vec::new(),
            stack: Vec::new(),
            frames: Vec::new(),
            pc: 0,
            machine,
            max_iterations: 1_000_000,
            iteration_count: 0,
            functions: module.functions,
        }
    }

    /// Set the max iteration limit (default 1_000_000).
    pub fn with_max_iterations(mut self, max: u64) -> Self {
        self.max_iterations = max;
        self
    }

    /// Execute the bytecode to completion, returning the final stack value.
    pub fn run(&mut self) -> Result<Value, VmError> {
        loop {
            if self.iteration_count >= self.max_iterations {
                return Err(VmError::IterationLimitExceeded { limit: self.max_iterations });
            }
            self.iteration_count += 1;

            let op = match self.fetch() {
                Ok(op) => op,
                Err(VmError::Halt) => break,
                Err(e) => return Err(e),
            };

            if let OpCode::Halt = op {
                break;
            }

            self.execute(op)?;
        }

        Ok(self.stack_pop().unwrap_or(Value::Unit))
    }

    // ── Fetch / Execute ────────────────────────────────────────────────────

    fn fetch(&mut self) -> Result<OpCode, VmError> {
        if self.pc >= self.code.len() {
            return Err(VmError::Halt);
        }
        let byte = self.code[self.pc];
        self.pc += 1;
        OpCode::try_from(byte).map_err(|_| VmError::InvalidBytecode(format!("unknown opcode byte: {byte:#04x}")))
    }

    fn execute(&mut self, op: OpCode) -> Result<(), VmError> {
        match op {
            OpCode::Push => {
                let idx = self.read_u16()?;
                let c = self.constants.get(idx as usize)
                    .ok_or(VmError::ConstantBounds { index: idx as usize, max: self.constants.len() })?
                    .clone();
                self.stack_push(c);
            }
            OpCode::Pop => {
                self.stack_pop()
                    .ok_or_else(|| VmError::StackUnderflow { opcode: "Pop" })?;
            }
            OpCode::Dup => {
                let top = self.stack_peek()?.clone();
                self.stack_push(top);
            }
            OpCode::Const(idx) => {
                let c = self.constants.get(idx as usize)
                    .ok_or(VmError::ConstantBounds { index: idx as usize, max: self.constants.len() })?
                    .clone();
                self.stack_push(c);
            }
            OpCode::Load(idx) => {
                let v = self.locals.get(idx as usize)
                    .ok_or(VmError::LocalBounds { index: idx as usize, max: self.locals.len() })?
                    .clone();
                self.stack_push(v);
            }
            OpCode::Store(idx) => {
                let v = self.stack_pop()
                    .ok_or(VmError::StackUnderflow { opcode: "Store" })?;
                if idx as usize >= self.locals.len() {
                    self.locals.resize(idx as usize + 1, Value::Unit);
                }
                self.locals[idx as usize] = v;
            }
            OpCode::Goto(offset) => {
                // offset is a raw byte displacement; add it to current pc
                self.pc = self.pc.wrapping_add(offset as usize);
            }
            OpCode::IfGoto(offset) => {
                let cond = self.pop_bool()?;
                if cond {
                    self.pc = self.pc.wrapping_add(offset as usize);
                }
            }
            OpCode::Call(arg_count, _local_count) => {
                let func = self.stack_pop()
                    .ok_or(VmError::StackUnderflow { opcode: "Call" })?;
                match func {
                    Value::Function(f) => {
                        // Save current frame (with current locals)
                        let old_locals = std::mem::take(&mut self.locals);
                        let frame = Frame::new(old_locals.len(), self.pc);
                        self.frames.push(frame);

                        // Pop arguments into new locals
                        self.locals = vec![Value::Unit; f.locals];
                        for i in (0..arg_count).rev() {
                            let arg = self.stack_pop()
                                .ok_or(VmError::StackUnderflow { opcode: "Call" })?;
                            self.locals[i as usize] = arg;
                        }

                        // Push function's constants as our constants
                        self.code = f.code;
                        self.constants = f.constants;
                        self.pc = 0;
                    }
                    Value::Unit | Value::Number(_) | Value::Bool(_) | Value::String(_) | Value::List(_) => {
                        return Err(VmError::TypeError { expected: "Function", got: func.type_name().into() });
                    }
                }
            }
            OpCode::Return => {
                let result = self.stack_pop().unwrap_or(Value::Unit);
                if let Some(frame) = self.frames.pop() {
                    self.locals = frame.locals;
                    self.pc = frame.return_pc;
                }
                self.stack_push(result);
                return Ok(());
            }
            OpCode::Halt => {
                return Err(VmError::Halt);
            }
            OpCode::Add => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack_push(Value::Number(a + b));
            }
            OpCode::Sub => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack_push(Value::Number(a - b));
            }
            OpCode::Mul => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack_push(Value::Number(a * b));
            }
            OpCode::Div => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                if b == 0.0 {
                    return Err(VmError::DivisionByZero);
                }
                self.stack_push(Value::Number(a / b));
            }
            OpCode::Gate => {
                // Stack: [..., gate_id (String), state (String)]
                let state = self.pop_string()?;
                let id = self.pop_string()?;
                self.machine.set_gate(&id, &state)
                    .map_err(|e| VmError::MachineError(e.to_string()))?;
                self.stack_push(Value::Unit);
            }
            OpCode::SensorRead => {
                // Stack: [..., sensor_id (String)]
                let id = self.pop_string()?;
                let val = self.machine.read_sensor(&id)
                    .map_err(|e| VmError::MachineError(e.to_string()))?;
                self.stack_push(Value::Number(val));
            }
            OpCode::Log => {
                // Stack: [..., message (Value)]
                let msg = self.stack_pop().unwrap_or(Value::Unit);
                println!("[ML] {}", msg);
                self.stack_push(Value::Unit);
            }
            OpCode::MakeClosure(upval_count) => {
                // For now, just push a placeholder unit
                let _: Vec<Value> = (0..upval_count)
                    .map(|_| self.stack_pop().unwrap_or(Value::Unit))
                    .collect();
                self.stack_push(Value::Unit);
            }
        }
        Ok(())
    }

    // ── Operand decoding ──────────────────────────────────────────────────

    fn read_u16(&mut self) -> Result<u16, VmError> {
        if self.pc + 1 >= self.code.len() {
            return Err(VmError::InvalidBytecode("unexpected end of bytecode".into()));
        }
        let hi = self.code[self.pc] as u16;
        let lo = self.code[self.pc + 1] as u16;
        self.pc += 2;
        Ok((hi << 8) | lo)
    }

    // ── Stack helpers ────────────────────────────────────────────────────

    fn stack_push(&mut self, v: Value) {
        self.stack.push(v);
    }

    fn stack_pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn stack_peek(&self) -> Result<Value, VmError> {
        self.stack.last().cloned()
            .ok_or_else(|| VmError::StackUnderflow { opcode: "Peek" })
    }

    fn pop_number(&mut self) -> Result<f64, VmError> {
        self.stack_pop()
            .and_then(|v| v.as_number())
            .ok_or_else(|| VmError::TypeError {
                expected: "Number",
                got: self.stack.last().map(|v| v.type_name().into()).unwrap_or_default(),
            })
    }

    fn pop_bool(&mut self) -> Result<bool, VmError> {
        Ok(self.stack_pop().map(|v| v.as_bool()).unwrap_or(false))
    }

    fn pop_string(&mut self) -> Result<String, VmError> {
        match self.stack_pop() {
            Some(Value::String(s)) => Ok(s),
            Some(v) => Err(VmError::TypeError { expected: "String", got: v.type_name().into() }),
            None => Err(VmError::StackUnderflow { opcode: "PopString" }),
        }
    }
}

// ---------------------------------------------------------------------------
// Disassembler
// ---------------------------------------------------------------------------

/// Human-readable disassembly of bytecode.
pub fn disassemble(code: &[u8], constants: &[Value]) -> String {
    let mut out = String::new();
    let mut pc = 0;
    while pc < code.len() {
        let (line, next) = opcode::disassemble_op(code, pc);
        out.push_str(&format!("{:04x}: {}\n", pc, line));
        pc = next;
    }
    out
}
