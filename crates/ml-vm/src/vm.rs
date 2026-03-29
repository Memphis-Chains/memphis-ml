//! Simple Bytecode VM for Memphis Language (ML)
//!
//! Features:
//! - Stack overflow protection (MAX_STACK_DEPTH)
//! - Tail call optimization (reusable call frames)
//! - Rich error messages with stack traces
//! - Call-frame-based execution model

use crate::{VMError, OpCode, Value, Closure};

/// Maximum depth of the value stack. Prevents stack overflow attacks.
pub const MAX_STACK_DEPTH: usize = 1024;

/// Maximum number of nested function calls.
pub const MAX_CALL_FRAMES: usize = 256;

/// A single call frame — analogous to a stack frame in native code.
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Program counter within this frame's bytecode.
    pub pc: usize,
    /// Local variables for this function.
    locals: Vec<Value>,
    /// Bytecode being executed in this frame.
    code: Vec<u8>,
    /// Constant pool for this function.
    constants: Vec<Value>,
    /// Function name, for stack traces.
    pub name: String,
    /// Source location (line number) of the current instruction.
    pub line: u32,
}

impl CallFrame {
    fn new(code: Vec<u8>, constants: Vec<Value>, name: String, locals_count: usize) -> Self {
        Self {
            pc: 0,
            locals: vec![Value::Unit; locals_count.max(1)],
            code,
            constants,
            name,
            line: 1,
        }
    }
}

/// A call frame entry on the VM's call stack (used for stack traces).
#[derive(Debug, Clone)]
pub struct FrameEntry {
    pub name: String,
    pub pc: usize,
    pub line: u32,
}

// ---------------------------------------------------------------------------
// VM State
// ---------------------------------------------------------------------------

pub struct VM {
    /// The active call frame.
    frame: CallFrame,
    /// Call-frame stack for stack traces (populated on function call).
    call_stack: Vec<FrameEntry>,
    /// Value stack (operand stack).
    stack: Vec<Value>,
    /// Shared constants (top-level module constants).
    constants: Vec<Value>,
}

impl VM {
    pub fn new(code: Vec<u8>, constants: Vec<Value>) -> Self {
        let frame = CallFrame::new(code, constants.clone(), "<main>".to_string(), 32);
        Self {
            frame,
            call_stack: Vec::new(),
            stack: Vec::with_capacity(MAX_STACK_DEPTH),
            constants,
        }
    }

    /// Create a VM from a compiled module.
    pub fn with_module(module: &crate::CompiledModule) -> Self {
        let frame = CallFrame::new(
            module.code.clone(),
            module.constants.clone(),
            "<main>".to_string(),
            32,
        );
        Self {
            frame,
            call_stack: Vec::new(),
            stack: Vec::with_capacity(MAX_STACK_DEPTH),
            constants: module.constants.clone(),
        }
    }

    // ── Stack operations ───────────────────────────────────────────────────

    fn check_stack_overflow(&self) -> Result<(), VMError> {
        if self.stack.len() >= MAX_STACK_DEPTH {
            return Err(VMError::StackOverflow {
                depth: self.stack.len(),
                limit: MAX_STACK_DEPTH,
                trace: self.build_trace(),
            });
        }
        Ok(())
    }

    fn push(&mut self, v: Value) -> Result<(), VMError> {
        self.check_stack_overflow()?;
        self.stack.push(v);
        Ok(())
    }

    fn pop(&mut self) -> Result<Value, VMError> {
        self.stack
            .pop()
            .ok_or_else(|| VMError::StackUnderflow {
                opcode: "pop".to_string(),
                trace: self.build_trace(),
            })
    }

    fn pop_number(&self) -> Result<f64, VMError> {
        match self.stack.last() {
            Some(Value::Number(n)) => Ok(*n),
            Some(v) => Err(VMError::TypeError {
                expected: "Number",
                got: v.clone(),
                trace: self.build_trace(),
            }),
            None => Err(VMError::StackUnderflow {
                opcode: "pop_number".to_string(),
                trace: self.build_trace(),
            }),
        }
    }

    fn pop_bool(&self) -> Result<bool, VMError> {
        match self.stack.last() {
            Some(Value::Bool(b)) => Ok(*b),
            Some(Value::Number(n)) => Ok(*n != 0.0),
            Some(v) => Err(VMError::TypeError {
                expected: "Bool",
                got: v.clone(),
                trace: self.build_trace(),
            }),
            None => Err(VMError::StackUnderflow {
                opcode: "pop_bool".to_string(),
                trace: self.build_trace(),
            }),
        }
    }

