//! Simple Bytecode VM for Memphis Language (ML)
//! Clean implementation with correct PC management.
//!
//! PC Management rules:
//! - fetch() reads opcode at pc, advances pc past it
//! - Multi-byte opcodes (Const, Load, Store, Jmp) read operands in execute(), advance pc past them
//! - Single-byte opcodes do NOT advance pc in execute (fetch already did)

use crate::{VMError, OpCode, Value};

pub struct VM {
    code: Vec<u8>,
    constants: Vec<Value>,
    stack: Vec<Value>,
    locals: Vec<Value>,
    pc: usize,
}

impl VM {
    pub fn new(code: Vec<u8>, constants: Vec<Value>) -> Self {
        Self {
            code,
            constants,
            stack: Vec::new(),
            locals: vec![Value::Unit; 32], // pre-allocated locals
            pc: 0,
        }
    }

    /// Read next byte and advance PC by 1
    fn read_byte(&mut self) -> Result<u8, VMError> {
        if self.pc >= self.code.len() {
            return Err(VMError::InvalidBytecode("unexpected end of code".into()));
        }
        let b = self.code[self.pc];
        self.pc += 1;
        Ok(b)
    }

    /// Read next 2 bytes as big-endian u16 and advance PC by 2
    fn read_u16(&mut self) -> Result<u16, VMError> {
        if self.pc + 1 >= self.code.len() {
            return Err(VMError::InvalidBytecode("unexpected end of code".into()));
        }
        let hi = self.code[self.pc] as u16;
        let lo = self.code[self.pc + 1] as u16;
        self.pc += 2;
        Ok((hi << 8) | lo)
    }

    fn push(&mut self, v: Value) { self.stack.push(v); }
    fn pop(&mut self) -> Result<Value, VMError> {
        self.stack.pop().ok_or(VMError::StackUnderflow { opcode: OpCode::Halt })
    }
    fn pop_number(&mut self) -> Result<f64, VMError> {
        match self.stack.pop() {
            Some(Value::Number(n)) => Ok(n),
            Some(v) => Err(VMError::TypeError { expected: "Number", got: v }),
            None => Err(VMError::TypeError { expected: "Number", got: Value::Unit }),
        }
    }
    fn pop_bool(&mut self) -> Result<bool, VMError> {
        match self.stack.pop() {
            Some(Value::Bool(b)) => Ok(b),
            Some(Value::Number(n)) => Ok(n != 0.0),
            Some(v) => Err(VMError::TypeError { expected: "Bool", got: v }),
            None => Err(VMError::TypeError { expected: "Bool", got: Value::Unit }),
        }
    }

    pub fn run(&mut self) -> Result<Value, VMError> {
        loop {
            if self.pc >= self.code.len() {
                return Ok(self.stack.pop().unwrap_or(Value::Unit));
            }
            let op = self.fetch()?;
            self.execute(op)?;
        }
    }

    fn fetch(&mut self) -> Result<OpCode, VMError> {
        let byte = self.read_byte()?;
        OpCode::try_from(byte)
    }

    fn execute(&mut self, op: OpCode) -> Result<(), VMError> {
        match op {
            OpCode::Halt => return Ok(()),

            OpCode::Const(idx_hint) => {
                // idx_hint is ignored; read actual index
                let idx = self.read_u16()? as usize;
                let c = self.constants.get(idx)
                    .ok_or(VMError::ConstantBounds { index: idx, max: self.constants.len() })?
                    .clone();
                self.stack.push(c);
            }

            OpCode::Push => {
                // Push reads next u16 as constant index
                let idx = self.read_u16()? as usize;
                let c = self.constants.get(idx)
                    .ok_or(VMError::ConstantBounds { index: idx, max: self.constants.len() })?
                    .clone();
                self.stack.push(c);
            }

            OpCode::Load(idx_hint) => {
                let idx = self.read_byte()? as usize;
                let v = self.locals.get(idx)
                    .ok_or(VMError::LocalBounds { index: idx, max: self.locals.len() })?
                    .clone();
                self.stack.push(v);
            }

            OpCode::Store(idx_hint) => {
                let idx = self.read_byte()? as usize;
                let v = self.pop()?;
                // Auto-expand locals if needed
                while self.locals.len() <= idx {
                    self.locals.push(Value::Unit);
                }
                self.locals[idx] = v;
            }

            OpCode::Pop => {
                self.stack.pop();
            }

            OpCode::Dup => {
                if let Some(top) = self.stack.last() {
                    self.stack.push(top.clone());
                }
            }

            OpCode::Jmp(offset_hint) => {
                let offset = self.read_u16()? as i32;
                self.pc = ((self.pc as i32) + offset - 3) as usize;
            }

            OpCode::JmpIf(offset_hint) => {
                let offset = self.read_u16()? as i32;
                let cond = self.pop_bool()?;
                if cond {
                    self.pc = ((self.pc as i32) + offset - 3) as usize;
                }
            }

            OpCode::JmpIfNot(offset_hint) => {
                let offset = self.read_u16()? as i32;
                let cond = self.pop_bool()?;
                if !cond {
                    self.pc = ((self.pc as i32) + offset - 3) as usize;
                }
            }

            OpCode::Return => {
                let v = self.stack.pop().unwrap_or(Value::Unit);
                self.stack.clear();
                self.stack.push(v);
            }

            OpCode::Add => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Number(a + b));
            }
            OpCode::Sub => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Number(a - b));
            }
            OpCode::Mul => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Number(a * b));
            }
            OpCode::Div => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Number(a / b));
            }
            OpCode::Mod => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Number(a % b));
            }
            OpCode::Eq => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a == b));
            }
            OpCode::Ne => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a != b));
            }
            OpCode::Lt => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a < b));
            }
            OpCode::Gt => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a > b));
            }
            OpCode::Le => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a <= b));
            }
            OpCode::Ge => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a >= b));
            }
            OpCode::And => {
                let b = self.pop_bool()?;
                let a = self.pop_bool()?;
                self.stack.push(Value::Bool(a && b));
            }
            OpCode::Or => {
                let b = self.pop_bool()?;
                let a = self.pop_bool()?;
                self.stack.push(Value::Bool(a || b));
            }
            OpCode::Not => {
                let a = self.pop_bool()?;
                self.stack.push(Value::Bool(!a));
            }

            OpCode::GateOn => {
                // GateOn: pops id, sets gate on
                // For now: no-op or error
            }
            OpCode::GateOff => {}
            OpCode::GateToggle => {}
            OpCode::ReadTemp => {
                self.stack.push(Value::Number(22.0)); // mock
            }
            OpCode::ReadHumidity => {
                self.stack.push(Value::Number(50.0)); // mock
            }
            OpCode::ReadBool => {
                self.stack.push(Value::Bool(false)); // mock
            }
            OpCode::Actuate => {
                self.stack.pop(); self.stack.pop(); // pop value and id
            }
            OpCode::MakeClosure(_) => {
                self.stack.push(Value::Unit);
            }
            OpCode::Call(_, _) => {
                return Err(VMError::InvalidBytecode("Call not implemented in simple VM".into()));
            }
            OpCode::LoadArg(_) => {
                self.stack.push(Value::Unit);
            }
        }
        Ok(())
    }
}
