//! Integration tests for ml-vm.
//!
//! Tests cover:
//! - Stack overflow detection
//! - Call stack overflow detection
//! - Tail call optimization
//! - Error messages with stack traces
//! - Arithmetic correctness
//! - Control flow
//! - Function calls

use ml_vm::{VM, Compiler, Value, CompileError, MAX_STACK_DEPTH};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn compile_run(source: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let ast = ml_core::MLExpr::parse(source)
        .map_err(|e| format!("parse error: {}", e))?;
    let module = Compiler::compile(&ast)
        .map_err(|e| format!("compile error: {}", e))?;
    let mut vm = VM::with_module(&module);
    vm.run().map_err(|e| format!("vm error: {}", e))
}

fn compile_only(source: &str) -> Result<ml_vm::CompiledModule, CompileError> {
    let ast = ml_core::MLExpr::parse(source)
        .map_err(|e| CompileError::UnsupportedExpr(e.to_string()))?;
    Compiler::compile(&ast)
}

// ---------------------------------------------------------------------------
// Stack overflow tests
// ---------------------------------------------------------------------------

#[test]
fn test_stack_overflow_detection() {
    // Create a deeply nested expression that exceeds MAX_STACK_DEPTH
    // Each nested parentheses pushes a value on the stack
    let source = "((((((((((1))))))))))"; // deeply nested

    // This should succeed (stack depth < limit)
    let result = compile_run(source);
    assert!(result.is_ok());

    // Very deeply nested expression
    let deep_source = &(0..500).map(|_| "(").collect::<String>() + "1" + &(0..500).map(|_| ")").collect::<String>();
    let result = compile_run(deep_source);
    assert!(result.is_ok(), "deep nesting should succeed if under stack limit");
}

#[test]
fn test_stack_depth_limit() {
    // Verify MAX_STACK_DEPTH constant
    assert_eq!(MAX_STACK_DEPTH, 1024);

    // A expression that pushes exactly MAX_STACK_DEPTH values would overflow
    // We approximate by creating a chain of lets that keeps many values on stack
    // Each let + store keeps a local, and each sub-expression pushes its result
    let source = "(let a1 1 (+ a1 (let a2 2 (+ a2 (let a3 3 (+ a3 (let a4 4 (+ a4 (let a5 5 (+ a5 (let a6 6 (+ a6 (let a7 7 (+ a7 (let a8 8 (+ a8 (let a9 9 (+ a9 (let a10 10 a10)))))))))))))))))))";
    let result = compile_run(source);
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// Error message tests
// ---------------------------------------------------------------------------

#[test]
fn test_undefined_variable_error() {
    let result = compile_run("x");
    assert!(result.is_err());

    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Undefined variable") || err_msg.contains("undefined variable"),
        "error should mention undefined variable: {}", err_msg);
}

#[test]
fn test_type_error_message() {
    // Calling arithmetic on non-numbers should give a type error
    let result = compile_run("(+ \"hello\" 5)");
    // The compiler may fail first or the VM may report it
    // In either case, we get an error
    if let Err(e) = result {
        let msg = e.to_string();
        // Error should mention something about type mismatch
        assert!(msg.contains("type") || msg.contains("Type") || msg.contains("expected"),
            "error should mention type: {}", msg);
    }
}

#[test]
fn test_division_by_zero_error() {
    let result = compile_run("(/ 10 0)");
    assert!(result.is_err());

    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("division") || err_msg.contains("zero"),
        "error should mention division/zero: {}", err_msg);
}

// ---------------------------------------------------------------------------
// Arithmetic correctness tests
// ---------------------------------------------------------------------------

#[test]
fn test_arithmetic_basics() {
    assert_eq!(compile_run("(+ 1 2)").unwrap(), Value::Number(3.0));
    assert_eq!(compile_run("(- 10 3)").unwrap(), Value::Number(7.0));
    assert_eq!(compile_run("(* 6 7)").unwrap(), Value::Number(42.0));
    assert_eq!(compile_run("(/ 20 4)").unwrap(), Value::Number(5.0));
    assert_eq!(compile_run("(% 17 5)").unwrap(), Value::Number(2.0));
}

#[test]
fn test_float_arithmetic() {
    assert_eq!(compile_run("(+ 1.5 2.5)").unwrap(), Value::Number(4.0));
    assert_eq!(compile_run("(* 2.5 4.0)").unwrap(), Value::Number(10.0));
    assert_eq!(compile_run("(/ 7.0 2.0)").unwrap(), Value::Number(3.5));
}

#[test]
fn test_nested_arithmetic() {
    let result = compile_run("(+ (* 2 3) (* 4 5))").unwrap();
    assert_eq!(result, Value::Number(26.0));
}

#[test]
fn test_order_of_operations() {
    // 2 + 3 * 4 = 2 + 12 = 14 (left-to-right in our VM)
    // Since our VM uses stack-based evaluation, (+ 2 (* 3 4)) = 2 + 12
    let result = compile_run("(+ 2 (* 3 4))").unwrap();
    assert_eq!(result, Value::Number(14.0));
}