    // ── Bytecode reading ───────────────────────────────────────────────────

    fn read_byte(&mut self) -> Result<u8, VMError> {
        if self.frame.pc >= self.frame.code.len() {
            return Err(self.frame_error("unexpected end of bytecode"));
        }
        let b = self.frame.code[self.frame.pc];
        self.frame.pc += 1;
        Ok(b)
    }

    fn read_u16(&mut self) -> Result<u16, VMError> {
        if self.frame.pc + 1 >= self.frame.code.len() {
            return Err(self.frame_error("unexpected end of bytecode reading u16"));
        }
        let hi = self.frame.code[self.frame.pc] as u16;
        let lo = self.frame.code[self.frame.pc + 1] as u16;
        self.frame.pc += 2;
        Ok((hi << 8) | lo)
    }

    // ── Error helpers ───────────────────────────────────────────────────────

    fn frame_error(&self, msg: impl Into<String>) -> VMError {
        VMError::Runtime {
            message: msg.into(),
            frame: FrameEntry {
                name: self.frame.name.clone(),
                pc: self.frame.pc.saturating_sub(1),
                line: self.frame.line,
            },
            trace: self.build_trace(),
        }
    }

    /// Build a stack trace from the call stack.
    fn build_trace(&self) -> Vec<FrameEntry> {
        let mut trace = vec![FrameEntry {
            name: self.frame.name.clone(),
            pc: self.frame.pc,
            line: self.frame.line,
        }];
        trace.extend(self.call_stack.iter().cloned());
        trace
    }

    // ── Main execution loop ────────────────────────────────────────────────

    pub fn run(&mut self) -> Result<Value, VMError> {
        loop {
            if self.frame.pc >= self.frame.code.len() {
                // Normal exit — return top of stack (or Unit if empty)
                return Ok(self.stack.pop().unwrap_or(Value::Unit));
            }

            // Update line number (bytecode stores line at start of each basic block)
            self.update_line();

            let op = self.fetch()?;
            self.execute(op)?;
        }
    }

    /// Try to read a line number from bytecode.
    /// Line info is stored as a sentinel prefix: 0xFE <u16 line> before an instruction.
    fn update_line(&mut self) {
        // Simple approach: scan backward for the last line marker
        // For performance, in production you'd store line per instruction
        if self.frame.pc > 0 && self.frame.pc < self.frame.code.len() {
            let byte = self.frame.code[self.frame.pc];
            if byte == 0xFE && self.frame.pc + 2 < self.frame.code.len() {
                let line = ((self.frame.code[self.frame.pc + 1] as u16) << 8)
                    | (self.frame.code[self.frame.pc + 2] as u16);
                self.frame.line = line as u32;
            }
        }
    }

    fn fetch(&mut self) -> Result<OpCode, VMError> {
        let byte = self.read_byte()?;
        OpCode::try_from(byte)
    }

    // ── Tail Call Optimization ─────────────────────────────────────────────
    //
    // Tail call = a function call immediately followed by Return.
    // Instead of pushing a new frame, we reuse the current frame:
    // 1. Replace this frame's code/constants/locals with the called function's
    // 2. Save the return address in the call_stack for trace purposes
    // 3. Jump to the called function's entry
    //
    // The compiler emits `TailCall(arity, local_count)` when it detects tail position.

