//! Simple Bytecode VM for Memphis Language (ML)
//!
//! PC Management (SIMPLE model):
//! - fetch(): reads byte at pc, advances pc by 1
//! - execute(): reads operands, advances pc PAST the instruction
//! - For Const/Load/Store: pc advances past opcode (1) + operand (2) = 3 bytes total
//! - For Jmp/JmpIf/JmpIfNot: pc advances past opcode (1) + offset (2) = 3 bytes total
//!   Then sets pc = offset (absolute target)
//! - For Halt: sets pc = code.len() (forces exit on next fetch)
//! - Single-byte opcodes: fetch() already advanced pc; execute() does nothing else

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
            locals: vec![Value::Unit; 32],
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
    fn pop_number(&self) -> Result<f64, VMError> {
        match self.stack.last() {
            Some(Value::Number(n)) => Ok(*n),
            Some(Value::Bool(b)) => Ok(if *b { 1.0 } else { 0.0 }),
            _ => Err(VMError::TypeError { expected: "Number", got: Value::Unit }),
        }
    }
    fn pop_bool(&self) -> Result<bool, VMError> {
        match self.stack.last() {
            Some(Value::Bool(b)) => Ok(*b),
            Some(Value::Number(n)) => Ok(*n != 0.0),
            _ => Err(VMError::TypeError { expected: "Bool", got: Value::Unit }),
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
            OpCode::Halt => {
                // Set pc past end to force exit on next iteration
                self.pc = self.code.len() + 1;
                return Ok(());
            }

            OpCode::Const(_) => {
                // Already advanced past opcode (1 byte), now read index (2 bytes)
                let idx = self.read_u16()? as usize;
                let c = self.constants.get(idx)
                    .ok_or(VMError::ConstantBounds { index: idx, max: self.constants.len() })?
                    .clone();
                self.stack.push(c);
                // pc is now at next instruction (after 1 opcode + 2 index)
            }

            OpCode::Push => {
                let idx = self.read_u16()? as usize;
                let c = self.constants.get(idx)
                    .ok_or(VMError::ConstantBounds { index: idx, max: self.constants.len() })?
                    .clone();
                self.stack.push(c);
            }

            OpCode::Load(_) => {
                let idx = self.read_byte()? as usize;
                let v = self.locals.get(idx)
                    .ok_or(VMError::LocalBounds { index: idx, max: self.locals.len() })?
                    .clone();
                self.stack.push(v);
            }

            OpCode::Store(_) => {
                let idx = self.read_byte()? as usize;
                let v = self.pop()?;
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

            OpCode::Jmp(_) => {
                // pc already advanced past opcode (1 byte)
                // Read offset (2 bytes) — this positions pc at end of instruction
                let _offset = self.read_u16()?;
                // _offset is the absolute target byte position
                // No adjustment needed — just set pc = _offset
                // But wait: we need to add back the 2 bytes we just "read past"
                // Actually: pc is at position AFTER the 2 offset bytes
                // We want pc to be at the offset value itself
                // Since we already advanced pc by 2 (read_u16), we need:
                // pc = _offset (the target position in the bytecode)
                // But pc is currently at offset_end = patch_offset + 3
                // So: new_pc = _offset (absolute)
                // But we need: new_pc = _offset + (pc - (offset_end))
                // = _offset + (current_pc - (patch_offset + 3))
                // This is getting complicated...
                
                // SIMPLE APPROACH: the bytecode encodes RELATIVE offset from current pc
                // offset = target - (pc after instruction)
                // new_pc = pc + offset = pc + (target - pc) = target ✓
                //
                // The bytecode encodes: offset = target - (patch_offset + 3)
                // VM: pc += offset (where offset is read from bytecode)
                // new_pc = (patch_offset + 3) + offset = patch_offset + 3 + (target - patch_offset - 3) = target ✓
                //
                // Current: pc is at patch_offset + 3 (after reading 3 bytes)
                // offset = bytecode_value (relative from pc after instruction)
                // new_pc = pc + offset
                self.pc = ((self.pc as i32) + (_offset as i32)) as usize;
            }

            OpCode::JmpIf(_) => {
                let _offset = self.read_u16()?;
                let cond = self.pop_bool()?;
                if cond {
                    // Same as Jmp: pc += offset
                    self.pc = ((self.pc as i32) + (_offset as i32)) as usize;
                }
            }

            OpCode::JmpIfNot(_) => {
                let _offset = self.read_u16()?;
                let cond = self.pop_bool()?;
                if !cond {
                    self.pc = ((self.pc as i32) + (_offset as i32)) as usize;
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

            OpCode::GateOn | OpCode::GateOff | OpCode::GateToggle => {}
            OpCode::ReadTemp => { self.stack.push(Value::Number(22.0)); }
            OpCode::ReadHumidity => { self.stack.push(Value::Number(50.0)); }
            OpCode::ReadBool => { self.stack.push(Value::Bool(false)); }
            OpCode::Actuate => { self.stack.pop(); self.stack.pop(); }
            OpCode::MakeClosure(_) => { self.stack.push(Value::Unit); }
            OpCode::Call(_, _) => {
                return Err(VMError::InvalidBytecode("Call not implemented".into()));
            }
            OpCode::LoadArg(_) => { self.stack.push(Value::Unit); }
        }
        Ok(())
    }
}
