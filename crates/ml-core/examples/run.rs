// ML-Core Examples — demonstrative programs
//
// Run with: cargo run -p ml-core --example run -- "(program)"
// Or list all: cargo run -p ml-core --example run

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let programs = vec![
        ("hello", r#"(log "Hello from ML!")"#, "Print a greeting"),

        ("gate-on", "(gate garage on)", "Turn a gate on"),
        ("gate-off", "(gate door off)", "Turn a gate off"),

        ("read-temp", "(read temp.living_room)", "Read temperature sensor"),

        ("let-basic", "(let x 10 x)", "Basic variable binding"),
        ("let-expr", "(let x 5 (+ x 3))", "Variable in expression"),
        ("let-nested", "(let x 1 (let y 2 (+ x y)))", "Nested let bindings"),

        ("binary-plus", "(+ 3 5)", "Addition"),
        ("binary-mult", "(* 4 7)", "Multiplication"),
        ("binary-mixed", "(+ (* 2 3) (- 10 4))", "Mixed operators"),

        ("comparison", "(if (> 10 5) (log \"10 > 5\") (log \"no\"))", "Conditional with comparison"),

        ("while-loop", "(let i 0 (while (< i 3) (begin (log i) (set i (+ i 1)))))", "While loop with counter"),

        ("sequence", "(gate garage on) (wait 1000) (gate garage off)", "Sequence: gate on, wait, gate off"),

        ("server-cooling",
         r#"(if (> (read temp.server) 40) (gate fan on) (gate fan off))"#,
         "Server room cooling logic"),

        ("smart-lights",
         r#"(let time 22 (if (> time 20) (gate lights off) (gate lights on)))"#,
         "Smart lights based on time"),

        ("temp-monitor",
         r#"(let temp 45 (if (> temp 50) (begin (log "HOT!") (gate alarm on)) (log "OK")))"#,
         "Temperature monitor with alert"),

        ("read-all-rooms",
         r#"(begin (log (read temp.living_room)) (log (read temp.garage)) (log (read temp.outside)))"#,
         "Read all room temperatures"),
    ];

    if args.len() < 2 {
        println!("ML-Core — Memphis Language Examples");
        println!("===================================\n");
        println!("Usage: ml-run '(program)'  or  ml-run <example-name>\n");
        println!("Built-in examples:");
        for (name, _src, desc) in &programs {
            println!("  {:20} — {}", name, desc);
        }
        println!("\nOr run a raw ML expression directly.");
        return;
    }

    let input = &args[1];

    // Check if it's an example name
    if let Some((_name, src, desc)) = programs.iter().find(|(n, _, _)| *n == input) {
        println!("Running example: {} — {}\n", input, desc);
        run_src(src);
        return;
    }

    // Treat as raw ML source
    run_src(input);
}

fn run_src(source: &str) {
    println!("[ML] Source: {}\n", source);

    let expr = match ml_core::parser::Parser::new(source).parse() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[ML] Parse error: {}", e);
            std::process::exit(1);
        }
    };
    println!("[ML] AST: {:?}\n", expr);

    let machine = ml_core::MockMachine::new();
    let mut runtime = ml_core::Runtime::new(machine);

    match runtime.execute(expr) {
        Ok(result) => println!("\n[ML] Result: {:?}\n", result),
        Err(e) => {
            eprintln!("[ML] Runtime error: {}", e);
            std::process::exit(1);
        }
    }
}