    fn do_call(&mut self, arity: u8, local_count: u8) -> Result<(), VMError> {
        if self.call_stack.len() >= MAX_CALL_FRAMES {
            return Err(VMError::CallStackOverflow {
                depth: self.call_stack.len(),
                limit: MAX_CALL_FRAMES,
                trace: self.build_trace(),
            });
        }

        // Pop the closure + args from the value stack
        // Stack layout: [closure, arg0, arg1, ..., argN]
        // We need to collect args, then look up the function
        if self.stack.len() < (arity as usize) + 1 {
            return Err(VMError::ArityMismatch {
                arg_count: self.stack.len().saturating_sub(1),
                arity: arity as usize,
                trace: self.build_trace(),
            });
        }

        let func = match self.stack[self.stack.len() - (arity as usize) - 1].clone() {
            Value::Function(f) => f,
            Value::Closure(box Closure { func, .. }) => func,
            v => {
                return Err(VMError::TypeError {
                    expected: "Function",
                    got: v,
                    trace: self.build_trace(),
                })
            }
        };

        // Verify arity
        if func.arity != arity {
            return Err(VMError::ArityMismatch {
                arg_count: arity as usize,
                arity: func.arity as usize,
                trace: self.build_trace(),
            });
        }

        // Pop args and closure into a new frame's locals
        let total_locals = (arity as usize).max(func.locals).max(1);
        let mut new_locals = vec![Value::Unit; total_locals];
        for i in 0..arity as usize {
            new_locals[i] = self.stack.pop().unwrap_or(Value::Unit);
        }
        // Pop the closure
        self.stack.pop();

        // Push current frame onto call stack for trace
        self.call_stack.push(FrameEntry {
            name: self.frame.name.clone(),
            pc: self.frame.pc,
            line: self.frame.line,
        });

        // Replace current frame with the new one (tail-call reuse)
        self.frame = CallFrame::new(func.code, func.constants, format!("<fn>"), total_locals);
        self.frame.locals[..new_locals.len()].copy_from_slice(&new_locals);
        self.frame.pc = 0;

        Ok(())
    }

    /// Tail-call variant: reuses the current frame without growing call_stack.
    /// The stack must have exactly: [closure, arg0, ..., argN]
    fn do_tail_call(&mut self, arity: u8) -> Result<(), VMError> {
        if self.stack.len() < (arity as usize) + 1 {
            return Err(VMError::ArityMismatch {
                arg_count: self.stack.len().saturating_sub(1),
                arity: arity as usize,
                trace: self.build_trace(),
            });
        }

        let func = match self.stack[self.stack.len() - (arity as usize) - 1].clone() {
            Value::Function(f) => f,
            Value::Closure(box Closure { func, .. }) => func,
            v => {
                return Err(VMError::TypeError {
                    expected: "Function",
                    got: v,
                    trace: self.build_trace(),
                })
            }
        };

        if func.arity != arity {
            return Err(VMError::ArityMismatch {
                arg_count: arity as usize,
                arity: func.arity as usize,
                trace: self.build_trace(),
            });
        }

        // Collect args
        let mut args = Vec::with_capacity(arity as usize);
        for _ in 0..arity as usize {
            args.push(self.stack.pop().unwrap_or(Value::Unit));
        }
        self.stack.pop(); // pop closure

        // Reuse frame: just replace code, constants, and reset pc
        self.frame.code = func.code;
        self.frame.constants = func.constants;
        self.frame.pc = 0;
        self.frame.line = 1;

        // Resize locals and fill args
        let total = args.len().max(func.locals).max(1);
        self.frame.locals.resize(total, Value::Unit);
        for (i, arg) in args.into_iter().enumerate() {
            self.frame.locals[i] = arg;
        }

        Ok(())
    }

    /// Return from current frame. If call_stack is non-empty, pop back;
    /// otherwise, exit VM (leave result on value stack).
    fn do_return(&mut self) -> Result<(), VMError> {
        let result = self.stack.pop().unwrap_or(Value::Unit);

        if let Some(caller) = self.call_stack.pop() {
            // Pop back to caller's frame
            // We reconstruct the frame from the call_stack entry
            // For simplicity, we save result and swap — in a full impl we'd store frame data
            // For now, signal return by setting pc past end and restoring basic state
            self.frame.pc = self.frame.code.len(); // force exit of current frame
            self.push(result)?;
        } else {
            // No caller — normal exit. Leave result on stack.
            self.frame.pc = self.frame.code.len();
            self.stack.push(result);
        }

        Ok(())
    }

    // ── Instruction execution ─────────────────────────────────────────────

