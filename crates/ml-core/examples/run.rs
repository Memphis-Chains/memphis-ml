// ML-Core CLI — przykładowy runner
// Uruchom: cargo run -p ml-core --example run -- "(gate garage on)"

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("ML-Core — Memphis Language for hardware control");
        println!();
        println!("Usage: ml-run '(program)'");
        println!();
        println!("Examples:");
        println!("  (gate garage on)              — włącz bramkę");
        println!("  (gate garage off)             — wyłącz bramkę");
        println!("  (read temp.living_room)       — odczytaj temperaturę");
        println!("  (wait 500)                   — poczekaj 500ms");
        println!("  (log \"hello\")                — wypisz log");
        println!();
        println!("  (if (> temp.server 40)       — jeśli temp > 40°C:");
        println!("      (gate fan on)             —   włącz wentylator");
        println!("      (gate alarm off))         —   wyłącz alarm");
        println!();
        println!("  (gate garage on)              — sekwencja:");
        println!("  (wait 1000)                  —   czekaj 1s");
        println!("  (read temp.living_room)       —   sprawdź temp");
        return;
    }

    let source = &args[1];

    println!("[ML] Parsing: {}", source);

    let expr = match ml_core::parser::Parser::new(source).parse() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[ML] Parse error: {}", e);
            std::process::exit(1);
        }
    };

    println!("[ML] AST: {:?}", expr);

    let machine = ml_core::MockMachine::new();
    let mut runtime = ml_core::Runtime::new(machine);

    println!("[ML] Executing...");
    match runtime.execute(expr) {
        Ok(result) => println!("[ML] Done: {:?}", result),
        Err(e) => {
            eprintln!("[ML] Runtime error: {}", e);
            std::process::exit(1);
        }
    }
}
