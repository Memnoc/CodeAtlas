pub mod map;
pub mod parsers;
pub mod scan;
pub mod semantics;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "codeatlas",
    version,
    about = "Map a codebase: structure and relationships"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan a repository and write .codeatlas/knowledge-graph.json
    Scan {
        /// Repository root (defaults to the current directory)
        path: Option<PathBuf>,
    },
    /// Print the JSON Schema of the map contract
    Schema,
}

pub fn run() -> ExitCode {
    match Cli::parse().command {
        Command::Scan { path } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            let result = scan::scan(&root).and_then(|graph| {
                scan::save(&root, &graph)?;
                Ok(graph)
            });
            match result {
                Ok(graph) => {
                    eprintln!("mapped {} files", graph.nodes.len());
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Schema => {
            let schema = schemars::schema_for!(map::KnowledgeGraph);
            println!("{}", serde_json::to_string_pretty(&schema).unwrap());
            ExitCode::SUCCESS
        }
    }
}
