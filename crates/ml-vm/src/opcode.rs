//! Bytecode opcodes for the ML VM.
//!
//! Opcodes are serialized as variable-length byte sequences.
//! Single-byte opcodes: stack ops, arithmetic, logic
//! Multi-byte opcodes: CONST(idx16), Load(idx8), Store(idx8), Jmp(offset16), etc.

use serde::{Deserialize, Serialize};

/// Bytecode opcodes for the Memphis Language VM.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OpCode {
    // ── Stack ops ────────────────────────────────────────────────────────────
    /// Push — reads next 2 bytes as constant pool index
    Push,
    /// Pop — discards top of stack
    Pop,
    /// Dup — duplicates top of stack
    Dup,

    // ── Constants ────────────────────────────────────────────────────────────
    /// Const — reads next 2 bytes as constant pool index, pushes value
    Const(u16),

    // ── Locals ───────────────────────────────────────────────────────────────
    /// Load — reads next byte as local slot index, pushes value
    Load(u8),
    /// Store — reads next byte as local slot index, pops value into slot
    Store(u8),

    // ── Control flow ───────────────────────────────────────────────────────────
    /// Unconditional jump — reads next 2 bytes as PC offset
    Goto(u16),
    /// Conditional jump — reads next 2 bytes; jumps if top of stack is truthy
    IfGoto(u16),
    /// Call — reads arg_count and local_count bytes; pops function then args
    Call(u8, u8),
    /// Return from function
    Return,
    /// Halt execution
    Halt,

    // ── Arithmetic ────────────────────────────────────────────────────────────
    Add,
    Sub,
    Mul,
    Div,

    // ── Gate operations ────────────────────────────────────────────────────────
    /// Gate — reads next 2 bytes as constant-pool index for gate id string,
    /// then pops the target state from stack. Uses machine.set_gate().
    Gate,
    /// SensorRead — reads next 2 bytes as constant-pool index for sensor id,
    /// pushes numeric reading onto stack. Uses machine.read_sensor().
    SensorRead,
    /// Log — reads next 2 bytes as constant-pool index for message string,
    /// prints it. Uses machine.log().
    Log,

    // ── Native / special ──────────────────────────────────────────────────────
    /// MakeClosure — reads upvalue_count byte; creates a closure
    MakeClosure(u8),
}

impl OpCode {
    /// Encode this opcode as a byte vector.
    pub fn encode(self) -> Vec<u8> {
        match self {
            OpCode::Push        => vec![0x00],
            OpCode::Pop         => vec![0x01],
            OpCode::Dup         => vec![0x02],
            OpCode::Const(idx)  => vec![0x03, (idx >> 8) as u8, idx as u8],
            OpCode::Load(idx)   => vec![0x04, idx],
            OpCode::Store(idx)  => vec![0x05, idx],
            OpCode::Goto(off)   => vec![0x06, (off >> 8) as u8, off as u8],
            OpCode::IfGoto(off) => vec![0x07, (off >> 8) as u8, off as u8],
            OpCode::Call(ac, lc)=> vec![0x08, ac, lc],
            OpCode::Return      => vec![0x09],
            OpCode::Halt        => vec![0x0A],
            OpCode::Add         => vec![0x10],
            OpCode::Sub         => vec![0x11],
            OpCode::Mul         => vec![0x12],
            OpCode::Div         => vec![0x13],
            OpCode::Gate        => vec![0x20],
            OpCode::SensorRead  => vec![0x21],
            OpCode::Log         => vec![0x22],
            OpCode::MakeClosure(upvals) => vec![0x30, upvals],
        }
    }

    /// Approximate encoded size in bytes.
    pub fn size(self) -> usize {
        match self {
            OpCode::Const(_) | OpCode::Goto(_) | OpCode::IfGoto(_) => 3,
            OpCode::Load(_) | OpCode::Store(_) => 2,
            OpCode::Call(_, _) => 3,
            OpCode::MakeClosure(_) => 2,
            _ => 1,
        }
    }
}

impl TryFrom<u8> for OpCode {
    type Error = &'static str;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0x00 => Ok(OpCode::Push),
            0x01 => Ok(OpCode::Pop),
            0x02 => Ok(OpCode::Dup),
            0x03 => Ok(OpCode::Const(0)),     // placeholder; caller must read operand
            0x04 => Ok(OpCode::Load(0)),
            0x05 => Ok(OpCode::Store(0)),
            0x06 => Ok(OpCode::Goto(0)),
            0x07 => Ok(OpCode::IfGoto(0)),
            0x08 => Ok(OpCode::Call(0, 0)),
            0x09 => Ok(OpCode::Return),
            0x0A => Ok(OpCode::Halt),
            0x10 => Ok(OpCode::Add),
            0x11 => Ok(OpCode::Sub),
            0x12 => Ok(OpCode::Mul),
            0x13 => Ok(OpCode::Div),
            0x20 => Ok(OpCode::Gate),
            0x21 => Ok(OpCode::SensorRead),
            0x22 => Ok(OpCode::Log),
            0x30 => Ok(OpCode::MakeClosure(0)),
            _ => Err("unknown opcode"),
        }
    }
}

/// Disassemble a single instruction for debugging.
pub fn disassemble_op(code: &[u8], pc: usize) -> (String, usize) {
    if pc >= code.len() {
        return ("<end>".to_string(), pc);
    }
    let byte = code[pc];
    match OpCode::try_from(byte) {
        Ok(op) => {
            match op {
                OpCode::Const(_) | OpCode::Goto(_) | OpCode::IfGoto(_) => {
                    if pc + 2 < code.len() {
                        let hi = code[pc + 1] as u16;
                        let lo = code[pc + 2] as u16;
                        let idx = (hi << 8) | lo;
                        let next = pc + 3;
                        let disp = match op {
                            OpCode::Const(_) => format!("CONST ${:#x}", idx),
                            OpCode::Goto(_)  => format!("GOTO   {:04x}", idx),
                            OpCode::IfGoto(_) => format!("IF_GOTO {:04x}", idx),
                            _ => unreachable!(),
                        };
                        (disp, next)
                    } else {
                        ("<truncated>".to_string(), pc + 1)
                    }
                }
                OpCode::Load(idx) | OpCode::Store(idx) => {
                    (format!("{:?}{:#x}", op, idx), pc + 2)
                }
                OpCode::Call(ac, lc) => {
                    (format!("CALL   {} {}", ac, lc), pc + 3)
                }
                OpCode::MakeClosure(upvals) => {
                    (format!("MAKE_CLOSURE {}", upvals), pc + 2)
                }
                _ => (format!("{:?}", op), pc + 1),
            }
        }
        Err(_) => (format!("UNKNOWN {:#04x}", byte), pc + 1),
    }
}
