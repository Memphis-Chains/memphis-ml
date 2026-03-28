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
        
        /// Data directory for Memphis chains
        #[arg(short, long)]
        data_dir: Option<PathBuf>,
    },
    
    /// Run ML program from file
    RunFile {
        /// ML source file
        file: PathBuf,
        
        #[arg(short, long, default_value = "mock")]
        machine: MachineType,
    },
    
    /// Parse only (don't execute)
    Parse {
        /// ML program to parse
        program: String,
        
        /// Show AST
        #[arg(short, long)]
        ast: bool,
    },
    
    /// Start a P2P node
    Node {
        /// Listen address
        #[arg(short, long, default_value = "127.0.0.1:9000")]
        addr: String,
        
        /// Machine ID for this node
        #[arg(short, long)]
        machine_id: Option<String>,
    },
    
    /// List available hardware
    List {
        #[arg(short, long, default_value = "mock")]
        machine: MachineType,
    },
}

#[derive(clap::ValueEnum, Clone)]
enum MachineType {
    Mock,
    Gpio,
    Http,
    Memphis,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Run { program, machine, data_dir } => {
            let result = run_program(&program, machine, data_dir.as_deref()).await?;
            println!("Result: {:?}", result);
        }
        Commands::RunFile { file, machine } => {
            let program = tokio::fs::read_to_string(&file).await?;
            let result = run_program(&program, machine, None).await?;
            println!("Result: {:?}", result);
        }
        Commands::Parse { program, ast } => {
            let expr = ml_core::parser::Parser::new(&program).parse()?;
            if ast {
                println!("AST: {:?}", expr);
            } else {
                println!("Parse OK: {:?}", expr);
            }
        }
        Commands::Node { addr, machine_id } => {
            let node_id = machine_id.unwrap_or_else(|| uuid::v4().to_string());
            println!("Starting node {} on {}", node_id, addr);
            let mut node = ml_p2p::Node::bind(&addr).await?;
            node.run().await?;
        }
        Commands::List { machine } => {
            println!("Available hardware for {:?}:", machine);
            println!("  - Gates: gpio (mock)");
            println!("  - Sensors: temp.*, humidity.*, motion.*");
        }
    }
    
    Ok(())
}

async fn run_program(
    program: &str,
    machine_type: MachineType,
    _data_dir: Option<&Path>,
) -> anyhow::Result<ml_core::MLValue> {
    use ml_core::{Parser, Runtime, MockMachine};
    
    let expr = Parser::new(program).parse()
        .map_err(|e| anyhow::anyhow!("parse error: {}", e))?;
    
    // Build machine based on type
    let machine: Box<dyn ml_hal::Gate + ml_hal::Sensor + ml_hal::Actuator> = match machine_type {
        MachineType::Mock => Box::new(MockMachine::new()),
        MachineType::Gpio => {
            // TODO: implement real GPIO
            Box::new(MockMachine::new())
        }
        MachineType::Http => {
            // TODO: implement HTTP backend
            Box::new(MockMachine::new())
        }
        MachineType::Memphis => {
            // TODO: implement Memphis bridge
            Box::new(MockMachine::new())
        }
    };
    
    let mut runtime = Runtime::new(machine);
    let result = runtime.execute(expr)
        .map_err(|e| anyhow::anyhow!("runtime error: {}", e))?;
    
    Ok(result)
}
