//! Benchmark suite for ml-vm using Criterion.
//!
//! Run with: cargo bench -p ml-vm
//!
//! Measures:
//! - Arithmetic expression evaluation
//! - Function call overhead
//! - Tail call performance (vs regular call)
//! - Loop iteration performance
//! - Memory usage (stack depth)
//! - Comparison with native Rust execution

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ml_vm::{VM, Compiler, Value, CompiledModule};

// ---------------------------------------------------------------------------
// Benchmark helpers
// ---------------------------------------------------------------------------

fn compile_run(source: &str) -> Value {
    let ast = ml_core::MLExpr::parse(source).expect("parse failed");
    let module = Compiler::compile(&ast).expect("compile failed");
    let mut vm = VM::with_module(&module);
    vm.run().expect("vm run failed")
}

// Native Rust equivalents for comparison
fn native_add(a: f64, b: f64) -> f64 { a + b }
fn native_fib(n: u64) -> u64 {
    if n < 2 { n } else { native_fib(n - 1) + native_fib(n - 2) }
}
fn native_factorial(n: u64) -> u64 {
    if n <= 1 { 1 } else { n * native_factorial(n - 1) }
}
fn native_ackermann(m: u64, n: u64) -> u64 {
    if m == 0 { n + 1 }
    else if n == 0 { native_ackermann(m - 1, 1) }
    else { native_ackermann(m - 1, native_ackermann(m, n - 1)) }
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

fn bench_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("arithmetic");

    // Simple arithmetic
    let source = "(+ (* 123.45 67.89) (- 987.6 123.4))";
    group.bench_function("simple_arithmetic", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Nested arithmetic
    let source = "(+ (* (+ 1 2) (+ 3 4)) (* (- 10 5) (- 20 10)))";
    group.bench_function("nested_arithmetic", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Chained operations
    let source = "(+ (- (* (/ 100 2) 3) 10) 5)";
    group.bench_function("chained_ops", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Native comparison: simple arithmetic
    group.bench_function("native_simple", |b| {
        b.iter(|| native_add(black_box(123.45 * 67.89), black_box(987.6 - 123.4)))
    });

    group.finish();
}

fn bench_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison");

    group.bench_function("eq_true", |b| {
        b.iter(|| compile_run(black_box("(== 42 42)")))
    });
    group.bench_function("eq_false", |b| {
        b.iter(|| compile_run(black_box("(== 1 2)"))
        )
    });
    group.bench_function("lt_true", |b| {
        b.iter(|| compile_run(black_box("(< 3 5)")))
    });
    group.bench_function("chain_comparison", |b| {
        b.iter(|| compile_run(black_box("(if (&& (< 1 10) (> 5 3) (== 2 2)) 1 0)")))
    });

    group.finish();
}

fn bench_function_call(c: &mut Criterion) {
    let mut group = c.benchmark_group("function_call");

    // Factorial via named defn
    let source = r#"
        (defn factorial [n]
          (if (<= n 1) 1 (* n (factorial (- n 1)))))
        (factorial 10)
    "#;
    group.bench_function("factorial_10", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Fibonacci (naive exponential)
    let source = r#"
        (defn fib [n]
          (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))
        (fib 15)
    "#;
    group.bench_function("fib_15", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Native factorial comparison
    group.bench_function("native_factorial_10", |b| {
        b.iter(|| black_box(native_factorial(10)))
    });

    // Native fib comparison
    group.bench_function("native_fib_15", |b| {
        b.iter(|| black_box(native_fib(15)))
    });

    group.finish();
}

fn bench_tail_call(c: &mut Criterion) {
    let mut group = c.benchmark_group("tail_call");

    // Tail-recursive factorial (tail-call optimized)
    let source = r#"
        (defn fact-tr [n acc]
          (if (<= n 1) acc (fact-tr (- n 1) (* n acc))))
        (fact-tr 10 1)
    "#;
    group.bench_function("tail_factorial_10", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Tail-recursive fib
    let source = r#"
        (defn fib-tr [n a b]
          (if (< n 2) a (fib-tr (- n 1) b (+ a b))))
        (fib-tr 20 0 1)
    "#;
    group.bench_function("tail_fib_20", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Deep tail recursion (tests stack frame reuse)
    let source = r#"
        (defn sum-down [n acc]
          (if (<= n 0) acc (sum-down (- n 1) (+ n acc))))
        (sum-down 1000 0)
    "#;
    group.bench_function("tail_sum_1000", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Native equivalent
    group.bench_function("native_sum_1_to_1000", |b| {
        let mut acc = 0u64;
        for i in 1..=1000 { acc += i; }
        b.iter(|| black_box(acc))
    });

    group.finish();
}

fn bench_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("loop");

    // While loop with counter
    let source = r#"
        (defn sum-to [n]
          (let loop [i n acc 0]
            (if (<= i 0) acc (loop (- i 1) (+ acc i)))))
        (sum-to 1000)
    "#;
    group.bench_function("while_sum_1000", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Simple iteration
    let source = "(let loop [i 1000 acc 0] (if (<= i 0) acc (loop (- i 1) (+ acc i))))";
    group.bench_function("iter_1000", |b| {
        b.iter(|| compile_run(black_box(source)))
    });

    // Native equivalent
    group.bench_function("native_sum_1_to_1000", |b| {
        let mut acc = 0u64;
        for i in 1..=1000 { acc += i; }
        b.iter(|| black_box(acc))
    });

    group.finish();
}

fn bench_stack_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("stack");

    // Small expression
    let source = "(+ 1 2)";
    group.bench_function("depth_1", |b| {
        b.iter(|| {
            let ast = ml_core::MLExpr::parse(source).unwrap();
            let module = Compiler::compile(&ast).unwrap();
            let mut vm = VM::with_module(&module);
            vm.run()
        })
    });

    // Nested let (stack depth increases)
    let source = "(let x1 1 (let x2 2 (let x3 3 (let x4 4 (let x5 5 (+ x1 (+ x2 (+ x3 (+ x4 x5)))))))))";
    group.bench_function("depth_5", |b| {
        b.iter(|| {
            let ast = ml_core::MLExpr::parse(source).unwrap();
            let module = Compiler::compile(&ast).unwrap();
            let mut vm = VM::with_module(&module);
            vm.run()
        })
    });

    group.finish();
}

fn bench_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation");

    let sources = [
        ("simple", "(+ 3 5)"),
        ("let", "(let x 10 (+ x 5))"),
        ("nested", "(+ (+ 1 2) (+ 3 4))"),
        ("if", "(if true 42 0)"),
        ("factorial", r#"(defn factorial [n] (if (<= n 1) 1 (* n (factorial (- n 1))))) (factorial 10)"#),
    ];

    for (name, source) in sources {
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            let ast = ml_core::MLExpr::parse(source).unwrap();
            b.iter(|| {
                Compiler::compile(black_box(&ast))
            })
        });
    }

    group.finish();
}

fn bench_bytecode_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("bytecode_size");

    let cases = [
        ("number", "42"),
        ("add", "(+ 1 2)"),
        ("let", "(let x 10 x)"),
        ("factorial", r#"(defn f [n] (if (<= n 1) 1 (* n (f (- n 1))))) (f 10)"#),
    ];

    for (name, source) in cases {
        let ast = ml_core::MLExpr::parse(source).unwrap();
        let module = Compiler::compile(&ast).unwrap();
        let bytecode_size = module.code.len();
        let constants_count = module.constants.len();

        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let ast = ml_core::MLExpr::parse(source).unwrap();
                let module = Compiler::compile(&ast).unwrap();
                black_box((module.code.len(), module.constants.len()))
            })
        });
        // Log size info
        eprintln!("{}: bytecode={} bytes, constants={}", name, bytecode_size, constants_count);
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_arithmetic,
    bench_comparison,
    bench_function_call,
    bench_tail_call,
    bench_loop,
    bench_stack_usage,
    bench_compilation,
    bench_bytecode_size,
);

criterion_main!(benches);
