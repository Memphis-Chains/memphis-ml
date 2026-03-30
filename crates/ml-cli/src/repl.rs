//! ML REPL — interactive Memphis Language shell

use anyhow::Result;
use ml_core::{MockMachine, Runtime, MLValue};
use ml_core::parser::Parser;
use std::io::{self, Write};

const REPL_HELP: &str = r#"ML REPL — Memphis Language
  Type ML expressions to evaluate them.
  :help     — show this help
  :quit     — exit the REPL
  :reset    — reset runtime state (vars & functions)

Examples:
  (+ 1 2)                → 3
  (log "hello")          → prints [ML] hello
  (let x 10 (log x))     → prints [ML] 10
  (fn add (a b) (+ a b))  — define named function
  (add 3 4)              → 7
"#;

pub fn run_repl() -> Result<()> {
    let mut runtime = Runtime::new(MockMachine::new());

    println!("{}", REPL_HELP);
    println!("Type :help for commands, :quit to exit.\n");

    loop {
        print!("ml> ");
        io::stdout().flush()?;

        let line = read_line()?;
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        // Handle meta-commands
        if line.starts_with(':') {
            match handle_meta_command(line, &mut runtime) {
                MetaResult::Quit => {
                    println!("Goodbye!");
                    break;
                }
                MetaResult::Skip => {
                    continue;
                }
            }
        } else {
            // Parse and execute
            match run_line(&mut runtime, line) {
                Ok(Some(v)) => {
                    println!("  = {}", format_value(&v));
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("  error: {}", e);
                }
            }
        }
    }

    Ok(())
}

fn read_line() -> Result<String> {
    let mut buf = String::new();
    let n = std::io::stdin().read_line(&mut buf)?;
    if n == 0 {
        // EOF
        std::process::exit(0);
    }
    Ok(buf)
}

fn run_line(runtime: &mut Runtime<MockMachine>, source: &str) -> anyhow::Result<Option<MLValue>, anyhow::Error> {
    // Try to parse as a single expression first
    let expr = match Parser::new(source).parse() {
        Ok(expr) => expr,
        Err(e) => {
            // Try wrapping in begin block (multi-expression on one line)
            match Parser::new(&format!("(begin {})", source)).parse() {
                Ok(expr) => expr,
                Err(_) => {
                    anyhow::bail!("parse error: {}", e)
                }
            }
        }
    };

    // Execute
    let result = runtime.execute(expr)
        .map_err(|e| anyhow::anyhow!("runtime error: {}", e))?;

    Ok(Some(result))
}

fn handle_meta_command(line: &str, runtime: &mut Runtime<MockMachine>) -> MetaResult {
    match line {
        ":help" | ":h" | "help" => {
            println!("{}", REPL_HELP);
            MetaResult::Skip
        }
        ":quit" | ":q" | "exit" => MetaResult::Quit,
        ":reset" => {
            // Rebuild the runtime to reset all state
            *runtime = Runtime::new(MockMachine::new());
            println!("  runtime reset.");
            MetaResult::Skip
        }
        _ => {
            eprintln!("  unknown command: {}. Try :help", line);
            MetaResult::Skip
        }
    }
}

fn format_value(v: &MLValue) -> String {
    match v {
        MLValue::Unit => "()".into(),
        MLValue::Bool(true) => "true".into(),
        MLValue::Bool(false) => "false".into(),
        MLValue::Number(n) => {
            if n.fract() == 0.0 {
                format!("{:.0}", n)
            } else {
                n.to_string()
            }
        }
        MLValue::String(s) => format!("\"{}\"", s),
        MLValue::Fn(args, _) => format!("<fn ({})>", args.join(" ")),
        MLValue::Closure(c) => format!("<closure ({})>", c.args.join(" ")),
        MLValue::Nil => "nil".into(),
        MLValue::Return(_) => "<return>".into(),
    }
}

enum MetaResult {
    Skip,
    Quit,
}