    fn execute(&mut self, op: OpCode) -> Result<(), VMError> {
        match op {
            OpCode::Halt => {
                self.frame.pc = self.frame.code.len();
                return Ok(());
            }

            OpCode::Const(idx) => {
                let idx = idx as usize;
                let c = self.frame.constants.get(idx).cloned().ok_or({
                    VMError::ConstantBounds {
                        index: idx,
                        max: self.frame.constants.len(),
                        trace: self.build_trace(),
                    }
                })?;
                self.push(c)?;
            }

            OpCode::Push => {
                let idx = self.read_u16()? as usize;
                let c = self.frame.constants.get(idx).cloned().ok_or({
                    VMError::ConstantBounds {
                        index: idx,
                        max: self.frame.constants.len(),
                        trace: self.build_trace(),
                    }
                })?;
                self.push(c)?;
            }

            OpCode::Load(idx) => {
                let idx = idx as usize;
                let v = self.frame.locals.get(idx).cloned().unwrap_or(Value::Unit);
                self.push(v)?;
            }

            OpCode::Store(idx) => {
                let idx = idx as usize;
                let v = self.pop()?;
                if idx >= self.frame.locals.len() {
                    self.frame.locals.resize(idx + 1, Value::Unit);
                }
                self.frame.locals[idx] = v;
            }

            OpCode::Pop => {
                self.stack.pop();
            }

            OpCode::Dup => {
                if let Some(top) = self.stack.last().cloned() {
                    self.push(top)?;
                }
            }

            OpCode::Jmp(offset) => {
                // offset is relative from the PC after reading it (patch_offset + 3)
                let target = ((self.frame.pc as i32) + (offset as i32)) as usize;
                self.frame.pc = target;
            }

            OpCode::JmpIf(offset) => {
                let offset = offset as i32;
                let cond = self.pop_bool()?;
                if cond {
                    self.frame.pc = ((self.frame.pc as i32) + offset) as usize;
                }
            }

            OpCode::JmpIfNot(offset) => {
                let offset = offset as i32;
                let cond = self.pop_bool()?;
                if !cond {
                    self.frame.pc = ((self.frame.pc as i32) + offset) as usize;
                }
            }

            OpCode::Return => {
                self.do_return()?;
            }

            OpCode::Call(arg_count, _) => {
                self.do_call(arg_count, 0)?;
            }

            OpCode::TailCall(arity) => {
                self.do_tail_call(arity)?;
            }

            OpCode::Add => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.push(Value::Number(a + b))?;
            }
            OpCode::Sub => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.push(Value::Number(a - b))?;
            }
            OpCode::Mul => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.push(Value::Number(a * b))?;
            }
            OpCode::Div => {
                let b = self.pop_number()?;
                if b == 0.0 {
                    return Err(VMError::DivisionByZero {
                        trace: self.build_trace(),
                    });
                }
                let a = self.pop_number()?;
                self.push(Value::Number(a / b))?;
            }
            OpCode::Mod => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.push(Value::Number(a % b))?;
            }
            OpCode::Eq => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(Value::Bool(a == b))?;
            }
            OpCode::Ne => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(Value::Bool(a != b))?;
            }
            OpCode::Lt => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.push(Value::Bool(a < b))?;
            }
            OpCode::Gt => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.push(Value::Bool(a > b))?;
            }
            OpCode::Le => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.push(Value::Bool(a <= b))?;
            }
            OpCode::Ge => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.push(Value::Bool(a >= b))?;
            }
            OpCode::And => {
                let b = self.pop_bool()?;
                let a = self.pop_bool()?;
                self.push(Value::Bool(a && b))?;
            }
            OpCode::Or => {
                let b = self.pop_bool()?;
                let a = self.pop_bool()?;
                self.push(Value::Bool(a || b))?;
            }
            OpCode::Not => {
                let a = self.pop_bool()?;
                self.push(Value::Bool(!a))?;
            }

            OpCode::GateOn | OpCode::GateOff | OpCode::GateToggle => {}
            OpCode::ReadTemp => self.push(Value::Number(22.0))?,
            OpCode::ReadHumidity => self.push(Value::Number(50.0))?,
            OpCode::ReadBool => self.push(Value::Bool(false))?,
            OpCode::Actuate => {
                self.stack.pop();
                self.stack.pop();
            }
            OpCode::MakeClosure(upval_count) => {
                // Simplified: emit a unit placeholder. Full impl would capture upvalues.
                let _ = upval_count;
                self.push(Value::Unit)?;
            }
            OpCode::LoadArg(idx) => {
                let v = self.frame.locals.get(idx as usize).cloned().unwrap_or(Value::Unit);
                self.push(v)?;
            }
        }
        Ok(())
    }
}

// Note: Closure and Function types are defined in lib.rs and imported via crate::

impl VM {
    /// Get a reference to the current call stack for debugging.
    pub fn call_stack_trace(&self) -> Vec<FrameEntry> {
        self.build_trace()
    }

    /// Get current stack depth.
    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }
}
