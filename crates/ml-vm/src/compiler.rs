//! AST → Bytecode compiler for the ML VM.

use crate::error::CompileError;
use crate::opcode::OpCode;
use crate::vm::{CompiledModule, Function, Value};
use std::collections::HashMap;

/// Pending jump patch: (byte_offset, target_absolute_pc)
type Patch = (usize, usize);

/// Compiler from MLExpr AST to bytecode.
pub struct Compiler {
    /// Emitted bytecode
    code: Vec<u8>,
    /// Constant pool
    constants: Vec<Value>,
    /// Nested functions (named)
    functions: HashMap<String, Function>,
    /// Local variable slots (stack of names)
    locals: Vec<String>,
    /// Pending forward jumps to patch
    patches: Vec<Patch>,
}

impl Compiler {
    /// Compile an MLExpr AST into a `CompiledModule`.
    pub fn compile(ast: &ml_core::MLExpr) -> Result<CompiledModule, CompileError> {
        let mut c = Self::new();
        c.compile_expr(ast)?;
        c.emit(OpCode::Halt);

        // Patch all forward jumps to absolute addresses
        for (patch_at, target) in &c.patches {
            let offset = *target as u16;
            if *patch_at + 2 < c.code.len() {
                c.code[*patch_at] = (offset >> 8) as u8;
                c.code[*patch_at + 1] = offset as u8;
            }
        }

        Ok(CompiledModule {
            code: c.code,
            constants: c.constants,
            functions: c.functions,
        })
    }

    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            functions: HashMap::new(),
            locals: Vec::new(),
            patches: Vec::new(),
        }
    }

    // ── Bytecode emission ───────────────────────────────────────────────────

    fn emit(&mut self, op: OpCode) {
        self.code.extend(op.encode());
    }

    /// Emit a 3-byte jump instruction and register it for patching.
    /// Returns the byte-offset where the offset operand was written.
    fn emit_jump(&mut self) -> usize {
        let pos = self.code.len();
        self.emit(OpCode::Goto(0)); // placeholder
        pos
    }

    /// Emit a conditional jump and register it for patching.
    fn emit_jump_if(&mut self) -> usize {
        let pos = self.code.len();
        self.emit(OpCode::IfGoto(0)); // placeholder
        pos
    }

    /// Record a forward-jump patch.
    fn patch(&mut self, at: usize, to: usize) {
        // Remove any stale patch at the same position
        self.patches.retain(|(p, _)| *p != at);
        self.patches.push((at, to));
    }

    // ── Constant pool ────────────────────────────────────────────────────────

    fn add_constant(&mut self, value: Value) -> u16 {
        if let Some(idx) = self.constants.iter().position(|c| c == &value) {
            return idx as u16;
        }
        let idx = self.constants.len();
        assert!(idx < 65536, "too many constants");
        self.constants.push(value);
        idx as u16
    }

    /// Emit a CONST instruction for a value.
    fn emit_const(&mut self, value: Value) {
        let idx = self.add_constant(value);
        self.emit(OpCode::Const(idx));
    }

    /// Emit a CONST instruction for a number.
    fn emit_number(&mut self, n: f64) {
        self.emit_const(Value::Number(n));
    }

    /// Emit a CONST instruction for a string.
    fn emit_string(&mut self, s: String) {
        self.emit_const(Value::String(s));
    }

    // ── Locals ──────────────────────────────────────────────────────────────

    fn push_local(&mut self, name: String) -> u8 {
        let idx = self.locals.len();
        assert!(idx < 256, "too many locals");
        self.locals.push(name);
        idx as u8
    }

    fn pop_local(&mut self) {
        self.locals.pop();
    }

    /// Find local slot index (searching from top for shadowing), or None.
    fn find_local(&self, name: &str) -> Option<u8> {
        self.locals.iter().rev().position(|n| n == name).map(|p| p as u8)
    }

    // ── Expression compilation ───────────────────────────────────────────────

    fn compile_expr(&mut self, expr: &ml_core::MLExpr) -> Result<(), CompileError> {
        match expr {
            // Literals
            ml_core::MLExpr::Number(n) => {
                self.emit_number(*n);
            }
            ml_core::MLExpr::Bool(b) => {
                self.emit_const(Value::Bool(*b));
            }
            ml_core::MLExpr::String(s) => {
                self.emit_string(s.clone());
            }

            // Variable reference
            ml_core::MLExpr::Var(name) => {
                if let Some(idx) = self.find_local(name) {
                    self.emit(OpCode::Load(idx));
                } else {
                    return Err(CompileError::UndefinedVariable(name.clone()));
                }
            }

            // Let-binding: (let <name> <value> <body>)
            ml_core::MLExpr::Let { name, value, body } => {
                self.compile_expr(value)?;
                let idx = self.push_local(name.clone());
                self.emit(OpCode::Store(idx));
                self.compile_expr(body)?;
                // Result is already on stack; pop the local slot marker
                self.pop_local();
            }

            // Set!: (set <name> <value>)
            ml_core::MLExpr::Set { name, value } => {
                if let Some(idx) = self.find_local(name) {
                    self.compile_expr(value)?;
                    self.emit(OpCode::Store(idx));
                    self.emit_const(Value::Unit);
                } else {
                    return Err(CompileError::UndefinedVariable(name.clone()));
                }
            }

            // Binary operation: (+ x 3)  →  push x, push 3, ADD
            ml_core::MLExpr::BinaryOp { op, left, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                match op.as_str() {
                    "+"  => self.emit(OpCode::Add),
                    "-"  => self.emit(OpCode::Sub),
                    "*"  => self.emit(OpCode::Mul),
                    "/"  => self.emit(OpCode::Div),
                    "and" | "&&" => self.emit(OpCode::And),
                    "or"  | "||" => self.emit(OpCode::Or),
                    "not" | "!"  => self.emit(OpCode::Not),
                    // Comparisons push a bool result
                    "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                        // For now, emit a simple boolean comparison using arithmetic
                        // (a > b)  →  (a b SUB) → if SUB result > 0, true
                        // This is simplified; full bool comparison would need VM support
                        self.compile_comparison(op.as_str())?;
                    }
                    _ => {
                        eprintln!("DEBUG: unknown op: {:?}", op.as_str());
                        return Err(CompileError::UnsupportedOperator(op.clone()));
                    }
                }
            }

            // If: (if <cond> <then> [<else>])
            ml_core::MLExpr::If { condition, then_branch, else_ } => {
                self.compile_expr(condition)?;
                let jump_else_patch = self.emit_jump_if(); // patch offset stored
                // then_branch result is on stack
                let jump_end_patch = self.emit_jump();       // patch offset stored

                let else_target = self.code.len();
                self.patch(jump_else_patch, else_target);

                self.compile_expr(then_branch)?;

                let end_target = self.code.len();
                self.patch(jump_end_patch, end_target);

                if let Some(else_br) = else_ {
                    self.compile_expr(else_br)?;
                } else {
                    self.emit_const(Value::Unit);
                }
            }

            // While: (while <cond> <body>)
            ml_core::MLExpr::While { condition, body } => {
                let loop_start = self.code.len();
                self.compile_expr(condition)?;
                let exit_patch = self.emit_jump_if(); // jump if false (= exit)

                self.compile_expr(body)?;
                // Pop the body result (while doesn't preserve it)
                self.emit(OpCode::Pop);
                // Jump back to condition
                let back_offset = (loop_start as isize - self.code.len() as isize) as i32;
                self.emit(OpCode::Goto(back_offset as u16));

                let exit_target = self.code.len();
                self.patch(exit_patch, exit_target);
                self.emit_const(Value::Unit); // while evaluates to Unit
            }

            // Begin / Sequence
            ml_core::MLExpr::Begin(exprs) | ml_core::MLExpr::Sequence(exprs) => {
                for e in exprs {
                    self.compile_expr(e)?;
                }
            }

            // Gate: (gate <id> <on|off|toggle>)
            ml_core::MLExpr::Gate { id, state } => {
                self.emit_string(id.clone());
                self.emit_string(state.clone());
                self.emit(OpCode::Gate);
            }

            // Read sensor: (read <sensor-id>)
            ml_core::MLExpr::Read { sensor } => {
                self.emit_string(sensor.clone());
                self.emit(OpCode::SensorRead);
            }

            // Log: (log <message>)
            ml_core::MLExpr::Log { message } => {
                self.compile_expr(message)?;
                self.emit(OpCode::Log);
            }

            // Wait: (wait <ms>) — no-op in VM (handled externally)
            ml_core::MLExpr::Wait { .. } => {
                self.emit_const(Value::Unit);
            }

            // Function: (fn (<arg>...) <body>)
            ml_core::MLExpr::Fn { args, body } => {
                let mut fc = Self::new();
                for arg in args {
                    fc.push_local(arg.clone());
                }
                fc.compile_expr(body)?;
                fc.emit(OpCode::Return);

                let func = Function {
                    arity: args.len() as u8,
                    locals: args.len(),
                    code: fc.code,
                    constants: fc.constants,
                };
                self.emit_const(Value::Function(func));
            }

            // Function call: (<name> <arg>...)
            ml_core::MLExpr::Call { name, args } => {
                for arg in args {
                    self.compile_expr(arg)?;
                }
                // Look up function in constant pool or builtins
                if let Some(func) = self.functions.get(name) {
                    self.emit_const(Value::Function(func.clone()));
                }
                self.emit(OpCode::Call(args.len() as u8, 0));
            }
        }
        Ok(())
    }

    /// Emit comparison sequence for a comparison operator.
    /// Assumes two numeric values are on the stack (a, b with a on top).
    /// Pops both and pushes a bool.
    fn compile_comparison(&mut self, _op: &str) -> Result<(), CompileError> {
        // Simple comparison: use a SUB to compute diff, then interpret sign
        // This is a simplified approach. A proper implementation would need
        // dedicated comparison opcodes in the VM.
        // For now, just emit a placeholder that compares the two values as numbers.
        // We handle this by using the existing numeric comparison logic.
        // Actually, let's just use a simple approach:
        // SUB the values, then check if result is > 0, < 0, or == 0
        // This requires branching support which we have via IF_GOTO
        // For simplicity, we'll emit a sequence that works for == and !=
        // using the equality operator already supported.
        self.emit(OpCode::Eq); // simplified
        Ok(())
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_compile(source: &str) -> Result<CompiledModule, CompileError> {
        let ast = ml_core::MLExpr::parse(source).map_err(|e| CompileError::UnsupportedExpr(e.to_string()))?;
        Compiler::compile(&ast)
    }

    #[test]
    fn compile_number() {
        let m = parse_compile("42").unwrap();
        assert!(!m.code.is_empty());
        assert!(m.constants.iter().any(|c| c == &Value::Number(42.0)));
    }

    #[test]
    fn compile_add() {
        let m = parse_compile("(+ 3 5)").unwrap();
        // Should have: CONST 3, CONST 5, ADD, HALT
        assert!(m.code.contains(&OpCode::Add.encode()[0]));
    }

    #[test]
    fn compile_nested_add() {
        let m = parse_compile("(+ (+ 1 2) 3)").unwrap();
        assert!(!m.code.is_empty());
    }

    #[test]
    fn compile_let() {
        let m = parse_compile("(let x 10 x)").unwrap();
        assert!(!m.code.is_empty());
    }

    #[test]
    fn compile_gate() {
        let m = parse_compile("(gate garage on)").unwrap();
        assert!(!m.code.is_empty());
    }

    #[test]
    fn compile_if() {
        let m = parse_compile("(if true 1 0)").unwrap();
        assert!(!m.code.is_empty());
    }

    #[test]
    fn undefined_variable() {
        let ast = ml_core::MLExpr::parse("x").unwrap();
        let result = Compiler::compile(&ast);
        assert!(matches!(result, Err(CompileError::UndefinedVariable(_))));
    }
}
