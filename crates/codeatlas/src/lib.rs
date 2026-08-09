pub mod enrich;
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
        /// After the structural scan, fill summary slots through the
        /// enrichment provider (ADR-0004)
        #[arg(long)]
        enrich: bool,
    },
    /// Print the JSON Schema of the map contract
    Schema,
}

pub fn run() -> ExitCode {
    match Cli::parse().command {
        Command::Scan { path, enrich } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            // The structural map is always built and saved first — with
            // stored annotations re-attached where content is unchanged
            // (ADR-0005) — so any enrichment failure leaves a complete map
            // behind (story 14).
            let result = scan::scan(&root).and_then(|mut graph| {
                enrich::AnnotationStore::load(&root).reattach(&root, &mut graph);
                scan::save(&root, &graph)?;
                Ok(graph)
            });
            let mut graph = match result {
                Ok(graph) => graph,
                Err(err) => {
                    eprintln!("error: {err:#}");
                    return ExitCode::FAILURE;
                }
            };
            eprintln!("mapped {} files", graph.nodes.len());
            if !enrich {
                return ExitCode::SUCCESS;
            }
            match enrich::run(&root, &mut graph) {
                Ok(count) => {
                    eprintln!("enriched {count} nodes");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err:#} (the structural map is intact)");
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
