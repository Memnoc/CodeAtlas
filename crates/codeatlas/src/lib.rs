pub mod diff;
pub mod enrich;
pub mod map;
pub mod parsers;
pub mod scan;
pub mod semantics;
pub mod serve;
pub mod share;

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
        /// enrichment provider (ADR-0004). The default provider is the
        /// Claude API (credentials: ANTHROPIC_API_KEY, or an `ant auth
        /// login` profile); CODEATLAS_ENRICH_PROVIDER overrides it.
        #[arg(long)]
        enrich: bool,
        /// Model for the Claude enrichment provider (default:
        /// claude-opus-5). Ignored by non-Claude providers.
        #[arg(long, requires = "enrich")]
        model: Option<String>,
    },
    /// Overlay a git diff's impact on the map: changed nodes and their
    /// one-hop blast radius, written to .codeatlas/diff-overlay.json.
    /// Deterministic — git in, overlay out, no LLM, no network.
    Diff {
        /// Repository root holding .codeatlas/ (defaults to the current
        /// directory)
        path: Option<PathBuf>,
    },
    /// Print the JSON Schema of the map contract
    Schema,
    /// Export the map as one self-contained, redacted HTML file at
    /// .codeatlas/share.html — opens by double-click, no server, no
    /// external requests. LLM-enriched prose is redacted (allowlist over
    /// the map contract, ADR-0006); the artifact discloses what was
    /// removed.
    Share {
        /// Repository root holding .codeatlas/ (defaults to the current
        /// directory)
        path: Option<PathBuf>,
    },
    /// Serve the embedded dashboard and the local map on 127.0.0.1
    Serve {
        /// Repository root holding .codeatlas/ (defaults to the current
        /// directory)
        path: Option<PathBuf>,
        /// Port on 127.0.0.1 (0 lets the OS pick a free one). There is
        /// deliberately no --host: the server only ever binds loopback
        /// (ADR-0006).
        #[arg(long, default_value_t = 4173)]
        port: u16,
    },
}

pub fn run() -> ExitCode {
    match Cli::parse().command {
        Command::Scan {
            path,
            enrich,
            model,
        } => {
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
            match enrich::run(&root, &mut graph, model.as_deref()) {
                Ok(enrich::Outcome::NothingToEnrich) => {
                    eprintln!(
                        "nothing to enrich: every slot is already enriched \
                         or the map is empty"
                    );
                    ExitCode::SUCCESS
                }
                Ok(enrich::Outcome::Enriched(count)) => {
                    eprintln!("enriched {count} slots");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err:#} (the structural map is intact)");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Diff { path } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            match diff::run(&root) {
                Ok(overlay) => {
                    eprintln!(
                        "{} changed nodes, {} affected — overlay at {}/{}",
                        overlay.changed.len(),
                        overlay.affected.len(),
                        scan::OUTPUT_DIR,
                        diff::OVERLAY_FILE
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Serve { path, port } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            match serve::serve(&root, port) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("error: {err:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Share { path } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            match share::run(&root) {
                Ok(summary) => {
                    if summary.redacted.is_empty() {
                        eprintln!("nothing redacted: the map contains no LLM-enriched prose");
                    } else {
                        let fields: Vec<String> = summary
                            .redacted
                            .iter()
                            .map(|(field, count)| format!("{field} ×{count}"))
                            .collect();
                        eprintln!("redacted {}", fields.join(", "));
                    }
                    println!("share artifact at {}", summary.path.display());
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Schema => {
            let schema = map::contract_schema();
            println!("{}", serde_json::to_string_pretty(&schema).unwrap());
            ExitCode::SUCCESS
        }
    }
}