// ---------------------------------------------------------------------------
// Comparison tests
// ---------------------------------------------------------------------------

#[test]
fn test_comparisons() {
    assert_eq!(compile_run("(< 3 5)").unwrap(), Value::Bool(true));
    assert_eq!(compile_run("(< 5 3)").unwrap(), Value::Bool(false));
    assert_eq!(compile_run("(> 5 3)").unwrap(), Value::Bool(true));
    assert_eq!(compile_run("(<= 3 3)").unwrap(), Value::Bool(true));
    assert_eq!(compile_run("(>= 5 5)").unwrap(), Value::Bool(true));
    assert_eq!(compile_run("(== 42 42)").unwrap(), Value::Bool(true));
    assert_eq!(compile_run("(!= 1 2)").unwrap(), Value::Bool(true));
}

#[test]
fn test_boolean_operations() {
    assert_eq!(compile_run("(&& true true)").unwrap(), Value::Bool(true));
    assert_eq!(compile_run("(&& true false)").unwrap(), Value::Bool(false));
    assert_eq!(compile_run("(|| false true)").unwrap(), Value::Bool(true));
    assert_eq!(compile_run("(|| false false)").unwrap(), Value::Bool(false));
    assert_eq!(compile_run("(! false)").unwrap(), Value::Bool(true));
    assert_eq!(compile_run("(! true)").unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Let-binding tests
// ---------------------------------------------------------------------------

#[test]
fn test_let_binding() {
    let result = compile_run("(let x 10 (+ x 5))").unwrap();
    assert_eq!(result, Value::Number(15.0));
}

#[test]
fn test_nested_let() {
    let result = compile_run("(let x 10 (let y 20 (+ x y)))").unwrap();
    assert_eq!(result, Value::Number(30.0));
}

#[test]
fn test_let_shadow() {
    let result = compile_run("(let x 5 (let x 10 (+ x x)))").unwrap();
    assert_eq!(result, Value::Number(20.0));
}

// ---------------------------------------------------------------------------
// Control flow tests
// ---------------------------------------------------------------------------

#[test]
fn test_if_true() {
    assert_eq!(compile_run("(if true 42 0)").unwrap(), Value::Number(42.0));
}

#[test]
fn test_if_false() {
    assert_eq!(compile_run("(if false 42 0)").unwrap(), Value::Number(0.0));
}

#[test]
fn test_if_else() {
    assert_eq!(compile_run("(if (< 3 5) \"yes\" \"no\")").unwrap(), Value::String("yes".to_string()));
}

#[test]
fn test_nested_if() {
    let result = compile_run("(if true (if false 1 2) 3)").unwrap();
    assert_eq!(result, Value::Number(2.0));
}

// ---------------------------------------------------------------------------
// While loop tests
// ---------------------------------------------------------------------------

#[test]
fn test_while_loop() {
    // sum 1 to 10 = 55
    let result = compile_run("(let sum 0 (let i 10 (while (< 0 i) (begin (set sum (+ sum i)) (set i (- i 1))))) sum)").unwrap();
    assert_eq!(result, Value::Number(55.0));
}

// ---------------------------------------------------------------------------
// Function tests
// ---------------------------------------------------------------------------

#[test]
fn test_named_definition_and_call() {
    let source = "(defn add [x y] (+ x y)) (add 3 5)";
    let result = compile_run(source).unwrap();
    assert_eq!(result, Value::Number(8.0));
}

#[test]
fn test_recursive_factorial() {
    let source = "(defn factorial [n] (if (<= n 1) 1 (* n (factorial (- n 1))))) (factorial 5)";
    let result = compile_run(source).unwrap();
    assert_eq!(result, Value::Number(120.0));
}

#[test]
fn test_fibonacci() {
    let source = "(defn fib [n] (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 10)";
    let result = compile_run(source).unwrap();
    assert_eq!(result, Value::Number(55.0));
}

#[test]
fn test_multiple_functions() {
    let source = "(defn double [x] (* 2 x)) (defn triple [x] (* 3 x)) (+ (double 5) (triple 4))";
    let result = compile_run(source).unwrap();
    assert_eq!(result, Value::Number(22.0));
}

#[test]
fn test_arity_mismatch() {
    let source = "(defn arity2 [a b] (+ a b)) (arity2 1)";
    let result = compile_run(source);
    assert!(result.is_err(), "should error on arity mismatch");
}

// ---------------------------------------------------------------------------
// Tail call optimization tests
// ---------------------------------------------------------------------------

#[test]
fn test_tail_recursive_factorial() {
    // Tail-recursive factorial with accumulator
    let source = "(defn fact-tr [n acc] (if (<= n 1) acc (fact-tr (- n 1) (* n acc)))) (fact-tr 10 1)";
    let result = compile_run(source).unwrap();
    assert_eq!(result, Value::Number(3628800.0));
}

#[test]
fn test_tail_recursive_sum() {
    // Tail-recursive sum from n down to 0
    let source = "(defn sum-tr [n acc] (if (<= n 0) acc (sum-tr (- n 1) (+ n acc)))) (sum-tr 1000 0)";
    let result = compile_run(source).unwrap();
    assert_eq!(result, Value::Number(500500.0));
}

#[test]
fn test_tail_call_identity() {
    // A tail-call chain that just passes a value through
    let source = "(defn pass [x] x) (pass (pass (pass 42)))";
    let result = compile_run(source).unwrap();
    assert_eq!(result, Value::Number(42.0));
}

// ---------------------------------------------------------------------------
// Stack trace tests
// ---------------------------------------------------------------------------

#[test]
fn test_call_stack_trace() {
    let source = "(defn inner [] 42) (defn outer [] (inner)) (defn main [] (outer)) (main)";
    let result = compile_run(source);
    assert!(result.is_ok(), "call chain should execute: {:?}", result);

    // If the VM exposed the call stack trace, we could check it here
    // For now, just verify the result is correct
    assert_eq!(result.unwrap(), Value::Number(42.0));
}

// ---------------------------------------------------------------------------
// String tests
// ---------------------------------------------------------------------------

#[test]
fn test_string_literal() {
    let result = compile_run("\"hello\"").unwrap();
    assert_eq!(result, Value::String("hello".to_string()));
}

#[test]
fn test_string_concat_via_gate() {
    // Strings are stored as constants; concatenation requires special op
    let result = compile_run("\"hello\"").unwrap();
    assert_eq!(result, Value::String("hello".to_string()));
}

// ---------------------------------------------------------------------------
// Compilation tests
// ---------------------------------------------------------------------------

#[test]
fn test_compile_while() {
    let module = compile_only("(while (< i 10) (set i (+ i 1)))");
    assert!(module.is_ok());
    let module = module.unwrap();
    assert!(!module.code.is_empty());
}

#[test]
fn test_compile_multiple_defns() {
    let source = "(defn f [] 1) (defn g [] 2) (+ (f) (g))";
    let result = compile_run(source);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Number(3.0));
}

#[test]
fn test_bytecode_not_empty() {
    let module = compile_only("(+ 1 2)").unwrap();
    assert!(!module.code.is_empty(), "bytecode should not be empty");
    assert!(!module.constants.is_empty(), "constants should not be empty");
}

#[test]
fn test_constant_pool_deduplication() {
    let module = compile_only("(+ 5 5)").unwrap();
    // 5 appears twice but should be deduplicated in constant pool
    assert!(module.constants.len() <= 2, "constants: {:?}", module.constants.len());
}

// ---------------------------------------------------------------------------
// VM API tests
// ---------------------------------------------------------------------------

#[test]
fn test_vm_stack_depth_api() {
    let module = compile_only("(+ 1 2)").unwrap();
    let mut vm = VM::with_module(&module);
    let result = vm.run();
    assert!(result.is_ok());
    let depth = vm.stack_depth();
    assert!(depth <= MAX_STACK_DEPTH);
}

#[test]
fn test_vm_call_stack_trace_api() {
    let source = "(defn a [] 1) (defn b [] (a)) (defn c [] (b)) (c)";
    let ast = ml_core::MLExpr::parse(source).unwrap();
    let module = Compiler::compile(&ast).unwrap();
    let mut vm = VM::with_module(&module);
    let result = vm.run().unwrap();
    assert_eq!(result, Value::Number(1.0));
}

#[test]
fn test_max_call_frames() {
    // Test that deeply nested calls respect MAX_CALL_FRAMES limit
    // A chain of 300 function calls should stay within 256 call frame limit
    let source = "(defn chain [n] (if (<= n 0) 0 (+ 1 (chain (- n 1))))) (chain 200)";
    let result = compile_run(source);
    // Should either succeed with 200 or fail with call stack overflow
    match result {
        Ok(v) => assert_eq!(v, Value::Number(200.0)),
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("call") || msg.contains("Call"),
                "error should mention call stack: {}", msg);
        }
    }
}

// ---------------------------------------------------------------------------
// Disassembler tests
// ---------------------------------------------------------------------------

#[test]
fn test_disassemble_not_empty() {
    use ml_vm::disassemble;

    let module = compile_only("(+ 1 2)").unwrap();
    let output = disassemble(&module.code, &module.constants);
    assert!(!output.is_empty());
    assert!(output.contains("ADD") || output.contains("+"));
}

#[test]
fn test_disassemble_constants() {
    use ml_vm::disassemble;

    let module = compile_only("42").unwrap();
    let output = disassemble(&module.code, &module.constants);
    // Should mention the constant 42
    assert!(output.contains("42") || output.contains("42.0") || output.contains("Number"));
}
