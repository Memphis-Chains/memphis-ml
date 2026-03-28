//! ML CLI — Memphis Language REPL and runner

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ml")]
#[command(about = "ML - Memphis Language CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and evaluate an ML expression
    Run {
        /// ML program to run
        program: String,

        /// Machine type
        #[arg(short, long, default_value = "mock")]
        machine: MachineType,
    },

    /// Run ML program from file
    RunFile {
        /// ML source file
        path: PathBuf,

        /// Machine type
        #[arg(short, long, default_value = "mock")]
        machine: MachineType,
    },

    /// List available examples
    Examples,
}

#[derive(clap::ValueEnum, Clone)]
enum MachineType {
    Mock,
    Gpio,
    Http,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { program, machine } => {
            run_program(&program, machine)
        }
        Commands::RunFile { path, machine } => {
            let src = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("read {}: {}", path.display(), e))?;
            run_program(&src, machine)
        }
        Commands::Examples => {
            println!("ML Examples:");
            println!("  hello          — log a greeting");
            println!("  gate-on        — turn a gate on");
            println!("  read-temp      — read temperature");
            println!("  let-basic      — variable binding");
            println!("  binary-plus    — addition");
            println!("  while-loop     — while loop");
            println!("  server-cooling — temperature monitoring");
            println!("  ...");
            println!("\nRun: ml run '(program)'");
            Ok(())
        }
    }
}

fn run_program(source: &str, _machine: MachineType) -> anyhow::Result<()> {
    let expr = ml_core::parser::Parser::new(source)
        .parse()
        .map_err(|e| anyhow::anyhow!("parse error: {}", e))?;

    let machine: ml_core::MockMachine = ml_core::MockMachine::new();
    let mut runtime = ml_core::Runtime::new(machine);

    runtime.execute(expr)
        .map_err(|e| anyhow::anyhow!("runtime error: {}", e))?;

    println!("[OK]");
    Ok(())
}
